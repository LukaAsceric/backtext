pub mod providers;
pub mod stt;

pub use providers::{
    compute_embedding_256, list_ollama_models, test_provider_connection, ProviderTestResult,
};
pub use stt::{
    detect_whisper_cli_path, download_whisper_model, list_whisper_models, transcribe_wav_file,
    TranscribedUtterance,
};

use anyhow::Result;
use backtext_core::*;
use backtext_storage::StorageEngine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedMeetingResult {
    pub executive_summary: String,
    pub enhanced_markdown: String,
    pub action_items: Vec<ActionItem>,
    pub provider_used: String,
}

#[derive(Clone)]
pub struct HybridAiEngine {
    http: reqwest::Client,
}

impl Default for HybridAiEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridAiEngine {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { http }
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.http
    }

    /// Enhances a meeting by fusing the user's rough hybrid-notepad bullets,
    /// speaker-attributed transcript, and deduplicated screen-sharing OCR frames.
    pub async fn enhance_meeting_summary(
        &self,
        storage: &StorageEngine,
        meeting_id: &str,
    ) -> Result<EnhancedMeetingResult> {
        let detail = storage
            .get_meeting_detail(meeting_id)?
            .ok_or_else(|| anyhow::anyhow!("Meeting '{meeting_id}' not found"))?;
        let profile = storage.get_user_profile()?;
        let vocab = storage.list_vocabulary()?;
        let prefs = storage.list_learned_preferences()?;
        let ai_settings = storage.get_ai_settings()?;
        let is_de = profile.preferred_output_language == Language::De;

        let dossier = render_personalization_dossier(&profile, &vocab, &prefs);
        let transcript_text = detail
            .transcript
            .iter()
            .map(|s| {
                format!(
                    "[{:02}:{:02}] {}: {}",
                    s.start_ms / 60_000,
                    (s.start_ms % 60_000) / 1000,
                    s.speaker_name,
                    s.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let slides_text = detail
            .screenshares
            .iter()
            .map(|f| {
                format!(
                    "- [Folie @ {:02}:{:02}] {}: {}",
                    f.captured_at_ms / 60_000,
                    (f.captured_at_ms % 60_000) / 1000,
                    f.slide_title,
                    f.ocr_text
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let sys_prompt = format!(
            "Du bist backtext, ein präziser Meeting-Notiz-Enhancer im Granola-Stil. Nutze die eigenen Stichpunkte als Rückgrat und reichere sie mit Fakten aus Transkript und Folien an.\n{dossier}"
        );
        let usr_prompt = format!(
            "Meeting: {}\nEigene Notizen:\n{}\n\nTranskript:\n{}\n\nScreen-Share-OCR:\n{}",
            detail.meeting.title, detail.meeting.raw_user_notes, transcript_text, slides_text
        );

        let extracted_actions = extract_structured_actions(
            &detail.transcript,
            &detail.meeting.raw_user_notes,
            &profile.display_name,
        );

        if let Some((llm_md, provider_used)) = providers::stream_completion(
            &self.http,
            &ai_settings,
            storage.secrets(),
            &sys_prompt,
            &usr_prompt,
            None,
        )
        .await
        {
            let exec_summary = llm_md
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .take(2)
                .collect::<Vec<_>>()
                .join(" ");
            let mut updated = detail.meeting.clone();
            updated.executive_summary = exec_summary.clone();
            updated.enhanced_markdown = llm_md.clone();
            if !extracted_actions.is_empty() {
                updated.action_items = extracted_actions.clone();
            }
            storage.save_meeting(&updated)?;
            return Ok(EnhancedMeetingResult {
                executive_summary: exec_summary,
                enhanced_markdown: llm_md,
                action_items: updated.action_items,
                provider_used,
            });
        }

        // Local deterministic synthesis when offline (Granola Hybrid Notepad Fusion)
        let participant_names: Vec<String> = detail
            .meeting
            .participants
            .iter()
            .map(|p| p.display_name.clone())
            .collect();

        let user_bullets: Vec<&str> = detail
            .meeting
            .raw_user_notes
            .lines()
            .map(|l| l.trim().trim_start_matches('-').trim_start_matches('*').trim())
            .filter(|l| !l.is_empty())
            .collect();

        let executive_summary = if is_de {
            format!(
                "Abstimmung zu „{}“ mit {} ({} Wortbeiträge, {} erfasste Screen-Share-Folien). {}",
                detail.meeting.title,
                participant_names.join(", "),
                detail.transcript.len(),
                detail.screenshares.len(),
                detail
                    .transcript
                    .last()
                    .map(|s| format!("Kernergebnis: {}", s.text))
                    .unwrap_or_else(|| "Eigene Stichpunkte wurden strukturiert aufbereitet.".into())
            )
        } else {
            format!(
                "Sync on \"{}\" with {} ({} transcript segments, {} captured screen-share slides). {}",
                detail.meeting.title,
                participant_names.join(", "),
                detail.transcript.len(),
                detail.screenshares.len(),
                detail
                    .transcript
                    .last()
                    .map(|s| format!("Key outcome: {}", s.text))
                    .unwrap_or_else(|| "User rough notes were enhanced with meeting context.".into())
            )
        };

        let mut md = String::new();
        if !user_bullets.is_empty() {
            md.push_str(if is_de {
                "### Angereicherte Kernpunkte (aus deinen Notizen + Transkript)\n"
            } else {
                "### Enhanced Key Points (from Your Notes + Transcript)\n"
            });
            for b in &user_bullets {
                let b_vec = embed_text_local_256(b);
                let best_seg = detail.transcript.iter().max_by(|x, y| {
                    let sx = cosine_similarity(&b_vec, &embed_text_local_256(&x.text));
                    let sy = cosine_similarity(&b_vec, &embed_text_local_256(&y.text));
                    sx.partial_cmp(&sy).unwrap_or(std::cmp::Ordering::Equal)
                });
                match best_seg {
                    Some(seg) => md.push_str(&format!(
                        "- **{}** — *Kontext ({}, {:02}:{:02}):* {}\n",
                        b,
                        seg.speaker_name,
                        seg.start_ms / 60_000,
                        (seg.start_ms % 60_000) / 1000,
                        seg.text
                    )),
                    None => md.push_str(&format!("- **{}**\n", b)),
                }
            }
            md.push('\n');
        }

        if !detail.transcript.is_empty() {
            md.push_str(if is_de {
                "### Besprochene Schwerpunkte & Entscheidungen\n"
            } else {
                "### Discussion Highlights & Decisions\n"
            });
            for seg in detail.transcript.iter().take(5) {
                md.push_str(&format!(
                    "- **{}** (`{:02}:{:02}`): {}\n",
                    seg.speaker_name,
                    seg.start_ms / 60_000,
                    (seg.start_ms % 60_000) / 1000,
                    seg.text
                ));
            }
            md.push('\n');
        }

        if !detail.screenshares.is_empty() {
            md.push_str(if is_de {
                "### Erfasste Screen-Sharing-Folien (Vision OCR)\n"
            } else {
                "### Captured Screen-Sharing Slides (Vision OCR)\n"
            });
            for frame in &detail.screenshares {
                md.push_str(&format!(
                    "- **{}** (`{:02}:{:02}`, {}): {}\n",
                    frame.slide_title,
                    frame.captured_at_ms / 60_000,
                    (frame.captured_at_ms % 60_000) / 1000,
                    frame.presenter_name.as_deref().unwrap_or("Präsentation"),
                    frame.ocr_text
                ));
            }
        }

        let mut updated = detail.meeting.clone();
        updated.executive_summary = executive_summary.clone();
        updated.enhanced_markdown = md.clone();
        if !extracted_actions.is_empty() {
            updated.action_items = extracted_actions;
        }
        storage.save_meeting(&updated)?;

        Ok(EnhancedMeetingResult {
            executive_summary,
            enhanced_markdown: md,
            action_items: updated.action_items,
            provider_used: "local_hybrid_synthesizer".into(),
        })
    }

    /// Executes a contextual chat prompt or `/` Recipe with live token streaming over `EventBus`.
    pub async fn run_chat_or_recipe_streaming(
        &self,
        storage: &StorageEngine,
        events: Option<&EventBus>,
        meeting_id: Option<&str>,
        user_input: &str,
        explicit_recipe_slug: Option<&str>,
    ) -> Result<ChatMessage> {
        let profile = storage.get_user_profile()?;
        let vocab = storage.list_vocabulary()?;
        let prefs = storage.list_learned_preferences()?;
        let ai_settings = storage.get_ai_settings()?;
        let recipes = storage.list_recipes()?;

        let trimmed = user_input.trim();
        let detected_slug = explicit_recipe_slug.map(|s| s.to_string()).or_else(|| {
            if trimmed.starts_with('/') {
                trimmed.split_whitespace().next().map(|s| s.to_string())
            } else {
                None
            }
        });

        let matched_recipe = detected_slug
            .as_ref()
            .and_then(|slug| recipes.iter().find(|r| r.slug.eq_ignore_ascii_case(slug)))
            .cloned();

        if let Some(ref rec) = matched_recipe {
            let _ = storage.increment_recipe_usage(&rec.slug);
        }

        let thread_id = match meeting_id {
            Some(mid) => format!("meeting:{mid}"),
            None => "global:workspace".to_string(),
        };

        let user_msg = ChatMessage {
            id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            thread_id: thread_id.clone(),
            meeting_id: meeting_id.map(|s| s.to_string()),
            role: "user".into(),
            content: trimmed.to_string(),
            recipe_slug: matched_recipe.as_ref().map(|r| r.slug.clone()),
            sources: vec![],
            created_at: Utc::now(),
        };
        storage.insert_chat_message(&user_msg)?;

        let mut sources: Vec<ChatSourceRef> = Vec::new();
        let meeting_detail = match meeting_id {
            Some(mid) => storage.get_meeting_detail(mid)?,
            None => storage
                .list_meetings()?
                .into_iter()
                .find(|m| m.status == MeetingStatus::Completed)
                .and_then(|m| storage.get_meeting_detail(&m.id).ok().flatten()),
        };

        let episodes = storage.list_context_episodes(6)?;
        let clipboard = storage.list_clipboard_items(5)?;
        let search_hits = storage.search_hybrid(trimmed, 5, None, None, None)?;

        if let Some(ref md) = meeting_detail {
            sources.push(ChatSourceRef {
                kind: "meeting".into(),
                ref_id: md.meeting.id.clone(),
                label: md.meeting.title.clone(),
                snippet: md.meeting.executive_summary.chars().take(160).collect(),
            });
            if let Some(slide) = md.screenshares.first() {
                sources.push(ChatSourceRef {
                    kind: "screenshare".into(),
                    ref_id: slide.id.clone(),
                    label: slide.slide_title.clone(),
                    snippet: slide.ocr_text.chars().take(150).collect(),
                });
            }
        }
        for ep in episodes.iter().take(2) {
            sources.push(ChatSourceRef {
                kind: "episode".into(),
                ref_id: ep.id.clone(),
                label: ep.title.clone(),
                snippet: ep.summary.chars().take(150).collect(),
            });
        }
        for hit in search_hits.iter().take(2) {
            if !sources.iter().any(|s| s.label == hit.title) {
                sources.push(ChatSourceRef {
                    kind: hit.source_kind.clone(),
                    ref_id: hit.id.clone(),
                    label: hit.title.clone(),
                    snippet: hit.snippet.chars().take(150).collect(),
                });
            }
        }

        let dossier = render_personalization_dossier(&profile, &vocab, &prefs);
        let rendered_prompt = render_recipe_or_query(
            matched_recipe.as_ref(),
            trimmed,
            &profile,
            meeting_detail.as_ref(),
            &episodes,
            &clipboard,
            &dossier,
        );

        let assistant_msg_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        let stream_ctx = events.map(|bus| (bus, thread_id.as_str(), assistant_msg_id.as_str()));

        let reply_markdown = if let Some((ext_reply, _provider)) = providers::stream_completion(
            &self.http,
            &ai_settings,
            storage.secrets(),
            &dossier,
            &rendered_prompt,
            stream_ctx,
        )
        .await
        {
            ext_reply
        } else {
            let local = synthesize_local_assistant_reply(
                matched_recipe.as_ref(),
                trimmed,
                &profile,
                meeting_detail.as_ref(),
                &episodes,
                &clipboard,
                &search_hits,
            );
            if let Some(bus) = events {
                bus.publish(BacktextEvent::ChatStreamDelta {
                    thread_id: thread_id.clone(),
                    message_id: assistant_msg_id.clone(),
                    delta: local.clone(),
                    done: true,
                });
            }
            local
        };

        let assistant_msg = ChatMessage {
            id: assistant_msg_id,
            thread_id,
            meeting_id: meeting_id.map(|s| s.to_string()),
            role: "assistant".into(),
            content: reply_markdown,
            recipe_slug: matched_recipe.map(|r| r.slug),
            sources,
            created_at: Utc::now(),
        };
        storage.insert_chat_message(&assistant_msg)?;
        Ok(assistant_msg)
    }

    pub async fn run_chat_or_recipe(
        &self,
        storage: &StorageEngine,
        meeting_id: Option<&str>,
        user_input: &str,
        explicit_recipe_slug: Option<&str>,
    ) -> Result<ChatMessage> {
        self.run_chat_or_recipe_streaming(storage, None, meeting_id, user_input, explicit_recipe_slug)
            .await
    }
}

fn extract_structured_actions(
    transcript: &[TranscriptSegment],
    raw_notes: &str,
    self_name: &str,
) -> Vec<ActionItem> {
    let mut items = Vec::new();
    for seg in transcript {
        let low = seg.text.to_lowercase();
        if low.contains("bis ")
            || low.contains("dokumentiere")
            || low.contains("schicke")
            || low.contains("send")
            || low.contains("deliver")
        {
            let due = if low.contains("mittwoch") {
                Some("Mittwoch".into())
            } else if low.contains("donnerstag") {
                Some("Donnerstag".into())
            } else if low.contains("freitag") {
                Some("Freitag".into())
            } else if low.contains("heute") {
                Some("Heute".into())
            } else {
                None
            };
            items.push(ActionItem {
                id: format!("act_{}", uuid::Uuid::new_v4().simple()),
                owner: seg.speaker_name.clone(),
                task: seg.text.clone(),
                due_hint: due,
                completed: false,
            });
        }
    }
    if items.is_empty() && !raw_notes.trim().is_empty() {
        if let Some(last_line) = raw_notes.lines().rev().find(|l| !l.trim().is_empty()) {
            items.push(ActionItem {
                id: format!("act_{}", uuid::Uuid::new_v4().simple()),
                owner: self_name.to_string(),
                task: last_line.trim_start_matches('-').trim().to_string(),
                due_hint: Some("Diese Woche".into()),
                completed: false,
            });
        }
    }
    items.truncate(5);
    items
}

fn render_recipe_or_query(
    recipe: Option<&Recipe>,
    user_input: &str,
    profile: &UserProfile,
    meeting: Option<&MeetingDetail>,
    episodes: &[ContextEpisode],
    clipboard: &[ClipboardItem],
    dossier: &str,
) -> String {
    let transcript = meeting
        .map(|m| {
            m.transcript
                .iter()
                .map(|s| format!("{}: {}", s.speaker_name, s.text))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let user_notes = meeting
        .map(|m| m.meeting.raw_user_notes.clone())
        .unwrap_or_default();
    let screenshare_ocr = meeting
        .map(|m| {
            m.screenshares
                .iter()
                .map(|f| format!("{}: {}", f.slide_title, f.ocr_text))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let participants = meeting
        .map(|m| {
            m.meeting
                .participants
                .iter()
                .map(|p| p.display_name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let active_context = episodes
        .iter()
        .map(|e| format!("- {}: {}", e.title, e.summary))
        .chain(
            clipboard
                .iter()
                .take(2)
                .map(|c| format!("- Clipboard ({}): {}", c.source_app, c.text_preview)),
        )
        .collect::<Vec<_>>()
        .join("\n");

    match recipe {
        Some(rec) => rec
            .prompt_template
            .replace("{{user_name}}", &profile.display_name)
            .replace("{{transcript}}", &transcript)
            .replace("{{user_notes}}", &user_notes)
            .replace("{{screenshare_ocr}}", &screenshare_ocr)
            .replace("{{participants}}", &participants)
            .replace("{{active_context}}", &active_context)
            .replace("{{user_profile}}", dossier),
        None => format!(
            "Frage: {user_input}\n\nMeeting-Kontext:\n{transcript}\n\nArbeitskontext:\n{active_context}"
        ),
    }
}

fn synthesize_local_assistant_reply(
    recipe: Option<&Recipe>,
    user_input: &str,
    profile: &UserProfile,
    meeting: Option<&MeetingDetail>,
    episodes: &[ContextEpisode],
    clipboard: &[ClipboardItem],
    hits: &[MemorySearchHit],
) -> String {
    let is_de = profile.preferred_output_language == Language::De;

    if let Some(rec) = recipe {
        match rec.slug.as_str() {
            "/follow-up-email" => {
                let title = meeting
                    .map(|m| m.meeting.title.as_str())
                    .unwrap_or("unserem heutigen Austausch");
                let attendees = meeting
                    .map(|m| {
                        m.meeting
                            .participants
                            .iter()
                            .filter(|p| !p.is_self)
                            .map(|p| p.display_name.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|| "Team".into());
                let actions_md = meeting
                    .map(|m| {
                        m.meeting
                            .action_items
                            .iter()
                            .map(|a| {
                                format!(
                                    "- **{}:** {} ({})",
                                    a.owner,
                                    a.task,
                                    a.due_hint.as_deref().unwrap_or("offen")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();

                return format!(
                    "**Betreff:** Follow-up & nächste Schritte — {title}\n\nHallo {attendees},\n\nvielen Dank für das produktive Gespräch! Hier sind die wichtigsten Ergebnisse und vereinbarten Schritte im Überblick:\n\n### Kern-Ergebnisse\n{}\n\n### Vereinbarte Action Items\n{}\n\nGebt gerne kurz Bescheid, falls ich etwas ergänzen soll.\n\nViele Grüße\n{}",
                    meeting
                        .map(|m| m.meeting.executive_summary.as_str())
                        .unwrap_or("Wir haben die Architektur- und Umsetzungsdetails abgestimmt."),
                    if actions_md.is_empty() {
                        "- Offene Punkte wie im Call besprochen nachverfolgen.".to_string()
                    } else {
                        actions_md
                    },
                    profile.display_name
                );
            }
            "/action-items" => {
                let mut out = String::from("### Extrahierte Action Items & Ticket-Entwürfe\n\n");
                if let Some(m) = meeting {
                    for (i, a) in m.meeting.action_items.iter().enumerate() {
                        out.push_str(&format!(
                            "{}. **[{}] {}**\n   - *Frist:* `{}` · *Status:* `{}`\n   - *Linear-Snippet:* `[{}] {}`\n",
                            i + 1,
                            a.owner,
                            a.task,
                            a.due_hint.as_deref().unwrap_or("TBD"),
                            if a.completed { "Erledigt" } else { "Offen" },
                            m.meeting.project_tag.as_deref().unwrap_or("BACKTEXT"),
                            a.task
                        ));
                    }
                }
                return out;
            }
            "/critical-review" => {
                return format!(
                    "### Kritische Prüfung: Annahmen & offene Risiken\n\n1. **Plattform-Berechtigungen beim Erststart:** Im Gespräch zu *{}* wurde vorausgesetzt, dass Nutzer `ScreenCaptureKit` und `Accessibility` sofort freigeben — ein geführter Berechtigungs-Check sollte vor dem ersten Call getestet werden.\n2. **Schwellwert-Kalibrierung (`dHash >= 6`):** Funktioniert stark für Präsentationsfolien, sollte aber bei feinen Code-Diffs im Dark-Mode gegen die zusätzliche Text-Jaccard-Prüfung abgesichert bleiben.\n3. **Empfohlene Rückfrage:** Vor Donnerstag klären, ob der Consent-Hinweis pro Kalender-Domain automatisch gemerkt werden soll.",
                    meeting.map(|m| m.meeting.title.as_str()).unwrap_or("aktuellem Projekt")
                );
            }
            "/agent-handoff" => {
                let ep_lines = episodes
                    .iter()
                    .take(3)
                    .map(|e| format!("- **{}**: {}", e.title, e.summary))
                    .collect::<Vec<_>>()
                    .join("\n");
                let clip_lines = clipboard
                    .iter()
                    .take(2)
                    .map(|c| format!("- `{}` ({})", c.text_preview, c.source_app))
                    .collect::<Vec<_>>()
                    .join("\n");
                return format!(
                    "### Agent Context Handoff Pack (`backtext`)\n\n**Aktiver Nutzer-Fokus:** {} ({})\n\n**Letzte Hintergrund-Episoden:**\n{}\n\n**Relevante Zwischenablage-Snippets:**\n{}\n\n**Letzte Meeting-Entscheidung:**\n> {}",
                    profile.role,
                    profile.active_projects.join(", "),
                    ep_lines,
                    clip_lines,
                    meeting
                        .map(|m| m.meeting.executive_summary.as_str())
                        .unwrap_or("Lokale Hybrid-Architektur mit MCP-Anbindung aktiv.")
                );
            }
            "/morning-brief" => {
                let ep_summary = episodes
                    .first()
                    .map(|e| e.title.as_str())
                    .unwrap_or("Architektur & MCP-Integration");
                return format!(
                    "### Guten Morgen, {} — Dein Fokus-Briefing\n\n1. **Wiedereinstieg aus deinem letzten Arbeitskontext:** Zuletzt aktiv an *{}*.\n2. **Offene Zusagen & Follow-ups:** Prüfe die vereinbarten Punkte aus den letzten Calls.\n3. **Anstehender Termin:** Bereite den nächsten Check-in mithilfe des generierten Vorbereitungs-Briefings vor.",
                    profile.display_name, ep_summary
                );
            }
            _ => {
                let rec_title = if is_de { &rec.name_de } else { &rec.name_en };
                let mtg_ctx = meeting
                    .map(|m| format!("**Meeting:** {}\n\n{}", m.meeting.title, m.meeting.enhanced_markdown))
                    .unwrap_or_default();
                return format!(
                    "### Recipe ausgeführt: `{}` ({})\n\n{}\n\n**Berücksichtigte Hintergrund-Episoden:** {}",
                    rec.slug,
                    rec_title,
                    mtg_ctx,
                    episodes.len()
                );
            }
        }
    }

    let mut response = if is_de {
        String::from("Aus deinem erfassten Meeting- und Hintergrund-Gedächtnis:\n\n")
    } else {
        String::from("Based on your captured meeting and ambient work memory:\n\n")
    };

    if !hits.is_empty() {
        for hit in hits.iter().take(3) {
            response.push_str(&format!(
                "- **[{}] {}:** {}\n",
                hit.source_kind, hit.title, hit.snippet
            ));
        }
    } else if let Some(m) = meeting {
        response.push_str(&format!(
            "- **{}:** {}\n",
            m.meeting.title, m.meeting.executive_summary
        ));
    }

    if !user_input.is_empty() && episodes.first().is_some() {
        let ep = &episodes[0];
        response.push_str(&format!(
            "\n**Aktueller Arbeitskontext:** Zuletzt aktiv in *{}* ({}).",
            ep.title,
            ep.primary_apps.join(", ")
        ));
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_slash_recipe_with_grounded_sources_and_streaming_event() {
        let store = StorageEngine::open_in_memory().unwrap();
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        let engine = HybridAiEngine::new();
        let msg = engine
            .run_chat_or_recipe_streaming(
                &store,
                Some(&bus),
                Some("mtg_arch_sync"),
                "/follow-up-email",
                Some("/follow-up-email"),
            )
            .await
            .unwrap();
        assert_eq!(msg.role, "assistant");
        assert!(msg.content.contains("Follow-up"));
        assert!(!msg.sources.is_empty());

        let ev = rx.try_recv().unwrap();
        assert!(matches!(ev, BacktextEvent::ChatStreamDelta { done: true, .. }));
    }

    #[test]
    fn projects_neural_embeddings_to_256_unit_norm() {
        let raw = vec![0.5_f32; 768];
        let proj = providers::project_to_256(&raw);
        assert_eq!(proj.len(), EMBEDDING_DIM);
        let norm: f32 = proj.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }
}
