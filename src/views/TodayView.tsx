import React, { useEffect, useRef, useState } from 'react';
import {
  Mic,
  Search,
  ArrowRight,
  Layers,
  Calendar,
  CheckCircle2,
  ShieldCheck,
  RefreshCw,
  Trash2,
  Lock,
} from 'lucide-react';
import type {
  ChatMessage,
  ChatScope,
  ContextEpisode,
  DaemonTelemetry,
  Language,
  Meeting,
  MemorySearchHit,
  PermissionStatus,
  Recipe,
} from '../types';
import { t } from '../lib/i18n';
import { RecipeChatBar } from '../components/RecipeChatBar';
import { FormattedMarkdown } from '../components/FormattedMarkdown';

interface TodayViewProps {
  lang: Language;
  permissions: PermissionStatus;
  telemetry: DaemonTelemetry;
  meetings: Meeting[];
  episodes: ContextEpisode[];
  recipes: Recipe[];
  globalMessages: ChatMessage[];
  chatScope?: { scope: ChatScope; label: string } | null;
  onClearChatScope?: () => void;
  streamingText?: string | null;
  chatBusy: boolean;
  onStartAdhocMeeting: () => Promise<void>;
  onStartScheduledMeeting: (meetingId: string) => Promise<void>;
  onOpenMeeting: (meetingId: string) => void;
  onSearchMemory: (query: string) => Promise<MemorySearchHit[]>;
  onRunGlobalChat: (input: string, recipeSlug?: string) => Promise<void>;
  onRequestPermission: (kind: string) => Promise<void>;
  onRefreshPermissions: () => Promise<void>;
  onClearDemoData: () => Promise<void>;
  onSyncCalendars: () => Promise<void>;
}

