import React, { useEffect, useState } from 'react';
import {
  Cpu,
  KeyRound,
  Download,
  Calendar,
  RefreshCw,
  CheckCircle2,
  Lock,
  Sparkles,
  Trash2,
  Plus,
  Command,
  ShieldCheck,
  Mic,
  Square,
} from 'lucide-react';
import type {
  AiProviderSettings,
  CalendarFeedConfig,
  DaemonTelemetry,
  DictationEntry,
  Language,
  PermissionStatus,
  ProviderTestResult,
  UpdateCheckResult,
  UserProfile,
  WhisperModelInfo,
} from '../types';

interface SettingsViewProps {
  lang: Language;
  profile: UserProfile;
  permissions: PermissionStatus;
  telemetry: DaemonTelemetry;
  aiSettings: AiProviderSettings;
  whisperModels: WhisperModelInfo[];
  ollamaModels: string[];
  calendarFeeds: CalendarFeedConfig[];
  dictations: DictationEntry[];
  voiceTypingActive: boolean;
  onSaveProfile: (profile: UserProfile) => Promise<void>;
  onSaveAiSettings: (settings: AiProviderSettings) => Promise<void>;
  onSetApiKey: (provider: string, apiKey: string) => Promise<void>;
  onDeleteApiKey: (provider: string) => Promise<void>;
  onTestProvider: (provider: string) => Promise<ProviderTestResult>;
  onFetchProviderModels: (provider: string) => Promise<string[]>;
  onDownloadWhisperModel: (modelId: string) => Promise<void>;
  onSetGlobalShortcut: (shortcut: string) => Promise<string>;
  onSetVoiceTypingShortcut: (shortcut: string) => Promise<string>;
  onToggleVoiceTyping: () => Promise<void>;
  onAddCalendarFeed: (name: string, icsUrl: string) => Promise<void>;
  onDeleteCalendarFeed: (id: string) => Promise<void>;
  onSyncCalendars: () => Promise<void>;
  onToggleAutostart: (enabled: boolean) => Promise<void>;
  onCheckUpdates: () => Promise<UpdateCheckResult>;
  onRequestPermission: (kind: string) => Promise<void>;
  onRefreshPermissions: () => Promise<void>;
  onClearDemoData: () => Promise<void>;
}

const KEY_PROVIDERS = [
  {
    id: 'openrouter',
    label: 'OpenRouter (Multi-Model Gateway)',
    field: 'openrouter_api_key_set',
    placeholder: 'sk-or-v1-…',
  },
  {
    id: 'primalabs',
    label: 'PrimaLabs (Serverless Open-Weight · ZDR)',
    field: 'primalabs_api_key_set',
    placeholder: 'pl_…',
  },
  {
    id: 'anthropic',
    label: 'Anthropic Claude',
    field: 'anthropic_api_key_set',
    placeholder: 'sk-ant-api03-…',
  },
  {
    id: 'openai',
    label: 'OpenAI (GPT & Whisper)',
    field: 'openai_api_key_set',
    placeholder: 'sk-proj-…',
  },
  {
    id: 'gemini',
    label: 'Google Gemini',
    field: 'gemini_api_key_set',
    placeholder: 'AIza…',
  },
  {
    id: 'deepgram',
    label: 'Deepgram Nova-3 (STT + Diarisierung)',
    field: 'deepgram_api_key_set',
    placeholder: 'dg_…',
  },
  {
    id: 'groq',
    label: 'Groq Whisper (Ultra-Fast STT)',
    field: 'groq_api_key_set',
    placeholder: 'gsk_…',
  },
] as const;

const PROVIDER_DEFAULT_MODELS: Record<string, string> = {
  hybrid_auto: 'claude-sonnet-4-20250514',
  openrouter: 'anthropic/claude-sonnet-4',
  primalabs: 'primalabs-ai/DeepSeek-V4.1-Flash-1017',
  anthropic: 'claude-sonnet-4-20250514',
  openai: 'gpt-4o-mini',
  gemini: 'gemini-2.5-flash',
  local_ollama: 'qwen3:8b',
};

const PROVIDER_PRESET_MODELS: Record<string, string[]> = {
  openrouter: [
    'anthropic/claude-sonnet-4',
    'anthropic/claude-3.7-sonnet',
    'openai/gpt-4o-mini',
    'openai/gpt-4.1',
    'google/gemini-2.5-flash',
    'deepseek/deepseek-chat-v3-0324',
    'moonshotai/kimi-k3.1-pro',
    'openrouter/auto',
  ],
  primalabs: [
    'primalabs-ai/DeepSeek-V4.1-Flash-1017',
    'primalabs-ai/Kimi-K3.1-Pro',
    'primalabs-ai/MiMo-V2.6-Pro',
    'primalabs-ai/GLM-5.3-Flash',
    'primalabs-ai/Nemotron-3.5-Lightning-NVFP4',
    'primalabs-ai/Nemotron-3.5-Super-VL-NVFP4',
  ],
  anthropic: [
    'claude-sonnet-4-20250514',
    'claude-3-7-sonnet-latest',
    'claude-3-5-haiku-latest',
  ],
  openai: ['gpt-4o-mini', 'gpt-4.1', 'gpt-4.1-mini', 'o4-mini'],
  gemini: ['gemini-2.5-flash', 'gemini-2.5-pro', 'gemini-2.0-flash'],
};

