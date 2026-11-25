import React, { useEffect, useRef, useState } from 'react';
import {
  Mic,
  Square,
  Sparkles,
  Users,
  Monitor,
  CheckSquare,
  Plus,
  PanelLeft,
  Trash2,
  AlertCircle,
  Settings as SettingsIcon,
  Radio,
} from 'lucide-react';
import type {
  Language,
  Meeting,
  MeetingDetail,
  Recipe,
  RecordingAudioLevel,
} from '../types';
import { formatDurationMs, sourceBadgeLabel, t } from '../lib/i18n';
import { RecipeChatBar } from '../components/RecipeChatBar';
import { FormattedMarkdown } from '../components/FormattedMarkdown';

interface MeetingWorkspaceProps {
  lang: Language;
  platform: string;
  meetings: Meeting[];
  selectedDetail: MeetingDetail | null;
  recordingLevel: RecordingAudioLevel | null;
  recipes: Recipe[];
  streamingText?: string | null;
  busy: boolean;
  onSelectMeeting: (id: string) => Promise<void>;
  onStartAdhocMeeting: () => Promise<void>;
  onStartScheduledMeeting: (meetingId: string) => Promise<void>;
  onDeleteMeeting: (meetingId: string) => Promise<void>;
  onRenameMeeting: (meetingId: string, title: string) => Promise<void>;
  onSimulateLiveStep: (meetingId: string) => Promise<void>;
  onStopMeeting: (meetingId: string) => Promise<void>;
  onSaveUserNotes: (meetingId: string, notes: string) => Promise<void>;
  onEnhanceMeeting: (meetingId: string) => Promise<void>;
  onToggleActionItem: (meetingId: string, actionId: string) => Promise<void>;
  onAssignSpeaker: (meetingId: string, speakerLabel: string, personName: string) => Promise<void>;
  onRunMeetingChat: (meetingId: string, input: string, recipeSlug?: string) => Promise<void>;
  onOpenSettings: () => void;
}

