import React, { useCallback, useEffect, useState } from 'react';
import {
  Calendar,
  FileText,
  Layers,
  Users,
  Sparkles,
  Sliders,
  Settings,
  Sun,
  Moon,
  Mic,
  Square,
  PanelLeftClose,
  PanelLeftOpen,
} from 'lucide-react';
import type {
  AiProviderSettings,
  BootstrapPayload,
  CalendarFeedConfig,
  CaptureStatus,
  ChatMessage,
  ChatScope,
  DictationEntry,
  Language,
  McpAuditEntry,
  McpGovernancePolicy,
  Meeting,
  MeetingDetail,
  MeetingFolder,
  MeetingParticipant,
  MeetingTemplate,
  MemorySearchHit,
  PermissionStatus,
  Person,
  PrivacyRule,
  ProviderTestResult,
  Recipe,
  RecordingAudioLevel,
  ScreenShareFrame,
  ScrubberPreview,
  TranscriptSegment,
  UpdateCheckResult,
  UpdateInstallProgressPayload,
  UserProfile,
  VocabularyEntry,
  VoiceTypingStatePayload,
  LearnedPreference,
  WhisperModelInfo,
} from './types';
import {
  callRpc,
  isTauriRuntime,
  setNativeAutostart,
  setNativeGlobalShortcut,
  setNativeVoiceTypingShortcut,
  subscribeBacktextEvents,
  subscribeOpenMeeting,
} from './lib/ipc';
import { t } from './lib/i18n';
import { TodayView } from './views/TodayView';
import { MeetingWorkspace } from './views/MeetingWorkspace';
import { ContextTimelineView } from './views/ContextTimelineView';
import { PeopleAndProjectsView } from './views/PeopleAndProjectsView';
import { RecipesStudioView } from './views/RecipesStudioView';
import { PersonalizationView } from './views/PersonalizationView';
import { SettingsView } from './views/SettingsView';
import { HoverModal } from './components/HoverModal';
import { DictationHud } from './components/DictationHud';

type NavKey =
  | 'today'
  | 'meetings'
  | 'context'
  | 'people'
  | 'recipes'
  | 'personalization'
  | 'settings';

export const App: React.FC = () => {
  const winParam =
    typeof window !== 'undefined'
      ? new URLSearchParams(window.location.search).get('window')
      : null;

  if (winParam === 'dictation') {
    return <DictationHud />;
  }

  return <MainApp isHoverStandalone={winParam === 'hover'} />;
};

