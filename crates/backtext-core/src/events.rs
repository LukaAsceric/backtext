use crate::models::{
    CaptureStatus, ContextEpisode, McpAuditEntry, MeetingParticipant, ScreenShareFrame,
    TranscriptSegment,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload", rename_all = "snake_case")]
pub enum BacktextEvent {
    CaptureStatusChanged(CaptureStatus),
    MeetingDetected {
        meeting_id: String,
        title: String,
        detected_app: String,
        confidence: f32,
    },
    TranscriptSegmentAdded(TranscriptSegment),
    ParticipantUpdated {
        meeting_id: String,
        participant: MeetingParticipant,
    },
    ScreenShareCaptured(ScreenShareFrame),
    MeetingSummarized {
        meeting_id: String,
        executive_summary: String,
    },
    ContextEpisodeUpdated(ContextEpisode),
    McpToolInvoked(McpAuditEntry),
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<BacktextEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: BacktextEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BacktextEvent> {
        self.sender.subscribe()
    }
}
