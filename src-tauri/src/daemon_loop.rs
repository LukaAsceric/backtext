use crate::controller::AppController;
use backtext_context::{
    compact_snapshots_to_episode, evaluate_clipboard_capture, evaluate_context_candidate,
    FilterDecision, RawContextCandidate,
};
use backtext_core::*;
use backtext_meeting::{DetectorPhase, DetectorTransition, MeetingDetector};
use chrono::Utc;
use std::collections::HashSet;
use std::time::Duration;

/// Spawns the autonomous background context capture, meeting auto-detection,
/// and periodic calendar synchronization loop.
pub fn spawn_background_daemon(controller: AppController) {
    tokio::spawn(async move {
        let mut detector = MeetingDetector::default();
        let mut last_focus_key = String::new();
        let mut focus_dwell_ms: u64 = 0;
        let mut accepted_since_compact: u32 = 0;
        let mut last_clip_hash: Option<String> = None;
        let mut notified_meeting_ids: HashSet<String> = HashSet::new();
        let mut tick_count: u64 = 0;

        controller.update_telemetry(|t| {
            t.loop_running = true;
            t.poll_interval_secs = 4;
        });

        let mut interval = tokio::time::interval(Duration::from_secs(4));
        // Skip burst catch-up ticks if laptop wakes from sleep
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            tick_count += 1;

            // Periodic calendar sync (on startup tick 2 and every ~5 minutes)
            if tick_count == 2 || tick_count % 75 == 0 {
                if let Ok(imported) = controller.sync_calendars().await {
                    if imported > 0 {
                        controller
                            .events
                            .publish(BacktextEvent::CalendarSynced { imported_count: imported });
                    }
                }
            }

            // Pre-meeting 5-minute notification check
            if tick_count % 15 == 0 {
                if let Ok(meetings) = controller.storage.list_meetings() {
                    let now = Utc::now();
                    for m in meetings
                        .into_iter()
                        .filter(|m| m.status == MeetingStatus::Scheduled)
                    {
                        let mins = (m.started_at - now).num_minutes();
                        if (0..=5).contains(&mins) && !notified_meeting_ids.contains(&m.id) {
                            notified_meeting_ids.insert(m.id.clone());
                            let _ = controller
                                .sensors
                                .send_native_notification(
                                    &format!("Gleich startet: {}", m.title),
                                    "Dein Vorbereitungs-Briefing in backtext ist bereit.",
                                )
                                .await;
                        }
                    }
                }
            }

            let status = controller.current_capture_status();
            let perms = controller.sensors.check_permissions().await;

            // Respect Pause mode, locked screen, and idle > 120s
            if matches!(status, CaptureStatus::Paused { .. })
                || perms.screen_locked
                || perms.idle_seconds > 120
            {
                controller.update_telemetry(|t| {
                    t.ticks_processed = tick_count;
                    t.last_tick_at = Some(Utc::now());
                });
                continue;
            }

            let is_recording_meeting = matches!(status, CaptureStatus::RecordingMeeting { .. });

            // 1. Active Window Observation & 3-Stage Ambient Context Filter
            if let Ok(obs) = controller.sensors.observe_active_window().await {
                let is_self_app = obs.app_bundle_id.eq_ignore_ascii_case("ai.backtext.desktop")
                    || obs.app_name.eq_ignore_ascii_case("backtext");

                if !is_self_app && !obs.app_name.is_empty() {
                    let focus_key = format!(
                        "{}::{}::{:?}",
                        obs.app_bundle_id, obs.window_title, obs.browser_url
                    );
                    if focus_key == last_focus_key {
                        focus_dwell_ms = focus_dwell_ms.saturating_add(4_000);
                    } else {
                        last_focus_key = focus_key;
                        focus_dwell_ms = 4_000;
                    }

                    // Only trigger heavy Vision OCR fallback when NOT in a live meeting (screenshare capture handles slides during meetings)
                    let mut visible_text = obs.visible_text.clone();
                    let mut extraction_method = obs.extraction_method;
                    let mut dhash_u64 = obs.dhash_u64;

                    if !is_recording_meeting
                        && visible_text.chars().count() < 40
                        && perms.screen_capture_granted
                        && !obs.is_private_browsing
                        && !obs.is_secure_field_focused
                        && (focus_dwell_ms == 4_000 || focus_dwell_ms == 8_000)
                    {
                        if let Ok(shot) = controller.sensors.capture_window_screenshot_ocr(false).await {
                            if shot.ok && !shot.ocr_text.trim().is_empty() {
                                visible_text = shot.ocr_text;
                                dhash_u64 = shot.dhash_u64;
                                extraction_method = ExtractionMethod::VisionOcr;
                            }
                        }
                    }

                    let rules = controller.storage.list_privacy_rules().unwrap_or_default();
                    let profile = controller.storage.get_user_profile().unwrap_or_default();
                    let vocab = controller.storage.list_vocabulary().unwrap_or_default();
                    let prev_snap = controller
                        .storage
                        .list_recent_snapshots(1)
                        .ok()
                        .and_then(|v| v.into_iter().next());

                    let candidate = RawContextCandidate {
                        app_bundle_id: obs.app_bundle_id.clone(),
                        app_name: obs.app_name.clone(),
                        window_title: obs.window_title.clone(),
                        browser_url: obs.browser_url.clone(),
                        dwell_ms: focus_dwell_ms,
                        dhash_u64,
                        visible_text,
                        extraction_method,
                        is_private_browsing: obs.is_private_browsing,
                        is_secure_field_focused: obs.is_secure_field_focused,
                    };

                    let decision = evaluate_context_candidate(
                        candidate,
                        prev_snap.as_ref(),
                        &rules,
                        &profile,
                        &vocab,
                    );

                    match decision {
                        FilterDecision::Accepted { snapshot } => {
                            if controller.storage.insert_context_snapshot(&snapshot).is_ok() {
                                accepted_since_compact += 1;
                                controller.update_telemetry(|t| {
                                    t.snapshots_accepted += 1;
                                    t.last_window_app = Some(snapshot.app_name.clone());
                                    t.last_window_title = Some(snapshot.window_title.clone());
                                });

                                // Upgrade vector embedding asynchronously if Ollama/OpenAI neural embeddings are enabled
                                if let Ok(ai_cfg) = controller.storage.get_ai_settings() {
                                    if ai_cfg.embedding_provider != "local_hybrid" {
                                        let emb = backtext_ai::compute_embedding_256(
                                            controller.ai.http_client(),
                                            &format!("{} {}", snapshot.window_title, snapshot.extracted_text),
                                            &ai_cfg,
                                            controller.storage.secrets(),
                                        )
                                        .await;
                                        let _ = controller.storage.update_memory_vector_embedding(
                                            &format!("snapshot:{}", snapshot.id),
                                            &emb,
                                        );
                                    }
                                }

                                let has_no_episodes = controller
                                    .storage
                                    .list_context_episodes(1)
                                    .map(|e| e.is_empty())
                                    .unwrap_or(false);

                                if has_no_episodes || accepted_since_compact >= 2 {
                                    accepted_since_compact = 0;
                                    if let Ok(recent) = controller.storage.list_recent_snapshots(5) {
                                        if let Some(ep) = compact_snapshots_to_episode(&recent) {
                                            if controller.storage.upsert_context_episode(&ep).is_ok() {
                                                controller.events.publish(
                                                    BacktextEvent::ContextEpisodeUpdated(ep),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        FilterDecision::MergedDuplicate {
                            previous_snapshot_id,
                            added_dwell_ms,
                        } => {
                            let _ = controller
                                .storage
                                .update_snapshot_dwell(&previous_snapshot_id, added_dwell_ms.min(4_000));
                            controller.update_telemetry(|t| {
                                t.last_window_app = Some(obs.app_name.clone());
                                t.last_window_title = Some(obs.window_title.clone());
                            });
                        }
                        FilterDecision::BlockedByPrivacy { .. }
                        | FilterDecision::DroppedAsNoise { .. } => {
                            controller.update_telemetry(|t| {
                                t.snapshots_filtered += 1;
                                t.last_window_app = Some(obs.app_name.clone());
                                if !obs.is_private_browsing && !obs.is_secure_field_focused {
                                    t.last_window_title = Some(obs.window_title.clone());
                                }
                            });
                        }
                    }
                }
            }

            // 2. Safe Clipboard Monitor (skips ConcealedType / 1Password)
            if let Ok(Some(clip)) = controller.sensors.read_safe_clipboard().await {
                let rules = controller.storage.list_privacy_rules().unwrap_or_default();
                let profile = controller.storage.get_user_profile().unwrap_or_default();
                let vocab = controller.storage.list_vocabulary().unwrap_or_default();
                if let Some(item) = evaluate_clipboard_capture(
                    &clip.source_app,
                    &clip.text,
                    clip.is_concealed,
                    last_clip_hash.as_deref(),
                    &rules,
                    &profile,
                    &vocab,
                ) {
                    last_clip_hash = Some(item.content_hash.clone());
                    let _ = controller.storage.insert_clipboard_item(&item);
                }
            }

            // 3. Autonomous Bot-Free Meeting Start / End Detection & Screen-Share Capture
            if controller.take_manual_stop_flag() {
                detector.mark_manually_stopped(Utc::now(), 90);
            }

            // During active meetings, throttle browser/window audio-env polling to every 12s (3 ticks)
            // and skip 4s EventKit calendar queries to keep CPU/GPU cool.
            let should_poll_audio_env = !is_recording_meeting || tick_count % 3 == 0;
            if should_poll_audio_env {
                if let Ok(audio_sig) = controller.sensors.observe_audio_environment().await {
                    let profile = controller.storage.get_user_profile().unwrap_or_default();
                    let cal_events = if is_recording_meeting {
                        Vec::new()
                    } else {
                        controller
                            .sensors
                            .upcoming_calendar_events()
                            .await
                            .unwrap_or_default()
                    };

                    // Sync detector with manually started meetings
                    if let CaptureStatus::RecordingMeeting {
                        ref meeting_id,
                        started_at,
                        ..
                    } = status
                    {
                        if matches!(
                            detector.phase,
                            DetectorPhase::Idle | DetectorPhase::Candidate { .. }
                        ) {
                            detector.mark_active(meeting_id.clone(), started_at);
                        }
                        // Capture deduplicated Screen-Share slides during active macOS meetings every ~12s
                        if cfg!(target_os = "macos")
                            && tick_count % 3 == 0
                            && perms.screen_capture_granted
                        {
                            let _ = controller.capture_live_screenshare_if_changed(meeting_id).await;
                        }
                    } else if matches!(detector.phase, DetectorPhase::Active { .. }) {
                        detector.mark_idle();
                    }

                    let transition = detector.evaluate_tick(
                        Utc::now(),
                        &audio_sig,
                        &cal_events,
                        profile.auto_record_meetings,
                    );

                    match transition {
                        DetectorTransition::StartMeeting {
                            title,
                            app,
                            confidence,
                            ..
                        } => {
                            if let Ok(detail) = controller.start_live_meeting(
                                None,
                                Some(title.clone()),
                                Some(app.clone()),
                            ) {
                                detector.mark_active(detail.meeting.id.clone(), detail.meeting.started_at);
                                controller.events.publish(BacktextEvent::MeetingDetected {
                                    meeting_id: detail.meeting.id.clone(),
                                    title: title.clone(),
                                    detected_app: app,
                                    confidence,
                                });
                                if profile.consent_reminder_enabled {
                                    let _ = controller
                                        .sensors
                                        .send_native_notification(
                                            &format!("Meeting erkannt: {title}"),
                                            "Bot-freie Aufnahme gestartet. Bitte denke an die Einwilligung der Teilnehmer.",
                                        )
                                        .await;
                                }
                            }
                        }
                        DetectorTransition::EndMeeting { meeting_id } => {
                            let _ = controller
                                .stop_and_summarize_meeting_internal(&meeting_id, false)
                                .await;
                        }
                        _ => {}
                    }
                }
            }

            let phase_label = match &detector.phase {
                DetectorPhase::Idle => "idle",
                DetectorPhase::Candidate { .. } => "candidate",
                DetectorPhase::Active { .. } => "active_recording",
                DetectorPhase::GraceCooldown { .. } => "grace_cooldown",
            }
            .to_string();

            let updated_tel = controller.update_telemetry(|t| {
                t.ticks_processed = tick_count;
                t.last_tick_at = Some(Utc::now());
                t.meeting_detector_phase = phase_label;
            });
            if tick_count == 1 || tick_count % 2 == 0 {
                controller
                    .events
                    .publish(BacktextEvent::DaemonTelemetryUpdated(updated_tel));
            }
        }
    });
}
