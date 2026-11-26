pub mod providers;
pub mod stt;

pub use providers::{
    compute_embedding_256, list_ollama_models, list_provider_models, test_provider_connection,
    ProviderTestResult,
};
pub use stt::{
    detect_whisper_cli_path, download_whisper_model, list_whisper_models, stt_readiness_status,
    transcribe_wav_file, TranscribedUtterance,
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
            "Du bist backtext, ein präziser Meeting-Notiz-Enhancer im Granola-Stil. Nutze die eigenen Stichpunkte als Rückgrat und reichere sie mit Fakten aus Transkript und Folien an.\n\
            WICHTIGE FORMATREGELN:\n\
            1. Setze bei wichtigen Aussagen, Entscheidungen und Fakten direkt inline den exakten Transkript-Zeitstempel im Format [MM:SS] (z. B. [00:42], [02:15]), damit der Nutzer per Klick direkt zur Originalstelle im Transkript springen kann.\n\
            2. Wenn im Meeting echte, konkrete Aufgaben vereinbart wurden, liste sie am Ende unter der Überschrift `### Action Items` strikt im Format:\n\
               - [ ] Verantwortlicher: Konkrete Aufgabe (Frist)\n\
               Wenn keine echten Aufgaben vereinbart wurden, lass den Abschnitt `### Action Items` komplett weg.\n\
            3. Erfinde niemals Fakten oder Aufgaben, die nicht in Notizen, Transkript oder Folien vorkommen.\n{dossier}"
        );
        let usr_prompt = format!(
            "Meeting: {}\nEigene Notizen:\n{}\n\nTranskript:\n{}\n\nScreen-Share-OCR:\n{}",
            detail.meeting.title, detail.meeting.raw_user_notes, transcript_text, slides_text
        );

        let fallback_actions = extract_structured_actions(
            &detail.transcript,
            &detail.meeting.raw_user_notes,
            &profile.display_name,
        );

        let outcome = providers::stream_completion(
            &self.http,
            &ai_settings,
            storage.secrets(),
            &sys_prompt,
            &usr_prompt,
            None,
        )
        .await;

        if let Some((llm_md, provider_used)) = outcome.result {
            let (cleaned_md, parsed_llm_actions) =
                parse_and_strip_llm_action_items(&llm_md, &profile.display_name);
            let exec_summary = cleaned_md
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .take(2)
                .collect::<Vec<_>>()
                .join(" ");
            let mut updated = detail.meeting.clone();
            updated.executive_summary = exec_summary.clone();
            updated.enhanced_markdown = cleaned_md.clone();
            // Always trust LLM's parsed action items (even if empty -> replaces stale false positives!)
            updated.action_items = parsed_llm_actions;
            storage.save_meeting(&updated)?;
            return Ok(EnhancedMeetingResult {
                executive_summary: exec_summary,
                enhanced_markdown: cleaned_md,
                action_items: updated.action_items,
                provider_used,
            });
        }

        let extracted_actions = fallback_actions;
        let provider_error_banner = outcome.last_error.map(|err| {
            format!("*Hinweis ({err}) — lokale Offline-Synthese verwendet.*\n\n")
        });

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

        let mut md = provider_error_banner.unwrap_or_default();
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
                        "- **{}** [{:02}:{:02}] — *{}*: {}\n",
                        b,
                        seg.start_ms / 60_000,
                        (seg.start_ms % 60_000) / 1000,
                        seg.speaker_name,
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
            for seg in detail.transcript.iter().take(6) {
                md.push_str(&format!(
                    "- **{}** [{:02}:{:02}]: {}\n",
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
                    "- **{}** [{:02}:{:02}] ({}): {}\n",
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
        updated.action_items = extracted_actions;
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

        let outcome = providers::stream_completion(
            &self.http,
            &ai_settings,
            storage.secrets(),
            &dossier,
            &rendered_prompt,
            stream_ctx,
        )
        .await;

        let reply_markdown = if let Some((ext_reply, _provider)) = outcome.result {
            ext_reply
        } else {
            let mut local = synthesize_local_assistant_reply(
                matched_recipe.as_ref(),
                trimmed,
                &profile,
                meeting_detail.as_ref(),
                &episodes,
                &clipboard,
                &search_hits,
            );
            if let Some(err) = outcome.last_error {
                local = format!(
                    "*KI-Provider Hinweis ({err}) — lokaler Offline-Assistent genutzt.*\n\n{local}"
                );
            }
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

fn is_action_items_heading(trimmed: &str) -> bool {
    let lower = trimmed.to_lowercase();
    let is_heading_like = trimmed.starts_with('#')
        || (trimmed.starts_with("**") && trimmed.ends_with("**"))
        || (trimmed.starts_with("__") && trimmed.ends_with("__"));
    if !is_heading_like {
        return false;
    }
    let clean = lower
        .trim_start_matches('#')
        .trim()
        .trim_matches('*')
        .trim_matches('_')
        .trim_end_matches(':')
        .trim();
    clean == "action items"
        || clean == "action-items"
        || clean == "aufgaben"
        || clean == "vereinbarte aufgaben"
        || clean == "vereinbarte action items"
        || clean == "offene aufgaben"
        || clean == "to-dos"
        || clean == "todos"
        || clean == "next steps"
        || clean == "nächste schritte"
}

fn is_any_heading(trimmed: &str) -> bool {
    trimmed.starts_with('#')
        || (trimmed.starts_with("**")
            && trimmed.ends_with("**")
            && trimmed.len() > 4
            && !trimmed.contains('\n'))
}

fn strip_checkbox_prefix(trimmed: &str) -> Option<(bool, &str)> {
    // Handles: "- [ ] ...", "* [x] ...", "• [ ] ...", "1. [ ] ...", "[ ] ..."
    let mut rest = trimmed;
    if let Some(r) = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("* "))
        .or_else(|| rest.strip_prefix("• "))
    {
        rest = r.trim_start();
    } else if let Some(dot_idx) = rest.find(". ") {
        if dot_idx > 0 && dot_idx <= 3 && rest[..dot_idx].chars().all(|c| c.is_ascii_digit()) {
            rest = rest[dot_idx + 2..].trim_start();
        }
    }

    if let Some(after) = rest.strip_prefix("[ ]") {
        Some((false, after.trim()))
    } else if let Some(after) = rest
        .strip_prefix("[x]")
        .or_else(|| rest.strip_prefix("[X]"))
        .or_else(|| rest.strip_prefix("[✓]"))
    {
        Some((true, after.trim()))
    } else {
        None
    }
}

fn strip_bullet_prefix(trimmed: &str) -> Option<&str> {
    if let Some(r) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("• "))
    {
        return Some(r.trim());
    }
    if let Some(dot_idx) = trimmed.find(". ") {
        if dot_idx > 0 && dot_idx <= 3 && trimmed[..dot_idx].chars().all(|c| c.is_ascii_digit()) {
            return Some(trimmed[dot_idx + 2..].trim());
        }
    }
    None
}

fn parse_action_item_body(body: &str, completed: bool, default_owner: &str) -> Option<ActionItem> {
    let clean_body = body.trim();
    if clean_body.len() < 4 {
        return None;
    }
    let low = clean_body.to_lowercase();
    if low.starts_with("keine ")
        || low == "keine"
        || low == "none"
        || low == "n/a"
        || low.starts_with("no action items")
    {
        return None;
    }

    // Extract optional trailing "(Frist)" hint
    let mut main_part = clean_body;
    let mut due_hint: Option<String> = None;
    if main_part.ends_with(')') {
        if let Some(open_idx) = main_part.rfind('(') {
            let candidate_due = main_part[open_idx + 1..main_part.len() - 1].trim();
            if candidate_due.len() >= 2
                && candidate_due.len() <= 42
                && !candidate_due.contains("http")
            {
                let before = main_part[..open_idx].trim();
                if before.len() >= 4 {
                    due_hint = Some(candidate_due.to_string());
                    main_part = before;
                }
            }
        }
    }

    // Split Owner & Task on ": ", " — ", " – ", or " - " (when owner prefix is short & plausible)
    let mut owner = default_owner.to_string();
    let mut task = main_part.to_string();
    for sep in [" — ", " – ", ": ", " - "] {
        if let Some(idx) = main_part.find(sep) {
            let left = main_part[..idx]
                .trim()
                .trim_matches('*')
                .trim_matches('`')
                .trim_matches('[')
                .trim_matches(']')
                .trim();
            let right = main_part[idx + sep.len()..].trim();
            let left_words = left.split_whitespace().count();
            if !left.is_empty()
                && left.len() <= 36
                && left_words <= 4
                && !right.is_empty()
                && !left.contains("http")
            {
                owner = left.to_string();
                task = right.to_string();
                break;
            }
        }
    }

    // Clean any residual markdown escapes in task text
    let task_clean = task
        .replace("\\*", "*")
        .replace("\\_", "_")
        .trim()
        .to_string();
    if task_clean.is_empty() {
        return None;
    }

    Some(ActionItem {
        id: format!("act_{}", uuid::Uuid::new_v4().simple()),
        owner,
        task: task_clean,
        due_hint,
        completed,
    })
}

/// Extracts structured `ActionItem` entries from LLM markdown output (e.g. `### Action Items`
/// with `- [ ] Owner — Task (Due)` or `- [ ] Owner: Task (Due)`) and strips the duplicated
/// raw markdown action block so the UI renders interactive checkboxes cleanly without `- [ ]` noise.
pub fn parse_and_strip_llm_action_items(
    llm_md: &str,
    default_owner: &str,
) -> (String, Vec<ActionItem>) {
    let lines: Vec<&str> = llm_md.lines().collect();
    let mut keep_mask = vec![true; lines.len()];
    let mut items: Vec<ActionItem> = Vec::new();
    let mut in_action_section = false;
    let mut action_heading_idx: Option<usize> = None;
    let mut section_parsed_any = false;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if is_action_items_heading(trimmed) {
            in_action_section = true;
            action_heading_idx = Some(idx);
            section_parsed_any = false;
            continue;
        }

        if in_action_section && is_any_heading(trimmed) && !is_action_items_heading(trimmed) {
            if section_parsed_any {
                if let Some(h_idx) = action_heading_idx {
                    keep_mask[h_idx] = false;
                }
            }
            in_action_section = false;
            action_heading_idx = None;
        }

        if let Some((completed, body)) = strip_checkbox_prefix(trimmed) {
            if let Some(item) = parse_action_item_body(body, completed, default_owner) {
                items.push(item);
                keep_mask[idx] = false;
                if in_action_section {
                    section_parsed_any = true;
                }
            }
            continue;
        }

        if in_action_section {
            if trimmed.is_empty() {
                continue;
            }
            if let Some(body) = strip_bullet_prefix(trimmed) {
                if let Some(item) = parse_action_item_body(body, false, default_owner) {
                    items.push(item);
                    keep_mask[idx] = false;
                    section_parsed_any = true;
                } else {
                    // e.g. "- Keine Action Items" -> hide boilerplate inside action section
                    keep_mask[idx] = false;
                    section_parsed_any = true;
                }
            }
        }
    }

    if in_action_section && section_parsed_any {
        if let Some(h_idx) = action_heading_idx {
            keep_mask[h_idx] = false;
        }
    }

    let mut cleaned_lines: Vec<&str> = Vec::with_capacity(lines.len());
    for (idx, line) in lines.iter().enumerate() {
        if keep_mask[idx] {
            // Collapse duplicate blank lines produced by stripping
            if line.trim().is_empty()
                && cleaned_lines
                    .last()
                    .map(|l| l.trim().is_empty())
                    .unwrap_or(false)
            {
                continue;
            }
            cleaned_lines.push(line);
        }
    }
    while cleaned_lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        cleaned_lines.pop();
    }

    items.truncate(12);
    (cleaned_lines.join("\n"), items)
}

