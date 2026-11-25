import React, { useEffect, useState } from 'react';
import {
  Mic,
  Square,
  Sparkles,
  UserCheck,
  Monitor,
  CheckSquare,
  Plus,
  Radio,
} from 'lucide-react';
import type { Language, Meeting, MeetingDetail, Recipe } from '../types';
import { formatDurationMs, sourceBadgeLabel, t } from '../lib/i18n';
import { RecipeChatBar } from '../components/RecipeChatBar';
import { FormattedMarkdown } from '../components/FormattedMarkdown';

interface MeetingWorkspaceProps {
  lang: Language;
  meetings: Meeting[];
  selectedDetail: MeetingDetail | null;
  recipes: Recipe[];
  streamingText?: string | null;
  busy: boolean;
  onSelectMeeting: (id: string) => Promise<void>;
  onStartAdhocMeeting: () => Promise<void>;
  onSimulateLiveStep: (meetingId: string) => Promise<void>;
  onCaptureRealAudioTurn: (meetingId: string) => Promise<void>;
  onStopMeeting: (meetingId: string) => Promise<void>;
  onSaveUserNotes: (meetingId: string, notes: string) => Promise<void>;
  onEnhanceMeeting: (meetingId: string) => Promise<void>;
  onToggleActionItem: (meetingId: string, actionId: string) => Promise<void>;
  onAssignSpeaker: (meetingId: string, speakerLabel: string, personName: string) => Promise<void>;
  onRunMeetingChat: (meetingId: string, input: string, recipeSlug?: string) => Promise<void>;
}

