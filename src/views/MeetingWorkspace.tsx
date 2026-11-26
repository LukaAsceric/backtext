import React, { useEffect, useMemo, useRef, useState } from 'react';
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
  UserPlus,
  X,
  Search,
  Folder,
  FolderPlus,
  MessageSquare,
  Pencil,
  Eye,
  SplitSquareVertical,
  History,
  ArrowUpRight,
  Repeat,
  LayoutTemplate,
  ListChecks,
  Maximize2,
  Minimize2,
  RotateCcw,
} from 'lucide-react';
import type {
  ChatScope,
  Language,
  Meeting,
  MeetingDetail,
  MeetingFolder,
  MeetingTemplate,
  Person,
  Recipe,
  RecordingAudioLevel,
} from '../types';
import { formatDurationMs, sourceBadgeLabel, t } from '../lib/i18n';
import { RecipeChatBar } from '../components/RecipeChatBar';
import { FormattedMarkdown } from '../components/FormattedMarkdown';
import { RichNotepad } from '../components/RichNotepad';

export interface MeetingTemplateDraft {
  id?: string;
  name_de: string;
  name_en?: string;
  description_de: string;
  description_en?: string;
  icon_key?: string;
  sections_prompt: string;
}

interface MeetingWorkspaceProps {
  lang: Language;
  platform: string;
  contentWidth: 'standard' | 'wide' | 'full';
  meetings: Meeting[];
  templates: MeetingTemplate[];
  defaultTemplateId: string;
  folders: MeetingFolder[];
  persons: Person[];
  selectedDetail: MeetingDetail | null;
  recordingLevel: RecordingAudioLevel | null;
  recipes: Recipe[];
  streamingText?: string | null;
  busy: boolean;
  navCollapsed: boolean;
  onSetNavCollapsed: (collapsed: boolean) => void;
  onSelectMeeting: (id: string) => Promise<void>;
  onStartAdhocMeeting: () => Promise<void>;
  onStartScheduledMeeting: (meetingId: string) => Promise<void>;
  onDeleteMeeting: (meetingId: string) => Promise<void>;
  onRenameMeeting: (meetingId: string, title: string) => Promise<void>;
  onSimulateLiveStep: (meetingId: string) => Promise<void>;
  onStopMeeting: (meetingId: string) => Promise<void>;
  onSaveUserNotes: (meetingId: string, notes: string) => Promise<void>;
  onSaveEnhancedNotes: (meetingId: string, enhancedMarkdown: string) => Promise<void>;
  onEnhanceMeeting: (meetingId: string) => Promise<void>;
  onSetMeetingTemplate: (meetingId: string, templateId: string, regenerate: boolean) => Promise<void>;
  onSaveTemplate: (tpl: MeetingTemplateDraft) => Promise<void>;
  onDeleteTemplate: (id: string) => Promise<void>;
  onResetTemplate: (id: string) => Promise<void>;
  onSetDefaultTemplate: (id: string) => Promise<void>;
  onOpenTemplateManager: () => void;
  onSaveFolder: (folder: { id?: string; name: string; color: string; description?: string }) => Promise<void>;
  onDeleteFolder: (id: string) => Promise<void>;
  onSetMeetingFolders: (meetingId: string, folderIds: string[]) => Promise<void>;
  onAddMeetingsToFolder: (meetingIds: string[], folderId: string) => Promise<void>;
  onAddSeriesAutoFolder: (folderId: string, seriesKey: string) => Promise<void>;
  onStartScopedChat: (scope: ChatScope, label: string) => void;
  onToggleActionItem: (meetingId: string, actionId: string) => Promise<void>;
  onAssignSpeaker: (meetingId: string, speakerLabel: string, personName: string) => Promise<void>;
  onAddParticipant: (
    meetingId: string,
    displayName: string,
    email?: string,
    personId?: string,
  ) => Promise<void>;
  onRemoveParticipant: (meetingId: string, speakerLabel: string) => Promise<void>;
  onRunMeetingChat: (meetingId: string, input: string, recipeSlug?: string) => Promise<void>;
  onOpenSettings: () => void;
}

const FOLDER_COLORS = ['#2F6B53', '#B85838', '#B87C2E', '#4F6D8A', '#7A5488'];

function chronologicalBucket(
  m: Meeting,
  lang: Language,
): { key: string; label: string; order: number } {
  if (m.status === 'live') {
    return { key: 'live', label: lang === 'de' ? 'Live' : 'Live', order: 0 };
  }
  const dt = new Date(m.started_at);
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const startOfYesterday = startOfToday - 86_400_000;
  const startOfWeek = startOfToday - 6 * 86_400_000;
  const ts = dt.getTime();

  if (ts >= startOfToday) {
    return { key: 'today', label: lang === 'de' ? 'Heute' : 'Today', order: 1 };
  }
  if (ts >= startOfYesterday) {
    return { key: 'yesterday', label: lang === 'de' ? 'Gestern' : 'Yesterday', order: 2 };
  }
  if (ts >= startOfWeek) {
    return { key: 'week', label: lang === 'de' ? 'Diese Woche' : 'This Week', order: 3 };
  }
  return { key: 'older', label: lang === 'de' ? 'Älter' : 'Older', order: 4 };
}