export const MeetingWorkspace: React.FC<MeetingWorkspaceProps> = ({
  lang,
  platform,
  meetings,
  selectedDetail,
  recordingLevel,
  recipes,
  streamingText,
  busy,
  onSelectMeeting,
  onStartAdhocMeeting,
  onStartScheduledMeeting,
  onDeleteMeeting,
  onRenameMeeting,
  onSimulateLiveStep,
  onStopMeeting,
  onSaveUserNotes,
  onEnhanceMeeting,
  onToggleActionItem,
  onAssignSpeaker,
  onRunMeetingChat,
  onOpenSettings,
}) => {
  const current = selectedDetail?.meeting ?? null;
  const isLive = current?.status === 'live';

  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [activeTab, setActiveTab] = useState<'notes' | 'enhanced' | 'transcript' | 'slides'>('notes');
  const [draftNotes, setDraftNotes] = useState('');
  const [draftTitle, setDraftTitle] = useState('');
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved'>('idle');
  const [showPeoplePopover, setShowPeoplePopover] = useState(false);
  const [renamingLabel, setRenamingLabel] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');

  const saveTimerRef = useRef<number | null>(null);
  const lastMeetingIdRef = useRef<string | null>(null);

  // Sync editor state and pick the most helpful default tab when switching meetings
  useEffect(() => {
    if (!current) return;
    if (lastMeetingIdRef.current !== current.id) {
      lastMeetingIdRef.current = current.id;
      setDraftNotes(current.raw_user_notes ?? '');
      setDraftTitle(current.title ?? '');
      setShowPeoplePopover(false);
      setRenamingLabel(null);
      setSaveState('idle');
      // Granola focus: live meetings always open directly on your clean notepad;
      // completed meetings with AI notes open on AI Notes.
      if (current.status === 'live') {
        setActiveTab('notes');
      } else if (current.enhanced_markdown) {
        setActiveTab('enhanced');
      } else {
        setActiveTab('notes');
      }
    }
  }, [current?.id, current?.status, current?.enhanced_markdown]);

  // Keep title synced if renamed externally
  useEffect(() => {
    if (current && current.title !== draftTitle && document.activeElement?.id !== 'meeting-title-input') {
      setDraftTitle(current.title);
    }
  }, [current?.title]);

  const scheduleAutoSave = (meetingId: string, nextText: string) => {
    setSaveState('saving');
    if (saveTimerRef.current) {
      window.clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = window.setTimeout(async () => {
      try {
        await onSaveUserNotes(meetingId, nextText);
        setSaveState('saved');
      } catch {
        setSaveState('idle');
      }
    }, 650);
  };

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
    };
  }, []);

  const activeRecLevel =
    isLive && recordingLevel && recordingLevel.meeting_id === current?.id ? recordingLevel : null;

  const elapsedDisplay = activeRecLevel
    ? formatDurationMs(activeRecLevel.elapsed_secs * 1000)
    : '00:00';

  return (
    <div className="granola-shell">
      <div className={`granola-layout ${sidebarOpen ? '' : 'sidebar-collapsed'}`}>
        {/* Collapsible Meeting List */}
        {sidebarOpen && (
          <aside className="granola-list-pane">
            <div className="granola-list-header">
              <span style={{ fontWeight: 600, fontSize: 13 }}>
                {lang === 'de' ? 'Meetings' : 'Meetings'}
              </span>
              <button
                type="button"
                id="new-meeting-btn"
                className="btn btn-recording"
                style={{ padding: '5px 10px', fontSize: 12 }}
                onClick={onStartAdhocMeeting}
              >
                <Plus size={13} />
                {lang === 'de' ? 'Neu' : 'New'}
              </button>
            </div>

            <div className="granola-list-scroll">
              {meetings.length === 0 && (
                <div style={{ padding: 12, fontSize: 12.5, color: 'var(--text-tertiary)' }}>
                  {lang === 'de'
                    ? 'Noch keine Meetings. Klicke oben auf „Neu“, um sofort aufzunehmen.'
                    : 'No meetings yet. Click New to start recording.'}
                </div>
              )}
              {meetings.map((m) => (
                <div
                  key={m.id}
                  className={`meeting-list-card ${current?.id === m.id ? 'active' : ''}`}
                  style={{ position: 'relative' }}
                  onClick={() => onSelectMeeting(m.id)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      onSelectMeeting(m.id);
                    }
                  }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', width: '100%', gap: 6 }}>
                    <span
                      style={{
                        fontWeight: 600,
                        fontSize: 13,
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {m.title}
                    </span>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 4, flexShrink: 0 }}>
                      {m.status === 'live' && <span className="pill pill-terracotta">LIVE</span>}
                      <button
                        type="button"
                        className="icon-ghost-btn"
                        title={lang === 'de' ? 'Meeting löschen' : 'Delete meeting'}
                        onClick={(e) => {
                          e.stopPropagation();
                          onDeleteMeeting(m.id);
                        }}
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11.5, color: 'var(--text-tertiary)' }}>
                    <span>
                      {new Date(m.started_at).toLocaleDateString(lang === 'de' ? 'de-DE' : 'en-US', {
                        day: '2-digit',
                        month: 'short',
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                    </span>
                    <span>
                      {m.transcript_count > 0 ? `${m.transcript_count} Seg.` : m.status === 'scheduled' ? 'Geplant' : ''}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </aside>
        )}

        {/* Calm Granola-Style Focused Document Canvas */}
        {current && selectedDetail ? (
          <section className="granola-canvas">
            {/* Minimal Document Top Bar */}
            <div className="granola-topbar">
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <button
                  type="button"
                  className="mini-btn"
                  title={sidebarOpen ? 'Liste ausblenden (Fokus-Modus)' : 'Liste einblenden'}
                  onClick={() => setSidebarOpen((v) => !v)}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}
                >
                  <PanelLeft size={13} />
                  {!sidebarOpen && (lang === 'de' ? 'Liste' : 'List')}
                </button>

                {/* Clean 4-pill view switcher */}
                <div className="tab-strip">
                  <button
                    type="button"
                    id="meeting-tab-notes"
                    className={`tab-btn ${activeTab === 'notes' ? 'active' : ''}`}
                    onClick={() => setActiveTab('notes')}
                  >
                    {t(lang, 'notepadTab')}
                  </button>
                  <button
                    type="button"
                    id="meeting-tab-enhanced"
                    className={`tab-btn ${activeTab === 'enhanced' ? 'active' : ''}`}
                    onClick={() => setActiveTab('enhanced')}
                  >
                    {t(lang, 'enhancedTab')}
                  </button>
                  <button
                    type="button"
                    id="meeting-tab-transcript"
                    className={`tab-btn ${activeTab === 'transcript' ? 'active' : ''}`}
                    onClick={() => setActiveTab('transcript')}
                  >
                    {t(lang, 'transcriptTab')} ({selectedDetail.transcript.length})
                  </button>
                  <button
                    type="button"
                    id="meeting-tab-slides"
                    className={`tab-btn ${activeTab === 'slides' ? 'active' : ''}`}
                    onClick={() => setActiveTab('slides')}
                  >
                    {t(lang, 'slidesTab')} ({selectedDetail.screenshares.length})
                  </button>
                </div>
              </div>

              {/* Right Recording Control */}
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                {isLive ? (
                  <>
                    <div className="live-meter-pill" title={activeRecLevel?.stt_status || 'Dual-Channel Aufnahme aktiv'}>
                      <span className="status-dot recording" />
                      <span className="mono-ts" style={{ color: 'var(--text-primary)', fontWeight: 600 }}>
                        {elapsedDisplay}
                      </span>
                      <div className="audio-bars" aria-label="Audio Level">
                        <span
                          className="audio-bar mic"
                          style={{
                            height: `${Math.max(22, Math.round((activeRecLevel?.mic_level ?? 0.15) * 100))}%`,
                          }}
                        />
                        <span
                          className="audio-bar sys"
                          style={{
                            height: `${Math.max(18, Math.round((activeRecLevel?.system_level ?? 0.1) * 100))}%`,
                          }}
                        />
                      </div>
                    </div>

                    {platform === 'macos-simulator' && (
                      <button
                        type="button"
                        id="simulate-live-step-btn"
                        className="mini-btn"
                        disabled={busy}
                        onClick={() => onSimulateLiveStep(current.id)}
                      >
                        <Radio size={11} style={{ marginRight: 4, verticalAlign: 'middle' }} />
                        {t(lang, 'simulateStep')}
                      </button>
                    )}

                    <button
                      type="button"
                      id="stop-live-meeting-btn"
                      className="btn btn-recording"
                      style={{ padding: '6px 12px', fontSize: 12.5 }}
                      disabled={busy}
                      onClick={async () => {
                        await onStopMeeting(current.id);
                        setActiveTab('enhanced');
                      }}
                    >
                      <Square size={12} />
                      {t(lang, 'stopAndSummarize')}
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      type="button"
                      className="btn btn-recording"
                      style={{ padding: '6px 12px', fontSize: 12.5 }}
                      disabled={busy}
                      onClick={() => onStartScheduledMeeting(current.id)}
                    >
                      <Mic size={13} />
                      {t(lang, 'startBotFree')}
                    </button>
                    <button
                      type="button"
                      id="reenhance-meeting-btn"
                      className="btn"
                      style={{ padding: '6px 12px', fontSize: 12.5 }}
                      disabled={busy}
                      onClick={async () => {
                        await onEnhanceMeeting(current.id);
                        setActiveTab('enhanced');
                      }}
                    >
                      <Sparkles size={13} />
                      {t(lang, 'reEnhance')}
                    </button>
                  </>
                )}
              </div>
            </div>

            {/* Subtle STT / Audio Warning Banner (only when recording and attention needed) */}
            {isLive && activeRecLevel && (activeRecLevel.warning || !activeRecLevel.stt_ready) && (
              <div className="granola-notice-bar">
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.2 }}>
                  <AlertCircle size={14} style={{ flexShrink: 0, color: 'var(--accent-ochre)' }} />
                  <span>{activeRecLevel.warning || activeRecLevel.stt_status}</span>
                </div>
                <button
                  type="button"
                  className="mini-btn"
                  onClick={onOpenSettings}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: 4, flexShrink: 0 }}
                >
                  <SettingsIcon size={11} />
                  {lang === 'de' ? 'Einstellungen' : 'Settings'}
                </button>
              </div>
            )}

            {/* Minimal Editorial Title & Quiet Metadata Row */}
            <div className="granola-doc-header">
              <input
                id="meeting-title-input"
                className="granola-title-input"
                value={draftTitle}
                onChange={(e) => setDraftTitle(e.target.value)}
                onBlur={() => {
                  if (draftTitle.trim() && draftTitle.trim() !== current.title) {
                    onRenameMeeting(current.id, draftTitle.trim());
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    (e.target as HTMLInputElement).blur();
                  }
                }}
                placeholder={lang === 'de' ? 'Meeting-Titel…' : 'Meeting title…'}
              />

              <div className="granola-meta-row">
                <span className="mono-ts">
                  {new Date(current.started_at).toLocaleString(lang === 'de' ? 'de-DE' : 'en-US', {
                    weekday: 'short',
                    day: '2-digit',
                    month: 'short',
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </span>
                <span>·</span>
                <button
                  type="button"
                  className="mini-btn"
                  onClick={() => setShowPeoplePopover((v) => !v)}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}
                >
                  <Users size={11} />
                  {current.participants.length}{' '}
                  {lang === 'de' ? 'Teilnehmer' : 'Participants'}
                </button>
                {activeTab === 'notes' && (
                  <span className="mono-ts" style={{ marginLeft: 'auto' }}>
                    {saveState === 'saving'
                      ? lang === 'de'
                        ? 'Speichert…'
                        : 'Saving…'
                      : saveState === 'saved'
                        ? lang === 'de'
                          ? 'Gespeichert'
                          : 'Saved'
                        : ''}
                  </span>
                )}
              </div>

              {/* Compact On-Demand Participants & Voiceprint Popover */}
              {showPeoplePopover && (
                <div className="granola-people-popover">
                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
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
                        <span>{p.display_name}</span>
                        <span className="mono-ts">· {Math.round(p.confidence * 100)}%</span>
                      </button>
                    ))}
                  </div>
                  {renamingLabel && (
                    <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap', marginTop: 8 }}>
                      <input
                        className="input-field"
                        style={{ maxWidth: 220, padding: '5px 9px', fontSize: 12.5 }}
                        value={renameValue}
                        onChange={(e) => setRenameValue(e.target.value)}
                        placeholder="Vor- und Nachname"
                      />
                      <button
                        type="button"
                        className="btn btn-primary"
                        style={{ padding: '5px 10px', fontSize: 12 }}
                        onClick={async () => {
                          if (renameValue.trim()) {
                            await onAssignSpeaker(current.id, renamingLabel, renameValue.trim());
                            setRenamingLabel(null);
                          }
                        }}
                      >
                        {lang === 'de' ? 'Speichern & Stimme lernen' : 'Save & Learn Voice'}
                      </button>
                      <button
                        type="button"
                        className="mini-btn"
                        onClick={() => setRenamingLabel(null)}
                      >
                        {lang === 'de' ? 'Abbrechen' : 'Cancel'}
                      </button>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Main Distraction-Free Canvas Body */}
            <div className="granola-body">
              {activeTab === 'notes' && (
                <textarea
                  id="meeting-rough-notes-editor"
                  className="granola-notepad-textarea"
                  value={draftNotes}
                  onChange={(e) => {
                    const next = e.target.value;
                    setDraftNotes(next);
                    scheduleAutoSave(current.id, next);
                  }}
                  onBlur={() => {
                    if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
                    onSaveUserNotes(current.id, draftNotes);
                    setSaveState('saved');
                  }}
                  placeholder={
                    lang === 'de'
                      ? 'Notiere einfach deine Stichpunkte während des Gesprächs…\n\nbacktext zeichnet im Hintergrund auf und verdichtet deine Notizen danach automatisch mit Transkript & geteilten Folien.'
                      : 'Jot rough thoughts freely during the call…\n\nbacktext records quietly in the background and enhances your notes with transcript & slide facts afterward.'
                  }
                />
              )}

              {activeTab === 'enhanced' && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
                  <div className="granola-enhanced-sheet">
                    <FormattedMarkdown
                      text={
                        current.enhanced_markdown ||
                        current.prep_brief ||
                        (lang === 'de'
                          ? 'Noch keine KI-Notizen generiert. Klicke oben auf **Beenden** oder **Mit KI verdichten**.'
                          : 'No AI notes generated yet. Click **Finish** or **Enhance with AI** above.')
                      }
                    />
                  </div>

                  {current.action_items.length > 0 && (
                    <div>
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
              )}

              {activeTab === 'transcript' && (
                <div className="transcript-list">
                  {selectedDetail.transcript.length === 0 && (
                    <div style={{ color: 'var(--text-secondary)', fontSize: 13.5, padding: '18px 4px' }}>
                      {isLive
                        ? lang === 'de'
                          ? 'Aufnahme läuft — gesprochene Abschnitte erscheinen hier automatisch alle paar Sekunden.'
                          : 'Recording active — spoken segments appear here automatically every few seconds.'
                        : lang === 'de'
                          ? 'Noch keine Transkript-Segmente für dieses Meeting.'
                          : 'No transcript segments for this meeting yet.'}
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
                                setShowPeoplePopover(true);
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
                            {seg.channel === 'microphone' ? 'Ich' : 'System-Audio'}
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
                    <div style={{ color: 'var(--text-secondary)', fontSize: 13.5, padding: '18px 4px' }}>
                      {lang === 'de'
                        ? 'Noch keine geteilten Folien für dieses Meeting erfasst.'
                        : 'No shared slides captured for this meeting yet.'}
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
                              {frame.presenter_name ?? 'Screen Share'}
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
            </div>

            {/* Granola-Style Anchored Bottom AI Chat & / Recipes Bar */}
            <RecipeChatBar
              lang={lang}
              meetingId={current.id}
              scopeFilter="single_meeting"
              compact
              recipes={recipes}
              messages={selectedDetail.chat_messages}
              streamingText={streamingText}
              busy={busy}
              onExecute={(inp, slug) => onRunMeetingChat(current.id, inp, slug)}
            />
          </section>
        ) : (
          <section
            className="granola-canvas"
            style={{ alignItems: 'center', justifyContent: 'center', gap: 12, padding: 48 }}
          >
            <div style={{ fontFamily: 'var(--font-editorial)', fontSize: 22, fontWeight: 600 }}>
              {lang === 'de' ? 'Bereit für dein nächstes Meeting' : 'Ready for your next meeting'}
            </div>
            <p style={{ margin: 0, color: 'var(--text-secondary)', fontSize: 13.5 }}>
              {lang === 'de'
                ? 'Starte eine neue Aufnahme, mache entspannt deine Notizen und frag jederzeit die KI.'
                : 'Start a new recording, jot notes calmly, and ask AI anytime.'}
            </p>
            <button type="button" className="btn btn-recording" onClick={onStartAdhocMeeting}>
              <Mic size={14} />
              {t(lang, 'newAdhocMeeting')}
            </button>
          </section>
        )}
      </div>
    </div>
  );
};