fn extract_structured_actions(
    transcript: &[TranscriptSegment],
    raw_notes: &str,
    self_name: &str,
) -> Vec<ActionItem> {
    let mut items = Vec::new();
    for seg in transcript {
        let low = seg.text.to_lowercase();
        if low.contains("bis mittwoch")
            || low.contains("bis donnerstag")
            || low.contains("bis freitag")
            || low.contains("bis morgen")
            || low.contains("ich übernehme")
            || low.contains("dokumentiere")
            || low.contains("schicke")
            || low.contains("i will send")
            || low.contains("i'll send")
            || low.contains("action item")
        {
            let due = if low.contains("mittwoch") {
                Some("Mittwoch".into())
            } else if low.contains("donnerstag") {
                Some("Donnerstag".into())
            } else if low.contains("freitag") {
                Some("Freitag".into())
            } else if low.contains("morgen") {
                Some("Morgen".into())
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

    // Only parse explicit task syntax from raw user notes — never fabricate tasks from arbitrary notes!
    for line in raw_notes.lines() {
        let trimmed = line.trim();
        if let Some((completed, body)) = strip_checkbox_prefix(trimmed) {
            if let Some(item) = parse_action_item_body(body, completed, self_name) {
                items.push(item);
            }
            continue;
        }
        let low = trimmed.to_lowercase();
        for prefix in ["todo:", "to-do:", "action:", "aufgabe:"] {
            if low.starts_with(prefix) {
                let body = trimmed[prefix.len()..].trim();
                if let Some(item) = parse_action_item_body(body, false, self_name) {
                    items.push(item);
                }
                break;
            }
        }
    }

    items.truncate(8);
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
                .map(|f| {
                    format!(
                        "[{:02}:{:02}] {}: {}",
                        f.captured_at_ms / 60_000,
                        (f.captured_at_ms % 60_000) / 1000,
                        f.slide_title,
                        f.ocr_text
                    )
                })
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
            "Frage: {user_input}\n\nHinweis: Wenn du dich auf Aussagen aus dem Transkript beziehst, nenne direkt den exakten Zeitstempel im Format [MM:SS].\n\nMeeting-Kontext:\n{transcript}\n\nArbeitskontext:\n{active_context}"
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

    #[test]
    fn parses_and_strips_llm_action_items_from_markdown() {
        let raw_md = "### Kontext\n- Der Film „Paprika“ läuft im Hintergrund [00:12].\n\n### Action Items\n- [ ] Luka — Aufnahme als privat markieren/verwerfen (sofort)\n- [x] **Clara Novak**: MCP Spec teilen (Freitag)\n";
        let (cleaned, items) = parse_and_strip_llm_action_items(raw_md, "Luka");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].owner, "Luka");
        assert_eq!(items[0].task, "Aufnahme als privat markieren/verwerfen");
        assert_eq!(items[0].due_hint.as_deref(), Some("sofort"));
        assert!(!items[0].completed);

        assert_eq!(items[1].owner, "Clara Novak");
        assert_eq!(items[1].task, "MCP Spec teilen");
        assert_eq!(items[1].due_hint.as_deref(), Some("Freitag"));
        assert!(items[1].completed);

        assert!(cleaned.contains("[00:12]"));
        assert!(!cleaned.contains("### Action Items"));
        assert!(!cleaned.contains("[ ]"));
    }

    #[test]
    fn offline_action_extractor_does_not_fabricate_tasks_from_casual_notes() {
        let items = extract_structured_actions(&[], "Ich schaue gerade den Film „Paprika“", "Luka");
        assert!(items.is_empty());

        let explicit = extract_structured_actions(
            &[],
            "- [ ] Luka: Datenschutz prüfen (Heute)\nTODO: Clara — Slides versenden (Freitag)",
            "Luka",
        );
        assert_eq!(explicit.len(), 2);
        assert_eq!(explicit[0].task, "Datenschutz prüfen");
        assert_eq!(explicit[0].due_hint.as_deref(), Some("Heute"));
        assert_eq!(explicit[1].owner, "Clara");
        assert_eq!(explicit[1].task, "Slides versenden");
    }
}
