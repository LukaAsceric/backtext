use anyhow::Result;
use backtext_ai::{EnhancedMeetingResult, HybridAiEngine};
use backtext_context::{
    compact_snapshots_to_episode, evaluate_clipboard_capture, evaluate_context_candidate,
    preview_filter_on_sample, FilterDecision, RawContextCandidate, ScrubberPreview,
};
use backtext_core::*;
use backtext_mcp::{McpClientConfigSnippets, McpServer};
use backtext_meeting::{ScreenShareDeduplicator, SpeakerAttributionSession};
use backtext_platform::{default_platform_sensors, PermissionStatus, PlatformSensors};
use backtext_storage::StorageEngine;
use chrono::{Duration, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

struct LiveMeetingRuntime {
    attribution: SpeakerAttributionSession,
    screenshare_dedup: ScreenShareDeduplicator,
    sim_step: usize,
    elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapPayload {
    pub capture_status: CaptureStatus,
    pub permissions: PermissionStatus,
    pub profile: UserProfile,
    pub meetings: Vec<Meeting>,
    pub episodes: Vec<ContextEpisode>,
    pub snapshots: Vec<ContextSnapshot>,
    pub clipboard: Vec<ClipboardItem>,
    pub persons: Vec<Person>,
    pub recipes: Vec<Recipe>,
    pub privacy_rules: Vec<PrivacyRule>,
    pub vocabulary: Vec<VocabularyEntry>,
    pub learned_preferences: Vec<LearnedPreference>,
    pub mcp_policy: McpGovernancePolicy,
    pub mcp_snippets: McpClientConfigSnippets,
    pub mcp_audit_logs: Vec<McpAuditEntry>,
    pub ai_settings: AiProviderSettings,
}

#[derive(Clone)]
pub struct AppController {
    pub storage: StorageEngine,
    pub events: EventBus,
    pub ai: HybridAiEngine,
    pub mcp: McpServer,
    pub sensors: Arc<dyn PlatformSensors>,
    capture_status: Arc<Mutex<CaptureStatus>>,
    live_sessions: Arc<Mutex<HashMap<String, LiveMeetingRuntime>>>,
}

impl AppController {
    pub fn open_default() -> Result<Self> {
        let db_path = std::env::var("BACKTEXT_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".backtext-data").join("backtext.db")
            });
        Self::open_at(db_path)
    }

    pub fn open_at(db_path: PathBuf) -> Result<Self> {
        let storage = StorageEngine::open(db_path)?;
        let events = EventBus::default();
        let ai = HybridAiEngine::new();
        let mcp = McpServer::new(storage.clone(), events.clone());
        let sensors = default_platform_sensors();
        Ok(Self {
            storage,
            events,
            ai,
            mcp,
            sensors,
            capture_status: Arc::new(Mutex::new(CaptureStatus::Active)),
            live_sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn bootstrap(&self) -> Result<BootstrapPayload> {
        let permissions = self.sensors.check_permissions().await;
        let status = self.current_capture_status();
        let bin_hint = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|dir| dir.join("backtext-mcp").display().to_string()));
        Ok(BootstrapPayload {
            capture_status: status,
            permissions,
            profile: self.storage.get_user_profile()?,
            meetings: self.storage.list_meetings()?,
            episodes: self.storage.list_context_episodes(25)?,
            snapshots: self.storage.list_recent_snapshots(30)?,
            clipboard: self.storage.list_clipboard_items(20)?,
            persons: self.storage.list_persons()?,
            recipes: self.storage.list_recipes()?,
            privacy_rules: self.storage.list_privacy_rules()?,
            vocabulary: self.storage.list_vocabulary()?,
            learned_preferences: self.storage.list_learned_preferences()?,
            mcp_policy: self.storage.get_mcp_policy()?,
            mcp_snippets: self.mcp.client_config_snippets(bin_hint.as_deref())?,
            mcp_audit_logs: self.storage.list_mcp_audit_logs(40)?,
            ai_settings: self.storage.get_ai_settings()?,
        })
    }

    pub fn current_capture_status(&self) -> CaptureStatus {
        let mut guard = self.capture_status.lock();
        if let CaptureStatus::Paused { until: Some(until_dt), .. } = &*guard {
            if Utc::now() >= *until_dt {
                *guard = CaptureStatus::Active;
            }
        }
        guard.clone()
    }

    pub fn set_capture_pause(&self, minutes: Option<i64>, reason: Option<String>) -> CaptureStatus {
        let next = match minutes {
            None => CaptureStatus::Active,
            Some(0) => CaptureStatus::Paused {
                until: None,
                reason: reason.or_else(|| Some("Bis zum manuellen Fortsetzen pausiert".into())),
            },
            Some(m) => CaptureStatus::Paused {
                until: Some(Utc::now() + Duration::minutes(m)),
                reason: reason.or_else(|| Some(format!("Für {m} Minuten pausiert"))),
            },
        };
        *self.capture_status.lock() = next.clone();
        self.events.publish(BacktextEvent::CaptureStatusChanged(next.clone()));
        next
    }

    pub fn start_live_meeting(
        &self,
        existing_meeting_id: Option<String>,
        title: Option<String>,
        detected_app: Option<String>,
    ) -> Result<MeetingDetail> {
        let profile = self.storage.get_user_profile()?;
        let now = Utc::now();

        let mut meeting = if let Some(ref mid) = existing_meeting_id {
            if let Some(detail) = self.storage.get_meeting_detail(mid)? {
                let mut m = detail.meeting;
                m.status = MeetingStatus::Live;
                m.started_at = now;
                m
            } else {
                anyhow::bail!("Meeting '{mid}' not found");
            }
        } else {
            let new_id = format!("mtg_{}", uuid::Uuid::new_v4().simple());
            Meeting {
                id: new_id,
                title: title
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| "Live Produkt- & Agenten-Sync".to_string()),
                calendar_event_id: None,
                detected_app: detected_app.unwrap_or_else(|| "Zoom (Auto-Erkennung ohne Bot)".into()),
                project_tag: profile.active_projects.first().cloned(),
                started_at: now,
                ended_at: None,
                status: MeetingStatus::Live,
                template_id: "general".into(),
                prep_brief: Some("Bot-freie Dual-Channel-Aufnahme aktiv (Kanal 0: Eigenes Mikrofon · Kanal 1: ScreenCaptureKit System-Audio).".into()),
                raw_user_notes: String::new(),
                executive_summary: String::new(),
                enhanced_markdown: String::new(),
                action_items: vec![],
                participants: vec![MeetingParticipant {
                    speaker_label: "SELF".into(),
                    display_name: format!("{} (Ich)", profile.display_name),
                    person_id: None,
                    email: None,
                    is_self: true,
                    identification_source: IdentificationSource::MicSelf,
                    confidence: 1.0,
                    talk_time_ms: 0,
                }],
                transcript_count: 0,
                screenshare_count: 0,
            }
        };

        if !meeting.participants.iter().any(|p| p.is_self) {
            meeting.participants.insert(
                0,
                MeetingParticipant {
                    speaker_label: "SELF".into(),
                    display_name: format!("{} (Ich)", profile.display_name),
                    person_id: None,
                    email: None,
                    is_self: true,
                    identification_source: IdentificationSource::MicSelf,
                    confidence: 1.0,
                    talk_time_ms: 0,
                },
            );
        }

        self.storage.save_meeting(&meeting)?;

        let cal_attendees: Vec<(String, String)> = meeting
            .participants
            .iter()
            .filter(|p| !p.is_self)
            .map(|p| {
                (
                    p.display_name.clone(),
                    p.email.clone().unwrap_or_else(|| "invitee@company.io".into()),
                )
            })
            .collect();

        self.live_sessions.lock().insert(
            meeting.id.clone(),
            LiveMeetingRuntime {
                attribution: SpeakerAttributionSession::new(
                    &meeting.id,
                    &profile.display_name,
                    cal_attendees,
                ),
                screenshare_dedup: ScreenShareDeduplicator::default(),
                sim_step: 0,
                elapsed_ms: 0,
            },
        );

        let status = CaptureStatus::RecordingMeeting {
            meeting_id: meeting.id.clone(),
            meeting_title: meeting.title.clone(),
            started_at: now,
        };
        *self.capture_status.lock() = status.clone();
        self.events.publish(BacktextEvent::CaptureStatusChanged(status));

        self.storage
            .get_meeting_detail(&meeting.id)?
            .ok_or_else(|| anyhow::anyhow!("Failed to load started meeting"))
    }

    /// Feeds either a real dictated/transcribed audio chunk OR a realistic multi-modal simulation step
    /// (Mic Self, Voiceprint Recognition, Visual Active-Speaker Recognition, and Screen-Share OCR Slide).
    pub fn simulate_live_meeting_activity(&self, meeting_id: &str) -> Result<MeetingDetail> {
        let profile = self.storage.get_user_profile()?;
        let mut sessions = self.live_sessions.lock();
        let rt = sessions
            .entry(meeting_id.to_string())
            .or_insert_with(|| LiveMeetingRuntime {
                attribution: SpeakerAttributionSession::new(
                    meeting_id,
                    &profile.display_name,
                    vec![("Clara Novak".into(), "clara.novak@velox-ai.com".into())],
                ),
                screenshare_dedup: ScreenShareDeduplicator::default(),
                sim_step: 0,
                elapsed_ms: 0,
            });

        let step = rt.sim_step % 4;
        rt.sim_step += 1;
        let start_ms = rt.elapsed_ms + 2_000;
        let end_ms = start_ms + 14_000;
        rt.elapsed_ms = end_ms;

        let (channel, text, emb, visual_hint, maybe_slide) = match step {
            0 => (
                AudioChannel::Microphone,
                "Willkommen! Lass uns kurz auf die Live-Integration unseres MCP-Servers mit deinen Coding-Agenten schauen.",
                vec![0.0; EMBEDDING_DIM],
                None,
                None,
            ),
            1 => (
                AudioChannel::System,
                "Sehr gerne! Ich teile kurz meinen Bildschirm mit den Messwerten aus Cursor: Wenn get_live_context aktiv ist, sparen wir uns jedes manuelle Kopieren von Tickets und Meeting-Notizen.",
                embed_text_local_256("voiceprint::clara_novak_clear_mezzo_en_de_210hz"),
                None,
                Some((
                    0x91A2_F00D_77CC_1011_u64,
                    "Cursor + backtext MCP Workflow",
                    vec![
                        "Zero-Prompt Backstory: Agent liest aktive Episode & letztes Meeting automatisch".to_string(),
                        "Durchschnittliche Zeitersparnis pro Feature-Ticket: ~11 Minuten".to_string(),
                        "Datenschutz: API-Keys & 1Password wurden zu 100% lokal herausgefiltert".to_string(),
                    ],
                    Some("Clara Novak".to_string()),
                )),
            ),
            2 => (
                AudioChannel::System,
                "Und von meiner Seite aus dem Security-Team: Super, dass wir über das Audit-Log jederzeit sehen, welcher Client auf Transkripte zugreift. Ich schicke bis Freitag die Freigabe-Checkliste.",
                embed_text_local_256("voiceprint::jan_berger_new_security_lead_140hz"),
                Some("Jan Berger"),
                None,
            ),
            _ => (
                AudioChannel::Microphone,
                "Klasse, danke euch beiden! Ich übernehme bis morgen die erweiterte `/agent-handoff` Recipe-Vorlage und stelle das Update bereit.",
                vec![0.0; EMBEDDING_DIM],
                None,
                Some((
                    0x22EE_8811_00AA_55FF_u64,
                    "Vereinbarte Roadmap & Rollout",
                    vec![
                        "Luka: /agent-handoff Recipe mit Clipboard-Filter bis morgen ausrollen".to_string(),
                        "Jan Berger: Security-Freigabe-Checkliste bis Freitag senden".to_string(),
                        "Clara Novak: Multi-Meeting-Test über 10 Kunden-Calls starten".to_string(),
                    ],
                    Some("Clara Novak".to_string()),
                )),
            ),
        };

        // Trigger twice across settle threshold so ScreenShareDeduplicator validates stability
        if let Some((dhash, slide_title, ocr_lines, presenter)) = maybe_slide {
            let _ = rt.screenshare_dedup.should_capture_frame(dhash, start_ms);
            if rt
                .screenshare_dedup
                .should_capture_frame(dhash, start_ms + 1_800)
            {
                let frame = ScreenShareDeduplicator::build_screenshare_frame(
                    meeting_id,
                    start_ms + 1_800,
                    presenter,
                    dhash,
                    slide_title,
                    &ocr_lines,
                );
                self.storage.insert_screenshare_frame(&frame)?;
                self.events.publish(BacktextEvent::ScreenShareCaptured(frame));
            }
        }

        let (seg, participant) = rt.attribution.attribute_segment(
            &self.storage,
            channel,
            start_ms,
            end_ms,
            text,
            &emb,
            visual_hint,
        )?;
        // Repeat second vote for visual speaker promotion if new speaker
        if let Some(vh) = visual_hint {
            let (_, promoted_part) = rt.attribution.attribute_segment(
                &self.storage,
                channel,
                end_ms,
                end_ms + 100,
                text,
                &emb,
                Some(vh),
            )?;
            let mut seg_final = seg;
            seg_final.speaker_name = promoted_part.display_name.clone();
            seg_final.person_id = promoted_part.person_id.clone();
            self.storage.insert_transcript_segment(&seg_final)?;
            if let Some(mut detail) = self.storage.get_meeting_detail(meeting_id)? {
                upsert_participant_vec(&mut detail.meeting.participants, promoted_part);
                self.storage.save_meeting(&detail.meeting)?;
            }
        } else {
            self.storage.insert_transcript_segment(&seg)?;
            if let Some(mut detail) = self.storage.get_meeting_detail(meeting_id)? {
                upsert_participant_vec(&mut detail.meeting.participants, participant);
                self.storage.save_meeting(&detail.meeting)?;
            }
        }

        drop(sessions);
        self.storage
            .get_meeting_detail(meeting_id)?
            .ok_or_else(|| anyhow::anyhow!("Meeting not found"))
    }

    pub async fn stop_and_summarize_meeting(&self, meeting_id: &str) -> Result<MeetingDetail> {
        self.live_sessions.lock().remove(meeting_id);
        if let Some(mut detail) = self.storage.get_meeting_detail(meeting_id)? {
            detail.meeting.status = MeetingStatus::Completed;
            detail.meeting.ended_at = Some(Utc::now());
            self.storage.save_meeting(&detail.meeting)?;
        }

        *self.capture_status.lock() = CaptureStatus::Active;
        self.events
            .publish(BacktextEvent::CaptureStatusChanged(CaptureStatus::Active));

        let _ = self.ai.enhance_meeting_summary(&self.storage, meeting_id).await?;
        self.storage
            .get_meeting_detail(meeting_id)?
            .ok_or_else(|| anyhow::anyhow!("Meeting not found after summary"))
    }

    pub async fn enhance_meeting(&self, meeting_id: &str) -> Result<EnhancedMeetingResult> {
        self.ai.enhance_meeting_summary(&self.storage, meeting_id).await
    }

    pub fn toggle_action_item(&self, meeting_id: &str, action_id: &str) -> Result<MeetingDetail> {
        let mut detail = self
            .storage
            .get_meeting_detail(meeting_id)?
            .ok_or_else(|| anyhow::anyhow!("Meeting not found"))?;
        for item in &mut detail.meeting.action_items {
            if item.id == action_id {
                item.completed = !item.completed;
            }
        }
        self.storage.save_meeting(&detail.meeting)?;
        self.storage
            .get_meeting_detail(meeting_id)?
            .ok_or_else(|| anyhow::anyhow!("Meeting not found"))
    }

    pub fn capture_ambient_sample(
        &self,
        app_name: String,
        window_title: String,
        browser_url: Option<String>,
        visible_text: String,
        clipboard_text: Option<String>,
    ) -> Result<Value> {
        if matches!(self.current_capture_status(), CaptureStatus::Paused { .. }) {
            return Ok(serde_json::json!({
                "decision": "paused",
                "reason": "Hintergrund-Kontexterfassung ist aktuell pausiert."
            }));
        }

        let rules = self.storage.list_privacy_rules()?;
        let profile = self.storage.get_user_profile()?;
        let vocab = self.storage.list_vocabulary()?;
        let prev = self.storage.list_recent_snapshots(1)?.into_iter().next();

        let candidate = RawContextCandidate {
            app_bundle_id: format!("app.{}", app_name.to_lowercase().replace(' ', "")),
            app_name: app_name.clone(),
            window_title,
            browser_url,
            dwell_ms: 18_000,
            dhash_u64: Utc::now().timestamp_millis() as u64,
            visible_text,
            extraction_method: ExtractionMethod::Accessibility,
            is_private_browsing: false,
            is_secure_field_focused: false,
        };

        let decision = evaluate_context_candidate(candidate, prev.as_ref(), &rules, &profile, &vocab);
        if let FilterDecision::Accepted { ref snapshot } = decision {
            self.storage.insert_context_snapshot(snapshot)?;
            let recent = self.storage.list_recent_snapshots(4)?;
            if let Some(ep) = compact_snapshots_to_episode(&recent) {
                self.storage.upsert_context_episode(&ep)?;
            }
        }

        if let Some(clip) = clipboard_text.filter(|c| !c.trim().is_empty()) {
            let prev_hash = self
                .storage
                .list_clipboard_items(1)?
                .into_iter()
                .next()
                .map(|c| c.content_hash);
            if let Some(item) = evaluate_clipboard_capture(
                &app_name,
                &clip,
                false,
                prev_hash.as_deref(),
                &rules,
                &profile,
                &vocab,
            ) {
                self.storage.insert_clipboard_item(&item)?;
            }
        }

        Ok(serde_json::to_value(decision)?)
    }

    pub fn preview_privacy_filter(
        &self,
        app_name: &str,
        window_title: &str,
        browser_url: Option<&str>,
        raw_text: &str,
    ) -> Result<ScrubberPreview> {
        let rules = self.storage.list_privacy_rules()?;
        let profile = self.storage.get_user_profile()?;
        let vocab = self.storage.list_vocabulary()?;
        Ok(preview_filter_on_sample(
            app_name,
            window_title,
            browser_url,
            raw_text,
            &rules,
            &profile,
            &vocab,
        ))
    }

    /// Dispatches a named command from either Tauri IPC or the HTTP JSON bridge.
    pub async fn dispatch_rpc(&self, cmd: &str, args: Value) -> Result<Value> {
        match cmd {
            "get_bootstrap" => Ok(serde_json::to_value(self.bootstrap().await?)?),
            "set_capture_pause" => {
                let minutes = args.get("minutes").and_then(|v| v.as_i64());
                let reason = args.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
                Ok(serde_json::to_value(self.set_capture_pause(minutes, reason))?)
            }
            "get_meeting_detail" => {
                let id = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::to_value(self.storage.get_meeting_detail(id)?)?)
            }
            "start_live_meeting" => {
                let existing = args
                    .get("existingMeetingId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let title = args.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
                let app = args
                    .get("detectedApp")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Ok(serde_json::to_value(
                    self.start_live_meeting(existing, title, app)?,
                )?)
            }
            "simulate_live_meeting_activity" => {
                let id = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::to_value(self.simulate_live_meeting_activity(id)?)?)
            }
            "stop_and_summarize_meeting" => {
                let id = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::to_value(self.stop_and_summarize_meeting(id).await?)?)
            }
            "update_meeting_notes" => {
                let id = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let notes = args.get("rawNotes").and_then(|v| v.as_str()).unwrap_or("");
                self.storage.update_meeting_user_notes(id, notes)?;
                Ok(serde_json::json!({ "ok": true }))
            }
            "enhance_meeting" => {
                let id = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let res = self.enhance_meeting(id).await?;
                Ok(serde_json::to_value(res)?)
            }
            "toggle_action_item" => {
                let mid = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let aid = args.get("actionId").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::to_value(self.toggle_action_item(mid, aid)?)?)
            }
            "assign_speaker" => {
                let mid = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let label = args.get("speakerLabel").and_then(|v| v.as_str()).unwrap_or("");
                let name = args.get("personName").and_then(|v| v.as_str()).unwrap_or("");
                let email = args.get("email").and_then(|v| v.as_str());
                let _person = self
                    .storage
                    .assign_speaker_identity(mid, label, name, email, None)?;
                Ok(serde_json::to_value(self.storage.get_meeting_detail(mid)?)?)
            }
            "run_chat_or_recipe" => {
                let mid = args.get("meetingId").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                let input = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
                let slug = args.get("recipeSlug").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                let reply = self
                    .ai
                    .run_chat_or_recipe(&self.storage, mid, input, slug)
                    .await?;
                Ok(serde_json::to_value(reply)?)
            }
            "get_chat_messages" => {
                let thread_id = args
                    .get("threadId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("global:workspace");
                Ok(serde_json::to_value(
                    self.storage.list_chat_messages(thread_id)?,
                )?)
            }
            "save_recipe" => {
                let rec: Recipe = serde_json::from_value(args.get("recipe").cloned().unwrap_or_default())?;
                self.storage.upsert_recipe(&rec)?;
                Ok(serde_json::to_value(self.storage.list_recipes()?)?)
            }
            "delete_recipe" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                self.storage.delete_recipe(id)?;
                Ok(serde_json::to_value(self.storage.list_recipes()?)?)
            }
            "search_memory" => {
                let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let kind = args.get("sourceKind").and_then(|v| v.as_str());
                let proj = args.get("project").and_then(|v| v.as_str());
                Ok(serde_json::to_value(
                    self.storage.search_hybrid(q, 20, kind, proj, None)?,
                )?)
            }
            "capture_ambient_sample" => {
                let app = args.get("appName").and_then(|v| v.as_str()).unwrap_or("Arc Browser").to_string();
                let title = args.get("windowTitle").and_then(|v| v.as_str()).unwrap_or("Dokumentation").to_string();
                let url = args.get("browserUrl").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                let text = args.get("visibleText").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let clip = args.get("clipboardText").and_then(|v| v.as_str()).map(|s| s.to_string());
                self.capture_ambient_sample(app, title, url, text, clip)
            }
            "preview_privacy_filter" => {
                let app = args.get("appName").and_then(|v| v.as_str()).unwrap_or("Cursor");
                let title = args.get("windowTitle").and_then(|v| v.as_str()).unwrap_or("Workspace");
                let url = args.get("browserUrl").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                let raw = args.get("rawText").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::to_value(
                    self.preview_privacy_filter(app, title, url, raw)?,
                )?)
            }
            "purge_recent_context" => {
                let mins = args.get("minutes").and_then(|v| v.as_i64()).unwrap_or(15);
                let deleted = self.storage.purge_recent_context(mins)?;
                Ok(serde_json::json!({ "deleted": deleted }))
            }
            "save_user_profile" => {
                let prof: UserProfile =
                    serde_json::from_value(args.get("profile").cloned().unwrap_or_default())?;
                self.storage.save_user_profile(&prof)?;
                Ok(serde_json::to_value(self.storage.get_user_profile()?)?)
            }
            "enroll_self_voiceprint" => {
                let phrase = args
                    .get("samplePhrase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Hallo, hier spricht Luka in backtext.");
                Ok(serde_json::to_value(
                    self.storage.enroll_self_voiceprint(phrase)?,
                )?)
            }
            "upsert_vocabulary" => {
                let term = args.get("term").and_then(|v| v.as_str()).unwrap_or("").trim();
                let hint = args.get("phoneticHint").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                let cat = args.get("category").and_then(|v| v.as_str()).unwrap_or("Fachbegriff");
                let proj = args.get("projectTag").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                if !term.is_empty() {
                    self.storage.upsert_vocabulary(&VocabularyEntry {
                        id: format!("voc_{}", uuid::Uuid::new_v4().simple()),
                        term: term.to_string(),
                        phonetic_hint: hint,
                        category: cat.to_string(),
                        project_tag: proj,
                        created_at: Utc::now(),
                    })?;
                }
                Ok(serde_json::to_value(self.storage.list_vocabulary()?)?)
            }
            "delete_vocabulary" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                self.storage.delete_vocabulary(id)?;
                Ok(serde_json::to_value(self.storage.list_vocabulary()?)?)
            }
            "toggle_learned_preference" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let active = args.get("isActive").and_then(|v| v.as_bool()).unwrap_or(true);
                self.storage.toggle_learned_preference(id, active)?;
                Ok(serde_json::to_value(
                    self.storage.list_learned_preferences()?,
                )?)
            }
            "toggle_privacy_rule" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let enabled = args.get("isEnabled").and_then(|v| v.as_bool()).unwrap_or(true);
                self.storage.toggle_privacy_rule(id, enabled)?;
                Ok(serde_json::to_value(self.storage.list_privacy_rules()?)?)
            }
            "upsert_privacy_rule" => {
                let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("app_bundle");
                let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
                let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("Benutzerdefinierte Regel");
                if !pattern.trim().is_empty() {
                    self.storage.upsert_privacy_rule(&PrivacyRule {
                        id: format!("priv_{}", uuid::Uuid::new_v4().simple()),
                        kind: kind.to_string(),
                        pattern: pattern.trim().to_string(),
                        description: desc.to_string(),
                        is_enabled: true,
                        is_builtin: false,
                    })?;
                }
                Ok(serde_json::to_value(self.storage.list_privacy_rules()?)?)
            }
            "delete_privacy_rule" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                self.storage.delete_privacy_rule(id)?;
                Ok(serde_json::to_value(self.storage.list_privacy_rules()?)?)
            }
            "save_mcp_policy" => {
                let pol: McpGovernancePolicy =
                    serde_json::from_value(args.get("policy").cloned().unwrap_or_default())?;
                self.storage.save_mcp_policy(&pol)?;
                Ok(serde_json::to_value(self.storage.get_mcp_policy()?)?)
            }
            "save_ai_settings" => {
                let settings: AiProviderSettings =
                    serde_json::from_value(args.get("settings").cloned().unwrap_or_default())?;
                self.storage.save_ai_settings(&settings)?;
                Ok(serde_json::to_value(self.storage.get_ai_settings()?)?)
            }
            "invoke_mcp_inspector" => {
                let tool_name = args
                    .get("toolName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("get_live_context");
                let tool_args = args
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let client = args
                    .get("clientName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("backtext-inspector");
                let result = self
                    .mcp
                    .execute_tool(tool_name, &tool_args, client, "inspector-http")?;
                let audits = self.storage.list_mcp_audit_logs(30)?;
                Ok(serde_json::json!({
                    "result": result,
                    "audit_logs": audits
                }))
            }
            _ => anyhow::bail!("Unknown RPC command: {cmd}"),
        }
    }
}

fn upsert_participant_vec(list: &mut Vec<MeetingParticipant>, next: MeetingParticipant) {
    if let Some(existing) = list
        .iter_mut()
        .find(|p| p.speaker_label == next.speaker_label)
    {
        *existing = next;
    } else {
        list.push(next);
    }
}
