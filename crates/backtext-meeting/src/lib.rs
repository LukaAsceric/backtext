pub mod audio_dsp;

pub use audio_dsp::{
    detect_speech_spans, extract_acoustic_voiceprint_256, extract_voiceprint_for_window,
    extract_voiceprint_from_wav, read_wav_and_spans, read_wav_pcm16, SpeechSpan,
};

use anyhow::Result;
use backtext_context::dhash_hamming_distance;
use backtext_core::*;
use backtext_platform::{AudioEnvironmentSignal, CalendarMeetingEvent};
use backtext_storage::StorageEngine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// 1. Automatic Meeting Start & End State Machine
// =============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum DetectorPhase {
    Idle,
    Candidate {
        since: DateTime<Utc>,
        title_hint: String,
        app_hint: String,
        confidence: f32,
    },
    Active {
        meeting_id: String,
        started_at: DateTime<Utc>,
    },
    GraceCooldown {
        meeting_id: String,
        started_at: DateTime<Utc>,
        quiet_since: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DetectorTransition {
    None,
    CandidateDetected {
        title: String,
        app: String,
        confidence: f32,
    },
    StartMeeting {
        title: String,
        app: String,
        calendar_event_id: Option<String>,
        confidence: f32,
    },
    EndMeeting {
        meeting_id: String,
    },
}

pub struct MeetingDetector {
    pub phase: DetectorPhase,
    pub candidate_hold_secs: i64,
    pub cooldown_grace_secs: i64,
    pub suppress_auto_until: Option<DateTime<Utc>>,
}

impl Default for MeetingDetector {
    fn default() -> Self {
        Self {
            phase: DetectorPhase::Idle,
            candidate_hold_secs: 3,
            cooldown_grace_secs: 45,
            suppress_auto_until: None,
        }
    }
}

impl MeetingDetector {
    pub fn compute_signal_confidence(
        signal: &AudioEnvironmentSignal,
        calendar_match: Option<&CalendarMeetingEvent>,
    ) -> f32 {
        let mut score = 0.0_f32;
        if signal.microphone_in_use {
            score += 0.45;
        }
        if signal.detected_meeting_app.is_some() {
            score += 0.40;
        }
        if signal.has_call_controls_ui {
            score += 0.15;
        }
        if calendar_match.is_some() {
            score += 0.25;
        }
        score.clamp(0.0, 1.0)
    }

    pub fn evaluate_tick(
        &mut self,
        now: DateTime<Utc>,
        signal: &AudioEnvironmentSignal,
        calendar_events: &[CalendarMeetingEvent],
        auto_record: bool,
    ) -> DetectorTransition {
        let matched_cal = calendar_events.iter().find(|ev| {
            let mins_to_start = (ev.starts_at - now).num_minutes();
            let mins_since_end = (now - ev.ends_at).num_minutes();
            mins_to_start <= 10 && mins_since_end <= 15
        });

        let confidence = Self::compute_signal_confidence(signal, matched_cal);
        let is_live_signal = confidence >= 0.75;

        let inferred_app = signal
            .detected_meeting_app
            .clone()
            .unwrap_or_else(|| "System-Audio Meeting".to_string());
        let inferred_title = matched_cal
            .map(|c| c.title.clone())
            .or_else(|| signal.detected_meeting_title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| format!("Ad-hoc Call ({inferred_app})"));

        match self.phase.clone() {
            DetectorPhase::Idle => {
                if let Some(until) = self.suppress_auto_until {
                    if now < until {
                        return DetectorTransition::None;
                    }
                    self.suppress_auto_until = None;
                }
                if is_live_signal {
                    self.phase = DetectorPhase::Candidate {
                        since: now,
                        title_hint: inferred_title.clone(),
                        app_hint: inferred_app.clone(),
                        confidence,
                    };
                    return DetectorTransition::CandidateDetected {
                        title: inferred_title,
                        app: inferred_app,
                        confidence,
                    };
                }
                DetectorTransition::None
            }
            DetectorPhase::Candidate {
                since,
                title_hint,
                app_hint,
                ..
            } => {
                if !is_live_signal {
                    self.phase = DetectorPhase::Idle;
                    return DetectorTransition::None;
                }
                if (now - since).num_seconds() >= self.candidate_hold_secs && auto_record {
                    return DetectorTransition::StartMeeting {
                        title: title_hint,
                        app: app_hint,
                        calendar_event_id: matched_cal.map(|c| c.event_id.clone()),
                        confidence,
                    };
                }
                DetectorTransition::None
            }
            DetectorPhase::Active {
                meeting_id,
                started_at,
            } => {
                if !signal.microphone_in_use && signal.detected_meeting_app.is_none() {
                    self.phase = DetectorPhase::GraceCooldown {
                        meeting_id,
                        started_at,
                        quiet_since: now,
                    };
                }
                DetectorTransition::None
            }
            DetectorPhase::GraceCooldown {
                meeting_id,
                started_at,
                quiet_since,
            } => {
                if is_live_signal || signal.microphone_in_use {
                    // Reconnected within grace window — seamlessly resume!
                    self.phase = DetectorPhase::Active {
                        meeting_id,
                        started_at,
                    };
                    return DetectorTransition::None;
                }
                if (now - quiet_since).num_seconds() >= self.cooldown_grace_secs {
                    self.phase = DetectorPhase::Idle;
                    return DetectorTransition::EndMeeting { meeting_id };
                }
                DetectorTransition::None
            }
        }
    }

    pub fn mark_active(&mut self, meeting_id: String, started_at: DateTime<Utc>) {
        self.suppress_auto_until = None;
        self.phase = DetectorPhase::Active {
            meeting_id,
            started_at,
        };
    }

    pub fn mark_idle(&mut self) {
        self.phase = DetectorPhase::Idle;
    }

    pub fn mark_manually_stopped(&mut self, now: DateTime<Utc>, suppress_secs: i64) {
        self.phase = DetectorPhase::Idle;
        self.suppress_auto_until = Some(now + chrono::Duration::seconds(suppress_secs));
    }
}

// =============================================================================
// 2. 4-Stage Meeting Participant Identification & Voiceprint Clustering
// =============================================================================

#[derive(Debug, Clone)]
struct UnknownCluster {
    label: String,
    centroid: Vec<f32>,
    samples: u32,
    visual_name_votes: HashMap<String, u32>,
    resolved_person: Option<(Option<String>, String, IdentificationSource, f32)>,
}

pub struct SpeakerAttributionSession {
    pub meeting_id: String,
    pub self_name: String,
    pub calendar_attendees: Vec<(String, String)>, // (name, email)
    clusters: Vec<UnknownCluster>,
    talk_time_by_label: HashMap<String, u64>,
}

impl SpeakerAttributionSession {
    pub fn new(
        meeting_id: impl Into<String>,
        self_name: impl Into<String>,
        calendar_attendees: Vec<(String, String)>,
    ) -> Self {
        Self {
            meeting_id: meeting_id.into(),
            self_name: self_name.into(),
            calendar_attendees,
            clusters: Vec::new(),
            talk_time_by_label: HashMap::new(),
        }
    }

    /// Attributes an incoming speech segment using 4-stage multi-modal sensor fusion:
    /// 1. Microphone channel -> verified Self
    /// 2. System channel + Voiceprint Cosine Match (boosted by Calendar attendees)
    /// 3. Visual Active-Speaker tile correlation (OCR / Accessibility active border)
    /// 4. Online speaker embedding clustering (`SPK_1`, `SPK_2`, ...)
    pub fn attribute_segment(
        &mut self,
        storage: &StorageEngine,
        channel: AudioChannel,
        start_ms: u64,
        end_ms: u64,
        text: &str,
        voice_embedding: &[f32],
        visual_active_speaker_hint: Option<&str>,
    ) -> Result<(TranscriptSegment, MeetingParticipant)> {
        let duration_ms = end_ms.saturating_sub(start_ms);

        // Stage 1: Dedicated Microphone Channel -> 100% deterministic Self
        if channel == AudioChannel::Microphone {
            let label = "SELF".to_string();
            let total = self.talk_time_by_label.entry(label.clone()).or_insert(0);
            *total += duration_ms;

            let participant = MeetingParticipant {
                speaker_label: label.clone(),
                display_name: format!("{} (Ich)", self.self_name),
                person_id: None,
                email: None,
                is_self: true,
                identification_source: IdentificationSource::MicSelf,
                confidence: 1.0,
                talk_time_ms: *total,
            };
            let seg = TranscriptSegment {
                id: format!("seg_{}", uuid::Uuid::new_v4().simple()),
                meeting_id: self.meeting_id.clone(),
                speaker_label: label,
                speaker_name: self.self_name.clone(),
                person_id: None,
                channel,
                start_ms,
                end_ms,
                text: text.to_string(),
                confidence: 0.98,
            };
            return Ok((seg, participant));
        }

        // Stage 2: Check known Voiceprints in Storage (with Calendar Email Prior Boost)
        let cal_emails: Vec<String> = self
            .calendar_attendees
            .iter()
            .map(|(_, e)| e.clone())
            .collect();

        if let Some((person, sim)) = storage.match_voiceprint(voice_embedding, &cal_emails, 0.78)? {
            let label = format!("VP_{}", person.id);
            let total = self.talk_time_by_label.entry(label.clone()).or_insert(0);
            *total += duration_ms;

            // Reinforce voiceprint online when high confidence
            if sim >= 0.86 {
                let existing = storage
                    .list_person_voiceprints()?
                    .into_iter()
                    .find(|(p, _)| p.id == person.id)
                    .map(|(_, v)| v);
                if let Some(old_vp) = existing {
                    let updated = update_voiceprint_centroid(&old_vp, person.voiceprint_samples, voice_embedding);
                    let _ = storage.save_person_voiceprint(
                        &person.id,
                        &updated,
                        person.voiceprint_samples + 1,
                        sim.max(person.voiceprint_confidence),
                    );
                }
            }

            let participant = MeetingParticipant {
                speaker_label: label.clone(),
                display_name: person.full_name.clone(),
                person_id: Some(person.id.clone()),
                email: person.email.clone(),
                is_self: false,
                identification_source: IdentificationSource::Voiceprint,
                confidence: sim,
                talk_time_ms: *total,
            };
            let seg = TranscriptSegment {
                id: format!("seg_{}", uuid::Uuid::new_v4().simple()),
                meeting_id: self.meeting_id.clone(),
                speaker_label: label,
                speaker_name: person.full_name,
                person_id: Some(person.id),
                channel,
                start_ms,
                end_ms,
                text: text.to_string(),
                confidence: sim,
            };
            return Ok((seg, participant));
        }

        // Stage 3 & 4: Online Voice Clustering + Visual Active-Speaker Tile Fusion
        let mut matched_idx: Option<usize> = None;
        let mut best_sim = 0.0_f32;
        for (idx, cluster) in self.clusters.iter().enumerate() {
            let sim = cosine_similarity(voice_embedding, &cluster.centroid);
            if sim >= 0.74 && sim > best_sim {
                best_sim = sim;
                matched_idx = Some(idx);
            }
        }

        let idx = match matched_idx {
            Some(i) => {
                let c = &mut self.clusters[i];
                c.centroid = update_voiceprint_centroid(&c.centroid, c.samples, voice_embedding);
                c.samples += 1;
                i
            }
            None => {
                let next_num = self.clusters.len() + 1;
                let mut centroid = voice_embedding.to_vec();
                l2_normalize(&mut centroid);
                self.clusters.push(UnknownCluster {
                    label: format!("SPK_{next_num}"),
                    centroid,
                    samples: 1,
                    visual_name_votes: HashMap::new(),
                    resolved_person: None,
                });
                self.clusters.len() - 1
            }
        };

        // Accumulate visual active-speaker cue if Zoom/Meet/Teams highlights a speaker tile
        if let Some(raw_visual_name) = visual_active_speaker_hint {
            let clean_name = raw_visual_name.trim();
            if !clean_name.is_empty() {
                let votes = {
                    let c = &mut self.clusters[idx];
                    let entry = c.visual_name_votes.entry(clean_name.to_string()).or_insert(0);
                    *entry += 1;
                    *entry
                };

                let is_on_calendar = self
                    .calendar_attendees
                    .iter()
                    .any(|(n, _)| n.eq_ignore_ascii_case(clean_name));

                // Promote cluster if visual speaker matches calendar invitee (1+ hit) or 2+ consistent hits
                if votes >= 2 || is_on_calendar {
                    let cal_email = self
                        .calendar_attendees
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case(clean_name))
                        .map(|(_, e)| e.as_str());
                    let centroid_copy = self.clusters[idx].centroid.clone();
                    let label_copy = self.clusters[idx].label.clone();
                    let learned_person = storage.assign_speaker_identity(
                        &self.meeting_id,
                        &label_copy,
                        clean_name,
                        cal_email,
                        Some(&centroid_copy),
                    )?;
                    self.clusters[idx].resolved_person = Some((
                        Some(learned_person.id),
                        learned_person.full_name,
                        IdentificationSource::VisualActiveSpeaker,
                        0.90,
                    ));
                }
            }
        }

        let cluster = &self.clusters[idx];
        let total = self
            .talk_time_by_label
            .entry(cluster.label.clone())
            .or_insert(0);
        *total += duration_ms;

        let (person_id, display_name, source, conf) = match &cluster.resolved_person {
            Some((pid, name, src, c)) => (pid.clone(), name.clone(), *src, *c),
            None => (
                None,
                format!("Sprecher {}", idx + 1),
                IdentificationSource::UnidentifiedCluster,
                0.72,
            ),
        };

        let participant = MeetingParticipant {
            speaker_label: cluster.label.clone(),
            display_name: display_name.clone(),
            person_id: person_id.clone(),
            email: None,
            is_self: false,
            identification_source: source,
            confidence: conf,
            talk_time_ms: *total,
        };
        let seg = TranscriptSegment {
            id: format!("seg_{}", uuid::Uuid::new_v4().simple()),
            meeting_id: self.meeting_id.clone(),
            speaker_label: cluster.label.clone(),
            speaker_name: display_name,
            person_id,
            channel,
            start_ms,
            end_ms,
            text: text.to_string(),
            confidence: conf,
        };
        Ok((seg, participant))
    }
}