export const MeetingWorkspace: React.FC<MeetingWorkspaceProps> = ({
  lang,
  platform,
  contentWidth,
  meetings,
  templates,
  defaultTemplateId,
  folders,
  persons,
  selectedDetail,
  recordingLevel,
  recipes,
  streamingText,
  busy,
  navCollapsed,
  onSetNavCollapsed,
  onSelectMeeting,
  onStartAdhocMeeting,
  onStartScheduledMeeting,
  onDeleteMeeting,
  onRenameMeeting,
  onSimulateLiveStep,
  onStopMeeting,
  onSaveUserNotes,
  onSaveEnhancedNotes,
  onEnhanceMeeting,
  onSetMeetingTemplate,
  onSaveTemplate,
  onDeleteTemplate,
  onResetTemplate,
  onSetDefaultTemplate,
  onOpenTemplateManager,
  onSaveFolder,
  onDeleteFolder,
  onSetMeetingFolders,
  onAddMeetingsToFolder,
  onAddSeriesAutoFolder,
  onStartScopedChat,
  onToggleActionItem,
  onAssignSpeaker,
  onAddParticipant,
  onRemoveParticipant,
  onRunMeetingChat,
  onOpenSettings,
}) => {
  const current = selectedDetail?.meeting ?? null;
  const isLive = current?.status === 'live';

  const [isNarrow, setIsNarrow] = useState<boolean>(() =>
    typeof window !== 'undefined' ? window.innerWidth <= 1060 : false,
  );
  const [sidebarOpen, setSidebarOpen] = useState<boolean>(() =>
    typeof window !== 'undefined' ? window.innerWidth > 1060 && !isLive : true,
  );
  const [activeTab, setActiveTab] = useState<'notes' | 'enhanced' | 'transcript' | 'slides'>('notes');
  const [draftNotes, setDraftNotes] = useState('');
  const [draftEnhanced, setDraftEnhanced] = useState('');
  const [editingEnhanced, setEditingEnhanced] = useState(false);
  const [showRawCompare, setShowRawCompare] = useState(false);
  const [draftTitle, setDraftTitle] = useState('');
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved'>('idle');
  const [enhancedSaveState, setEnhancedSaveState] = useState<'idle' | 'saving' | 'saved'>('idle');

  // Sidebar search, folder filter & multi-select state
  const [searchQuery, setSearchQuery] = useState('');
  const [activeFolderId, setActiveFolderId] = useState<string | null>(null);
  const [showNewFolderForm, setShowNewFolderForm] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');
  const [newFolderColor, setNewFolderColor] = useState(FOLDER_COLORS[0]);
  const [multiSelectMode, setMultiSelectMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [bulkFolderId, setBulkFolderId] = useState('');

  // Popovers & Modals
  const [showPeoplePopover, setShowPeoplePopover] = useState(false);
  const [showFolderPopover, setShowFolderPopover] = useState(false);
  const [showSeriesCard, setShowSeriesCard] = useState(true);
  const [templateModalOpen, setTemplateModalOpen] = useState(false);
  const [tplDraft, setTplDraft] = useState<MeetingTemplateDraft>({
    name_de: '',
    description_de: '',
    sections_prompt:
      '### Kurzüberblick\n### Kernpunkte\n### Entscheidungen & Action Items',
  });

  const [renamingLabel, setRenamingLabel] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [newPartName, setNewPartName] = useState('');
  const [newPartEmail, setNewPartEmail] = useState('');
  const [highlightedSegmentId, setHighlightedSegmentId] = useState<string | null>(null);

  const saveTimerRef = useRef<number | null>(null);
  const enhancedSaveTimerRef = useRef<number | null>(null);
  const highlightTimerRef = useRef<number | null>(null);
  const lastMeetingIdRef = useRef<string | null>(null);
  const autoFocusedLiveIdRef = useRef<string | null>(null);

  // Track narrow/half-screen viewports so the meeting list automatically becomes a slide-over drawer
  useEffect(() => {
    const onResize = () => {
      const narrow = window.innerWidth <= 1060;
      setIsNarrow((prev) => {
        if (prev !== narrow) {
          setSidebarOpen(!narrow && !isLive);
        }
        return narrow;
      });
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [isLive]);

  // Meeting Focus Mode: automatically collapse BOTH main navigation & meeting list when a live meeting starts
  useEffect(() => {
    if (isLive && current?.id && autoFocusedLiveIdRef.current !== current.id) {
      autoFocusedLiveIdRef.current = current.id;
      setSidebarOpen(false);
      onSetNavCollapsed(true);
    }
  }, [isLive, current?.id, onSetNavCollapsed]);

  const isFocusMode = !sidebarOpen && navCollapsed;

  const toggleFocusMode = () => {
    if (isFocusMode) {
      onSetNavCollapsed(false);
      setSidebarOpen(!isNarrow);
    } else {
      onSetNavCollapsed(true);
      setSidebarOpen(false);
    }
  };

  // Sync editor state and pick the most helpful default tab when switching meetings
  useEffect(() => {
    if (!current) return;
    if (lastMeetingIdRef.current !== current.id) {
      lastMeetingIdRef.current = current.id;
      setDraftNotes(current.raw_user_notes ?? '');
      setDraftEnhanced(current.enhanced_markdown ?? '');
      setEditingEnhanced(false);
      setDraftTitle(current.title ?? '');
      setShowPeoplePopover(false);
      setShowFolderPopover(false);
      setRenamingLabel(null);
      setHighlightedSegmentId(null);
      setSaveState('idle');
      setEnhancedSaveState('idle');
      if (current.status === 'live') {
        setActiveTab('notes');
      } else if (current.enhanced_markdown) {
        setActiveTab('enhanced');
      } else {
        setActiveTab('notes');
      }
    }
  }, [current?.id, current?.status, current?.enhanced_markdown]);

  // Keep enhanced draft synced when updated externally (e.g., via /rewrite or template regeneration)
  useEffect(() => {
    if (
      current &&
      current.enhanced_markdown !== draftEnhanced &&
      document.activeElement?.id !== 'enhanced-notes-editor'
    ) {
      setDraftEnhanced(current.enhanced_markdown ?? '');
    }
  }, [current?.enhanced_markdown]);

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

  const scheduleEnhancedAutoSave = (meetingId: string, nextMd: string) => {
    setEnhancedSaveState('saving');
    if (enhancedSaveTimerRef.current) {
      window.clearTimeout(enhancedSaveTimerRef.current);
    }
    enhancedSaveTimerRef.current = window.setTimeout(async () => {
      try {
        await onSaveEnhancedNotes(meetingId, nextMd);
        setEnhancedSaveState('saved');
      } catch {
        setEnhancedSaveState('idle');
      }
    }, 650);
  };

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
      if (enhancedSaveTimerRef.current) window.clearTimeout(enhancedSaveTimerRef.current);
      if (highlightTimerRef.current) window.clearTimeout(highlightTimerRef.current);
    };
  }, []);

  // Scroll & highlight the exact transcript segment when a Granola-style [MM:SS] citation is clicked
  const handleJumpToTranscript = (segmentId: string) => {
    setActiveTab('transcript');
    setHighlightedSegmentId(segmentId);
    if (highlightTimerRef.current) {
      window.clearTimeout(highlightTimerRef.current);
    }
    window.setTimeout(() => {
      const el = document.getElementById(`transcript-seg-${segmentId}`);
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    }, 60);
    highlightTimerRef.current = window.setTimeout(() => {
      setHighlightedSegmentId((prev) => (prev === segmentId ? null : prev));
    }, 3400);
  };

  // Filtered + chronologically grouped meetings for sidebar
  const groupedMeetings = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    const filtered = meetings.filter((m) => {
      if (activeFolderId && !(m.folder_ids ?? []).includes(activeFolderId)) {
        return false;
      }
      if (!q) return true;
      const inTitle = m.title.toLowerCase().includes(q);
      const inSummary = (m.executive_summary ?? '').toLowerCase().includes(q);
      const inNotes = (m.raw_user_notes ?? '').toLowerCase().includes(q);
      const inParticipants = m.participants.some((p) =>
        p.display_name.toLowerCase().includes(q),
      );
      return inTitle || inSummary || inNotes || inParticipants;
    });

    const buckets = new Map<string, { label: string; order: number; items: Meeting[] }>();
    for (const m of filtered) {
      const b = chronologicalBucket(m, lang);
      const existing = buckets.get(b.key);
      if (existing) {
        existing.items.push(m);
      } else {
        buckets.set(b.key, { label: b.label, order: b.order, items: [m] });
      }
    }
    return Array.from(buckets.values()).sort((a, b) => a.order - b.order);
  }, [meetings, searchQuery, activeFolderId, lang]);

  // Previous meeting in the same recurring series (Feature 2.9)
  const previousInSeries = useMemo(() => {
    if (!current?.series_key) return null;
    const currentTs = new Date(current.started_at).getTime();
    const sameSeries = meetings
      .filter(
        (m) =>
          m.id !== current.id &&
          m.series_key &&
          m.series_key === current.series_key &&
          new Date(m.started_at).getTime() < currentTs,
      )
      .sort((a, b) => new Date(b.started_at).getTime() - new Date(a.started_at).getTime());
    return sameSeries[0] ?? null;
  }, [meetings, current]);

  // Existing contacts not yet added to this meeting (for 1-click quick add)
  const suggestedContacts = useMemo(() => {
    if (!current) return [];
    const existingIds = new Set(
      current.participants.map((p) => p.person_id).filter(Boolean) as string[],
    );
    const existingNames = new Set(
      current.participants.map((p) => p.display_name.toLowerCase()),
    );
    return persons.filter(
      (per) =>
        !existingIds.has(per.id) && !existingNames.has(per.full_name.toLowerCase()),
    );
  }, [persons, current]);

  const activeFolder = useMemo(
    () => folders.find((f) => f.id === activeFolderId) ?? null,
    [folders, activeFolderId],
  );

  const currentFolderIds = current?.folder_ids ?? [];
  const currentFolders = useMemo(
    () => folders.filter((f) => currentFolderIds.includes(f.id)),
    [folders, currentFolderIds],
  );

  const activeRecLevel =
    isLive && recordingLevel && recordingLevel.meeting_id === current?.id ? recordingLevel : null;

  const elapsedDisplay = activeRecLevel
    ? formatDurationMs(activeRecLevel.elapsed_secs * 1000)
    : '00:00';

  const toggleSelectedId = (id: string) => {
    setSelectedIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  };

  return (
    <div className="granola-shell">
      <div
        className={`granola-layout ${!sidebarOpen || isNarrow ? 'sidebar-collapsed' : ''}`}
      >
        {sidebarOpen && isNarrow && (
          <div
            className="granola-drawer-backdrop"
            onClick={() => setSidebarOpen(false)}
          />
        )}
        {/* Collapsible Meeting List with Search, Folders, Grouping & Multi-Select */}
        {sidebarOpen && (
          <aside className={`granola-list-pane ${isNarrow ? 'overlay-drawer' : ''}`}>
            <div className="granola-list-header" data-tauri-drag-region>
              <span style={{ fontWeight: 600, fontSize: 13 }}>
                {lang === 'de' ? 'Meetings' : 'Meetings'}
              </span>
              <div style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
                {isNarrow && (
                  <button
                    type="button"
                    className="mini-btn"
                    title={lang === 'de' ? 'Schließen' : 'Close'}
                    onClick={() => setSidebarOpen(false)}
                  >
                    <X size={12} />
                  </button>
                )}
                <button
                  type="button"
                  id="toggle-multiselect-btn"
                  className={`mini-btn ${multiSelectMode ? 'active-choice' : ''}`}
                  title={
                    lang === 'de'
                      ? 'Mehrfachauswahl für Ordner & Multi-Meeting-Chat'
                      : 'Multi-select for folders & multi-meeting chat'
                  }
                  onClick={() => {
                    setMultiSelectMode((v) => !v);
                    setSelectedIds([]);
                  }}
                >
                  <ListChecks size={12} />
                </button>
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
            </div>

            {/* Live Meeting Search */}
            <div className="sidebar-search-box">
              <Search size={12} style={{ color: 'var(--text-tertiary)', flexShrink: 0 }} />
              <input
                id="meeting-search-input"
                className="sidebar-search-input"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder={lang === 'de' ? 'Meetings durchsuchen…' : 'Search meetings…'}
              />
              {searchQuery && (
                <button
                  type="button"
                  className="icon-ghost-btn"
                  style={{ opacity: 1 }}
                  onClick={() => setSearchQuery('')}
                >
                  <X size={11} />
                </button>
              )}
            </div>

            {/* Folder Filter Strip + Quick Folder Creator */}
            <div className="folder-filter-strip">
              <button
                type="button"
                id="folder-filter-all"
                className={`folder-pill ${activeFolderId === null ? 'active' : ''}`}
                onClick={() => setActiveFolderId(null)}
              >
                {lang === 'de' ? 'Alle' : 'All'}
                <span className="mono-ts">{meetings.length}</span>
              </button>
              {folders.map((fld) => (
                <div
                  key={fld.id}
                  className={`folder-pill-wrap ${activeFolderId === fld.id ? 'active' : ''}`}
                >
                  <button
                    type="button"
                    className={`folder-pill ${activeFolderId === fld.id ? 'active' : ''}`}
                    onClick={() =>
                      setActiveFolderId((prev) => (prev === fld.id ? null : fld.id))
                    }
                    title={fld.description || fld.name}
                  >
                    <span className="folder-dot" style={{ background: fld.color }} />
                    <span>{fld.name}</span>
                    <span className="mono-ts">{fld.meeting_count}</span>
                  </button>
                  {activeFolderId === fld.id && (
                    <button
                      type="button"
                      id={`chat-folder-btn-${fld.id}`}
                      className="folder-chat-trigger"
                      title={
                        lang === 'de'
                          ? `Mit allen Meetings im Ordner „${fld.name}“ chatten`
                          : `Chat with all meetings in "${fld.name}"`
                      }
                      onClick={() =>
                        onStartScopedChat(
                          { folder_id: fld.id },
                          `${lang === 'de' ? 'Ordner' : 'Folder'}: ${fld.name}`,
                        )
                      }
                    >
                      <MessageSquare size={10} />
                    </button>
                  )}
                </div>
              ))}
              <button
                type="button"
                id="new-folder-toggle-btn"
                className="folder-pill folder-pill-add"
                onClick={() => setShowNewFolderForm((v) => !v)}
                title={lang === 'de' ? 'Neuen Ordner anlegen' : 'Create folder'}
              >
                <FolderPlus size={11} />
              </button>
            </div>

            {showNewFolderForm && (
              <form
                className="new-folder-inline"
                onSubmit={async (e) => {
                  e.preventDefault();
                  if (!newFolderName.trim()) return;
                  await onSaveFolder({ name: newFolderName.trim(), color: newFolderColor });
                  setNewFolderName('');
                  setShowNewFolderForm(false);
                }}
              >
                <input
                  id="new-folder-name-input"
                  className="input-field"
                  style={{ padding: '4px 8px', fontSize: 12 }}
                  value={newFolderName}
                  onChange={(e) => setNewFolderName(e.target.value)}
                  placeholder={lang === 'de' ? 'Ordnername…' : 'Folder name…'}
                  autoFocus
                />
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 6 }}>
                  <div style={{ display: 'flex', gap: 5 }}>
                    {FOLDER_COLORS.map((c) => (
                      <button
                        key={c}
                        type="button"
                        onClick={() => setNewFolderColor(c)}
                        style={{
                          width: 14,
                          height: 14,
                          borderRadius: 99,
                          background: c,
                          border: newFolderColor === c ? '2px solid var(--text-primary)' : 'none',
                          cursor: 'pointer',
                        }}
                      />
                    ))}
                  </div>
                  <div style={{ display: 'flex', gap: 4 }}>
                    <button type="submit" id="save-new-folder-btn" className="mini-btn active-choice">
                      {lang === 'de' ? 'Anlegen' : 'Create'}
                    </button>
                    <button
                      type="button"
                      className="mini-btn"
                      onClick={() => setShowNewFolderForm(false)}
                    >
                      <X size={10} />
                    </button>
                  </div>
                </div>
              </form>
            )}

            {/* Active Folder Action Banner */}
            {activeFolder && (
              <div className="active-folder-banner">
                <span style={{ fontSize: 11.5, color: 'var(--text-secondary)', display: 'inline-flex', alignItems: 'center', gap: 5 }}>
                  <Folder size={11} style={{ color: activeFolder.color }} />
                  <strong>{activeFolder.name}</strong>
                </span>
                <div style={{ display: 'flex', gap: 4 }}>
                  <button
                    type="button"
                    id="active-folder-chat-btn"
                    className="mini-btn active-choice"
                    style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                    onClick={() =>
                      onStartScopedChat(
                        { folder_id: activeFolder.id },
                        `${lang === 'de' ? 'Ordner' : 'Folder'}: ${activeFolder.name}`,
                      )
                    }
                  >
                    <MessageSquare size={10} />
                    {lang === 'de' ? 'Ordner-Chat' : 'Folder Chat'}
                  </button>
                  <button
                    type="button"
                    className="mini-btn"
                    title={lang === 'de' ? 'Ordner löschen' : 'Delete folder'}
                    onClick={async () => {
                      await onDeleteFolder(activeFolder.id);
                      setActiveFolderId(null);
                    }}
                  >
                    <Trash2 size={10} />
                  </button>
                </div>
              </div>
            )}

            {/* Bulk Action Bar when multi-selecting meetings */}
            {multiSelectMode && selectedIds.length > 0 && (
              <div className="bulk-select-bar" id="bulk-meeting-actions">
                <div style={{ fontSize: 11.5, fontWeight: 600 }}>
                  {selectedIds.length} {lang === 'de' ? 'ausgewählt' : 'selected'}
                </div>
                <div style={{ display: 'flex', gap: 5, flexWrap: 'wrap' }}>
                  <button
                    type="button"
                    id="bulk-chat-meetings-btn"
                    className="btn btn-primary"
                    style={{ padding: '4px 9px', fontSize: 11.5 }}
                    onClick={() => {
                      onStartScopedChat(
                        { meeting_ids: selectedIds },
                        `${selectedIds.length} ${lang === 'de' ? 'ausgewählte Meetings' : 'selected meetings'}`,
                      );
                    }}
                  >
                    <MessageSquare size={11} />
                    {lang === 'de' ? 'Chatten' : 'Chat'}
                  </button>
                  {folders.length > 0 && (
                    <select
                      className="select-field"
                      style={{ padding: '3px 6px', fontSize: 11.5, width: 'auto' }}
                      value={bulkFolderId}
                      onChange={async (e) => {
                        const fid = e.target.value;
                        if (!fid) return;
                        setBulkFolderId('');
                        await onAddMeetingsToFolder(selectedIds, fid);
                        setSelectedIds([]);
                        setMultiSelectMode(false);
                      }}
                    >
                      <option value="">{lang === 'de' ? '+ In Ordner…' : '+ Add to folder…'}</option>
                      {folders.map((f) => (
                        <option key={f.id} value={f.id}>
                          {f.name}
                        </option>
                      ))}
                    </select>
                  )}
                </div>
              </div>
            )}

            <div className="granola-list-scroll">
              {groupedMeetings.length === 0 && (
                <div style={{ padding: 12, fontSize: 12.5, color: 'var(--text-tertiary)' }}>
                  {searchQuery || activeFolderId
                    ? lang === 'de'
                      ? 'Keine Meetings für diesen Filter gefunden.'
                      : 'No meetings match this filter.'
                    : lang === 'de'
                      ? 'Noch keine Meetings. Klicke oben auf „Neu“, um sofort aufzunehmen.'
                      : 'No meetings yet. Click New to start recording.'}
                </div>
              )}

              {groupedMeetings.map((group) => (
                <div key={group.label} className="meeting-date-group">
                  <div className="meeting-group-label">{group.label}</div>
                  {group.items.map((m) => {
                    const isChecked = selectedIds.includes(m.id);
                    const mFolders = folders.filter((f) => (m.folder_ids ?? []).includes(f.id));
                    return (
                      <div
                        key={m.id}
                        className={`meeting-list-card ${current?.id === m.id ? 'active' : ''}`}
                        style={{ position: 'relative' }}
                        onClick={() => {
                          if (multiSelectMode) {
                            toggleSelectedId(m.id);
                          } else {
                            onSelectMeeting(m.id);
                            if (isNarrow) setSidebarOpen(false);
                          }
                        }}
                        role="button"
                        tabIndex={0}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault();
                            if (multiSelectMode) toggleSelectedId(m.id);
                            else {
                              onSelectMeeting(m.id);
                              if (isNarrow) setSidebarOpen(false);
                            }
                          }
                        }}
                      >
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', width: '100%', gap: 6 }}>
                          <div style={{ display: 'flex', alignItems: 'center', gap: 6, minWidth: 0 }}>
                            {multiSelectMode && (
                              <input
                                type="checkbox"
                                checked={isChecked}
                                onChange={() => toggleSelectedId(m.id)}
                                onClick={(e) => e.stopPropagation()}
                              />
                            )}
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
                          </div>
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
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 11.5, color: 'var(--text-tertiary)' }}>
                          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}>
                            {new Date(m.started_at).toLocaleDateString(lang === 'de' ? 'de-DE' : 'en-US', {
                              day: '2-digit',
                              month: 'short',
                              hour: '2-digit',
                              minute: '2-digit',
                            })}
                            {mFolders.map((f) => (
                              <span
                                key={f.id}
                                className="folder-dot"
                                style={{ background: f.color }}
                                title={f.name}
                              />
                            ))}
                          </span>
                          <span>
                            {m.transcript_count > 0
                              ? `${m.transcript_count} Seg.`
                              : m.status === 'scheduled'
                                ? 'Geplant'
                                : ''}
                          </span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              ))}
            </div>
          </aside>
        )}

        {/* Calm Granola-Style Focused Document Canvas */}
        {current && selectedDetail ? (
          <section className="granola-canvas" data-content-width={contentWidth}>
            {/* Minimal Document Top Bar */}
            <div className="granola-topbar" data-tauri-drag-region>
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' }}>
                <button
                  type="button"
                  id="toggle-meeting-list-btn"
                  className="mini-btn"
                  title={sidebarOpen ? 'Meeting-Liste ausblenden' : 'Meeting-Liste einblenden'}
                  onClick={() => setSidebarOpen((v) => !v)}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}
                >
                  <PanelLeft size={13} />
                  {!sidebarOpen && (lang === 'de' ? 'Liste' : 'List')}
                </button>

                <button
                  type="button"
                  id="meeting-focus-mode-btn"
                  className={`mini-btn ${isFocusMode ? 'active-choice' : ''}`}
                  title={
                    isFocusMode
                      ? lang === 'de'
                        ? 'Fokus-Modus beenden (Menü & Liste wieder anzeigen)'
                        : 'Exit Focus Mode'
                      : lang === 'de'
                        ? 'Meeting-Fokus-Modus (Hauptmenü & Liste einklappen)'
                        : 'Meeting Focus Mode (collapse navigation & list)'
                  }
                  onClick={toggleFocusMode}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                >
                  {isFocusMode ? <Minimize2 size={12} /> : <Maximize2 size={12} />}
                  {isFocusMode
                    ? lang === 'de'
                      ? 'Fokus'
                      : 'Focus'
                    : lang === 'de'
                      ? 'Fokus'
                      : 'Focus'}
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

            {/* Subtle STT / Audio Warning Banner */}
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
                  id="toggle-participants-btn"
                  className={`mini-btn ${showPeoplePopover ? 'active-choice' : ''}`}
                  onClick={() => {
                    setShowPeoplePopover((v) => !v);
                    setShowFolderPopover(false);
                  }}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}
                >
                  <Users size={11} />
                  {current.participants.length}{' '}
                  {lang === 'de' ? 'Teilnehmer' : 'Participants'}
                </button>

                {/* Folder Assignment Trigger & Pills */}
                <button
                  type="button"
                  id="toggle-folders-btn"
                  className={`mini-btn ${showFolderPopover ? 'active-choice' : ''}`}
                  onClick={() => {
                    setShowFolderPopover((v) => !v);
                    setShowPeoplePopover(false);
                  }}
                  style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}
                >
                  <Folder size={11} />
                  {currentFolders.length > 0
                    ? currentFolders.map((f) => f.name).join(', ')
                    : lang === 'de'
                      ? '+ Ordner'
                      : '+ Folder'}
                </button>

                {current.detected_app &&
                  !current.detected_app.toLowerCase().includes('dual-audio') &&
                  !current.detected_app.toLowerCase().includes('mikrofon + system') && (
                    <>
                      <span>·</span>
                      <span className="mono-ts">{current.detected_app}</span>
                    </>
                  )}
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
                {activeTab === 'enhanced' && editingEnhanced && (
                  <span className="mono-ts" style={{ marginLeft: 'auto', whiteSpace: 'nowrap' }}>
                    {enhancedSaveState === 'saving'
                      ? lang === 'de'
                        ? 'Speichert…'
                        : 'Saving…'
                      : enhancedSaveState === 'saved'
                        ? lang === 'de'
                          ? 'Gespeichert'
                          : 'Saved'
                        : ''}
                  </span>
                )}
              </div>

              {/* Folder Assignment + Recurring Series Auto-Add Popover */}
              {showFolderPopover && (
                <div className="granola-people-popover" id="meeting-folders-popover">
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
                    <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-secondary)' }}>
                      {lang === 'de'
                        ? 'Meeting-Ordner & Serien-Regeln'
                        : 'Meeting Folders & Recurring Series Rules'}
                    </span>
                    <button
                      type="button"
                      className="icon-ghost-btn"
                      style={{ opacity: 1 }}
                      onClick={() => setShowFolderPopover(false)}
                    >
                      <X size={13} />
                    </button>
                  </div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 7 }}>
                    {folders.map((fld) => {
                      const assigned = currentFolderIds.includes(fld.id);
                      const autoLinked =
                        Boolean(current.series_key) &&
                        fld.auto_series_keys.some(
                          (k) => k.toLowerCase() === current.series_key?.toLowerCase(),
                        );
                      return (
                        <div
                          key={fld.id}
                          style={{
                            display: 'flex',
                            justifyContent: 'space-between',
                            alignItems: 'center',
                            gap: 8,
                            fontSize: 12.5,
                          }}
                        >
                          <label style={{ display: 'inline-flex', alignItems: 'center', gap: 7, cursor: 'pointer' }}>
                            <input
                              type="checkbox"
                              checked={assigned}
                              onChange={async (e) => {
                                const nextIds = e.target.checked
                                  ? [...currentFolderIds, fld.id]
                                  : currentFolderIds.filter((id) => id !== fld.id);
                                await onSetMeetingFolders(current.id, nextIds);
                              }}
                            />
                            <span className="folder-dot" style={{ background: fld.color }} />
                            <span>{fld.name}</span>
                          </label>
                          {current.series_key && (
                            <button
                              type="button"
                              className={`mini-btn ${autoLinked ? 'active-choice' : ''}`}
                              style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                              disabled={autoLinked}
                              onClick={() => onAddSeriesAutoFolder(fld.id, current.series_key!)}
                              title={
                                lang === 'de'
                                  ? 'Alle bisherigen & zukünftigen Termine dieser Serie automatisch in diesen Ordner legen'
                                  : 'Auto-add past & future meetings in this recurring series'
                              }
                            >
                              <Repeat size={10} />
                              {autoLinked
                                ? lang === 'de'
                                  ? 'Serie aktiv'
                                  : 'Series auto'
                                : lang === 'de'
                                  ? 'Serie automatisch'
                                  : 'Auto-add series'}
                            </button>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Full Meeting Participant Management Popover */}
              {showPeoplePopover && (
                <div className="granola-people-popover" id="participants-manager-panel">
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
                    <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-secondary)' }}>
                      {lang === 'de'
                        ? 'Teilnehmer verwalten & Sprecher zuordnen'
                        : 'Manage Participants & Assign Speakers'}
                    </span>
                    <button
                      type="button"
                      className="icon-ghost-btn"
                      style={{ opacity: 1 }}
                      onClick={() => setShowPeoplePopover(false)}
                    >
                      <X size={13} />
                    </button>
                  </div>

                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                    {current.participants.map((p) => (
                      <div
                        key={p.speaker_label}
                        className="participant-chip"
                        title={`${sourceBadgeLabel(p.identification_source, lang)} (${Math.round(p.confidence * 100)}%)${p.email ? ` · ${p.email}` : ''}`}
                      >
                        <button
                          type="button"
                          className="participant-chip-main"
                          style={{ cursor: p.is_self ? 'default' : 'pointer' }}
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
                        {!p.is_self && (
                          <button
                            type="button"
                            className="participant-remove-btn"
                            title={lang === 'de' ? 'Aus Meeting entfernen' : 'Remove from meeting'}
                            onClick={() => onRemoveParticipant(current.id, p.speaker_label)}
                          >
                            <X size={11} />
                          </button>
                        )}
                      </div>
                    ))}
                  </div>

                  {renamingLabel && (
                    <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap', marginTop: 10 }}>
                      <input
                        className="input-field"
                        style={{ maxWidth: 220, padding: '5px 9px', fontSize: 12.5 }}
                        value={renameValue}
                        onChange={(e) => setRenameValue(e.target.value)}
                        placeholder={lang === 'de' ? 'Vor- und Nachname' : 'Full name'}
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

                  {/* Quick-Add Known Contacts */}
                  {suggestedContacts.length > 0 && (
                    <div style={{ marginTop: 10, paddingTop: 9, borderTop: '1px solid var(--border-subtle)' }}>
                      <div style={{ fontSize: 11.5, color: 'var(--text-tertiary)', marginBottom: 6 }}>
                        {lang === 'de' ? 'Bekannten Kontakt hinzufügen:' : 'Quick-add known contact:'}
                      </div>
                      <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                        {suggestedContacts.slice(0, 6).map((per) => (
                          <button
                            key={per.id}
                            type="button"
                            className="mini-btn"
                            style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                            onClick={() =>
                              onAddParticipant(current.id, per.full_name, per.email ?? undefined, per.id)
                            }
                          >
                            <Plus size={11} />
                            {per.full_name}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Add New Participant Form */}
                  <form
                    style={{
                      display: 'flex',
                      gap: 7,
                      flexWrap: 'wrap',
                      alignItems: 'center',
                      marginTop: 10,
                      paddingTop: 9,
                      borderTop: '1px solid var(--border-subtle)',
                    }}
                    onSubmit={async (e) => {
                      e.preventDefault();
                      if (!newPartName.trim()) return;
                      await onAddParticipant(
                        current.id,
                        newPartName.trim(),
                        newPartEmail.trim() || undefined,
                      );
                      setNewPartName('');
                      setNewPartEmail('');
                    }}
                  >
                    <input
                      id="add-participant-name-input"
                      className="input-field"
                      style={{ flex: '1 1 160px', padding: '5px 9px', fontSize: 12.5 }}
                      value={newPartName}
                      onChange={(e) => setNewPartName(e.target.value)}
                      placeholder={lang === 'de' ? 'Teilnehmer-Name…' : 'Participant name…'}
                    />
                    <input
                      id="add-participant-email-input"
                      className="input-field"
                      style={{ flex: '1 1 170px', padding: '5px 9px', fontSize: 12.5 }}
                      value={newPartEmail}
                      onChange={(e) => setNewPartEmail(e.target.value)}
                      placeholder={lang === 'de' ? 'E-Mail (optional)' : 'Email (optional)'}
                    />
                    <button
                      type="submit"
                      id="add-participant-submit-btn"
                      className="btn btn-primary"
                      style={{ padding: '5px 10px', fontSize: 12 }}
                      disabled={!newPartName.trim()}
                    >
                      <UserPlus size={12} />
                      {lang === 'de' ? 'Hinzufügen' : 'Add'}
                    </button>
                  </form>
                </div>
              )}

              {/* Recurring Series Context Card (Feature 2.9) */}
              {previousInSeries && (
                <div className="series-context-card" id="previous-series-meeting-card">
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8 }}>
                    <button
                      type="button"
                      onClick={() => setShowSeriesCard((v) => !v)}
                      style={{
                        background: 'transparent',
                        border: 'none',
                        padding: 0,
                        color: 'inherit',
                        cursor: 'pointer',
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 6,
                        fontSize: 12,
                        fontWeight: 600,
                      }}
                    >
                      <History size={13} style={{ color: 'var(--accent-sage)' }} />
                      <span>
                        {lang === 'de'
                          ? 'Vorheriges Meeting dieser Serie:'
                          : 'Previous meeting in this series:'}
                      </span>
                      <span className="mono-ts">
                        {new Date(previousInSeries.started_at).toLocaleDateString(
                          lang === 'de' ? 'de-DE' : 'en-US',
                          { day: '2-digit', month: 'short' },
                        )}
                      </span>
                    </button>
                    <button
                      type="button"
                      id="jump-prev-series-btn"
                      className="mini-btn"
                      style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                      onClick={() => onSelectMeeting(previousInSeries.id)}
                    >
                      {lang === 'de' ? 'Öffnen' : 'Open'}
                      <ArrowUpRight size={11} />
                    </button>
                  </div>
                  {showSeriesCard && (
                    <div style={{ fontSize: 12.4, color: 'var(--text-secondary)', marginTop: 6 }}>
                      {previousInSeries.executive_summary ||
                        previousInSeries.raw_user_notes.slice(0, 180)}
                      {previousInSeries.action_items.filter((a) => !a.completed).length > 0 && (
                        <div style={{ marginTop: 5, display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                          {previousInSeries.action_items
                            .filter((a) => !a.completed)
                            .slice(0, 2)
                            .map((a) => (
                              <span key={a.id} className="pill pill-ochre">
                                {lang === 'de' ? 'Offen:' : 'Open:'} {a.owner} —{' '}
                                {a.task.length > 56 ? `${a.task.slice(0, 56).trim()}…` : a.task}
                              </span>
                            ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Main Distraction-Free Canvas Body */}
            <div className="granola-body">
              {activeTab === 'notes' && (
                <RichNotepad
                  id="meeting-rough-notes-editor"
                  meetingId={current.id}
                  lang={lang}
                  value={draftNotes}
                  onChange={(next) => {
                    setDraftNotes(next);
                    scheduleAutoSave(current.id, next);
                  }}
                  onBlur={(finalMd) => {
                    if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
                    onSaveUserNotes(current.id, finalMd);
                    setSaveState('saved');
                  }}
                  placeholder={
                    lang === 'de'
                      ? 'Notiere einfach deine Stichpunkte während des Gesprächs…\n\nbacktext erkennt Listen automatisch, unterstützt Bilder per ⌘V / Drag & Drop und verdichtet deine Notizen danach mit Transkript & Folien.'
                      : 'Jot rough thoughts freely during the call…\n\nbacktext auto-formats lists, supports pasted/dropped images, and enhances your notes with transcript & slide facts afterward.'
                  }
                />
              )}

              {activeTab === 'enhanced' && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 14, minWidth: 0 }}>
                  {/* Template Switcher + Direct Edit + Compare Raw Notes Toolbar (Features 1.1 & 1.2) */}
                  <div className="enhanced-toolbar">
                    <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' }}>
                      <LayoutTemplate size={13} style={{ color: 'var(--text-tertiary)' }} />
                      <select
                        id="meeting-template-select"
                        className="select-field"
                        style={{ width: 'auto', padding: '4px 9px', fontSize: 12.2 }}
                        value={current.template_id || 'general'}
                        disabled={busy}
                        onChange={async (e) => {
                          const nextTpl = e.target.value;
                          await onSetMeetingTemplate(
                            current.id,
                            nextTpl,
                            Boolean(current.enhanced_markdown || selectedDetail.transcript.length > 0),
                          );
                        }}
                      >
                        {templates.map((tpl) => (
                          <option key={tpl.id} value={tpl.id}>
                            {lang === 'de' ? tpl.name_de : tpl.name_en}
                            {tpl.is_builtin ? '' : ' ★'}
                          </option>
                        ))}
                      </select>

                      <button
                        type="button"
                        id="open-template-modal-btn"
                        className="mini-btn"
                        onClick={() => {
                          setTplDraft({
                            name_de: '',
                            description_de: '',
                            sections_prompt:
                              '### Kontext & Ziel\n### Wichtigste Entscheidungen\n### Risiken & Blocker\n### Action Items',
                          });
                          setTemplateModalOpen(true);
                        }}
                        style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                      >
                        <Plus size={11} />
                        {lang === 'de' ? 'Template' : 'Template'}
                      </button>
                    </div>

                    <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' }}>
                      {current.raw_user_notes.trim().length > 0 && (
                        <button
                          type="button"
                          id="toggle-raw-compare-btn"
                          className={`mini-btn ${showRawCompare ? 'active-choice' : ''}`}
                          onClick={() => setShowRawCompare((v) => !v)}
                          style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                        >
                          <SplitSquareVertical size={11} />
                          {lang === 'de' ? 'Eigene Notizen' : 'My Notes'}
                        </button>
                      )}
                      <button
                        type="button"
                        id="edit-enhanced-notes-btn"
                        className={`mini-btn ${editingEnhanced ? 'active-choice' : ''}`}
                        onMouseDown={(e) => {
                          // Prevent textarea onBlur from firing before click completes
                          if (editingEnhanced) e.preventDefault();
                        }}
                        onClick={async () => {
                          if (editingEnhanced) {
                            if (enhancedSaveTimerRef.current) {
                              window.clearTimeout(enhancedSaveTimerRef.current);
                            }
                            setEditingEnhanced(false);
                            await onSaveEnhancedNotes(current.id, draftEnhanced);
                            setEnhancedSaveState('saved');
                          } else {
                            setDraftEnhanced(current.enhanced_markdown ?? '');
                            setEditingEnhanced(true);
                          }
                        }}
                        style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                      >
                        {editingEnhanced ? <Eye size={11} /> : <Pencil size={11} />}
                        {editingEnhanced
                          ? lang === 'de'
                            ? 'Fertig'
                            : 'Done'
                          : lang === 'de'
                            ? 'Bearbeiten'
                            : 'Edit'}
                      </button>
                    </div>
                  </div>

                  {/* Optional Side/Top Compare Box for User's Own Rough Bullets */}
                  {showRawCompare && current.raw_user_notes.trim().length > 0 && (
                    <div className="raw-notes-compare-box">
                      <div style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--text-tertiary)', marginBottom: 6 }}>
                        {lang === 'de' ? 'Deine eigenen Stichpunkte' : 'Your Rough Notes'}
                      </div>
                      <FormattedMarkdown text={current.raw_user_notes} />
                    </div>
                  )}

                  {editingEnhanced ? (
                    <textarea
                      id="enhanced-notes-editor"
                      className="granola-notepad-textarea"
                      style={{ minHeight: 340 }}
                      value={draftEnhanced}
                      onChange={(e) => {
                        const next = e.target.value;
                        setDraftEnhanced(next);
                        scheduleEnhancedAutoSave(current.id, next);
                      }}
                      onBlur={() => {
                        if (enhancedSaveTimerRef.current) {
                          window.clearTimeout(enhancedSaveTimerRef.current);
                        }
                        onSaveEnhancedNotes(current.id, draftEnhanced);
                        setEnhancedSaveState('saved');
                      }}
                      placeholder={
                        lang === 'de'
                          ? 'Bearbeite deine KI-Notizen direkt in Markdown… [MM:SS] Zeitstempel bleiben klickbar.'
                          : 'Edit your AI notes directly in Markdown… [MM:SS] citations stay clickable.'
                      }
                    />
                  ) : (
                    <div className="granola-enhanced-sheet">
                      <FormattedMarkdown
                        text={
                          current.enhanced_markdown ||
                          current.prep_brief ||
                          (lang === 'de'
                            ? 'Noch keine KI-Notizen generiert. Klicke oben auf **Beenden** oder **Mit KI verdichten**.'
                            : 'No AI notes generated yet. Click **Finish** or **Enhance with AI** above.')
                        }
                        transcript={selectedDetail.transcript}
                        onJumpToTranscript={handleJumpToTranscript}
                      />
                    </div>
                  )}

                  {current.action_items.length > 0 && (
                    <div>
                      <div className="section-heading">
                        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                          <CheckSquare size={14} />
                          {t(lang, 'actionItems')} ({current.action_items.length})
                        </span>
                      </div>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 7 }}>
                        {current.action_items.map((act) => (
                          <label
                            key={act.id}
                            className={`action-item-row ${act.completed ? 'completed' : ''}`}
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
                          ? 'Aufnahme läuft — gesprochene Abschnitte erscheinen hier automatisch bei jeder Sprechpause.'
                          : 'Recording active — spoken segments appear here automatically on each speech pause.'
                        : lang === 'de'
                          ? 'Noch keine Transkript-Segmente für dieses Meeting.'
                          : 'No transcript segments for this meeting yet.'}
                    </div>
                  )}
                  {selectedDetail.transcript.map((seg) => (
                    <div
                      key={seg.id}
                      id={`transcript-seg-${seg.id}`}
                      className={`transcript-row ${highlightedSegmentId === seg.id ? 'highlighted' : ''}`}
                    >
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

            {/* Granola-Style Anchored Bottom AI Chat & / Recipes Bar (supports /rewrite & Apply to Notes) */}
            <RecipeChatBar
              lang={lang}
              meetingId={current.id}
              scopeFilter="single_meeting"
              compact
              recipes={recipes}
              messages={selectedDetail.chat_messages}
              transcript={selectedDetail.transcript}
              onJumpToTranscript={handleJumpToTranscript}
              onApplyToNotes={async (md) => {
                await onSaveEnhancedNotes(current.id, md);
                setDraftEnhanced(md);
                setActiveTab('enhanced');
              }}
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

      {/* Custom & Built-In Meeting Template Studio Modal */}
      {templateModalOpen && (
        <div
          className="hover-backdrop"
          onClick={() => setTemplateModalOpen(false)}
          role="dialog"
          aria-modal="true"
        >
          <div
            className="template-modal-card"
            onClick={(e) => e.stopPropagation()}
            id="meeting-template-modal"
          >
            <div className="section-heading" style={{ marginBottom: 8 }}>
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <LayoutTemplate size={15} style={{ color: 'var(--accent-sage)' }} />
                {tplDraft.id
                  ? lang === 'de'
                    ? `Template bearbeiten: ${tplDraft.name_de}`
                    : `Edit Template: ${tplDraft.name_de}`
                  : lang === 'de'
                    ? 'Meeting-Templates verwalten'
                    : 'Manage Meeting Templates'}
              </span>
              <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                <button
                  type="button"
                  id="open-template-studio-link"
                  className="mini-btn"
                  onClick={() => {
                    setTemplateModalOpen(false);
                    onOpenTemplateManager();
                  }}
                >
                  {lang === 'de' ? 'Im Studio öffnen →' : 'Open Studio →'}
                </button>
                <button
                  type="button"
                  className="mini-btn"
                  onClick={() => setTemplateModalOpen(false)}
                >
                  <X size={12} />
                </button>
              </div>
            </div>

            <form
              style={{ display: 'flex', flexDirection: 'column', gap: 10 }}
              onSubmit={async (e) => {
                e.preventDefault();
                if (!tplDraft.name_de.trim()) return;
                await onSaveTemplate(tplDraft);
                setTplDraft({
                  name_de: '',
                  description_de: '',
                  sections_prompt:
                    '### Kurzüberblick\n### Kernpunkte\n### Entscheidungen & Action Items',
                });
              }}
            >
              <label style={{ fontSize: 12, fontWeight: 600 }}>
                {lang === 'de' ? 'Template-Name *' : 'Template Name *'}
                <input
                  id="custom-template-name-input"
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={tplDraft.name_de}
                  onChange={(e) => setTplDraft({ ...tplDraft, name_de: e.target.value })}
                  placeholder={lang === 'de' ? 'z. B. Kunden-QBR oder Sprint-Review' : 'e.g. Customer QBR'}
                  required
                />
              </label>
              <label style={{ fontSize: 12, fontWeight: 600 }}>
                {lang === 'de' ? 'Kurzbeschreibung' : 'Short Description'}
                <input
                  id="custom-template-desc-input"
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={tplDraft.description_de}
                  onChange={(e) => setTplDraft({ ...tplDraft, description_de: e.target.value })}
                  placeholder={
                    lang === 'de'
                      ? 'Wofür soll dieses Template verwendet werden?'
                      : 'When should this template be used?'
                  }
                />
              </label>
              <label style={{ fontSize: 12, fontWeight: 600 }}>
                {lang === 'de' ? 'Struktur & Abschnitts-Anweisungen (Prompt)' : 'Sections & Instructions'}
                <textarea
                  id="custom-template-prompt-input"
                  className="textarea-field"
                  style={{ marginTop: 4, minHeight: 110, fontFamily: 'var(--font-mono)', fontSize: 12 }}
                  value={tplDraft.sections_prompt}
                  onChange={(e) => setTplDraft({ ...tplDraft, sections_prompt: e.target.value })}
                />
              </label>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <button
                  type="submit"
                  id="save-custom-template-btn"
                  className="btn btn-primary"
                  disabled={!tplDraft.name_de.trim()}
                >
                  {tplDraft.id
                    ? lang === 'de'
                      ? 'Änderungen speichern'
                      : 'Save Changes'
                    : lang === 'de'
                      ? 'Template speichern'
                      : 'Save Template'}
                </button>
                {tplDraft.id && (
                  <button
                    type="button"
                    className="btn"
                    onClick={() =>
                      setTplDraft({
                        name_de: '',
                        description_de: '',
                        sections_prompt:
                          '### Kurzüberblick\n### Kernpunkte\n### Entscheidungen & Action Items',
                      })
                    }
                  >
                    {lang === 'de' ? 'Neu anlegen' : 'New Template'}
                  </button>
                )}
                <button
                  type="button"
                  className="btn"
                  onClick={() => setTemplateModalOpen(false)}
                >
                  {lang === 'de' ? 'Schließen' : 'Close'}
                </button>
              </div>
            </form>

            <div style={{ marginTop: 14, paddingTop: 12, borderTop: '1px solid var(--border-subtle)' }}>
              <div style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--text-tertiary)', marginBottom: 6 }}>
                {lang === 'de' ? 'Verfügbare Templates (alle bearbeitbar)' : 'Available Templates (all editable)'} ({templates.length})
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 190, overflowY: 'auto' }}>
                {templates.map((tpl) => {
                  const isDef = defaultTemplateId === tpl.id;
                  return (
                    <div
                      key={tpl.id}
                      className="transcript-row"
                      style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', padding: '7px 10px' }}
                    >
                      <div style={{ minWidth: 0 }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' }}>
                          <strong style={{ fontSize: 12.5 }}>
                            {lang === 'de' ? tpl.name_de : tpl.name_en}
                          </strong>
                          {isDef && <span className="pill pill-sage">Standard</span>}
                        </div>
                        <div style={{ fontSize: 11.5, color: 'var(--text-tertiary)' }}>
                          {lang === 'de' ? tpl.description_de : tpl.description_en}
                        </div>
                      </div>
                      <div style={{ display: 'flex', gap: 5, alignItems: 'center', flexShrink: 0 }}>
                        <button
                          type="button"
                          className="mini-btn"
                          onClick={() =>
                            setTplDraft({
                              id: tpl.id,
                              name_de: tpl.name_de,
                              name_en: tpl.name_en,
                              description_de: tpl.description_de,
                              description_en: tpl.description_en,
                              icon_key: tpl.icon_key,
                              sections_prompt: tpl.sections_prompt,
                            })
                          }
                          title={lang === 'de' ? 'Bearbeiten' : 'Edit'}
                        >
                          <Pencil size={11} />
                        </button>
                        {!isDef && (
                          <button
                            type="button"
                            className="mini-btn"
                            onClick={() => onSetDefaultTemplate(tpl.id)}
                          >
                            {lang === 'de' ? 'Standard' : 'Default'}
                          </button>
                        )}
                        {tpl.is_builtin ? (
                          <button
                            type="button"
                            className="mini-btn"
                            onClick={() => onResetTemplate(tpl.id)}
                            title={lang === 'de' ? 'Auf Standard zurücksetzen' : 'Reset'}
                          >
                            <RotateCcw size={11} />
                          </button>
                        ) : (
                          <button
                            type="button"
                            className="mini-btn"
                            onClick={() => onDeleteTemplate(tpl.id)}
                            title={lang === 'de' ? 'Löschen' : 'Delete'}
                          >
                            <Trash2 size={11} />
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