export const MeetingWorkspace: React.FC<MeetingWorkspaceProps> = ({
  lang,
  meetings,
  selectedDetail,
  recipes,
  streamingText,
  busy,
  onSelectMeeting,
  onStartAdhocMeeting,
  onSimulateLiveStep,
  onCaptureRealAudioTurn,
  onStopMeeting,
  onSaveUserNotes,
  onEnhanceMeeting,
  onToggleActionItem,
  onAssignSpeaker,
  onRunMeetingChat,
}) => {
  const [activeTab, setActiveTab] = useState<'notepad' | 'transcript' | 'slides'>('notepad');
  const [draftNotes, setDraftNotes] = useState('');
  const [renamingLabel, setRenamingLabel] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');

  useEffect(() => {
    if (selectedDetail) {
      setDraftNotes(selectedDetail.meeting.raw_user_notes ?? '');
    }
  }, [selectedDetail?.meeting.id, selectedDetail?.meeting.raw_user_notes]);

  const current = selectedDetail?.meeting;

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">
            {lang === 'de' ? 'Meetings & Hybrid Notepad' : 'Meetings & Hybrid Notepad'}
          </h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Bot-freie Dual-Audio-Aufnahme, 4-stufige Teilnehmer-Erkennung, deduplizierte Screen-Share-Folien & Granola-Recipes.'
              : 'Bot-free dual-audio capture, 4-stage speaker identification, deduplicated screen-share slides & Granola Recipes.'}
          </p>
        </div>
        <button type="button" className="btn btn-recording" onClick={onStartAdhocMeeting}>
          <Plus size={15} />
          {t(lang, 'newAdhocMeeting')}
        </button>
      </header>

      <div className="meetings-layout">
        {/* Left Meeting List */}
        <aside className="meeting-list-pane">
          {meetings.map((m) => (
            <button
              key={m.id}
              type="button"
              className={`meeting-list-card ${current?.id === m.id ? 'active' : ''}`}
              onClick={() => onSelectMeeting(m.id)}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', width: '100%', gap: 6 }}>
                <span style={{ fontWeight: 600, fontSize: 13 }}>{m.title}</span>
                {m.status === 'live' && <span className="pill pill-terracotta">LIVE</span>}
              </div>
              <span className="mono-ts">{m.detected_app}</span>
              <span style={{ fontSize: 11.5, color: 'var(--text-tertiary)' }}>
                {m.participants.length} {lang === 'de' ? 'Teilnehmer' : 'attendees'} · {m.screenshare_count} Folien
              </span>
            </button>
          ))}
        </aside>

        {/* Active Meeting Workspace */}
        {current && selectedDetail ? (
          <div className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
              <div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                  <span
                    className={`pill ${
                      current.status === 'live'
                        ? 'pill-terracotta'
                        : current.status === 'scheduled'
                          ? 'pill-ochre'
                          : 'pill-sage'
                    }`}
                  >
                    {current.status.toUpperCase()}
                  </span>
                  <span className="mono-ts">{current.detected_app}</span>
                  {current.project_tag && <span className="pill pill-neutral">{current.project_tag}</span>}
                </div>
                <h2
                  style={{
                    margin: '8px 0 4px',
                    fontFamily: 'var(--font-editorial)',
                    fontSize: 23,
                    fontWeight: 600,
                  }}
                >
                  {current.title}
                </h2>
                {/* Participant Pills with 4-Stage Attribution Source */}
                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginTop: 8 }}>
                  {current.participants.map((p) => (
                    <button
                      key={p.speaker_label}
                      type="button"
                      className="pill pill-neutral"
                      style={{ cursor: p.is_self ? 'default' : 'pointer' }}
                      title={`${sourceBadgeLabel(p.identification_source, lang)} (${Math.round(p.confidence * 100)}%)`}
                      onClick={() => {
                        if (!p.is_self) {
                          setRenamingLabel(p.speaker_label);
                          setRenameValue(p.display_name);
                        }
                      }}
                    >
                      <UserCheck size={12} />
                      <span>{p.display_name}</span>
                      <span className="mono-ts">
                        · {sourceBadgeLabel(p.identification_source, lang)} ({Math.round(p.confidence * 100)}%)
                      </span>
                    </button>
                  ))}
                </div>
              </div>

              {/* Live Controls */}
              <div style={{ display: 'flex', gap: 8, alignItems: 'flex-start', flexWrap: 'wrap' }}>
                {current.status === 'live' && (
                  <>
                    <button
                      type="button"
                      id="capture-real-audio-btn"
                      className="btn btn-sage"
                      disabled={busy}
                      onClick={() => onCaptureRealAudioTurn(current.id)}
                    >
                      <Mic size={14} />
                      {lang === 'de' ? 'Echtes Dual-Audio (6s)' : 'Capture Real Dual-Audio (6s)'}
                    </button>
                    <button
                      type="button"
                      id="simulate-live-step-btn"
                      className="btn"
                      disabled={busy}
                      onClick={() => onSimulateLiveStep(current.id)}
                    >
                      <Radio size={14} />
                      {t(lang, 'simulateStep')}
                    </button>
                    <button
                      type="button"
                      id="stop-live-meeting-btn"
                      className="btn btn-recording"
                      disabled={busy}
                      onClick={() => onStopMeeting(current.id)}
                    >
                      <Square size={13} />
                      {t(lang, 'stopAndSummarize')}
                    </button>
                  </>
                )}
                {current.status !== 'live' && (
                  <button
                    type="button"
                    id="reenhance-meeting-btn"
                    className="btn btn-sage"
                    disabled={busy}
                    onClick={() => onEnhanceMeeting(current.id)}
                  >
                    <Sparkles size={14} />
                    {t(lang, 'reEnhance')}
                  </button>
                )}
              </div>
            </div>

            {/* Inline Speaker Rename / Voiceprint Training Drawer */}
            {renamingLabel && (
              <div
                style={{
                  padding: '10px 14px',
                  borderRadius: 'var(--radius-sm)',
                  background: 'var(--accent-sage-soft)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 10,
                  flexWrap: 'wrap',
                }}
              >
                <span style={{ fontSize: 12.5, fontWeight: 600 }}>
                  {lang === 'de'
                    ? `Stimmprofil (${renamingLabel}) dauerhaft verknüpfen mit:`
                    : `Link voiceprint (${renamingLabel}) permanently to:`}
                </span>
                <input
                  className="input-field"
                  style={{ maxWidth: 240 }}
                  value={renameValue}
                  onChange={(e) => setRenameValue(e.target.value)}
                  placeholder="Vor- und Nachname"
                />
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={async () => {
                    if (renameValue.trim()) {
                      await onAssignSpeaker(current.id, renamingLabel, renameValue.trim());
                      setRenamingLabel(null);
                    }
                  }}
                >
                  {lang === 'de' ? 'Speichern & Stimmprofil lernen' : 'Save & Train Voiceprint'}
                </button>
                <button type="button" className="btn" onClick={() => setRenamingLabel(null)}>
                  {lang === 'de' ? 'Abbrechen' : 'Cancel'}
                </button>
              </div>
            )}

            {/* Sub-Tabs */}
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 10 }}>
              <div className="tab-strip">
                <button
                  type="button"
                  className={`tab-btn ${activeTab === 'notepad' ? 'active' : ''}`}
                  onClick={() => setActiveTab('notepad')}
                >
                  {t(lang, 'notepadTab')}
                </button>
                <button
                  type="button"
                  className={`tab-btn ${activeTab === 'transcript' ? 'active' : ''}`}
                  onClick={() => setActiveTab('transcript')}
                >
                  {t(lang, 'transcriptTab')} ({selectedDetail.transcript.length})
                </button>
                <button
                  type="button"
                  className={`tab-btn ${activeTab === 'slides' ? 'active' : ''}`}
                  onClick={() => setActiveTab('slides')}
                >
                  {t(lang, 'slidesTab')} ({selectedDetail.screenshares.length})
                </button>
              </div>
            </div>

            {activeTab === 'notepad' && (
              <div className="two-col-grid">
                <div>
                  <div className="section-heading">
                    <span>{t(lang, 'yourRoughNotes')}</span>
                    <span className="mono-ts">Auto-Save</span>
                  </div>
                  <textarea
                    id="meeting-rough-notes-editor"
                    className="notepad-editor"
                    value={draftNotes}
                    onChange={(e) => setDraftNotes(e.target.value)}
                    onBlur={() => onSaveUserNotes(current.id, draftNotes)}
                    placeholder={
                      lang === 'de'
                        ? '- Eigene Gedanken oder Stichpunkte tippen… backtext reichert sie automatisch mit Fakten aus Transkript & Folien an.'
                        : '- Jot rough bullets freely… backtext enhances them with transcript & slide facts.'
                    }
                  />

                  {current.action_items.length > 0 && (
                    <div style={{ marginTop: 18 }}>
                      <div className="section-heading">
                        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                          <CheckSquare size={14} />
                          {t(lang, 'actionItems')}
                        </span>
                      </div>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 7 }}>
                        {current.action_items.map((act) => (
                          <label
                            key={act.id}
                            style={{
                              display: 'flex',
                              alignItems: 'flex-start',
                              gap: 9,
                              padding: '8px 10px',
                              borderRadius: 6,
                              background: 'var(--bg-surface-subtle)',
                              cursor: 'pointer',
                              fontSize: 13,
                            }}
                          >
                            <input
                              type="checkbox"
                              checked={act.completed}
                              onChange={() => onToggleActionItem(current.id, act.id)}
                              style={{ marginTop: 3 }}
                            />
                            <span style={{ textDecoration: act.completed ? 'line-through' : 'none' }}>
                              <strong>{act.owner}:</strong> {act.task}{' '}
                              {act.due_hint && <span className="mono-ts">({act.due_hint})</span>}
                            </span>
                          </label>
                        ))}
                      </div>
                    </div>
                  )}
                </div>

                <div>
                  <div className="section-heading">
                    <span>{t(lang, 'enhancedSummary')}</span>
                    <span className="pill pill-sage">Granola-Fusion</span>
                  </div>
                  <div
                    style={{
                      background: 'var(--bg-surface-subtle)',
                      border: '1px solid var(--border-subtle)',
                      borderRadius: 'var(--radius-sm)',
                      padding: '14px 16px',
                      minHeight: 175,
                    }}
                  >
                    <FormattedMarkdown
                      text={
                        current.enhanced_markdown ||
                        current.prep_brief ||
                        (lang === 'de'
                          ? 'Noch keine Zusammenfassung generiert. Klicke auf „Beenden & Zusammenfassen“ oder „Mit KI neu verdichten“.'
                          : 'No enhanced summary yet. Click Finish & Summarize or Re-enhance Notes.')
                      }
                    />
                  </div>
                </div>
              </div>
            )}

            {activeTab === 'transcript' && (
              <div className="transcript-list">
                {selectedDetail.transcript.length === 0 && (
                  <div style={{ color: 'var(--text-secondary)', fontSize: 13 }}>
                    {lang === 'de'
                      ? 'Noch keine Sprachsegmente aufgezeichnet. Klicke oben auf „Live-Segment & Folie erfassen“.'
                      : 'No transcript segments yet. Click "Simulate Live Speech & Slide" above.'}
                  </div>
                )}
                {selectedDetail.transcript.map((seg) => (
                  <div key={seg.id} className="transcript-row">
                    <div className="transcript-meta">
                      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                        <button
                          type="button"
                          className="mini-btn"
                          style={{ fontWeight: 600, color: 'var(--text-primary)' }}
                          onClick={() => {
                            if (seg.channel !== 'microphone') {
                              setRenamingLabel(seg.speaker_label);
                              setRenameValue(seg.speaker_name);
                            }
                          }}
                        >
                          {seg.speaker_name}
                        </button>
                        <span
                          className={`pill ${
                            seg.channel === 'microphone' ? 'pill-sage' : 'pill-neutral'
                          }`}
                        >
                          <Mic size={10} />
                          {seg.channel === 'microphone' ? 'Mic (Ich)' : 'System-Audio'}
                        </span>
                      </div>
                      <span className="mono-ts">
                        {formatDurationMs(seg.start_ms)} – {formatDurationMs(seg.end_ms)}
                      </span>
                    </div>
                    <div style={{ fontSize: 13.8 }}>{seg.text}</div>
                  </div>
                ))}
              </div>
            )}

            {activeTab === 'slides' && (
              <div>
                {selectedDetail.screenshares.length === 0 ? (
                  <div style={{ color: 'var(--text-secondary)', fontSize: 13 }}>
                    {lang === 'de'
                      ? 'Noch keine Screen-Sharing-Folien für dieses Meeting erfasst.'
                      : 'No screen-sharing slides captured for this meeting yet.'}
                  </div>
                ) : (
                  <div className="slides-grid">
                    {selectedDetail.screenshares.map((frame) => (
                      <div key={frame.id} className="slide-card">
                        {frame.image_data_url && (
                          <img src={frame.image_data_url} alt={frame.slide_title} />
                        )}
                        <div className="slide-card-body">
                          <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8 }}>
                            <strong style={{ fontSize: 13 }}>{frame.slide_title}</strong>
                            <span className="mono-ts">@{formatDurationMs(frame.captured_at_ms)}</span>
                          </div>
                          <div className="mono-ts">
                            <Monitor size={11} style={{ verticalAlign: 'middle', marginRight: 4 }} />
                            {frame.presenter_name ?? 'Screen Share'} · dHash {frame.dhash_hex.slice(0, 10)}
                          </div>
                          <div style={{ fontSize: 12.2, color: 'var(--text-secondary)', marginTop: 4 }}>
                            {frame.ocr_text}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {/* Meeting Contextual Chat & Single-Meeting Recipes */}
            <RecipeChatBar
              lang={lang}
              meetingId={current.id}
              scopeFilter="single_meeting"
              recipes={recipes}
              messages={selectedDetail.chat_messages}
              streamingText={streamingText}
              busy={busy}
              onExecute={(inp, slug) => onRunMeetingChat(current.id, inp, slug)}
            />
          </div>
        ) : null}
      </div>
    </div>
  );
};
