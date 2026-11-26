use anyhow::Result;
use backtext_ai::{
    detect_whisper_cli_path, download_whisper_model, list_ollama_models, list_provider_models,
    list_whisper_models, stt_readiness_status, test_provider_connection, transcribe_wav_file,
    EnhancedMeetingResult, HybridAiEngine,
};
use backtext_context::{
    compact_snapshots_to_episode, evaluate_clipboard_capture, evaluate_context_candidate,
    preview_filter_on_sample, FilterDecision, RawContextCandidate, ScrubberPreview,
};
use backtext_core::*;
use backtext_mcp::{McpClientConfigSnippets, McpServer};
use backtext_meeting::{
    extract_voiceprint_from_wav, ScreenShareDeduplicator, SpeakerAttributionSession,
};
use backtext_platform::{
    default_platform_sensors, fetch_and_parse_ics_url, DualAudioStreamEvent, PermissionStatus,
    PlatformSensors,
};
use backtext_storage::secrets::{
    KEY_ANTHROPIC, KEY_DEEPGRAM, KEY_GEMINI, KEY_GROQ, KEY_OPENAI, KEY_OPENROUTER, KEY_PRIMALABS,
};
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
    pub telemetry: DaemonTelemetry,
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
    pub calendar_feeds: Vec<CalendarFeedConfig>,
    pub whisper_models: Vec<WhisperModelInfo>,
    pub ollama_models: Vec<String>,
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
    active_recorders: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    telemetry: Arc<Mutex<DaemonTelemetry>>,
    pending_manual_stop: Arc<Mutex<bool>>,
}