const SHORTCUT_PRESETS = [
  { value: 'Alt+Space', label: '⌥ Space (Standard)' },
  { value: 'CommandOrControl+Shift+Space', label: '⌘⇧ Space' },
  { value: 'Alt+Shift+Space', label: '⌥⇧ Space' },
  { value: 'CommandOrControl+Shift+B', label: '⌘⇧ B' },
  { value: 'CommandOrControl+Shift+K', label: '⌘⇧ K' },
  { value: 'Control+Space', label: '⌃ Space' },
] as const;

const VOICE_TYPING_PRESETS = [
  { value: 'Alt+Shift+D', label: '⌥⇧ D (Standard)' },
  { value: 'CommandOrControl+Shift+D', label: '⌘⇧ D' },
  { value: 'Alt+V', label: '⌥ V' },
] as const;

export const SettingsView: React.FC<SettingsViewProps> = ({
  lang,
  profile,
  permissions,
  telemetry,
  aiSettings,
  whisperModels,
  ollamaModels,
  calendarFeeds,
  dictations,
  voiceTypingActive,
  onSaveProfile,
  onSaveAiSettings,
  onSetApiKey,
  onDeleteApiKey,
  onTestProvider,
  onFetchProviderModels,
  onDownloadWhisperModel,
  onSetGlobalShortcut,
  onSetVoiceTypingShortcut,
  onToggleVoiceTyping,
  onAddCalendarFeed,
  onDeleteCalendarFeed,
  onSyncCalendars,
  onToggleAutostart,
  onCheckUpdates,
  onRequestPermission,
  onRefreshPermissions,
  onClearDemoData,
}) => {
  const [activeTab, setActiveTab] = useState<'ai' | 'local' | 'shortcuts' | 'system'>('ai');
  const [aiForm, setAiForm] = useState<AiProviderSettings>(aiSettings);
  const [aiSaved, setAiSaved] = useState(false);

  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [testResults, setTestResults] = useState<Record<string, ProviderTestResult>>({});
  const [fetchedModels, setFetchedModels] = useState<string[]>([]);
  const [fetchingModels, setFetchingModels] = useState(false);

  const [downloadingModelId, setDownloadingModelId] = useState<string | null>(null);
  const [shortcutDraft, setShortcutDraft] = useState(aiSettings.hover_shortcut || 'Alt+Space');
  const [shortcutFeedback, setShortcutFeedback] = useState<{ ok: boolean; msg: string } | null>(null);
  const [vtShortcutDraft, setVtShortcutDraft] = useState(
    aiSettings.voice_typing_shortcut || 'Alt+Shift+D',
  );
  const [vtFeedback, setVtFeedback] = useState<{ ok: boolean; msg: string } | null>(null);

  const [feedName, setFeedName] = useState('');
  const [feedUrl, setFeedUrl] = useState('');
  const [syncingCal, setSyncingCal] = useState(false);

  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [refreshingPerms, setRefreshingPerms] = useState(false);

  useEffect(() => {
    setAiForm(aiSettings);
    setShortcutDraft(aiSettings.hover_shortcut || 'Alt+Space');
    setVtShortcutDraft(aiSettings.voice_typing_shortcut || 'Alt+Shift+D');
  }, [aiSettings]);

  const modelSuggestions = Array.from(
    new Set([
      ...fetchedModels,
      ...(PROVIDER_PRESET_MODELS[aiForm.llm_provider] ?? []),
      ...(aiForm.llm_provider === 'local_ollama' ? ollamaModels : []),
      ...Object.values(PROVIDER_DEFAULT_MODELS),
    ]),
  );

  const handleProviderChange = (nextProvider: string) => {
    const defaultModel = PROVIDER_DEFAULT_MODELS[nextProvider] ?? aiForm.llm_model;
    setAiForm({
      ...aiForm,
      llm_provider: nextProvider,
      llm_model: defaultModel,
    });
    setFetchedModels([]);
  };

  const handleSaveAi = async () => {
    await onSaveAiSettings(aiForm);
    setAiSaved(true);
    setTimeout(() => setAiSaved(false), 1800);
  };

  const handleApplyShortcut = async (nextSpec: string) => {
    setShortcutDraft(nextSpec);
    setShortcutFeedback(null);
    try {
      const display = await onSetGlobalShortcut(nextSpec);
      setShortcutFeedback({
        ok: true,
        msg: lang === 'de' ? `Aktiv: ${display}` : `Active: ${display}`,
      });
    } catch (err) {
      setShortcutFeedback({
        ok: false,
        msg: err instanceof Error ? err.message : String(err),
      });
    }
  };

  const handleApplyVtShortcut = async (nextSpec: string) => {
    setVtShortcutDraft(nextSpec);
    setVtFeedback(null);
    try {
      const display = await onSetVoiceTypingShortcut(nextSpec);
      setVtFeedback({
        ok: true,
        msg: lang === 'de' ? `Aktiv: ${display}` : `Active: ${display}`,
      });
    } catch (err) {
      setVtFeedback({
        ok: false,
        msg: err instanceof Error ? err.message : String(err),
      });
    }
  };

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">{lang === 'de' ? 'Einstellungen' : 'Settings'}</h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'KI-Router (OpenRouter, PrimaLabs, Claude, OpenAI, Gemini), API-Tresor, lokale Modelle, Voice Typing & macOS-System.'
              : 'AI router (OpenRouter, PrimaLabs, Claude, OpenAI, Gemini), API key vault, local models, Voice Typing, and macOS system.'}
          </p>
        </div>

        <div className="tab-strip">
          <button
            type="button"
            id="settings-tab-ai"
            className={`tab-btn ${activeTab === 'ai' ? 'active' : ''}`}
            onClick={() => setActiveTab('ai')}
          >
            {lang === 'de' ? 'KI-Router & API-Keys' : 'AI Router & Keys'}
          </button>
          <button
            type="button"
            id="settings-tab-local"
            className={`tab-btn ${activeTab === 'local' ? 'active' : ''}`}
            onClick={() => setActiveTab('local')}
          >
            {lang === 'de' ? 'Lokale Modelle' : 'Local Models'}
          </button>
          <button
            type="button"
            id="settings-tab-shortcuts"
            className={`tab-btn ${activeTab === 'shortcuts' ? 'active' : ''}`}
            onClick={() => setActiveTab('shortcuts')}
          >
            {lang === 'de' ? 'Shortcuts, Voice & Kalender' : 'Shortcuts, Voice & Calendar'}
          </button>
          <button
            type="button"
            id="settings-tab-system"
            className={`tab-btn ${activeTab === 'system' ? 'active' : ''}`}
            onClick={() => setActiveTab('system')}
          >
            {lang === 'de' ? 'System & Updates' : 'System & Updates'}
          </button>
        </div>
      </header>

      {/* TAB 1: AI Router & API Key Vault */}
      {activeTab === 'ai' && (
        <div className="two-col-grid" style={{ alignItems: 'start' }}>
          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 13 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Cpu size={15} />
                {lang === 'de' ? 'KI-Router & Transkription' : 'AI Router & Transcription'}
              </span>
              {aiSaved && <span className="pill pill-sage">Gespeichert</span>}
            </h2>

            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              LLM-Provider (Streaming)
              <select
                id="llm-provider-select"
                className="select-field"
                style={{ marginTop: 4 }}
                value={aiForm.llm_provider}
                onChange={(e) => handleProviderChange(e.target.value)}
              >
                <option value="hybrid_auto">
                  Hybrid Auto (Claude → OpenRouter → PrimaLabs → OpenAI → Gemini → Ollama)
                </option>
                <option value="openrouter">OpenRouter (Multi-Model Gateway)</option>
                <option value="primalabs">PrimaLabs (Serverless Open-Weight · Zero Data Retention)</option>
                <option value="anthropic">Anthropic Claude (Streaming)</option>
                <option value="openai">OpenAI GPT (Streaming)</option>
                <option value="gemini">Google Gemini (Streaming)</option>
                <option value="local_ollama">100% Lokal via Ollama</option>
              </select>
            </label>

            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                <label htmlFor="llm-model-input" style={{ fontSize: 12.5, fontWeight: 600 }}>
                  LLM-Modellname
                </label>
                <button
                  type="button"
                  className="mini-btn"
                  disabled={fetchingModels}
                  onClick={async () => {
                    setFetchingModels(true);
                    try {
                      const list = await onFetchProviderModels(aiForm.llm_provider);
                      setFetchedModels(list);
                    } finally {
                      setFetchingModels(false);
                    }
                  }}
                >
                  {fetchingModels ? 'Lade…' : 'Modelle vom Provider abrufen'}
                </button>
              </div>
              <input
                id="llm-model-input"
                className="input-field"
                style={{ fontFamily: 'var(--font-mono)', fontSize: 12.5 }}
                value={aiForm.llm_model}
                onChange={(e) => setAiForm({ ...aiForm, llm_model: e.target.value })}
                list="provider-models-datalist"
              />
              <datalist id="provider-models-datalist">
                {modelSuggestions.map((m) => (
                  <option key={m} value={m} />
                ))}
              </datalist>
              {fetchedModels.length > 0 && (
                <div style={{ display: 'flex', gap: 5, flexWrap: 'wrap', marginTop: 8 }}>
                  {fetchedModels.slice(0, 8).map((m) => (
                    <button
                      key={m}
                      type="button"
                      className={`mini-btn ${aiForm.llm_model === m ? 'active-choice' : ''}`}
                      onClick={() => setAiForm({ ...aiForm, llm_model: m })}
                    >
                      {m}
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Spracherkennung (STT)
                <select
                  id="stt-provider-select"
                  className="select-field"
                  style={{ marginTop: 4 }}
                  value={aiForm.stt_provider}
                  onChange={(e) => setAiForm({ ...aiForm, stt_provider: e.target.value })}
                >
                  <option value="local_whisper">Lokal: Whisper.cpp (Apple Metal)</option>
                  <option value="deepgram">Cloud: Deepgram Nova-3 (inkl. Diarisierung)</option>
                  <option value="openai_audio">Cloud: OpenAI Whisper</option>
                  <option value="groq_whisper">Cloud: Groq Whisper Large-v3-Turbo</option>
                </select>
              </label>

              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Semantische Embeddings
                <select
                  className="select-field"
                  style={{ marginTop: 4 }}
                  value={aiForm.embedding_provider}
                  onChange={(e) => setAiForm({ ...aiForm, embedding_provider: e.target.value })}
                >
                  <option value="local_hybrid">Offline Multilingual Hash-256 (0.2ms)</option>
                  <option value="ollama">Ollama Lokal (nomic-embed-text)</option>
                  <option value="openai">OpenAI text-embedding-3-small</option>
                </select>
              </label>
            </div>

            <button
              type="button"
              id="save-ai-settings-btn"
              className="btn btn-primary"
              style={{ alignSelf: 'flex-start' }}
              onClick={handleSaveAi}
            >
              {lang === 'de' ? 'KI-Router speichern' : 'Save AI Router'}
            </button>
          </section>

          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <KeyRound size={15} />
                {lang === 'de' ? 'API-Schlüssel-Tresor' : 'API Key Vault'}
              </span>
              <span className="pill pill-sage">{telemetry.keychain_backend}</span>
            </h2>
            <p style={{ margin: 0, fontSize: 12.2, color: 'var(--text-secondary)' }}>
              {lang === 'de'
                ? 'Schlüssel werden verschlüsselt gespeichert und niemals im Klartext zurückgegeben.'
                : 'Keys are stored encrypted and never returned in plaintext.'}
            </p>

            {KEY_PROVIDERS.map((prov) => {
              const isSet = Boolean(aiForm[prov.field]);
              const testRes = testResults[prov.id];
              return (
                <div key={prov.id} className="transcript-row" style={{ gap: 7 }}>
                  <div className="transcript-meta">
                    <strong style={{ fontSize: 12.8 }}>{prov.label}</strong>
                    <span className={`pill ${isSet ? 'pill-sage' : 'pill-neutral'}`}>
                      {isSet ? 'Gespeichert' : 'Offen'}
                    </span>
                  </div>
                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                    <input
                      id={`apikey-input-${prov.id}`}
                      type="password"
                      className="input-field"
                      style={{ flex: 1, minWidth: 150, fontFamily: 'var(--font-mono)', fontSize: 12 }}
                      placeholder={isSet ? '•••••••• (neu eingeben zum Überschreiben)' : prov.placeholder}
                      value={keyDrafts[prov.id] ?? ''}
                      onChange={(e) => setKeyDrafts({ ...keyDrafts, [prov.id]: e.target.value })}
                    />
                    <button
                      type="button"
                      className="btn"
                      onClick={async () => {
                        const val = (keyDrafts[prov.id] ?? '').trim();
                        if (!val) return;
                        await onSetApiKey(prov.id, val);
                        setKeyDrafts({ ...keyDrafts, [prov.id]: '' });
                      }}
                    >
                      Speichern
                    </button>
                    {isSet && (
                      <button
                        type="button"
                        className="mini-btn"
                        onClick={async () => {
                          const res = await onTestProvider(prov.id);
                          setTestResults({ ...testResults, [prov.id]: res });
                        }}
                      >
                        Test
                      </button>
                    )}
                    {isSet && (
                      <button
                        type="button"
                        className="mini-btn"
                        title="Schlüssel entfernen"
                        onClick={() => onDeleteApiKey(prov.id)}
                      >
                        <Trash2 size={12} />
                      </button>
                    )}
                  </div>
                  {testRes && (
                    <div
                      className="mono-ts"
                      style={{ color: testRes.ok ? 'var(--accent-sage)' : 'var(--accent-terracotta)' }}
                    >
                      {testRes.ok ? '✓ ' : '✗ '}
                      {testRes.message} ({testRes.latency_ms} ms)
                    </div>
                  )}
                </div>
              );
            })}
          </section>
        </div>
      )}

      {/* TAB 2: Local Models (Whisper Metal & Ollama) */}
      {activeTab === 'local' && (
        <div className="two-col-grid">
          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 11 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Sparkles size={15} />
                Lokale Whisper-Modelle (Apple Metal)
              </span>
              <span className={`pill ${telemetry.whisper_cli_detected ? 'pill-sage' : 'pill-ochre'}`}>
                {telemetry.whisper_cli_detected ? 'whisper-cli bereit' : 'whisper-cli fehlt'}
              </span>
            </h2>
            <p style={{ margin: 0, fontSize: 12.5, color: 'var(--text-secondary)' }}>
              {telemetry.whisper_cli_detected
                ? 'Lokale Apple-Silicon-Transkription ist startklar. Lade ein Modell herunter und aktiviere es für 100% lokale Meeting-Transkripte.'
                : 'Installiere whisper-cpp (brew install whisper-cpp) oder nutze Deepgram/OpenAI/Groq unter KI-Router & API-Keys.'}
            </p>
            {whisperModels.map((m) => {
              const isSelected = aiForm.whisper_model_id === m.id;
              return (
                <div key={m.id} className="transcript-row">
                  <div className="transcript-meta">
                    <div>
                      <strong style={{ fontSize: 13 }}>{m.name}</strong>
                      <div className="mono-ts">
                        {m.size_mb} MB · {m.language_support}
                      </div>
                    </div>
                    <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                      {m.installed ? (
                        <>
                          {isSelected ? (
                            <span className="pill pill-sage">
                              <CheckCircle2 size={11} />
                              Aktiv
                            </span>
                          ) : (
                            <button
                              type="button"
                              className="mini-btn"
                              onClick={() =>
                                onSaveAiSettings({
                                  ...aiForm,
                                  stt_provider: 'local_whisper',
                                  whisper_model_id: m.id,
                                })
                              }
                            >
                              Aktivieren
                            </button>
                          )}
                        </>
                      ) : (
                        <button
                          type="button"
                          className="btn"
                          disabled={downloadingModelId !== null}
                          onClick={async () => {
                            setDownloadingModelId(m.id);
                            try {
                              await onDownloadWhisperModel(m.id);
                              await onSaveAiSettings({
                                ...aiForm,
                                stt_provider: 'local_whisper',
                                whisper_model_id: m.id,
                              });
                            } finally {
                              setDownloadingModelId(null);
                            }
                          }}
                        >
                          <Download size={13} />
                          {downloadingModelId === m.id || m.downloading
                            ? `${m.progress_pct}%…`
                            : 'Herunterladen'}
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              );
            })}

            <div
              className="transcript-row"
              style={{ marginTop: 4, gap: 8, background: 'var(--bg-surface-subtle)' }}
            >
              <div className="transcript-meta">
                <strong style={{ fontSize: 12.8 }}>
                  {lang === 'de'
                    ? 'Live-STT Latenz (VAD-Pausen-Flush)'
                    : 'Live STT Latency (VAD Pause Flush)'}
                </strong>
                <span className="pill pill-sage">
                  {lang === 'de' ? 'Max.' : 'Max'} {aiForm.stt_max_chunk_secs || 4}s
                </span>
              </div>
              <p style={{ margin: 0, fontSize: 12, color: 'var(--text-secondary)' }}>
                {lang === 'de'
                  ? 'backtext flusht gesprochene Sätze automatisch nach 0,7 s Sprechpause (1,5–2 s Latenz) oder spätestens nach dem gewählten Maximal-Intervall und trennt Remote-Sprecher pro Satz per 256-dim Voiceprint.'
                  : 'backtext flushes speech automatically after a 0.7s pause (~1.5–2s latency) or at most after the selected interval, separating remote speakers per utterance via 256-dim voiceprints.'}
              </p>
              <div style={{ display: 'flex', gap: 6 }}>
                {[3, 4, 6].map((sec) => (
                  <button
                    key={sec}
                    type="button"
                    id={`stt-chunk-btn-${sec}`}
                    className={`mini-btn ${(aiForm.stt_max_chunk_secs || 4) === sec ? 'active-choice' : ''}`}
                    onClick={() =>
                      onSaveAiSettings({ ...aiForm, stt_max_chunk_secs: sec })
                    }
                  >
                    {sec}s {sec === 4 ? (lang === 'de' ? '(Empfohlen)' : '(Recommended)') : ''}
                  </button>
                ))}
              </div>
            </div>
          </section>

          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Cpu size={15} />
                Ollama (Lokale LLMs & Embeddings)
              </span>
              <span className="mono-ts">{ollamaModels.length} erkannt</span>
            </h2>

            <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
              <label style={{ fontSize: 12.5, fontWeight: 600, flex: 1 }}>
                Ollama Endpoint URL
                <input
                  className="input-field"
                  style={{ marginTop: 4, fontFamily: 'var(--font-mono)', fontSize: 12 }}
                  value={aiForm.ollama_url}
                  onChange={(e) => setAiForm({ ...aiForm, ollama_url: e.target.value })}
                />
              </label>
              <button
                type="button"
                className="btn"
                onClick={async () => {
                  await onSaveAiSettings(aiForm);
                  const res = await onTestProvider('ollama');
                  setTestResults({ ...testResults, ollama: res });
                }}
              >
                Verbindung testen
              </button>
            </div>

            {testResults.ollama && (
              <div
                className="mono-ts"
                style={{ color: testResults.ollama.ok ? 'var(--accent-sage)' : 'var(--accent-terracotta)' }}
              >
                {testResults.ollama.message}
              </div>
            )}

            {ollamaModels.length > 0 ? (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-secondary)' }}>
                  Installierte lokale Modelle:
                </div>
                {ollamaModels.map((m) => (
                  <div
                    key={m}
                    className="transcript-row"
                    style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}
                  >
                    <span className="mono-ts" style={{ color: 'var(--text-primary)' }}>
                      {m}
                    </span>
                    <button
                      type="button"
                      className={`mini-btn ${
                        aiForm.llm_provider === 'local_ollama' && aiForm.llm_model === m
                          ? 'active-choice'
                          : ''
                      }`}
                      onClick={() =>
                        onSaveAiSettings({
                          ...aiForm,
                          llm_provider: 'local_ollama',
                          llm_model: m,
                        })
                      }
                    >
                      Als LLM nutzen
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <div className="transcript-row" style={{ fontSize: 12.5, color: 'var(--text-secondary)' }}>
                Kein lokaler Ollama-Server erkannt. Starte Ollama (z. B. <code>ollama pull qwen3:8b</code>), um 100% offline zu arbeiten.
              </div>
            )}
          </section>
        </div>
      )}

      {/* TAB 3: Hover Shortcut, System-Wide Voice Typing & Calendar */}
      {activeTab === 'shortcuts' && (
        <div className="two-col-grid" style={{ alignItems: 'start' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
            <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <h2 className="section-heading">
                <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                  <Command size={15} />
                  Systemweiter Hover-Shortcut
                </span>
                <span className="kbd">{telemetry.global_shortcut}</span>
              </h2>
              <p style={{ margin: 0, fontSize: 12.5, color: 'var(--text-secondary)' }}>
                Öffnet das vor Screen-Sharing geschützte Hover-Overlay über jeder macOS-App.
              </p>

              <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                {SHORTCUT_PRESETS.map((preset) => (
                  <button
                    key={preset.value}
                    type="button"
                    className={`mini-btn ${shortcutDraft === preset.value ? 'active-choice' : ''}`}
                    onClick={() => handleApplyShortcut(preset.value)}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>

              <div style={{ display: 'flex', gap: 8 }}>
                <input
                  id="hover-shortcut-input"
                  className="input-field"
                  style={{ fontFamily: 'var(--font-mono)', fontSize: 12.5 }}
                  value={shortcutDraft}
                  onChange={(e) => setShortcutDraft(e.target.value)}
                  placeholder="z. B. Alt+Space oder CommandOrControl+Shift+Space"
                />
                <button
                  type="button"
                  id="apply-shortcut-btn"
                  className="btn btn-primary"
                  onClick={() => handleApplyShortcut(shortcutDraft)}
                >
                  Übernehmen
                </button>
              </div>

              {shortcutFeedback && (
                <div
                  className="mono-ts"
                  style={{
                    color: shortcutFeedback.ok ? 'var(--accent-sage)' : 'var(--accent-terracotta)',
                  }}
                >
                  {shortcutFeedback.msg}
                </div>
              )}
            </section>

            {/* System-Wide Voice Typing Card (Feature 2.7) */}
            <section className="surface-sheet" id="voice-typing-settings-card" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <h2 className="section-heading">
                <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                  <Mic size={15} style={{ color: 'var(--accent-terracotta)' }} />
                  {lang === 'de' ? 'Systemweites Voice Typing (Diktat)' : 'System-Wide Voice Typing'}
                </span>
                <span className="kbd">{telemetry.voice_typing_shortcut || '⌥⇧ D'}</span>
              </h2>
              <p style={{ margin: 0, fontSize: 12.5, color: 'var(--text-secondary)' }}>
                {lang === 'de'
                  ? 'Drücke den Shortcut in einer beliebigen macOS-App (Slack, Mail, Cursor, Browser), sprich und drücke erneut: backtext transkribiert mit deinem Fachwörterbuch und fügt den Text direkt am Cursor ein.'
                  : 'Press the shortcut inside any macOS app to start/stop dictation: backtext transcribes using your custom dictionary and pastes directly at your cursor.'}
              </p>

              <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                {VOICE_TYPING_PRESETS.map((preset) => (
                  <button
                    key={preset.value}
                    type="button"
                    className={`mini-btn ${vtShortcutDraft === preset.value ? 'active-choice' : ''}`}
                    onClick={() => handleApplyVtShortcut(preset.value)}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>

              <div style={{ display: 'flex', gap: 8 }}>
                <input
                  id="voice-typing-shortcut-input"
                  className="input-field"
                  style={{ fontFamily: 'var(--font-mono)', fontSize: 12.5 }}
                  value={vtShortcutDraft}
                  onChange={(e) => setVtShortcutDraft(e.target.value)}
                  placeholder="z. B. Alt+Shift+D"
                />
                <button
                  type="button"
                  id="apply-vt-shortcut-btn"
                  className="btn btn-primary"
                  onClick={() => handleApplyVtShortcut(vtShortcutDraft)}
                >
                  Übernehmen
                </button>
              </div>

              {vtFeedback && (
                <div
                  className="mono-ts"
                  style={{
                    color: vtFeedback.ok ? 'var(--accent-sage)' : 'var(--accent-terracotta)',
                  }}
                >
                  {vtFeedback.msg}
                </div>
              )}

              <label style={{ display: 'flex', gap: 9, alignItems: 'center', fontSize: 12.8, cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={aiForm.voice_typing_ai_polish !== false}
                  onChange={(e) =>
                    onSaveAiSettings({ ...aiForm, voice_typing_ai_polish: e.target.checked })
                  }
                />
                {lang === 'de'
                  ? 'KI-Feinschliff (Zeichensetzung, Füllwörter entfernen & Wörterbuch anwenden)'
                  : 'AI polish (punctuation, filler removal & custom dictionary)'}
              </label>

              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <button
                  type="button"
                  id="test-voice-typing-btn"
                  className={voiceTypingActive ? 'btn btn-recording' : 'btn'}
                  onClick={onToggleVoiceTyping}
                >
                  {voiceTypingActive ? <Square size={12} /> : <Mic size={13} />}
                  {voiceTypingActive
                    ? lang === 'de'
                      ? 'Diktat stoppen & einfügen'
                      : 'Stop & Paste Dictation'
                    : lang === 'de'
                      ? 'Voice Typing jetzt testen'
                      : 'Test Voice Typing Now'}
                </button>
                <span className="mono-ts">{dictations.length} Diktate gespeichert</span>
              </div>

              {dictations.length > 0 && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 160, overflowY: 'auto' }}>
                  {dictations.slice(0, 5).map((d) => (
                    <div key={d.id} className="transcript-row" style={{ padding: '7px 10px' }}>
                      <div className="transcript-meta">
                        <span className="mono-ts">{d.target_app}</span>
                        <span className="mono-ts">
                          {new Date(d.created_at).toLocaleTimeString([], {
                            hour: '2-digit',
                            minute: '2-digit',
                          })}
                        </span>
                      </div>
                      <div style={{ fontSize: 12.5 }}>{d.cleaned_text}</div>
                    </div>
                  ))}
                </div>
              )}
            </section>

            <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <h2 className="section-heading">Meeting-Verhalten</h2>
              <label style={{ display: 'flex', gap: 9, alignItems: 'center', fontSize: 13, cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={profile.auto_record_meetings}
                  onChange={(e) =>
                    onSaveProfile({ ...profile, auto_record_meetings: e.target.checked })
                  }
                />
                Meetings bei erkanntem Call-/Mikrofon-Signal automatisch starten
              </label>
              <label style={{ display: 'flex', gap: 9, alignItems: 'center', fontSize: 13, cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={profile.consent_reminder_enabled}
                  onChange={(e) =>
                    onSaveProfile({ ...profile, consent_reminder_enabled: e.target.checked })
                  }
                />
                Einwilligungs-Erinnerung (Consent-Hinweis) bei Meeting-Start anzeigen
              </label>
            </section>
          </div>

          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div className="section-heading" style={{ marginBottom: 0 }}>
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Calendar size={15} />
                Kalender (EventKit + ICS-Feeds)
              </span>
              <button
                type="button"
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
                <RefreshCw size={13} />
                {syncingCal ? 'Synchronisiere…' : 'Synchronisieren'}
              </button>
            </div>
            <p style={{ margin: 0, fontSize: 12.5, color: 'var(--text-secondary)' }}>
              Apple Kalender wird nativ per EventKit gelesen. Optional kannst du geheime <code>.ics</code>-Links (Google Calendar, Outlook) hinzufügen.
            </p>

            {calendarFeeds.length > 0 && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 7 }}>
                {calendarFeeds.map((f) => (
                  <div
                    key={f.id}
                    className="transcript-row"
                    style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}
                  >
                    <div>
                      <strong>{f.name}</strong>
                      <div className="mono-ts">{f.ics_url}</div>
                    </div>
                    <button type="button" className="mini-btn" onClick={() => onDeleteCalendarFeed(f.id)}>
                      <Trash2 size={12} />
                    </button>
                  </div>
                ))}
              </div>
            )}

            <form
              onSubmit={async (e) => {
                e.preventDefault();
                if (!feedUrl.trim()) return;
                await onAddCalendarFeed(feedName.trim() || 'Externer Kalender', feedUrl.trim());
                setFeedName('');
                setFeedUrl('');
              }}
              style={{ display: 'flex', flexDirection: 'column', gap: 8 }}
            >
              <input
                className="input-field"
                value={feedName}
                onChange={(e) => setFeedName(e.target.value)}
                placeholder="Kalendername (z. B. Arbeit)"
              />
              <input
                className="input-field"
                style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}
                value={feedUrl}
                onChange={(e) => setFeedUrl(e.target.value)}
                placeholder="https://calendar.google.com/calendar/ical/.../basic.ics"
              />
              <button type="submit" className="btn" style={{ alignSelf: 'flex-start' }}>
                <Plus size={14} />
                ICS-Feed verbinden
              </button>
            </form>
          </section>
        </div>
      )}

      {/* TAB 4: System, Permissions & Updates */}
      {activeTab === 'system' && (
        <div className="two-col-grid">
          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 11 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <ShieldCheck size={15} />
                macOS Berechtigungen
              </span>
              <button
                type="button"
                id="refresh-permissions-btn"
                className="mini-btn"
                disabled={refreshingPerms}
                onClick={async () => {
                  setRefreshingPerms(true);
                  try {
                    await onRefreshPermissions();
                  } finally {
                    setRefreshingPerms(false);
                  }
                }}
              >
                <RefreshCw size={11} style={{ marginRight: 4, verticalAlign: 'middle' }} />
                {refreshingPerms ? 'Prüfe…' : 'Status neu prüfen'}
              </button>
            </h2>

            {(
              [
                ['accessibility', 'Bedienungshilfen (Fenstertitel & Text)', permissions.accessibility_granted],
                ['screen', 'Bildschirmaufnahme (System-Audio & OCR)', permissions.screen_capture_granted],
                ['microphone', 'Mikrofon (Eigene Stimme im Meeting)', permissions.microphone_granted],
                ['calendar', 'Kalender (EventKit Termin-Abgleich)', permissions.calendar_granted],
              ] as const
            ).map(([kind, label, ok]) => (
              <div
                key={kind}
                className="transcript-row"
                style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}
              >
                <div>
                  <strong style={{ fontSize: 13 }}>{label}</strong>
                </div>
                <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                  <span className={`pill ${ok ? 'pill-sage' : 'pill-ochre'}`}>
                    {ok ? 'Erteilt' : 'Ausstehend'}
                  </span>
                  {!ok && (
                    <button
                      type="button"
                      className="mini-btn active-choice"
                      onClick={() => onRequestPermission(kind)}
                    >
                      Freigeben
                    </button>
                  )}
                </div>
              </div>
            ))}

            <p style={{ margin: 0, fontSize: 12, color: 'var(--text-tertiary)' }}>
              Hinweis nach einem App-Update (ad-hoc signiert): Falls eine bereits aktivierte Berechtigung als „Ausstehend“ erscheint, entferne backtext in den macOS Systemeinstellungen kurz mit „−“ und füge es mit „+“ erneut hinzu.
            </p>
          </section>

          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Lock size={15} />
                Sicherheit, Autostart & Updates
              </span>
              <span className="pill pill-sage">v0.5.0</span>
            </h2>

            <div className="transcript-row">
              <div className="transcript-meta">
                <strong>Datenbank-Verschlüsselung</strong>
                <span className={`pill ${telemetry.db_encrypted ? 'pill-sage' : 'pill-ochre'}`}>
                  {telemetry.db_encrypted ? 'SQLCipher AES-256 aktiv' : 'SQLite WAL'}
                </span>
              </div>
              <div className="mono-ts">Schlüssel-Tresor: {telemetry.keychain_backend}</div>
            </div>

            <label
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '10px 12px',
                borderRadius: 6,
                background: 'var(--bg-surface-subtle)',
                cursor: 'pointer',
              }}
            >
              <span style={{ fontSize: 13 }}>
                <strong>Autostart beim macOS-Login</strong> (LaunchAgent)
              </span>
              <input
                id="autostart-toggle"
                type="checkbox"
                checked={telemetry.autostart_enabled}
                onChange={(e) => onToggleAutostart(e.target.checked)}
              />
            </label>

            <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
              <button
                type="button"
                id="check-updates-btn"
                className="btn"
                disabled={checkingUpdate}
                onClick={async () => {
                  setCheckingUpdate(true);
                  try {
                    const res = await onCheckUpdates();
                    setUpdateInfo(res);
                  } finally {
                    setCheckingUpdate(false);
                  }
                }}
              >
                <RefreshCw size={13} />
                {checkingUpdate ? 'Prüfe Releases…' : 'Nach Updates suchen'}
              </button>
              {updateInfo && (
                <span className={`pill ${updateInfo.update_available ? 'pill-terracotta' : 'pill-sage'}`}>
                  {updateInfo.update_available
                    ? `Update v${updateInfo.latest_version} verfügbar`
                    : `Aktuell (v${updateInfo.current_version})`}
                </span>
              )}
            </div>

            {telemetry.demo_data_present && (
              <div style={{ paddingTop: 10, borderTop: '1px solid var(--border-subtle)' }}>
                <button type="button" className="btn" onClick={onClearDemoData}>
                  <Trash2 size={13} />
                  Beispieldaten leeren & sauber starten
                </button>
              </div>
            )}
          </section>
        </div>
      )}
    </div>
  );
};