const MainApp: React.FC<{ isHoverStandalone: boolean }> = ({ isHoverStandalone }) => {
  const [data, setData] = useState<BootstrapPayload | null>(null);
  const [activeNav, setActiveNav] = useState<NavKey>('today');
  const [recipesInitialTab, setRecipesInitialTab] = useState<'recipes' | 'templates'>('recipes');
  const [navCollapsed, setNavCollapsed] = useState<boolean>(() =>
    typeof window !== 'undefined' ? window.innerWidth <= 1080 : false,
  );
  const [selectedMeeting, setSelectedMeeting] = useState<MeetingDetail | null>(null);
  const [recordingLevel, setRecordingLevel] = useState<RecordingAudioLevel | null>(null);
  const [globalMessages, setGlobalMessages] = useState<ChatMessage[]>([]);
  const [chatScope, setChatScope] = useState<{ scope: ChatScope; label: string } | null>(null);
  const [voiceTypingState, setVoiceTypingState] = useState<VoiceTypingStatePayload | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [updateProgress, setUpdateProgress] = useState<UpdateInstallProgressPayload | null>(null);
  const [streamingByThread, setStreamingByThread] = useState<Record<string, string>>({});
  const [hoverOpen, setHoverOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [theme, setTheme] = useState<'light' | 'dark'>(() =>
    (localStorage.getItem('backtext-theme') as 'light' | 'dark') || 'light',
  );

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('backtext-theme', theme);
  }, [theme]);

  useEffect(() => {
    const isMacDesktop =
      isTauriRuntime() &&
      typeof navigator !== 'undefined' &&
      /mac/i.test(navigator.platform || navigator.userAgent);
    if (isMacDesktop && !isHoverStandalone) {
      document.documentElement.setAttribute('data-titlebar', 'overlay');
    } else {
      document.documentElement.removeAttribute('data-titlebar');
    }
  }, [isHoverStandalone]);

  // Auto-adapt main navigation rail on half-screen / narrow desktop windows
  useEffect(() => {
    let lastCompact = window.innerWidth <= 1080;
    const onResize = () => {
      const compact = window.innerWidth <= 1080;
      if (compact !== lastCompact) {
        lastCompact = compact;
        setNavCollapsed(compact);
      }
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  // Silent non-blocking check for GitHub Releases shortly after startup
  useEffect(() => {
    if (isHoverStandalone) return;
    const timer = window.setTimeout(() => {
      callRpc<UpdateCheckResult>('check_for_updates')
        .then((res) => setUpdateInfo(res))
        .catch(() => {});
    }, 5500);
    return () => window.clearTimeout(timer);
  }, [isHoverStandalone]);

  const refreshPermissions = useCallback(async () => {
    try {
      const perms = await callRpc<PermissionStatus>('check_permissions');
      setData((prev) => (prev ? { ...prev, permissions: perms } : prev));
    } catch {
      // ignore transient errors
    }
  }, []);

  const refreshBootstrap = useCallback(
    async (preferredMeetingId?: string) => {
      try {
        const boot = await callRpc<BootstrapPayload>('get_bootstrap');
        setData(boot);
        const globalMsgs = await callRpc<ChatMessage[]>('get_chat_messages', {
          threadId: 'global:workspace',
        });
        setGlobalMessages(globalMsgs);

        const targetId =
          preferredMeetingId ??
          selectedMeeting?.meeting.id ??
          boot.meetings.find((m) => m.status === 'live')?.id ??
          boot.meetings.find((m) => m.status === 'completed')?.id ??
          boot.meetings[0]?.id;
        if (targetId) {
          const detail = await callRpc<MeetingDetail | null>('get_meeting_detail', {
            meetingId: targetId,
          });
          setSelectedMeeting(detail);
        } else {
          setSelectedMeeting(null);
        }
        setErrorMsg(null);
      } catch (err) {
        setErrorMsg(err instanceof Error ? err.message : String(err));
      }
    },
    [selectedMeeting?.meeting.id],
  );

  useEffect(() => {
    refreshBootstrap();
  }, []);

  // Auto-recheck macOS permissions whenever the user returns from System Settings
  useEffect(() => {
    const onFocus = () => {
      refreshPermissions();
    };
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [refreshPermissions]);

  // Subscribe to live backend streaming events (Tauri EventBus / HTTP SSE)
  useEffect(() => {
    const unsub = subscribeBacktextEvents((ev) => {
      if (ev.event === 'chat_stream_delta') {
        const payload = ev.payload as {
          thread_id: string;
          delta: string;
          done: boolean;
        };
        setStreamingByThread((prev) => {
          if (payload.done) {
            const copy = { ...prev };
            delete copy[payload.thread_id];
            return copy;
          }
          return {
            ...prev,
            [payload.thread_id]: (prev[payload.thread_id] ?? '') + payload.delta,
          };
        });
      } else if (ev.event === 'recording_audio_level') {
        setRecordingLevel(ev.payload as RecordingAudioLevel);
      } else if (ev.event === 'transcript_segment_added') {
        const seg = ev.payload as TranscriptSegment;
        setSelectedMeeting((prev) => {
          if (!prev || prev.meeting.id !== seg.meeting_id) return prev;
          if (prev.transcript.some((t) => t.id === seg.id)) return prev;
          return {
            ...prev,
            meeting: {
              ...prev.meeting,
              transcript_count: prev.transcript.length + 1,
            },
            transcript: [...prev.transcript, seg],
          };
        });
        setData((prev) =>
          prev
            ? {
                ...prev,
                meetings: prev.meetings.map((m) =>
                  m.id === seg.meeting_id
                    ? { ...m, transcript_count: m.transcript_count + 1 }
                    : m,
                ),
              }
            : prev,
        );
      } else if (ev.event === 'screenshare_captured') {
        const frame = ev.payload as ScreenShareFrame;
        setSelectedMeeting((prev) => {
          if (!prev || prev.meeting.id !== frame.meeting_id) return prev;
          if (prev.screenshares.some((f) => f.id === frame.id)) return prev;
          return {
            ...prev,
            meeting: {
              ...prev.meeting,
              screenshare_count: prev.screenshares.length + 1,
            },
            screenshares: [...prev.screenshares, frame],
          };
        });
      } else if (ev.event === 'participant_updated') {
        const p = ev.payload as { meeting_id: string; participant: MeetingParticipant };
        setSelectedMeeting((prev) => {
          if (!prev || prev.meeting.id !== p.meeting_id) return prev;
          const idx = prev.meeting.participants.findIndex(
            (x) => x.speaker_label === p.participant.speaker_label,
          );
          const nextParts = [...prev.meeting.participants];
          if (idx >= 0) nextParts[idx] = p.participant;
          else nextParts.push(p.participant);
          return {
            ...prev,
            meeting: { ...prev.meeting, participants: nextParts },
          };
        });
      } else if (ev.event === 'meeting_notes_updated') {
        const p = ev.payload as { meeting_id: string; enhanced_markdown: string };
        setSelectedMeeting((prev) =>
          prev && prev.meeting.id === p.meeting_id
            ? {
                ...prev,
                meeting: { ...prev.meeting, enhanced_markdown: p.enhanced_markdown },
              }
            : prev,
        );
        setData((prev) =>
          prev
            ? {
                ...prev,
                meetings: prev.meetings.map((m) =>
                  m.id === p.meeting_id
                    ? { ...m, enhanced_markdown: p.enhanced_markdown }
                    : m,
                ),
              }
            : prev,
        );
      } else if (ev.event === 'voice_typing_state_changed') {
        const p = ev.payload as VoiceTypingStatePayload;
        setVoiceTypingState(p);
        if (!p.active) {
          window.setTimeout(() => {
            setVoiceTypingState((cur) => (cur && !cur.active ? null : cur));
          }, 3400);
        }
      } else if (ev.event === 'daemon_telemetry_updated') {
        setData((prev) =>
          prev
            ? {
                ...prev,
                telemetry: ev.payload as BootstrapPayload['telemetry'],
              }
            : prev,
        );
      } else if (ev.event === 'capture_status_changed') {
        const nextStatus = ev.payload as CaptureStatus;
        if (nextStatus.mode !== 'recording_meeting') {
          setRecordingLevel(null);
        }
        setData((prev) => (prev ? { ...prev, capture_status: nextStatus } : prev));
      } else if (ev.event === 'update_install_progress') {
        setUpdateProgress(ev.payload as UpdateInstallProgressPayload);
      } else if (ev.event === 'model_download_progress') {
        const p = ev.payload as {
          model_id: string;
          progress_pct: number;
          installed: boolean;
        };
        setData((prev) =>
          prev
            ? {
                ...prev,
                whisper_models: prev.whisper_models.map((m) =>
                  m.id === p.model_id
                    ? {
                        ...m,
                        progress_pct: p.progress_pct,
                        installed: p.installed,
                        downloading: !p.installed && p.progress_pct < 100,
                      }
                    : m,
                ),
              }
            : prev,
        );
      } else if (ev.event === 'meeting_detected') {
        const payload = ev.payload as { meeting_id?: string };
        refreshBootstrap(payload.meeting_id);
      } else if (ev.event === 'meeting_summarized') {
        const payload = ev.payload as {
          meeting_id?: string;
          executive_summary?: string;
          updated_title?: string | null;
        };
        if (payload.meeting_id && payload.updated_title) {
          const mid = payload.meeting_id;
          const nextTitle = payload.updated_title;
          setSelectedMeeting((prev) =>
            prev && prev.meeting.id === mid
              ? { ...prev, meeting: { ...prev.meeting, title: nextTitle } }
              : prev,
          );
          setData((prev) =>
            prev
              ? {
                  ...prev,
                  meetings: prev.meetings.map((m) =>
                    m.id === mid ? { ...m, title: nextTitle } : m,
                  ),
                }
              : prev,
          );
        }
        refreshBootstrap(payload.meeting_id);
      } else if (
        ev.event === 'calendar_synced' ||
        ev.event === 'context_episode_updated' ||
        ev.event === 'context_snapshot_captured'
      ) {
        refreshBootstrap();
      }
    });
    return unsub;
  }, [refreshBootstrap]);

  // Clicking a native macOS notification brings backtext to the front and opens that exact meeting
  useEffect(() => {
    if (isHoverStandalone) return;
    return subscribeOpenMeeting(async (meetingId) => {
      try {
        const detail = await callRpc<MeetingDetail | null>('get_meeting_detail', {
          meetingId,
        });
        if (detail) {
          setSelectedMeeting(detail);
          setActiveNav('meetings');
        }
      } catch {
        setActiveNav('meetings');
      }
    });
  }, [isHoverStandalone]);

  // In-window keyboard shortcut fallback (Alt+Space / Escape)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.altKey && e.code === 'Space') {
        e.preventDefault();
        setHoverOpen((prev) => !prev);
      } else if (e.key === 'Escape') {
        setHoverOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  if (!data) {
    return (
      <div style={{ padding: 48, fontFamily: 'var(--font-sans)' }}>
        <strong>backtext</strong> wird initialisiert…
        {errorMsg && (
          <div style={{ marginTop: 12, color: 'var(--accent-terracotta)' }}>{errorMsg}</div>
        )}
      </div>
    );
  }

  const lang: Language = data.profile.ui_language ?? 'de';
  const status: CaptureStatus = data.capture_status;
  const globalStreamText = streamingByThread['global:workspace'] ?? null;
  const meetingStreamText = selectedMeeting
    ? (streamingByThread[`meeting:${selectedMeeting.meeting.id}`] ?? null)
    : null;
  const defaultTemplateId = data.ai_settings.default_meeting_template_id || 'general';
  const meetingContentWidth = data.profile.meeting_content_width ?? 'standard';

  const handlePause = async (minutes: number | null) => {
    const next = await callRpc<CaptureStatus>('set_capture_pause', { minutes });
    setData((prev) => (prev ? { ...prev, capture_status: next } : prev));
  };

  const handleStartAdhocMeeting = async () => {
    setBusy(true);
    try {
      const nowLabel = new Date().toLocaleTimeString(lang === 'de' ? 'de-DE' : 'en-US', {
        hour: '2-digit',
        minute: '2-digit',
      });
      const detail = await callRpc<MeetingDetail>('start_live_meeting', {
        title: lang === 'de' ? `Neues Meeting (${nowLabel})` : `New Meeting (${nowLabel})`,
        detectedApp: '',
      });
      setSelectedMeeting(detail);
      setActiveNav('meetings');
      setNavCollapsed(true);
      await refreshBootstrap(detail.meeting.id);
    } finally {
      setBusy(false);
    }
  };

  const handleStartScheduledMeeting = async (meetingId: string) => {
    setBusy(true);
    try {
      const detail = await callRpc<MeetingDetail>('start_live_meeting', {
        existingMeetingId: meetingId,
      });
      setSelectedMeeting(detail);
      setActiveNav('meetings');
      setNavCollapsed(true);
      await refreshBootstrap(detail.meeting.id);
    } finally {
      setBusy(false);
    }
  };

  const handleSetDefaultTemplate = async (templateId: string) => {
    const ai_settings = await callRpc<AiProviderSettings>('save_ai_settings', {
      settings: { ...data.ai_settings, default_meeting_template_id: templateId },
    });
    setData((prev) => (prev ? { ...prev, ai_settings } : prev));
  };

  const handleResetTemplate = async (id: string) => {
    const templates = await callRpc<MeetingTemplate[]>('reset_meeting_template', { id });
    setData((prev) => (prev ? { ...prev, templates } : prev));
  };

  const handleRunGlobalChat = async (input: string, recipeSlug?: string) => {
    setBusy(true);
    try {
      await callRpc<ChatMessage>('run_chat_or_recipe', {
        input,
        recipeSlug,
        scope: chatScope?.scope,
      });
      const updated = await callRpc<ChatMessage[]>('get_chat_messages', {
        threadId: 'global:workspace',
      });
      setGlobalMessages(updated);
      await refreshBootstrap();
    } finally {
      setBusy(false);
    }
  };

  const handleToggleSystemVoiceTyping = async () => {
    const res = await callRpc<{
      active: boolean;
      entry?: DictationEntry;
      dictations?: DictationEntry[];
    }>('toggle_voice_typing', { mode: 'system' });
    if (res.dictations) {
      setData((prev) => (prev ? { ...prev, dictations: res.dictations! } : prev));
    }
    setVoiceTypingState((prev) =>
      res.active
        ? {
            active: true,
            mode: 'system',
            status: lang === 'de' ? 'Voice Typing hört zu…' : 'Voice Typing listening…',
            text: null,
            target_app: null,
          }
        : res.entry
          ? {
              active: false,
              mode: 'system',
              status: res.entry.pasted
                ? `In ${res.entry.target_app} eingefügt`
                : 'Diktat gespeichert',
              text: res.entry.cleaned_text,
              target_app: res.entry.target_app,
            }
          : prev,
    );
  };

  if (isHoverStandalone) {
    return (
      <HoverModal
        open
        standaloneWindow
        lang={lang}
        frontSnapshot={data.snapshots[0]}
        recipes={data.recipes}
        messages={globalMessages}
        streamingText={globalStreamText}
        busy={busy}
        onClose={() => {}}
        onExecute={handleRunGlobalChat}
      />
    );
  }

  return (
    <div className={`app-shell ${navCollapsed ? 'nav-collapsed' : ''}`}>
      {/* Left Navigation Rail (Smoothly Collapsible to 74px Icon-Rail, aligned with macOS traffic lights) */}
      <aside className={`sidebar ${navCollapsed ? 'collapsed' : ''}`}>
        {/* Integrated macOS Overlay Titlebar Drag Strip */}
        <div className="sidebar-titlebar-drag" data-tauri-drag-region />

        <div className="brand-row" data-tauri-drag-region>
          <div className="brand-mark" title="backtext">
            <span className="brand-glyph">bt</span>
            <span className="brand-text">backtext</span>
          </div>
          <div className="brand-actions">
            <button
              type="button"
              id="toggle-main-sidebar-btn"
              className="mini-btn"
              onClick={() => setNavCollapsed((v) => !v)}
              title={
                navCollapsed
                  ? lang === 'de'
                    ? 'Hauptmenü ausklappen'
                    : 'Expand sidebar'
                  : lang === 'de'
                    ? 'Hauptmenü einklappen'
                    : 'Collapse sidebar'
              }
            >
              {navCollapsed ? <PanelLeftOpen size={12} /> : <PanelLeftClose size={12} />}
            </button>
            <button
              type="button"
              id="theme-toggle-btn"
              className="mini-btn brand-theme-btn"
              onClick={() => setTheme((prev) => (prev === 'light' ? 'dark' : 'light'))}
              title="Theme wechseln"
            >
              {theme === 'light' ? <Moon size={12} /> : <Sun size={12} />}
            </button>
          </div>
        </div>

        {/* Compact Status Dot when Sidebar is Collapsed to Icon-Rail */}
        <button
          type="button"
          className="sidebar-collapsed-status"
          onClick={() => setNavCollapsed(false)}
          title={
            status.mode === 'recording_meeting'
              ? `${t(lang, 'statusRecording')} — Klicken zum Aufklappen`
              : status.mode === 'paused'
                ? `${t(lang, 'statusPaused')} — Klicken zum Aufklappen`
                : `${t(lang, 'statusActive')} (MCP :${data.mcp_policy.http_port})`
          }
        >
          <span
            className={`status-dot ${
              status.mode === 'recording_meeting'
                ? 'recording'
                : status.mode === 'paused'
                  ? 'paused'
                  : 'active'
            }`}
          />
        </button>

        {/* Littlebird-inspired Live Context & Pause Control Pill (Fixed-width clipper prevents button squish during expand!) */}
        <div className="status-box-clip">
          <div className="status-box">
            <div className="status-indicator-row">
              <div className="status-dot-label">
                <span
                  className={`status-dot ${
                    status.mode === 'recording_meeting'
                      ? 'recording'
                      : status.mode === 'paused'
                        ? 'paused'
                        : 'active'
                  }`}
                />
                <span>
                  {status.mode === 'recording_meeting'
                    ? t(lang, 'statusRecording')
                    : status.mode === 'paused'
                      ? t(lang, 'statusPaused')
                      : t(lang, 'statusActive')}
                </span>
              </div>
              <span className="mono-ts">MCP :{data.mcp_policy.http_port}</span>
            </div>

            <div className="pause-pills">
              {status.mode === 'paused' ? (
                <button
                  type="button"
                  id="resume-capture-btn"
                  className="mini-btn active-choice"
                  onClick={() => handlePause(null)}
                >
                  {t(lang, 'resume')}
                </button>
              ) : (
                <>
                  <button type="button" className="mini-btn" onClick={() => handlePause(15)}>
                    Pause 15m
                  </button>
                  <button type="button" className="mini-btn" onClick={() => handlePause(60)}>
                    Pause 1h
                  </button>
                  <button type="button" className="mini-btn" onClick={() => handlePause(0)}>
                    Pause ∞
                  </button>
                </>
              )}
            </div>
          </div>
        </div>

        <div className="nav-section-label">{t(lang, 'navWorkspace')}</div>
        <nav className="nav-list">
          <button
            type="button"
            id="nav-today"
            title={t(lang, 'today')}
            className={`nav-item ${activeNav === 'today' ? 'selected' : ''}`}
            onClick={() => setActiveNav('today')}
          >
            <span className="nav-item-left">
              <Calendar size={15} />
              <span className="nav-label-text">{t(lang, 'today')}</span>
            </span>
          </button>

          <button
            type="button"
            id="nav-meetings"
            title={t(lang, 'meetings')}
            className={`nav-item ${activeNav === 'meetings' ? 'selected' : ''}`}
            onClick={() => setActiveNav('meetings')}
          >
            <span className="nav-item-left">
              <FileText size={15} />
              <span className="nav-label-text">{t(lang, 'meetings')}</span>
            </span>
            <span className="nav-badge">{data.meetings.length}</span>
          </button>

          <button
            type="button"
            id="nav-context"
            title={t(lang, 'context')}
            className={`nav-item ${activeNav === 'context' ? 'selected' : ''}`}
            onClick={() => setActiveNav('context')}
          >
            <span className="nav-item-left">
              <Layers size={15} />
              <span className="nav-label-text">{t(lang, 'context')}</span>
            </span>
            <span className="nav-badge">{data.episodes.length}</span>
          </button>

          <button
            type="button"
            id="nav-people"
            title={t(lang, 'people')}
            className={`nav-item ${activeNav === 'people' ? 'selected' : ''}`}
            onClick={() => setActiveNav('people')}
          >
            <span className="nav-item-left">
              <Users size={15} />
              <span className="nav-label-text">{t(lang, 'people')}</span>
            </span>
            <span className="nav-badge">{data.persons.length}</span>
          </button>
        </nav>

        <div className="nav-section-label">{t(lang, 'navSystem')}</div>
        <nav className="nav-list">
          <button
            type="button"
            id="nav-recipes"
            title={t(lang, 'recipes')}
            className={`nav-item ${activeNav === 'recipes' ? 'selected' : ''}`}
            onClick={() => {
              setRecipesInitialTab('recipes');
              setActiveNav('recipes');
            }}
          >
            <span className="nav-item-left">
              <Sparkles size={15} />
              <span className="nav-label-text">{t(lang, 'recipes')}</span>
            </span>
            <span className="nav-badge">{data.recipes.length + (data.templates?.length ?? 0)}</span>
          </button>

          <button
            type="button"
            id="nav-personalization"
            title={t(lang, 'personalization')}
            className={`nav-item ${activeNav === 'personalization' ? 'selected' : ''}`}
            onClick={() => setActiveNav('personalization')}
          >
            <span className="nav-item-left">
              <Sliders size={15} />
              <span className="nav-label-text">{t(lang, 'personalization')}</span>
            </span>
            <span className="nav-badge">{lang.toUpperCase()}</span>
          </button>
        </nav>

        <div className="sidebar-footer">
          <button
            type="button"
            id="nav-settings"
            title={
              updateInfo?.update_available
                ? `${t(lang, 'settings')} · Update v${updateInfo.latest_version} verfügbar`
                : t(lang, 'settings')
            }
            className={`nav-item sidebar-settings-btn ${activeNav === 'settings' ? 'selected' : ''}`}
            onClick={() => setActiveNav('settings')}
          >
            <span className="nav-item-left">
              <Settings size={15} />
              <span className="nav-label-text">{t(lang, 'settings')}</span>
            </span>
            {updateInfo?.update_available && (
              <span className="nav-badge" style={{ background: 'var(--accent-sage)', color: '#fff' }}>
                v{updateInfo.latest_version}
              </span>
            )}
          </button>
        </div>
      </aside>

      {/* Main View Canvas */}
      <main className={activeNav === 'meetings' ? 'workspace-main workspace-main-flush' : 'workspace-main'}>
        {activeNav !== 'meetings' && <div className="workspace-titlebar-drag" data-tauri-drag-region />}
        {activeNav === 'today' && (
          <TodayView
            lang={lang}
            permissions={data.permissions}
            telemetry={data.telemetry}
            meetings={data.meetings}
            episodes={data.episodes}
            recipes={data.recipes}
            globalMessages={globalMessages}
            chatScope={chatScope}
            onClearChatScope={() => setChatScope(null)}
            streamingText={globalStreamText}
            chatBusy={busy}
            onStartAdhocMeeting={handleStartAdhocMeeting}
            onStartScheduledMeeting={handleStartScheduledMeeting}
            onOpenMeeting={async (id) => {
              const detail = await callRpc<MeetingDetail | null>('get_meeting_detail', {
                meetingId: id,
              });
              setSelectedMeeting(detail);
              setActiveNav('meetings');
            }}
            onSearchMemory={(query) =>
              callRpc<MemorySearchHit[]>('search_memory', { query })
            }
            onRunGlobalChat={handleRunGlobalChat}
            onRequestPermission={async (kind) => {
              const updated = await callRpc<PermissionStatus>('request_permission', { kind });
              setData((prev) => (prev ? { ...prev, permissions: updated } : prev));
            }}
            onRefreshPermissions={refreshPermissions}
            onClearDemoData={async () => {
              await callRpc('clear_demo_data');
              await refreshBootstrap();
            }}
            onSyncCalendars={async () => {
              await callRpc('sync_calendars');
              await refreshBootstrap();
            }}
          />
        )}

        {activeNav === 'meetings' && (
          <MeetingWorkspace
            lang={lang}
            platform={data.permissions.platform}
            contentWidth={meetingContentWidth}
            meetings={data.meetings}
            templates={data.templates ?? []}
            defaultTemplateId={defaultTemplateId}
            folders={data.folders ?? []}
            persons={data.persons}
            selectedDetail={selectedMeeting}
            recordingLevel={recordingLevel}
            recipes={data.recipes}
            streamingText={meetingStreamText}
            busy={busy}
            navCollapsed={navCollapsed}
            onSetNavCollapsed={setNavCollapsed}
            onSelectMeeting={async (id) => {
              const detail = await callRpc<MeetingDetail | null>('get_meeting_detail', {
                meetingId: id,
              });
              setSelectedMeeting(detail);
            }}
            onStartAdhocMeeting={handleStartAdhocMeeting}
            onStartScheduledMeeting={handleStartScheduledMeeting}
            onDeleteMeeting={async (meetingId) => {
              const res = await callRpc<{ deleted: boolean; meetings: Meeting[] }>(
                'delete_meeting',
                { meetingId },
              );
              setData((prev) => (prev ? { ...prev, meetings: res.meetings } : prev));
              if (selectedMeeting?.meeting.id === meetingId) {
                const nextId = res.meetings[0]?.id;
                if (nextId) {
                  const detail = await callRpc<MeetingDetail | null>('get_meeting_detail', {
                    meetingId: nextId,
                  });
                  setSelectedMeeting(detail);
                } else {
                  setSelectedMeeting(null);
                }
              }
            }}
            onRenameMeeting={async (meetingId, title) => {
              const detail = await callRpc<MeetingDetail | null>('update_meeting_title', {
                meetingId,
                title,
              });
              if (detail) {
                setSelectedMeeting(detail);
                setData((prev) =>
                  prev
                    ? {
                        ...prev,
                        meetings: prev.meetings.map((m) =>
                          m.id === meetingId ? { ...m, title: detail.meeting.title } : m,
                        ),
                      }
                    : prev,
                );
              }
            }}
            onSimulateLiveStep={async (meetingId) => {
              setBusy(true);
              try {
                const detail = await callRpc<MeetingDetail>('simulate_live_meeting_activity', {
                  meetingId,
                });
                setSelectedMeeting(detail);
                await refreshBootstrap(meetingId);
              } finally {
                setBusy(false);
              }
            }}
            onStopMeeting={async (meetingId) => {
              setBusy(true);
              try {
                const detail = await callRpc<MeetingDetail>('stop_and_summarize_meeting', {
                  meetingId,
                });
                setSelectedMeeting(detail);
                setRecordingLevel(null);
                await refreshBootstrap(meetingId);
              } finally {
                setBusy(false);
              }
            }}
            onSaveUserNotes={async (meetingId, rawNotes) => {
              await callRpc('update_meeting_notes', { meetingId, rawNotes });
              setSelectedMeeting((prev) =>
                prev && prev.meeting.id === meetingId
                  ? { ...prev, meeting: { ...prev.meeting, raw_user_notes: rawNotes } }
                  : prev,
              );
            }}
            onSaveEnhancedNotes={async (meetingId, enhancedMarkdown) => {
              const detail = await callRpc<MeetingDetail | null>('update_meeting_enhanced_notes', {
                meetingId,
                enhancedMarkdown,
              });
              if (detail) {
                setSelectedMeeting(detail);
              }
              setData((prev) =>
                prev
                  ? {
                      ...prev,
                      meetings: prev.meetings.map((m) =>
                        m.id === meetingId ? { ...m, enhanced_markdown: enhancedMarkdown } : m,
                      ),
                    }
                  : prev,
              );
            }}
            onEnhanceMeeting={async (meetingId) => {
              setBusy(true);
              try {
                await callRpc('enhance_meeting', { meetingId });
                await refreshBootstrap(meetingId);
              } finally {
                setBusy(false);
              }
            }}
            onSetMeetingTemplate={async (meetingId, templateId, regenerate) => {
              setBusy(true);
              try {
                const res = await callRpc<{ detail: MeetingDetail | null; meetings: Meeting[] }>(
                  'set_meeting_template',
                  { meetingId, templateId, regenerate },
                );
                if (res.detail) setSelectedMeeting(res.detail);
                setData((prev) => (prev ? { ...prev, meetings: res.meetings } : prev));
              } finally {
                setBusy(false);
              }
            }}
            onSaveTemplate={async (template) => {
              const templates = await callRpc<MeetingTemplate[]>('save_meeting_template', {
                template,
              });
              setData((prev) => (prev ? { ...prev, templates } : prev));
            }}
            onDeleteTemplate={async (id) => {
              const templates = await callRpc<MeetingTemplate[]>('delete_meeting_template', { id });
              setData((prev) => (prev ? { ...prev, templates } : prev));
            }}
            onResetTemplate={handleResetTemplate}
            onSetDefaultTemplate={handleSetDefaultTemplate}
            onOpenTemplateManager={() => {
              setRecipesInitialTab('templates');
              setActiveNav('recipes');
            }}
            onSaveFolder={async (folder) => {
              const folders = await callRpc<MeetingFolder[]>('save_meeting_folder', { folder });
              setData((prev) => (prev ? { ...prev, folders } : prev));
            }}
            onDeleteFolder={async (id) => {
              const res = await callRpc<{ folders: MeetingFolder[]; meetings: Meeting[] }>(
                'delete_meeting_folder',
                { id },
              );
              setData((prev) =>
                prev ? { ...prev, folders: res.folders, meetings: res.meetings } : prev,
              );
            }}
            onSetMeetingFolders={async (meetingId, folderIds) => {
              const res = await callRpc<{
                detail: MeetingDetail | null;
                folders: MeetingFolder[];
                meetings: Meeting[];
              }>('set_meeting_folders', { meetingId, folderIds });
              if (res.detail) setSelectedMeeting(res.detail);
              setData((prev) =>
                prev ? { ...prev, folders: res.folders, meetings: res.meetings } : prev,
              );
            }}
            onAddMeetingsToFolder={async (meetingIds, folderId) => {
              const res = await callRpc<{ folders: MeetingFolder[]; meetings: Meeting[] }>(
                'add_meetings_to_folder',
                { meetingIds, folderId },
              );
              setData((prev) =>
                prev ? { ...prev, folders: res.folders, meetings: res.meetings } : prev,
              );
            }}
            onAddSeriesAutoFolder={async (folderId, seriesKey) => {
              const res = await callRpc<{ folders: MeetingFolder[]; meetings: Meeting[] }>(
                'add_series_auto_folder',
                { folderId, seriesKey },
              );
              setData((prev) =>
                prev ? { ...prev, folders: res.folders, meetings: res.meetings } : prev,
              );
              if (selectedMeeting) {
                const detail = await callRpc<MeetingDetail | null>('get_meeting_detail', {
                  meetingId: selectedMeeting.meeting.id,
                });
                if (detail) setSelectedMeeting(detail);
              }
            }}
            onStartScopedChat={(scope, label) => {
              setChatScope({ scope, label });
              setActiveNav('today');
            }}
            onToggleActionItem={async (meetingId, actionId) => {
              const detail = await callRpc<MeetingDetail>('toggle_action_item', {
                meetingId,
                actionId,
              });
              setSelectedMeeting(detail);
            }}
            onAssignSpeaker={async (meetingId, speakerLabel, personName) => {
              const detail = await callRpc<MeetingDetail>('assign_speaker', {
                meetingId,
                speakerLabel,
                personName,
              });
              setSelectedMeeting(detail);
              await refreshBootstrap(meetingId);
            }}
            onAddParticipant={async (meetingId, displayName, email, personId) => {
              const res = await callRpc<{ detail: MeetingDetail | null; persons: Person[] }>(
                'add_meeting_participant',
                { meetingId, displayName, email, personId },
              );
              if (res.detail) {
                setSelectedMeeting(res.detail);
              }
              setData((prev) => (prev ? { ...prev, persons: res.persons } : prev));
            }}
            onRemoveParticipant={async (meetingId, speakerLabel) => {
              const detail = await callRpc<MeetingDetail | null>('remove_meeting_participant', {
                meetingId,
                speakerLabel,
              });
              if (detail) {
                setSelectedMeeting(detail);
              }
            }}
            onRunMeetingChat={async (meetingId, input, recipeSlug) => {
              setBusy(true);
              try {
                await callRpc<ChatMessage>('run_chat_or_recipe', {
                  meetingId,
                  input,
                  recipeSlug,
                });
                await refreshBootstrap(meetingId);
              } finally {
                setBusy(false);
              }
            }}
            onOpenSettings={() => setActiveNav('settings')}
          />
        )}

        {activeNav === 'context' && (
          <ContextTimelineView
            lang={lang}
            captureStatus={status}
            telemetry={data.telemetry}
            permissions={data.permissions}
            episodes={data.episodes}
            snapshots={data.snapshots}
            clipboard={data.clipboard}
            onRefresh={() => refreshBootstrap()}
            onOpenSettings={() => setActiveNav('settings')}
            onPurgeRecent={async (minutes) => {
              await callRpc('purge_recent_context', { minutes });
              await refreshBootstrap();
            }}
            onCaptureSample={async (appName, windowTitle, browserUrl, visibleText, clipboardText) => {
              const res = await callRpc('capture_ambient_sample', {
                appName,
                windowTitle,
                browserUrl,
                visibleText,
                clipboardText,
              });
              await refreshBootstrap();
              return res;
            }}
            onPreviewScrubber={(appName, windowTitle, browserUrl, rawText) =>
              callRpc<ScrubberPreview>('preview_privacy_filter', {
                appName,
                windowTitle,
                browserUrl,
                rawText,
              })
            }
          />
        )}

        {activeNav === 'people' && (
          <PeopleAndProjectsView
            lang={lang}
            persons={data.persons}
            projects={data.profile.active_projects}
            meetings={data.meetings}
            episodes={data.episodes}
            onUpsertPerson={async (input) => {
              const persons = await callRpc<Person[]>('upsert_person', {
                id: input.id,
                fullName: input.fullName,
                email: input.email,
                company: input.company,
                roleNotes: input.roleNotes,
              });
              setData((prev) => (prev ? { ...prev, persons } : prev));
            }}
            onDeletePerson={async (personId) => {
              const persons = await callRpc<Person[]>('delete_person', { personId });
              setData((prev) => (prev ? { ...prev, persons } : prev));
            }}
            onChatWithPerson={(person) => {
              setChatScope({
                scope: { person_id: person.id },
                label: `${lang === 'de' ? 'Person' : 'Person'}: ${person.full_name}`,
              });
              setActiveNav('today');
            }}
          />
        )}

        {activeNav === 'recipes' && (
          <RecipesStudioView
            lang={lang}
            initialTab={recipesInitialTab}
            recipes={data.recipes}
            templates={data.templates ?? []}
            defaultTemplateId={defaultTemplateId}
            onSaveRecipe={async (recipe: Recipe) => {
              const recipes = await callRpc<Recipe[]>('save_recipe', { recipe });
              setData((prev) => (prev ? { ...prev, recipes } : prev));
            }}
            onDeleteRecipe={async (id: string) => {
              const recipes = await callRpc<Recipe[]>('delete_recipe', { id });
              setData((prev) => (prev ? { ...prev, recipes } : prev));
            }}
            onRunRecipeInToday={async (slug: string) => {
              setActiveNav('today');
              await handleRunGlobalChat(slug, slug);
            }}
            onSaveTemplate={async (template) => {
              const templates = await callRpc<MeetingTemplate[]>('save_meeting_template', {
                template,
              });
              setData((prev) => (prev ? { ...prev, templates } : prev));
            }}
            onDeleteTemplate={async (id) => {
              const templates = await callRpc<MeetingTemplate[]>('delete_meeting_template', { id });
              setData((prev) => (prev ? { ...prev, templates } : prev));
            }}
            onResetTemplate={handleResetTemplate}
            onSetDefaultTemplate={handleSetDefaultTemplate}
          />
        )}

        {activeNav === 'personalization' && (
          <PersonalizationView
            lang={lang}
            profile={data.profile}
            vocabulary={data.vocabulary}
            learnedPreferences={data.learned_preferences}
            onSaveProfile={async (profile: UserProfile) => {
              const saved = await callRpc<UserProfile>('save_user_profile', { profile });
              setData((prev) => (prev ? { ...prev, profile: saved } : prev));
            }}
            onEnrollSelfVoiceprint={async () => {
              const saved = await callRpc<UserProfile>('enroll_self_voiceprint', { durationSecs: 5 });
              setData((prev) => (prev ? { ...prev, profile: saved } : prev));
            }}
            onAddVocabulary={async (term, phoneticHint, category, projectTag) => {
              const vocabulary = await callRpc<VocabularyEntry[]>('upsert_vocabulary', {
                term,
                phoneticHint,
                category,
                projectTag,
              });
              setData((prev) => (prev ? { ...prev, vocabulary } : prev));
            }}
            onDeleteVocabulary={async (id) => {
              const vocabulary = await callRpc<VocabularyEntry[]>('delete_vocabulary', { id });
              setData((prev) => (prev ? { ...prev, vocabulary } : prev));
            }}
            onToggleLearnedPref={async (id, isActive) => {
              const learned_preferences = await callRpc<LearnedPreference[]>(
                'toggle_learned_preference',
                { id, isActive },
              );
              setData((prev) => (prev ? { ...prev, learned_preferences } : prev));
            }}
          />
        )}

        {activeNav === 'settings' && (
          <SettingsView
            lang={lang}
            profile={data.profile}
            permissions={data.permissions}
            telemetry={data.telemetry}
            aiSettings={data.ai_settings}
            whisperModels={data.whisper_models}
            ollamaModels={data.ollama_models}
            calendarFeeds={data.calendar_feeds}
            dictations={data.dictations ?? []}
            voiceTypingActive={Boolean(voiceTypingState?.active)}
            updateInfo={updateInfo}
            updateProgress={updateProgress}
            mcpPolicy={data.mcp_policy}
            mcpSnippets={data.mcp_snippets}
            mcpAuditLogs={data.mcp_audit_logs}
            privacyRules={data.privacy_rules}
            onSaveProfile={async (profile: UserProfile) => {
              const saved = await callRpc<UserProfile>('save_user_profile', { profile });
              setData((prev) => (prev ? { ...prev, profile: saved } : prev));
            }}
            onSaveAiSettings={async (settings: AiProviderSettings) => {
              const ai_settings = await callRpc<AiProviderSettings>('save_ai_settings', {
                settings,
              });
              setData((prev) => (prev ? { ...prev, ai_settings } : prev));
            }}
            onSetApiKey={async (provider, apiKey) => {
              const ai_settings = await callRpc<AiProviderSettings>('set_api_key', {
                provider,
                apiKey,
              });
              setData((prev) => (prev ? { ...prev, ai_settings } : prev));
            }}
            onDeleteApiKey={async (provider) => {
              const ai_settings = await callRpc<AiProviderSettings>('delete_api_key', { provider });
              setData((prev) => (prev ? { ...prev, ai_settings } : prev));
            }}
            onTestProvider={(provider) =>
              callRpc<ProviderTestResult>('test_provider_connection', { provider })
            }
            onFetchProviderModels={(provider) =>
              callRpc<string[]>('list_provider_models', { provider })
            }
            onDownloadWhisperModel={async (modelId) => {
              const info = await callRpc<WhisperModelInfo>('download_whisper_model', { modelId });
              setData((prev) =>
                prev
                  ? {
                      ...prev,
                      whisper_models: prev.whisper_models.map((m) =>
                        m.id === info.id ? info : m,
                      ),
                    }
                  : prev,
              );
            }}
            onSetGlobalShortcut={async (shortcut) => {
              const display = await setNativeGlobalShortcut(shortcut);
              setData((prev) =>
                prev
                  ? {
                      ...prev,
                      ai_settings: { ...prev.ai_settings, hover_shortcut: shortcut },
                      telemetry: { ...prev.telemetry, global_shortcut: display },
                    }
                  : prev,
              );
              return display;
            }}
            onSetVoiceTypingShortcut={async (shortcut) => {
              const display = await setNativeVoiceTypingShortcut(shortcut);
              setData((prev) =>
                prev
                  ? {
                      ...prev,
                      ai_settings: { ...prev.ai_settings, voice_typing_shortcut: shortcut },
                      telemetry: { ...prev.telemetry, voice_typing_shortcut: display },
                    }
                  : prev,
              );
              return display;
            }}
            onToggleVoiceTyping={handleToggleSystemVoiceTyping}
            onAddCalendarFeed={async (name, icsUrl) => {
              const calendar_feeds = await callRpc<CalendarFeedConfig[]>('add_calendar_feed', {
                name,
                icsUrl,
              });
              setData((prev) => (prev ? { ...prev, calendar_feeds } : prev));
              await refreshBootstrap();
            }}
            onDeleteCalendarFeed={async (id) => {
              const calendar_feeds = await callRpc<CalendarFeedConfig[]>('delete_calendar_feed', {
                id,
              });
              setData((prev) => (prev ? { ...prev, calendar_feeds } : prev));
            }}
            onSyncCalendars={async () => {
              await callRpc('sync_calendars');
              await refreshBootstrap();
            }}
            onToggleAutostart={async (enabled) => {
              const active = await setNativeAutostart(enabled);
              setData((prev) =>
                prev ? { ...prev, telemetry: { ...prev.telemetry, autostart_enabled: active } } : prev,
              );
            }}
            onCheckUpdates={async () => {
              const res = await callRpc<UpdateCheckResult>('check_for_updates');
              setUpdateInfo(res);
              return res;
            }}
            onInstallUpdate={async (downloadUrl, assetName) => {
              setUpdateProgress({
                stage: 'downloading',
                progress_pct: 2,
                message: 'Starte Update-Download…',
                error: null,
              });
              await callRpc('install_update', { downloadUrl, assetName });
            }}
            onSetupPersistentSigning={async () => {
              const resp = await callRpc<{
                result: { ok: boolean; ready: boolean; message: string };
                telemetry: BootstrapPayload['telemetry'];
              }>('setup_persistent_signing');
              if (resp.telemetry) {
                setData((prev) => (prev ? { ...prev, telemetry: resp.telemetry } : prev));
              }
              return resp.result;
            }}
            onRequestPermission={async (kind) => {
              const updated = await callRpc<PermissionStatus>('request_permission', { kind });
              setData((prev) => (prev ? { ...prev, permissions: updated } : prev));
            }}
            onRefreshPermissions={refreshPermissions}
            onClearDemoData={async () => {
              await callRpc('clear_demo_data');
              await refreshBootstrap();
            }}
            onSaveMcpPolicy={async (policy: McpGovernancePolicy) => {
              const saved = await callRpc<McpGovernancePolicy>('save_mcp_policy', { policy });
              setData((prev) => (prev ? { ...prev, mcp_policy: saved } : prev));
            }}
            onInvokeMcpInspector={async (toolName, args) => {
              const res = await callRpc<{ result: unknown; audit_logs: McpAuditEntry[] }>(
                'invoke_mcp_inspector',
                { toolName, arguments: args, clientName: 'backtext-ui-inspector' },
              );
              setData((prev) =>
                prev ? { ...prev, mcp_audit_logs: res.audit_logs } : prev,
              );
              await refreshBootstrap();
              return res;
            }}
            onTogglePrivacyRule={async (id, isEnabled) => {
              const rules = await callRpc<PrivacyRule[]>('toggle_privacy_rule', { id, isEnabled });
              setData((prev) => (prev ? { ...prev, privacy_rules: rules } : prev));
            }}
            onAddPrivacyRule={async (kind, pattern, description) => {
              const rules = await callRpc<PrivacyRule[]>('upsert_privacy_rule', {
                kind,
                pattern,
                description,
              });
              setData((prev) => (prev ? { ...prev, privacy_rules: rules } : prev));
            }}
            onDeletePrivacyRule={async (id) => {
              const rules = await callRpc<PrivacyRule[]>('delete_privacy_rule', { id });
              setData((prev) => (prev ? { ...prev, privacy_rules: rules } : prev));
            }}
          />
        )}
      </main>

      {/* Floating Bottom Voice Typing Status Pill */}
      {voiceTypingState && (voiceTypingState.active || voiceTypingState.status) && (
        <div
          id="voice-typing-floating-pill"
          className={`voice-typing-pill ${voiceTypingState.active ? 'active' : ''}`}
        >
          <span className={`status-dot ${voiceTypingState.active ? 'recording' : 'active'}`} />
          <Mic size={13} />
          <span>{voiceTypingState.status}</span>
          {voiceTypingState.active && (
            <button
              type="button"
              className="mini-btn active-choice"
              style={{ display: 'inline-flex', alignItems: 'center', gap: 4, marginLeft: 4 }}
              onClick={handleToggleSystemVoiceTyping}
            >
              <Square size={10} />
              {lang === 'de' ? 'Stop' : 'Stop'}
            </button>
          )}
        </div>
      )}

      {/* In-window Hover Overlay Modal (for Web Preview & In-Window ⌥ Space) */}
      <HoverModal
        open={hoverOpen}
        lang={lang}
        frontSnapshot={data.snapshots[0]}
        recipes={data.recipes}
        messages={globalMessages}
        streamingText={globalStreamText}
        busy={busy}
        onClose={() => setHoverOpen(false)}
        onExecute={handleRunGlobalChat}
      />
    </div>
  );
};