// =============================================================================
// 3. Screen-Sharing Screenshot Deduplicator & Slide SVG Generator
// =============================================================================

pub struct ScreenShareDeduplicator {
    pub min_dhash_distance: u32,
    pub min_settle_ms: u64,
    last_saved_dhash: Option<u64>,
    pending_candidate: Option<(u64, u64)>, // (dhash, first_seen_ms)
}

impl Default for ScreenShareDeduplicator {
    fn default() -> Self {
        Self {
            min_dhash_distance: 6,
            min_settle_ms: 1_500,
            last_saved_dhash: None,
            pending_candidate: None,
        }
    }
}

impl ScreenShareDeduplicator {
    /// Returns `true` only when a screen-share frame represents a genuinely new slide/view
    /// (`dHash` distance >= 6) AND has stabilized on screen for at least `min_settle_ms`.
    pub fn should_capture_frame(&mut self, current_dhash: u64, elapsed_meeting_ms: u64) -> bool {
        if let Some(prev) = self.last_saved_dhash {
            if dhash_hamming_distance(prev, current_dhash) < self.min_dhash_distance {
                self.pending_candidate = None;
                return false;
            }
        }

        match self.pending_candidate {
            None => {
                self.pending_candidate = Some((current_dhash, elapsed_meeting_ms));
                false
            }
            Some((cand_hash, first_seen_ms)) => {
                if dhash_hamming_distance(cand_hash, current_dhash) >= 4 {
                    // Still actively scrolling/changing — reset settle timer
                    self.pending_candidate = Some((current_dhash, elapsed_meeting_ms));
                    false
                } else if elapsed_meeting_ms.saturating_sub(first_seen_ms) >= self.min_settle_ms {
                    self.last_saved_dhash = Some(current_dhash);
                    self.pending_candidate = None;
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn build_screenshare_frame(
        meeting_id: &str,
        captured_at_ms: u64,
        presenter_name: Option<String>,
        dhash: u64,
        slide_title: &str,
        ocr_lines: &[String],
    ) -> ScreenShareFrame {
        use base64::Engine;
        let mins = captured_at_ms / 60_000;
        let secs = (captured_at_ms % 60_000) / 1_000;
        let badge = format!("SCREEN SHARE · {mins:02}:{secs:02}");

        let mut bullets_svg = String::new();
        for (i, line) in ocr_lines.iter().take(5).enumerate() {
            let y = 136 + i * 34;
            let safe = line
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            bullets_svg.push_str(&format!(
                r##"<circle cx="44" cy="{cy}" r="4" fill="#D96B43"/><text x="60" y="{y}" font-family="Inter, -apple-system, sans-serif" font-size="14" fill="#1E2024">{safe}</text>"##,
                cy = y - 5,
                y = y,
                safe = safe
            ));
        }
        let safe_title = slide_title
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360"><rect width="640" height="360" rx="14" fill="#F6F4EE"/><rect x="16" y="16" width="608" height="328" rx="10" fill="#FFFFFF" stroke="#E2DDD3"/><rect x="36" y="38" width="176" height="24" rx="6" fill="#FDF1EC"/><text x="48" y="54" font-family="JetBrains Mono, monospace" font-size="11" font-weight="600" fill="#C2552D">{badge}</text><text x="36" y="96" font-family="Inter, -apple-system, sans-serif" font-size="20" font-weight="700" fill="#141619">{safe_title}</text><line x1="36" y1="112" x2="604" y2="112" stroke="#ECE8E0"/>{bullets_svg}</svg>"##
        );
        let b64 = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());

        ScreenShareFrame {
            id: format!("frame_{}", uuid::Uuid::new_v4().simple()),
            meeting_id: meeting_id.to_string(),
            captured_at_ms,
            presenter_name,
            dhash_hex: format!("{dhash:016x}"),
            image_data_url: Some(format!("data:image/svg+xml;base64,{b64}")),
            ocr_text: format!("{slide_title} — {}", ocr_lines.join(" · ")),
            slide_title: slide_title.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn detector_handles_auto_start_reconnect_and_cooldown_end() {
        let mut det = MeetingDetector {
            phase: DetectorPhase::Idle,
            candidate_hold_secs: 3,
            cooldown_grace_secs: 30,
            suppress_auto_until: None,
        };
        let t0 = Utc::now();
        let active_sig = AudioEnvironmentSignal {
            microphone_in_use: true,
            system_audio_active: true,
            detected_meeting_app: Some("Zoom".into()),
            detected_meeting_title: Some("Q4 Product Sync".into()),
            has_call_controls_ui: true,
        };

        let tr1 = det.evaluate_tick(t0, &active_sig, &[], true);
        assert!(matches!(tr1, DetectorTransition::CandidateDetected { .. }));

        let tr2 = det.evaluate_tick(t0 + Duration::seconds(4), &active_sig, &[], true);
        assert!(matches!(tr2, DetectorTransition::StartMeeting { .. }));
        det.mark_active("mtg_test".into(), t0 + Duration::seconds(4));

        let quiet_sig = AudioEnvironmentSignal {
            microphone_in_use: false,
            system_audio_active: false,
            detected_meeting_app: None,
            detected_meeting_title: None,
            has_call_controls_ui: false,
        };
        // Brief drop enters cooldown without ending immediately
        assert_eq!(
            det.evaluate_tick(t0 + Duration::seconds(20), &quiet_sig, &[], true),
            DetectorTransition::None
        );
        // Sustained quiet past 30s ends meeting automatically
        let tr_end = det.evaluate_tick(t0 + Duration::seconds(55), &quiet_sig, &[], true);
        assert_eq!(
            tr_end,
            DetectorTransition::EndMeeting {
                meeting_id: "mtg_test".into()
            }
        );
    }

    #[test]
    fn four_stage_speaker_attribution_identifies_mic_voiceprint_and_visual_speaker() {
        let store = StorageEngine::open_in_memory().unwrap();
        let mut session = SpeakerAttributionSession::new(
            "mtg_arch_sync",
            "Luka",
            vec![("Sophie Brandt".into(), "sophie@acme.io".into())],
        );

        // 1. Mic channel -> Self
        let (seg_self, part_self) = session
            .attribute_segment(
                &store,
                AudioChannel::Microphone,
                0,
                4000,
                "Willkommen im Call!",
                &vec![0.0; EMBEDDING_DIM],
                None,
            )
            .unwrap();
        assert!(part_self.is_self);
        assert_eq!(seg_self.speaker_name, "Luka");

        // 2. Known voiceprint -> Anna Weber
        let anna_vec = embed_text_local_256("voiceprint::anna_weber_warm_alto_de_pitch_195hz");
        let (seg_anna, part_anna) = session
            .attribute_segment(
                &store,
                AudioChannel::System,
                5000,
                9000,
                "Hi zusammen, ich zeige euch das neue Design.",
                &anna_vec,
                None,
            )
            .unwrap();
        assert_eq!(seg_anna.speaker_name, "Anna Weber");
        assert_eq!(part_anna.identification_source, IdentificationSource::Voiceprint);

        // 3. New unknown voice + Calendar-matched Visual Active-Speaker tile -> Sophie Brandt
        let sophie_vec = embed_text_local_256("voiceprint::sophie_brandt_new_voice_220hz");
        let (seg_sophie, part_sophie) = session
            .attribute_segment(
                &store,
                AudioChannel::System,
                10_000,
                15_000,
                "Super, auf unserer Seite steht das Release-Datum.",
                &sophie_vec,
                Some("Sophie Brandt"),
            )
            .unwrap();
        assert_eq!(seg_sophie.speaker_name, "Sophie Brandt");
        assert_eq!(
            part_sophie.identification_source,
            IdentificationSource::VisualActiveSpeaker
        );
    }

    #[test]
    fn screenshare_deduplicator_waits_for_settle_and_skips_identical_frames() {
        let mut dedup = ScreenShareDeduplicator::default();
        let slide_1 = 0xF0F0_F0F0_0000_1111_u64;
        assert!(!dedup.should_capture_frame(slide_1, 1000)); // starts settle timer
        assert!(dedup.should_capture_frame(slide_1, 2600));  // settled >= 1500ms -> capture!
        assert!(!dedup.should_capture_frame(slide_1 ^ 0b11, 5000)); // only 2 bits diff -> skip!
    }
}