pub fn format_shortcut_display(shortcut: &str) -> String {
    match shortcut {
        "Alt+Space" => "Alt+Space (⌥ Space)".to_string(),
        "CommandOrControl+Shift+Space" => "⌘⇧ Space".to_string(),
        "Alt+Shift+Space" => "⌥⇧ Space".to_string(),
        "CommandOrControl+Shift+B" => "⌘⇧ B".to_string(),
        "CommandOrControl+Shift+K" => "⌘⇧ K".to_string(),
        "Control+Space" => "⌃ Space".to_string(),
        other => other.to_string(),
    }
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
        let demo_present = storage.has_demo_data().unwrap_or(false);
        let saved_shortcut = storage
            .get_ai_settings()
            .map(|s| s.hover_shortcut)
            .unwrap_or_else(|_| "Alt+Space".into());

        let initial_telemetry = DaemonTelemetry {
            loop_running: false,
            poll_interval_secs: 4,
            ticks_processed: 0,
            snapshots_accepted: 0,
            snapshots_filtered: 0,
            last_window_app: None,
            last_window_title: None,
            last_tick_at: None,
            meeting_detector_phase: "idle".into(),
            db_encrypted: storage.is_encrypted(),
            keychain_backend: storage.secrets().backend_name().to_string(),
            autostart_enabled: false,
            global_shortcut: format_shortcut_display(&saved_shortcut),
            demo_data_present: demo_present,
            whisper_cli_detected: detect_whisper_cli_path().is_some(),
        };

        Ok(Self {
            storage,
            events,
            ai,
            mcp,
            sensors,
            capture_status: Arc::new(Mutex::new(CaptureStatus::Active)),
            live_sessions: Arc::new(Mutex::new(HashMap::new())),
            active_recorders: Arc::new(Mutex::new(HashMap::new())),
            telemetry: Arc::new(Mutex::new(initial_telemetry)),
            pending_manual_stop: Arc::new(Mutex::new(false)),
        })
    }

    pub fn take_manual_stop_flag(&self) -> bool {
        let mut guard = self.pending_manual_stop.lock();
        let was = *guard;
        *guard = false;
        was
    }

    pub fn update_telemetry<F: FnOnce(&mut DaemonTelemetry)>(&self, f: F) -> DaemonTelemetry {
        let mut guard = self.telemetry.lock();
        f(&mut guard);
        guard.demo_data_present = self.storage.has_demo_data().unwrap_or(false);
        guard.clone()
    }

    pub fn get_telemetry(&self) -> DaemonTelemetry {
        let mut guard = self.telemetry.lock();
        guard.demo_data_present = self.storage.has_demo_data().unwrap_or(false);
        guard.whisper_cli_detected = detect_whisper_cli_path().is_some();
        guard.clone()
    }

    pub async fn bootstrap(&self) -> Result<BootstrapPayload> {
        let permissions = self.sensors.check_permissions().await;
        let status = self.current_capture_status();
        let ai_settings = self.storage.get_ai_settings()?;
        let ollama_models =
            list_ollama_models(self.ai.http_client(), &ai_settings.ollama_url).await;
        let bin_hint = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|dir| dir.join("backtext-mcp").display().to_string()));
        Ok(BootstrapPayload {
            capture_status: status,
            permissions,
            telemetry: self.get_telemetry(),
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
            calendar_feeds: self.storage.list_calendar_feeds()?,
            whisper_models: list_whisper_models(),
            ollama_models,
            mcp_policy: self.storage.get_mcp_policy()?,
            mcp_snippets: self.mcp.client_config_snippets(bin_hint.as_deref())?,
            mcp_audit_logs: self.storage.list_mcp_audit_logs(40)?,
            ai_settings,
        })
    }

    pub fn current_capture_status(&self) -> CaptureStatus {
        let mut guard = self.capture_status.lock();
        if let CaptureStatus::Paused {
            until: Some(until_dt),
            ..
        } = &*guard
        {
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

        // Immediately start continuous dual-channel audio recording (Mic + ScreenCaptureKit System Audio)
        self.start_continuous_recorder_for_meeting(&meeting.id);

        self.storage
            .get_meeting_detail(&meeting.id)?
            .ok_or_else(|| anyhow::anyhow!("Failed to load started meeting"))
    }

    pub fn stop_recorder_for_meeting(&self, meeting_id: &str) {
        if let Some(tx) = self.active_recorders.lock().remove(meeting_id) {
            let _ = tx.send(());
        }
    }

    fn stop_all_recorders(&self) {
        let drains: Vec<tokio::sync::oneshot::Sender<()>> = self
            .active_recorders
            .lock()
            .drain()
            .map(|(_, tx)| tx)
            .collect();
        for tx in drains {
            let _ = tx.send(());
        }
    }

    fn meeting_recordings_dir(&self, meeting_id: &str) -> PathBuf {
        let base = std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".backtext-data").join("recordings"))
            .unwrap_or_else(|_| std::env::temp_dir().join("backtext-recordings"));
        base.join(meeting_id)
    }

    /// Spawns a persistent background dual-audio recording & live transcription task for `meeting_id`.
    /// Streams real-time audio levels every 500ms and transcribes seamless 8s WAV chunks.
    pub fn start_continuous_recorder_for_meeting(&self, meeting_id: &str) {
        self.stop_all_recorders();
        let Ok(rt) = tokio::runtime::Handle::try_current() else {
            return;
        };

        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        self.active_recorders
            .lock()
            .insert(meeting_id.to_string(), stop_tx);

        let ctrl = self.clone();
        let mid = meeting_id.to_string();
        rt.spawn(async move {
            let rec_dir = ctrl.meeting_recordings_dir(&mid);
            let _ = std::fs::create_dir_all(&rec_dir);

            let mut session = match ctrl
                .sensors
                .start_dual_audio_stream(&rec_dir.display().to_string(), 8)
                .await
            {
                Ok(s) => s,
                Err(err) => {
                    ctrl.events.publish(BacktextEvent::RecordingAudioLevel {
                        meeting_id: mid.clone(),
                        mic_level: 0.0,
                        system_level: 0.0,
                        mic_active: false,
                        system_active: false,
                        elapsed_secs: 0,
                        stt_ready: false,
                        stt_status: "Audio-Aufnahme konnte nicht gestartet werden".into(),
                        warning: Some(err.to_string()),
                    });
                    ctrl.active_recorders.lock().remove(&mid);
                    return;
                }
            };

            let mut tick_idx: u64 = 0;
            let mut cached_stt = (false, String::new());
            let mut last_stt_error: Option<String> = None;

            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        session.stop();
                        // Drain final flushed chunk if emitted during graceful shutdown
                        let _ = tokio::time::timeout(std::time::Duration::from_millis(2600), async {
                            while let Some(ev) = session.rx.recv().await {
                                if let DualAudioStreamEvent::Chunk { mic_wav_path, system_wav_path, .. } = ev {
                                    let _ = ctrl.process_recorded_audio_pair(&mid, mic_wav_path, system_wav_path).await;
                                }
                            }
                        }).await;
                        break;
                    }
                    maybe_ev = session.rx.recv() => {
                        let Some(ev) = maybe_ev else {
                            break;
                        };
                        match ev {
                            DualAudioStreamEvent::Level {
                                mic_rms,
                                system_rms,
                                mic_active,
                                system_active,
                                elapsed_secs,
                                error,
                            } => {
                                if tick_idx % 8 == 0 {
                                    if let Ok(ai_cfg) = ctrl.storage.get_ai_settings() {
                                        cached_stt = stt_readiness_status(&ai_cfg, ctrl.storage.secrets());
                                    }
                                }
                                tick_idx += 1;
                                ctrl.events.publish(BacktextEvent::RecordingAudioLevel {
                                    meeting_id: mid.clone(),
                                    mic_level: mic_rms,
                                    system_level: system_rms,
                                    mic_active,
                                    system_active,
                                    elapsed_secs,
                                    stt_ready: cached_stt.0,
                                    stt_status: cached_stt.1.clone(),
                                    warning: error.or_else(|| last_stt_error.clone()),
                                });
                            }
                            DualAudioStreamEvent::Chunk {
                                mic_wav_path,
                                system_wav_path,
                                ..
                            } => {
                                match ctrl
                                    .process_recorded_audio_pair(&mid, mic_wav_path, system_wav_path)
                                    .await
                                {
                                    Ok(_) => {
                                        last_stt_error = None;
                                    }
                                    Err(err) => {
                                        last_stt_error = Some(format!("Transkription: {err}"));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            ctrl.active_recorders.lock().remove(&mid);
        });
    }

    /// Captures a real macOS ScreenCaptureKit screenshot during a live meeting, checks dHash
    /// stability via `ScreenShareDeduplicator`, runs Apple Vision OCR, and persists new slides.
    pub async fn capture_live_screenshare_if_changed(&self, meeting_id: &str) -> Result<bool> {
        let shot = self.sensors.capture_window_screenshot_ocr(true).await?;
        if !shot.ok || shot.ocr_text.trim().len() < 12 {
            return Ok(false);
        }

        let elapsed_ms = {
            let mut sessions = self.live_sessions.lock();
            let Some(rt) = sessions.get_mut(meeting_id) else {
                return Ok(false);
            };
            rt.elapsed_ms = rt.elapsed_ms.saturating_add(4_000);
            let _ = rt.screenshare_dedup.should_capture_frame(shot.dhash_u64, rt.elapsed_ms);
            if !rt
                .screenshare_dedup
                .should_capture_frame(shot.dhash_u64, rt.elapsed_ms + 1_600)
            {
                return Ok(false);
            }
            rt.elapsed_ms
        };

        let first_line = shot
            .ocr_text
            .lines()
            .map(|l| l.trim())
            .find(|l| l.len() >= 4)
            .unwrap_or("Geteilter Bildschirm")
            .chars()
            .take(72)
            .collect::<String>();

        let frame = ScreenShareFrame {
            id: format!("frame_{}", uuid::Uuid::new_v4().simple()),
            meeting_id: meeting_id.to_string(),
            captured_at_ms: elapsed_ms,
            presenter_name: Some("Screen Share (macOS Vision OCR)".into()),
            dhash_hex: shot.dhash_hex,
            image_data_url: shot.image_data_url,
            ocr_text: shot.ocr_text,
            slide_title: first_line,
        };
        self.storage.insert_screenshare_frame(&frame)?;
        self.events.publish(BacktextEvent::ScreenShareCaptured(frame));
        Ok(true)
    }

    async fn process_recorded_audio_pair(
        &self,
        meeting_id: &str,
        mic_wav_path: Option<String>,
        system_wav_path: Option<String>,
    ) -> Result<usize> {
        let profile = self.storage.get_user_profile()?;
        let vocab = self.storage.list_vocabulary()?;
        let ai_settings = self.storage.get_ai_settings()?;
        let attendee_names: Vec<String> = self
            .storage
            .get_meeting_detail(meeting_id)?
            .map(|d| d.meeting.participants.into_iter().map(|p| p.display_name).collect())
            .unwrap_or_default();
        let vocab_prompt = build_stt_vocabulary_prompt(&profile, &vocab, &attendee_names);

        let visual_speaker_hint = self
            .sensors
            .observe_active_window()
            .await
            .ok()
            .and_then(|w| w.detected_active_speaker_tile);

        let mut added = 0usize;
        for (channel, maybe_wav) in [
            (AudioChannel::Microphone, mic_wav_path),
            (AudioChannel::System, system_wav_path),
        ] {
            let Some(wav_path_str) = maybe_wav else {
                continue;
            };
            let wav_path = PathBuf::from(&wav_path_str);
            let Ok((acoustic_vp, spans)) = extract_voiceprint_from_wav(&wav_path) else {
                continue;
            };
            if spans.is_empty() {
                continue;
            }
            let utterances = transcribe_wav_file(
                self.ai.http_client(),
                &wav_path,
                &vocab_prompt,
                &ai_settings,
                self.storage.secrets(),
            )
            .await?;

            for utt in utterances {
                let mut sessions = self.live_sessions.lock();
                let rt = sessions
                    .entry(meeting_id.to_string())
                    .or_insert_with(|| LiveMeetingRuntime {
                        attribution: SpeakerAttributionSession::new(
                            meeting_id,
                            &profile.display_name,
                            vec![],
                        ),
                        screenshare_dedup: ScreenShareDeduplicator::default(),
                        sim_step: 0,
                        elapsed_ms: 0,
                    });

                let base_ms = rt.elapsed_ms;
                let start_ms = base_ms + utt.start_ms;
                let end_ms = base_ms + utt.end_ms.max(utt.start_ms + 1000);
                rt.elapsed_ms = end_ms;

                let hint = utt.speaker_hint.as_deref().or(visual_speaker_hint.as_deref());
                let (seg, participant) = rt.attribution.attribute_segment(
                    &self.storage,
                    channel,
                    start_ms,
                    end_ms,
                    &utt.text,
                    &acoustic_vp,
                    hint,
                )?;
                drop(sessions);

                self.storage.insert_transcript_segment(&seg)?;
                self.events.publish(BacktextEvent::TranscriptSegmentAdded(seg));
                added += 1;
                if let Some(mut detail) = self.storage.get_meeting_detail(meeting_id)? {
                    upsert_participant_vec(&mut detail.meeting.participants, participant.clone());
                    self.storage.save_meeting(&detail.meeting)?;
                    self.events.publish(BacktextEvent::ParticipantUpdated {
                        meeting_id: meeting_id.to_string(),
                        participant,
                    });
                }
            }
        }
        Ok(added)
    }

    /// Records a single dual-channel audio turn (Mic + ScreenCaptureKit System Audio),
    /// extracts 256-dim acoustic MFCC/pitch voiceprints, runs configured STT,
    /// and attributes speakers into the live meeting transcript.
    pub async fn capture_real_dual_audio_turn(
        &self,
        meeting_id: &str,
        duration_secs: u64,
    ) -> Result<MeetingDetail> {
        let chunk_dir = std::env::temp_dir().join(format!(
            "backtext_chunk_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let capture = self
            .sensors
            .record_dual_audio_chunk(&chunk_dir.display().to_string(), duration_secs.clamp(3, 15))
            .await?;

        let _ = self
            .process_recorded_audio_pair(meeting_id, capture.mic_wav_path, capture.system_wav_path)
            .await;

        let _ = std::fs::remove_dir_all(&chunk_dir);
        self.storage
            .get_meeting_detail(meeting_id)?
            .ok_or_else(|| anyhow::anyhow!("Meeting not found"))
    }

    /// Feeds a realistic multi-modal simulation step (Mic Self, Voiceprint Recognition,
    /// Visual Active-Speaker Recognition, and Screen-Share OCR Slide).
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
        self.stop_and_summarize_meeting_internal(meeting_id, true)
            .await
    }

    pub async fn stop_and_summarize_meeting_internal(
        &self,
        meeting_id: &str,
        manual: bool,
    ) -> Result<MeetingDetail> {
        if manual {
            *self.pending_manual_stop.lock() = true;
        }
        // Stop continuous audio stream and allow brief graceful flush of the final audio chunk
        self.stop_recorder_for_meeting(meeting_id);
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;

        self.live_sessions.lock().remove(meeting_id);
        if let Some(mut detail) = self.storage.get_meeting_detail(meeting_id)? {
            detail.meeting.status = MeetingStatus::Completed;
            detail.meeting.ended_at = Some(Utc::now());
            self.storage.save_meeting(&detail.meeting)?;
        }

        *self.capture_status.lock() = CaptureStatus::Active;
        self.events
            .publish(BacktextEvent::CaptureStatusChanged(CaptureStatus::Active));

        let res = self.ai.enhance_meeting_summary(&self.storage, meeting_id).await?;
        self.events.publish(BacktextEvent::MeetingSummarized {
            meeting_id: meeting_id.to_string(),
            executive_summary: res.executive_summary,
        });

        self.storage
            .get_meeting_detail(meeting_id)?
            .ok_or_else(|| anyhow::anyhow!("Meeting not found after summary"))
    }

    pub fn delete_meeting(&self, meeting_id: &str) -> Result<bool> {
        self.stop_recorder_for_meeting(meeting_id);
        self.live_sessions.lock().remove(meeting_id);
        let was_active = matches!(
            &*self.capture_status.lock(),
            CaptureStatus::RecordingMeeting { meeting_id: active_id, .. } if active_id == meeting_id
        );
        if was_active {
            *self.capture_status.lock() = CaptureStatus::Active;
            self.events
                .publish(BacktextEvent::CaptureStatusChanged(CaptureStatus::Active));
        }
        let rec_dir = self.meeting_recordings_dir(meeting_id);
        let _ = std::fs::remove_dir_all(&rec_dir);
        self.storage.delete_meeting(meeting_id)
    }

    pub async fn enhance_meeting(&self, meeting_id: &str) -> Result<EnhancedMeetingResult> {
        self.ai.enhance_meeting_summary(&self.storage, meeting_id).await
    }

    /// Synchronizes macOS EventKit calendars + configured external `.ics` feeds into scheduled meetings.
    pub async fn sync_calendars(&self) -> Result<usize> {
        let mut all_events = self
            .sensors
            .upcoming_calendar_events()
            .await
            .unwrap_or_default();

        let mut feeds = self.storage.list_calendar_feeds()?;
        for feed in feeds.iter_mut().filter(|f| f.enabled && !f.ics_url.is_empty()) {
            if let Ok(ics_events) = fetch_and_parse_ics_url(&feed.ics_url).await {
                all_events.extend(ics_events);
                feed.last_synced_at = Some(Utc::now());
            }
        }
        let _ = self.storage.save_calendar_feeds(&feeds);

        let mut imported = 0_usize;
        for ev in all_events {
            let app_label = match ev.conference_url.as_deref() {
                Some(u) if u.contains("zoom.us") => "Zoom (Kalender)",
                Some(u) if u.contains("meet.google.com") => "Google Meet (Kalender)",
                Some(u) if u.contains("teams.microsoft") => "Microsoft Teams (Kalender)",
                _ => "Kalender-Termin (EventKit / ICS)",
            };
            let attendees: Vec<(String, String)> = ev
                .attendees
                .into_iter()
                .map(|a| (a.name, a.email))
                .collect();
            if self.storage.upsert_calendar_event_meeting(
                &ev.event_id,
                &ev.title,
                ev.starts_at,
                Some(ev.ends_at),
                app_label,
                &attendees,
            )? {
                imported += 1;
            }
        }
        Ok(imported)
    }

    /// Records real microphone audio (5s on macOS / synthetic fallback on Linux), extracts a
    /// genuine 256-dim acoustic MFCC/pitch voiceprint, and stores it in `user_profile`.
    pub async fn calibrate_real_voiceprint(&self, duration_secs: u64) -> Result<UserProfile> {
        let wav_path = std::env::temp_dir().join(format!(
            "backtext_self_vp_{}.wav",
            uuid::Uuid::new_v4().simple()
        ));
        let recorded = self
            .sensors
            .record_mic_wav(&wav_path.display().to_string(), duration_secs.clamp(3, 15))
            .await
            .unwrap_or(false);

        if recorded && wav_path.exists() {
            if let Ok((embedding, _spans)) = extract_voiceprint_from_wav(&wav_path) {
                let _ = std::fs::remove_file(&wav_path);
                let norm_sq: f32 = embedding.iter().map(|x| x * x).sum();
                if norm_sq > 0.1 {
                    return self.storage.save_self_voiceprint_embedding(&embedding);
                }
            }
        }
        let _ = std::fs::remove_file(&wav_path);
        self.storage
            .enroll_self_voiceprint("Kalibriertes Stimmprofil in backtext")
    }

    pub async fn check_for_updates(&self) -> Result<UpdateCheckResult> {
        let current = env!("CARGO_PKG_VERSION").to_string();
        let resp = self
            .ai
            .http_client()
            .get("https://api.github.com/repos/LukaAsceric/backtext/releases/latest")
            .header("User-Agent", "backtext-desktop")
            .timeout(std::time::Duration::from_secs(6))
            .send()
            .await;

        if let Ok(r) = resp {
            if r.status().is_success() {
                if let Ok(v) = r.json::<serde_json::Value>().await {
                    let latest = v["tag_name"]
                        .as_str()
                        .unwrap_or(&current)
                        .trim_start_matches('v')
                        .to_string();
                    let url = v["html_url"]
                        .as_str()
                        .unwrap_or("https://github.com/LukaAsceric/backtext/releases")
                        .to_string();
                    let notes = v["body"].as_str().unwrap_or("").to_string();
                    let update_available = latest != current && !latest.is_empty();
                    return Ok(UpdateCheckResult {
                        current_version: current,
                        latest_version: latest,
                        update_available,
                        release_url: url,
                        release_notes: notes,
                    });
                }
            }
        }

        Ok(UpdateCheckResult {
            current_version: current.clone(),
            latest_version: current,
            update_available: false,
            release_url: "https://github.com/LukaAsceric/backtext/releases".into(),
            release_notes: "Kein neueres Release gefunden (oder privates Repo ohne Token).".into(),
        })
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
            "get_daemon_telemetry" => Ok(serde_json::to_value(self.get_telemetry())?),
            "check_permissions" => Ok(serde_json::to_value(self.sensors.check_permissions().await)?),
            "request_permission" => {
                let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("screen");
                Ok(serde_json::to_value(self.sensors.request_permission(kind).await?)?)
            }
            "open_permission_settings" => {
                let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("screen");
                self.sensors.open_permission_settings(kind).await?;
                Ok(serde_json::json!({ "ok": true }))
            }
            "clear_demo_data" => {
                let removed = self.storage.clear_demo_data()?;
                self.update_telemetry(|t| t.demo_data_present = false);
                Ok(serde_json::json!({ "removed": removed }))
            }
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
            "capture_real_dual_audio_turn" => {
                let id = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let secs = args.get("durationSecs").and_then(|v| v.as_u64()).unwrap_or(6);
                Ok(serde_json::to_value(
                    self.capture_real_dual_audio_turn(id, secs).await?,
                )?)
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
            "update_meeting_title" => {
                let id = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
                self.storage.update_meeting_title(id, title)?;
                Ok(serde_json::to_value(self.storage.get_meeting_detail(id)?)?)
            }
            "delete_meeting" => {
                let id = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let deleted = self.delete_meeting(id)?;
                Ok(serde_json::json!({
                    "deleted": deleted,
                    "meetings": self.storage.list_meetings()?
                }))
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
            "add_meeting_participant" => {
                let mid = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let name = args
                    .get("displayName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let email = args
                    .get("email")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty());
                let person_id = args
                    .get("personId")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty());
                let detail = self
                    .storage
                    .add_meeting_participant(mid, name, email, person_id)?;
                let persons = self.storage.list_persons()?;
                Ok(serde_json::json!({
                    "detail": detail,
                    "persons": persons
                }))
            }
            "remove_meeting_participant" => {
                let mid = args.get("meetingId").and_then(|v| v.as_str()).unwrap_or("");
                let label = args
                    .get("speakerLabel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let detail = self.storage.remove_meeting_participant(mid, label)?;
                Ok(serde_json::to_value(detail)?)
            }
            "upsert_person" => {
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("per_{}", uuid::Uuid::new_v4().simple()));
                let full_name = args
                    .get("fullName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if full_name.is_empty() {
                    anyhow::bail!("Name darf nicht leer sein.");
                }
                let existing = self
                    .storage
                    .list_persons()?
                    .into_iter()
                    .find(|p| p.id == id);
                let email = args
                    .get("email")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let company = args
                    .get("company")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let role_notes = args
                    .get("roleNotes")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let person = Person {
                    id,
                    full_name,
                    email,
                    company,
                    role_notes,
                    voiceprint_samples: existing.as_ref().map(|p| p.voiceprint_samples).unwrap_or(0),
                    voiceprint_confidence: existing
                        .as_ref()
                        .map(|p| p.voiceprint_confidence)
                        .unwrap_or(0.0),
                    meeting_count: existing.as_ref().map(|p| p.meeting_count).unwrap_or(0),
                    open_commitments: existing
                        .as_ref()
                        .map(|p| p.open_commitments.clone())
                        .unwrap_or_default(),
                    last_seen_at: existing
                        .as_ref()
                        .and_then(|p| p.last_seen_at)
                        .or_else(|| Some(Utc::now())),
                };
                self.storage.upsert_person(&person)?;
                Ok(serde_json::to_value(self.storage.list_persons()?)?)
            }
            "delete_person" => {
                let pid = args.get("personId").and_then(|v| v.as_str()).unwrap_or("");
                self.storage.delete_person(pid)?;
                Ok(serde_json::to_value(self.storage.list_persons()?)?)
            }
            "run_chat_or_recipe" => {
                let mid = args.get("meetingId").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                let input = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
                let slug = args.get("recipeSlug").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                let reply = self
                    .ai
                    .run_chat_or_recipe_streaming(&self.storage, Some(&self.events), mid, input, slug)
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
                let secs = args.get("durationSecs").and_then(|v| v.as_u64()).unwrap_or(5);
                Ok(serde_json::to_value(
                    self.calibrate_real_voiceprint(secs).await?,
                )?)
            }
            "set_api_key" => {
                let provider = args.get("provider").and_then(|v| v.as_str()).unwrap_or("");
                let value = args.get("apiKey").and_then(|v| v.as_str()).unwrap_or("");
                let key_name = match provider {
                    "openai" => KEY_OPENAI,
                    "anthropic" => KEY_ANTHROPIC,
                    "gemini" => KEY_GEMINI,
                    "openrouter" => KEY_OPENROUTER,
                    "primalabs" => KEY_PRIMALABS,
                    "deepgram" => KEY_DEEPGRAM,
                    "groq" => KEY_GROQ,
                    _ => anyhow::bail!("Unknown API key provider: {provider}"),
                };
                self.storage.secrets().set_secret(key_name, value)?;
                Ok(serde_json::to_value(self.storage.get_ai_settings()?)?)
            }
            "delete_api_key" => {
                let provider = args.get("provider").and_then(|v| v.as_str()).unwrap_or("");
                let key_name = match provider {
                    "openai" => KEY_OPENAI,
                    "anthropic" => KEY_ANTHROPIC,
                    "gemini" => KEY_GEMINI,
                    "openrouter" => KEY_OPENROUTER,
                    "primalabs" => KEY_PRIMALABS,
                    "deepgram" => KEY_DEEPGRAM,
                    "groq" => KEY_GROQ,
                    _ => anyhow::bail!("Unknown API key provider: {provider}"),
                };
                self.storage.secrets().delete_secret(key_name)?;
                Ok(serde_json::to_value(self.storage.get_ai_settings()?)?)
            }
            "test_provider_connection" => {
                let provider = args.get("provider").and_then(|v| v.as_str()).unwrap_or("ollama");
                let settings = self.storage.get_ai_settings()?;
                let res = test_provider_connection(
                    self.ai.http_client(),
                    provider,
                    &settings,
                    self.storage.secrets(),
                )
                .await;
                Ok(serde_json::to_value(res)?)
            }
            "list_provider_models" => {
                let provider = args
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("primalabs");
                let settings = self.storage.get_ai_settings()?;
                let models = list_provider_models(
                    self.ai.http_client(),
                    provider,
                    &settings,
                    self.storage.secrets(),
                )
                .await;
                Ok(serde_json::to_value(models)?)
            }
            "set_hover_shortcut" => {
                let shortcut = args
                    .get("shortcut")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Alt+Space")
                    .trim()
                    .to_string();
                let mut settings = self.storage.get_ai_settings()?;
                settings.hover_shortcut = shortcut.clone();
                self.storage.save_ai_settings(&settings)?;
                let display = format_shortcut_display(&shortcut);
                let tel = self.update_telemetry(|t| t.global_shortcut = display.clone());
                self.events
                    .publish(BacktextEvent::DaemonTelemetryUpdated(tel));
                Ok(serde_json::json!({ "shortcut": shortcut, "display": display }))
            }
            "list_ollama_models" => {
                let settings = self.storage.get_ai_settings()?;
                let models = list_ollama_models(self.ai.http_client(), &settings.ollama_url).await;
                Ok(serde_json::to_value(models)?)
            }
            "list_whisper_models" => Ok(serde_json::to_value(list_whisper_models())?),
            "download_whisper_model" => {
                let model_id = args
                    .get("modelId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ggml-base");
                let info =
                    download_whisper_model(self.ai.http_client(), model_id, &self.events).await?;
                Ok(serde_json::to_value(info)?)
            }
            "sync_calendars" => {
                let imported = self.sync_calendars().await?;
                Ok(serde_json::json!({
                    "imported": imported,
                    "meetings": self.storage.list_meetings()?,
                    "feeds": self.storage.list_calendar_feeds()?
                }))
            }
            "add_calendar_feed" => {
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("Mein Kalender");
                let ics_url = args.get("icsUrl").and_then(|v| v.as_str()).unwrap_or("").trim();
                if !ics_url.is_empty() {
                    let mut feeds = self.storage.list_calendar_feeds()?;
                    feeds.push(CalendarFeedConfig {
                        id: format!("calfeed_{}", uuid::Uuid::new_v4().simple()),
                        name: name.to_string(),
                        ics_url: ics_url.to_string(),
                        enabled: true,
                        last_synced_at: None,
                    });
                    self.storage.save_calendar_feeds(&feeds)?;
                }
                let _ = self.sync_calendars().await;
                Ok(serde_json::to_value(self.storage.list_calendar_feeds()?)?)
            }
            "delete_calendar_feed" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let mut feeds = self.storage.list_calendar_feeds()?;
                feeds.retain(|f| f.id != id);
                self.storage.save_calendar_feeds(&feeds)?;
                Ok(serde_json::to_value(feeds)?)
            }
            "check_for_updates" => Ok(serde_json::to_value(self.check_for_updates().await?)?),
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