export const TodayView: React.FC<TodayViewProps> = ({
  lang,
  permissions,
  telemetry,
  meetings,
  episodes,
  recipes,
  globalMessages,
  chatScope,
  onClearChatScope,
  streamingText,
  chatBusy,
  onStartAdhocMeeting,
  onStartScheduledMeeting,
  onOpenMeeting,
  onSearchMemory,
  onRunGlobalChat,
  onRequestPermission,
  onRefreshPermissions,
  onClearDemoData,
  onSyncCalendars,
}) => {
  const [query, setQuery] = useState('');
  const [hits, setHits] = useState<MemorySearchHit[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [syncingCal, setSyncingCal] = useState(false);
  const [checkingPerms, setCheckingPerms] = useState(false);
  const chatSectionRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (chatScope && chatSectionRef.current) {
      chatSectionRef.current.scrollIntoView({ behavior: 'smooth', block: 'end' });
    }
  }, [chatScope]);

  const missingPermissions =
    !permissions.screen_capture_granted ||
    !permissions.microphone_granted ||
    !permissions.accessibility_granted ||
    !permissions.calendar_granted;

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim()) {
      setHits(null);
      return;
    }
    setSearching(true);
    try {
      const res = await onSearchMemory(query);
      setHits(res);
    } finally {
      setSearching(false);
    }
  };

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">{t(lang, 'todayTitle')}</h1>
          <p className="page-subtitle">{t(lang, 'todaySubtitle')}</p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            id="sync-calendars-btn"
            className="btn"
            disabled={syncingCal}
            onClick={async () => {
              setSyncingCal(true);
              try {
                await onSyncCalendars();
              } finally {
                setSyncingCal(false);
              }
            }}
          >
            <RefreshCw size={14} />
            {lang === 'de' ? 'Kalender abgleichen' : 'Sync Calendars'}
          </button>
          <button
            type="button"
            id="start-adhoc-meeting-btn"
            className="btn btn-recording"
            onClick={onStartAdhocMeeting}
          >
            <Mic size={15} />
            {t(lang, 'newAdhocMeeting')}
          </button>
        </div>
      </header>

      {/* Onboarding Permissions, Daemon Telemetry & Clean-Start Strip */}
      <section className="surface-sheet" style={{ padding: '14px 18px', display: 'flex', flexDirection: 'column', gap: 12 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 10 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
            <span className="pill pill-sage">
              <ShieldCheck size={12} />
              {telemetry.loop_running
                ? lang === 'de'
                  ? `Hintergrund-Daemon aktiv (${telemetry.ticks_processed} Ticks)`
                  : `Background Daemon Active (${telemetry.ticks_processed} ticks)`
                : 'Daemon bereit'}
            </span>
            <span className="pill pill-neutral">
              <Lock size={11} />
              {telemetry.db_encrypted ? 'SQLCipher AES-256' : 'SQLite WAL'} · {telemetry.keychain_backend}
            </span>
            {telemetry.last_window_app && (
              <span className="mono-ts">
                {lang === 'de' ? 'Zuletzt beobachtet:' : 'Last observed:'} {telemetry.last_window_app}
              </span>
            )}
          </div>

          {telemetry.demo_data_present && (
            <button
              type="button"
              id="clear-demo-data-btn"
              className="mini-btn"
              onClick={onClearDemoData}
              style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}
            >
              <Trash2 size={11} />
              {lang === 'de' ? 'Beispieldaten leeren & sauber starten' : 'Clear Demo Data'}
            </button>
          )}
        </div>

        {missingPermissions && (
          <div
            style={{
              paddingTop: 10,
              borderTop: '1px solid var(--border-subtle)',
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              flexWrap: 'wrap',
              gap: 10,
            }}
          >
            <span style={{ fontSize: 12.5, fontWeight: 600 }}>
              {lang === 'de'
                ? 'macOS Berechtigungen für automatische Kontexterfassung & bot-freie Meetings:'
                : 'macOS Permissions for Ambient Capture & Bot-Free Calls:'}
            </span>
            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', alignItems: 'center' }}>
              {(
                [
                  ['screen', 'ScreenCaptureKit', permissions.screen_capture_granted],
                  ['microphone', 'Mikrofon', permissions.microphone_granted],
                  ['accessibility', 'Bedienungshilfen', permissions.accessibility_granted],
                  ['calendar', 'Kalender (EventKit)', permissions.calendar_granted],
                ] as const
              ).map(([kind, label, ok]) => (
                <button
                  key={kind}
                  type="button"
                  className={`mini-btn ${ok ? '' : 'active-choice'}`}
                  onClick={() => onRequestPermission(kind)}
                >
                  {ok ? '✓ ' : '+ '}
                  {label}
                </button>
              ))}
              <button
                type="button"
                className="mini-btn"
                disabled={checkingPerms}
                onClick={async () => {
                  setCheckingPerms(true);
                  try {
                    await onRefreshPermissions();
                  } finally {
                    setCheckingPerms(false);
                  }
                }}
              >
                <RefreshCw size={10} style={{ marginRight: 4, verticalAlign: 'middle' }} />
                {checkingPerms ? '…' : lang === 'de' ? 'Neu prüfen' : 'Re-check'}
              </button>
            </div>
          </div>
        )}
      </section>

      {/* Hybrid Memory Search */}
      <section className="surface-sheet" style={{ padding: '14px 18px' }}>
        <form onSubmit={handleSearch} style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
          <Search size={17} style={{ color: 'var(--text-tertiary)', flexShrink: 0 }} />
          <input
            id="global-memory-search-input"
            className="input-field"
            style={{ border: 'none', padding: '4px 2px', background: 'transparent' }}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              if (!e.target.value.trim()) setHits(null);
            }}
            placeholder={t(lang, 'searchPlaceholder')}
          />
          {query && (
            <button type="submit" className="btn" disabled={searching}>
              {searching ? '…' : lang === 'de' ? 'Suchen' : 'Search'}
            </button>
          )}
        </form>

        {hits && (
          <div style={{ marginTop: 12, borderTop: '1px solid var(--border-subtle)', paddingTop: 12 }}>
            <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginBottom: 8 }}>
              {hits.length}{' '}
              {lang === 'de'
                ? 'Treffer (Hybrid BM25 + Vektor-RRF)'
                : 'results (Hybrid BM25 + Vector RRF)'}
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {hits.map((h) => (
                <div key={h.id} className="transcript-row">
                  <div className="transcript-meta">
                    <span style={{ fontWeight: 600 }}>{h.title}</span>
                    <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                      <span className="pill pill-sage">{h.source_kind}</span>
                      <span className="mono-ts">{Math.round(h.score * 100)}% Match</span>
                    </div>
                  </div>
                  <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{h.snippet}</div>
                </div>
              ))}
            </div>
          </div>
        )}
      </section>

      <div className="two-col-grid">
        {/* Upcoming, Live & Recent Meetings with Pre-Meeting Briefs */}
        <section className="surface-sheet">
          <h2 className="section-heading">
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
              <Calendar size={15} />
              {t(lang, 'upcomingMeetings')}
            </span>
            <span className="mono-ts">{meetings.length}</span>
          </h2>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            {meetings.length === 0 && (
              <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
                {lang === 'de'
                  ? 'Noch keine Meetings aufgezeichnet. Starte ein Ad-hoc Meeting oder synchronisiere deinen Kalender.'
                  : 'No meetings yet. Start an ad-hoc call or sync your calendar.'}
              </div>
            )}
            {meetings.slice(0, 4).map((m) => (
              <div
                key={m.id}
                style={{
                  padding: '14px 15px',
                  borderRadius: 'var(--radius-sm)',
                  border: '1px solid var(--border-subtle)',
                  background: 'var(--bg-surface-subtle)',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 8,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, alignItems: 'flex-start' }}>
                  <div>
                    <div style={{ fontWeight: 600, fontSize: 14.5 }}>{m.title}</div>
                    <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginTop: 2 }}>
                      {m.detected_app} · {m.participants.map((p) => p.display_name).join(', ')}
                    </div>
                  </div>
                  <span
                    className={`pill ${
                      m.status === 'live'
                        ? 'pill-terracotta'
                        : m.status === 'scheduled'
                          ? 'pill-ochre'
                          : 'pill-sage'
                    }`}
                  >
                    {m.status === 'live'
                      ? 'LIVE'
                      : m.status === 'scheduled'
                        ? lang === 'de'
                          ? 'Geplant'
                          : 'Scheduled'
                        : lang === 'de'
                          ? 'Aufgezeichnet'
                          : 'Recorded'}
                  </span>
                </div>

                {(m.prep_brief || m.executive_summary) && (
                  <div
                    style={{
                      fontSize: 12.8,
                      color: 'var(--text-secondary)',
                      background: 'var(--bg-surface)',
                      padding: '9px 11px',
                      borderRadius: 6,
                      border: '1px solid var(--border-subtle)',
                    }}
                  >
                    <FormattedMarkdown
                      text={(m.status === 'scheduled' ? m.prep_brief : m.executive_summary || m.prep_brief) ?? ''}
                    />
                  </div>
                )}

                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 2 }}>
                  <span className="mono-ts">
                    {m.transcript_count} Segmente · {m.screenshare_count} Folien · {m.action_items.length} Actions
                  </span>
                  <div style={{ display: 'flex', gap: 6 }}>
                    {m.status === 'scheduled' && (
                      <button
                        type="button"
                        className="btn btn-recording"
                        style={{ padding: '5px 10px', fontSize: 12 }}
                        onClick={() => onStartScheduledMeeting(m.id)}
                      >
                        <Mic size={13} />
                        {t(lang, 'startBotFree')}
                      </button>
                    )}
                    <button
                      type="button"
                      className="btn"
                      style={{ padding: '5px 10px', fontSize: 12 }}
                      onClick={() => onOpenMeeting(m.id)}
                    >
                      {t(lang, 'openWorkspace')}
                      <ArrowRight size={13} />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </section>

        {/* Compacted Ambient Work Episodes */}
        <section className="surface-sheet">
          <h2 className="section-heading">
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
              <Layers size={15} />
              {t(lang, 'recentEpisodes')}
            </span>
            <span className="pill pill-sage">Auto-Filtered</span>
          </h2>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 11 }}>
            {episodes.length === 0 && (
              <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
                {lang === 'de'
                  ? 'Der Hintergrund-Daemon verdichtet deine aktiven Fenster automatisch zu Episoden.'
                  : 'The background daemon automatically compacts active work into episodes.'}
              </div>
            )}
            {episodes.slice(0, 5).map((ep) => (
              <div key={ep.id} className="transcript-row">
                <div className="transcript-meta">
                  <span style={{ fontWeight: 600, fontSize: 13.5 }}>{ep.title}</span>
                  <span className="mono-ts">
                    {new Date(ep.ended_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                  </span>
                </div>
                <div style={{ fontSize: 12.8, color: 'var(--text-secondary)' }}>{ep.summary}</div>
                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginTop: 3 }}>
                  {ep.project_tag && (
                    <span className="pill pill-sage">
                      <CheckCircle2 size={11} />
                      {ep.project_tag}
                    </span>
                  )}
                  {ep.primary_apps.map((app) => (
                    <span key={app} className="pill pill-neutral">
                      {app}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </section>
      </div>

      {/* Workspace-wide Chat & Multi-Meeting / Folder / Person Scoped Chat */}
      <section ref={chatSectionRef} className="surface-sheet">
        <RecipeChatBar
          lang={lang}
          scopeFilter="all"
          scopeChip={
            chatScope && onClearChatScope
              ? { label: chatScope.label, onClear: onClearChatScope }
              : null
          }
          autoFocus={Boolean(chatScope)}
          recipes={recipes}
          messages={globalMessages}
          onOpenMeeting={onOpenMeeting}
          streamingText={streamingText}
          busy={chatBusy}
          onExecute={onRunGlobalChat}
        />
      </section>
    </div>
  );
};
