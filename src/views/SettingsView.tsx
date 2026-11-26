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
  UpdateInstallProgressPayload,
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
  updateInfo: UpdateCheckResult | null;
  updateProgress: UpdateInstallProgressPayload | null;
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
  onInstallUpdate: (downloadUrl: string, assetName?: string | null) => Promise<void>;
  onSetupPersistentSigning: () => Promise<{ ok: boolean; ready: boolean; message: string }>;
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
  updateInfo: externalUpdateInfo,
  updateProgress,
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
  onInstallUpdate,
  onSetupPersistentSigning,
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
  const [vtCustomSaved, setVtCustomSaved] = useState(false);

  const [feedName, setFeedName] = useState('');
  const [feedUrl, setFeedUrl] = useState('');
  const [syncingCal, setSyncingCal] = useState(false);

  const [localUpdateInfo, setLocalUpdateInfo] = useState<UpdateCheckResult | null>(
    externalUpdateInfo,
  );
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [installingUpdate, setInstallingUpdate] = useState(false);
  const [signingBusy, setSigningBusy] = useState(false);
  const [signingFeedback, setSigningFeedback] = useState<{ ok: boolean; msg: string } | null>(null);
  const [refreshingPerms, setRefreshingPerms] = useState(false);

  const updateInfo = localUpdateInfo ?? externalUpdateInfo;

  useEffect(() => {
    if (externalUpdateInfo) {
      setLocalUpdateInfo(externalUpdateInfo);
    }
  }, [externalUpdateInfo]);

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
              <span
                className={`pill ${
                  telemetry.whisper_server_detected || telemetry.whisper_cli_detected
                    ? 'pill-sage'
                    : 'pill-ochre'
                }`}
              >
                {telemetry.whisper_server_detected
                  ? 'Warm-Server bereit (kühl)'
                  : telemetry.whisper_cli_detected
                    ? 'whisper-cli bereit'
                    : 'whisper-cli fehlt'}
              </span>
            </h2>
            <p style={{ margin: 0, fontSize: 12.5, color: 'var(--text-secondary)' }}>
              {telemetry.whisper_server_detected
                ? 'Persistenter whisper-server (Apple Metal) gebündelt: Modell wird pro Meeting nur einmal geladen (statt 30×/Min) — deutlich kühler & schneller inkl. Pure-Rust Lautsprecher-Echo-Filter ohne Lautstärke-Absenkung.'
                : telemetry.whisper_cli_detected
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
                    ? 'Live-STT Latenz & Energiebilanz (VAD-Pausen-Flush)'
                    : 'Live STT Latency & Thermal Balance (VAD Pause Flush)'}
                </strong>
                <span className="pill pill-sage">
                  {lang === 'de' ? 'Max.' : 'Max'} {aiForm.stt_max_chunk_secs || 5}s
                </span>
              </div>
              <p style={{ margin: 0, fontSize: 12, color: 'var(--text-secondary)' }}>
                {lang === 'de'
                  ? 'backtext flusht gesprochene Sätze nach 0,7 s Sprechpause oder spätestens nach dem gewählten Intervall. 5 s bietet optimale Balance aus Echtzeit-Gefühl und kühler MacBook-Temperatur.'
                  : 'backtext flushes speech after a 0.7s pause or at most after the selected interval. 5s balances responsiveness with cool MacBook thermals.'}
              </p>
              <div style={{ display: 'flex', gap: 6 }}>
                {[4, 5, 8].map((sec) => (
                  <button
                    key={sec}
                    type="button"
                    id={`stt-chunk-btn-${sec}`}
                    className={`mini-btn ${(aiForm.stt_max_chunk_secs || 5) === sec ? 'active-choice' : ''}`}
                    onClick={() =>
                      onSaveAiSettings({ ...aiForm, stt_max_chunk_secs: sec })
                    }
                  >
                    {sec}s {sec === 5 ? (lang === 'de' ? '(Empfohlen)' : '(Recommended)') : ''}
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

              {/* Granular Voice Typing AI Polish Control (v0.6.0) */}
              <div
                className="transcript-row"
                id="voice-typing-polish-controls"
                style={{ gap: 9, background: 'var(--bg-surface-subtle)' }}
              >
                <div className="transcript-meta">
                  <strong style={{ fontSize: 12.5 }}>
                    {lang === 'de' ? 'KI-Feinschliff-Engine' : 'AI Polish Engine'}
                  </strong>
                  <span className="pill pill-neutral">
                    {(aiForm.voice_typing_polish_mode || (aiForm.voice_typing_ai_polish ? 'router' : 'off')) === 'off'
                      ? 'Nur Wörterbuch'
                      : aiForm.voice_typing_polish_mode === 'local_ollama'
                        ? `Lokal (${aiForm.voice_typing_ollama_model || 'qwen3:4b'})`
                        : 'Standard KI-Router'}
                  </span>
                </div>

                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                  {(
                    [
                      ['off', lang === 'de' ? 'Aus (nur Wörterbuch)' : 'Off (Dictionary only)'],
                      ['local_ollama', lang === 'de' ? 'Lokal via Ollama (schnell & offline)' : 'Local via Ollama'],
                      ['router', lang === 'de' ? 'Standard KI-Router' : 'Default AI Router'],
                    ] as const
                  ).map(([modeKey, label]) => {
                    const currentMode =
                      aiForm.voice_typing_polish_mode ||
                      (aiForm.voice_typing_ai_polish === false ? 'off' : 'router');
                    return (
                      <button
                        key={modeKey}
                        type="button"
                        id={`vt-polish-mode-${modeKey}`}
                        className={`mini-btn ${currentMode === modeKey ? 'active-choice' : ''}`}
                        onClick={() =>
                          onSaveAiSettings({
                            ...aiForm,
                            voice_typing_ai_polish: modeKey !== 'off',
                            voice_typing_polish_mode: modeKey,
                          })
                        }
                      >
                        {label}
                      </button>
                    );
                  })}
                </div>

                {aiForm.voice_typing_polish_mode === 'local_ollama' && (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                    <label style={{ fontSize: 12, fontWeight: 600 }}>
                      {lang === 'de'
                        ? 'Lokales Ollama-Modell für Diktat (leichtgewichtige 3B–4B Modelle empfohlen)'
                        : 'Local Ollama Model for Dictation (3B–4B recommended)'}
                      <input
                        id="vt-ollama-model-input"
                        className="input-field"
                        style={{ marginTop: 4, fontFamily: 'var(--font-mono)', fontSize: 12 }}
                        value={aiForm.voice_typing_ollama_model ?? 'qwen3:4b'}
                        onChange={(e) =>
                          setAiForm({ ...aiForm, voice_typing_ollama_model: e.target.value })
                        }
                        onBlur={() => onSaveAiSettings(aiForm)}
                        placeholder="qwen3:4b"
                      />
                    </label>
                    <div style={{ display: 'flex', gap: 5, flexWrap: 'wrap' }}>
                      {Array.from(
                        new Set([...ollamaModels.slice(0, 4), 'qwen3:4b', 'gemma3:4b', 'llama3.2:3b']),
                      ).map((m) => (
                        <button
                          key={m}
                          type="button"
                          className={`mini-btn ${(aiForm.voice_typing_ollama_model ?? 'qwen3:4b') === m ? 'active-choice' : ''}`}
                          onClick={() =>
                            onSaveAiSettings({ ...aiForm, voice_typing_ollama_model: m })
                          }
                        >
                          {m}
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {(aiForm.voice_typing_polish_mode || (aiForm.voice_typing_ai_polish === false ? 'off' : 'router')) !== 'off' && (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                    <div style={{ fontSize: 12, fontWeight: 600 }}>
                      {lang === 'de' ? 'Feinschliff-Stil' : 'Polish Style'}
                    </div>
                    <div style={{ display: 'flex', gap: 5, flexWrap: 'wrap' }}>
                      {(
                        [
                          ['minimal', lang === 'de' ? 'Minimal (nur Zeichensetzung & Füllwörter)' : 'Minimal'],
                          ['clean', lang === 'de' ? 'Klar & Professionell' : 'Clean & Pro'],
                          ['bullets', lang === 'de' ? 'Stichpunkte' : 'Bullet Points'],
                          ['custom', lang === 'de' ? 'Eigener Prompt' : 'Custom Prompt'],
                        ] as const
                      ).map(([styleKey, label]) => {
                        const curStyle = aiForm.voice_typing_polish_style || 'minimal';
                        return (
                          <button
                            key={styleKey}
                            type="button"
                            id={`vt-polish-style-${styleKey}`}
                            className={`mini-btn ${curStyle === styleKey ? 'active-choice' : ''}`}
                            onClick={() =>
                              onSaveAiSettings({ ...aiForm, voice_typing_polish_style: styleKey })
                            }
                          >
                            {label}
                          </button>
                        );
                      })}
                    </div>

                    {aiForm.voice_typing_polish_style === 'custom' && (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                        <textarea
                          id="vt-custom-prompt-input"
                          className="textarea-field"
                          rows={3}
                          style={{ fontSize: 12, fontFamily: 'var(--font-mono)' }}
                          value={aiForm.voice_typing_custom_prompt ?? ''}
                          onChange={(e) =>
                            setAiForm({ ...aiForm, voice_typing_custom_prompt: e.target.value })
                          }
                          placeholder={
                            lang === 'de'
                              ? 'z. B. Formuliere das Diktat als kurze, freundliche Slack-Nachricht auf Deutsch ohne Füllwörter.'
                              : 'e.g. Rewrite as a concise Slack message without fillers.'
                          }
                        />
                        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                          <button
                            type="button"
                            id="save-vt-custom-prompt-btn"
                            className="mini-btn active-choice"
                            onClick={async () => {
                              await onSaveAiSettings(aiForm);
                              setVtCustomSaved(true);
                              window.setTimeout(() => setVtCustomSaved(false), 1800);
                            }}
                          >
                            {lang === 'de' ? 'Prompt speichern' : 'Save Prompt'}
                          </button>
                          {vtCustomSaved && <span className="pill pill-sage">Gespeichert</span>}
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>

              {!permissions.accessibility_granted && (
                <div
                  className="transcript-row"
                  style={{
                    flexDirection: 'row',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    background: 'var(--accent-ochre-soft)',
                  }}
                >
                  <span style={{ fontSize: 12 }}>
                    {lang === 'de'
                      ? 'Für automatisches Einfügen in Word, Safari & externe Apps wird „Bedienungshilfen“ benötigt (In-Process ⌘V).'
                      : 'Accessibility permission is required to auto-paste into Word, Safari & external apps.'}
                  </span>
                  <button
                    type="button"
                    className="mini-btn active-choice"
                    onClick={() => onRequestPermission('accessibility')}
                  >
                    Freigeben
                  </button>
                </div>
              )}

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

      {/* TAB 4: System, Permissions & 1-Click In-App Updater */}
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
                ['accessibility', 'Bedienungshilfen (Fenstertitel, Text & Voice-Paste)', permissions.accessibility_granted],
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

            {/* Persistent Local Code-Signing Card (Preserves macOS TCC permissions across all future updates!) */}
            <div
              className="transcript-row"
              id="persistent-signing-card"
              style={{ marginTop: 4, gap: 8, background: 'var(--bg-surface-subtle)' }}
            >
              <div className="transcript-meta">
                <strong style={{ fontSize: 12.8 }}>
                  Dauerhafte Berechtigungen bei Updates (Lokale Signatur)
                </strong>
                <span
                  className={`pill ${telemetry.persistent_signing_ready ? 'pill-sage' : 'pill-ochre'}`}
                >
                  {telemetry.persistent_signing_ready
                    ? 'Zertifikat aktiv'
                    : 'Noch nicht eingerichtet'}
                </span>
              </div>
              <p style={{ margin: 0, fontSize: 12, color: 'var(--text-secondary)' }}>
                {telemetry.persistent_signing_ready
                  ? 'Das lokale Code-Signing-Zertifikat „backtext Local Signer“ ist im macOS-Schlüsselbund aktiv. Künftige In-App-Updates behalten automatisch alle Berechtigungen (Bedienungshilfen, Bildschirmaufnahme, Mikrofon).'
                  : 'Erstelle einmalig ein lokales Code-Signing-Zertifikat („backtext Local Signer“) in deinem macOS-Schlüsselbund. Künftige In-App-Updates signieren damit automatisch — du musst Berechtigungen danach nie wieder neu erteilen!'}
              </p>
              <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                <button
                  type="button"
                  id="setup-persistent-signing-btn"
                  className="btn"
                  disabled={signingBusy}
                  onClick={async () => {
                    setSigningBusy(true);
                    setSigningFeedback(null);
                    try {
                      const res = await onSetupPersistentSigning();
                      setSigningFeedback({ ok: res.ok, msg: res.message });
                    } catch (err) {
                      setSigningFeedback({
                        ok: false,
                        msg: err instanceof Error ? err.message : String(err),
                      });
                    } finally {
                      setSigningBusy(false);
                    }
                  }}
                >
                  <ShieldCheck size={13} />
                  {signingBusy
                    ? 'Richte ein…'
                    : telemetry.persistent_signing_ready
                      ? 'App jetzt neu signieren'
                      : 'Lokales Signatur-Zertifikat einrichten'}
                </button>
              </div>
              {signingFeedback && (
                <div
                  className="mono-ts"
                  style={{
                    color: signingFeedback.ok ? 'var(--accent-sage)' : 'var(--accent-terracotta)',
                  }}
                >
                  {signingFeedback.msg}
                </div>
              )}
            </div>
          </section>

          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Lock size={15} />
                Sicherheit, Autostart & In-App-Updates
              </span>
              <span className="pill pill-sage">v0.6.1</span>
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

            {/* 1-Click In-App Auto-Updater Card */}
            <div
              className="transcript-row"
              id="in-app-updater-card"
              style={{ gap: 9, background: 'var(--bg-surface-subtle)' }}
            >
              <div className="transcript-meta">
                <strong style={{ fontSize: 12.8 }}>In-App Auto-Updater (GitHub Releases)</strong>
                {updateInfo && (
                  <span
                    className={`pill ${updateInfo.update_available ? 'pill-terracotta' : 'pill-sage'}`}
                  >
                    {updateInfo.update_available
                      ? `Neu: v${updateInfo.latest_version}`
                      : `Aktuell (v${updateInfo.current_version})`}
                  </span>
                )}
              </div>
              <p style={{ margin: 0, fontSize: 12, color: 'var(--text-secondary)' }}>
                Lädt neue macOS-Versionen direkt herunter, entfernt das Quarantine-Flag, signiert mit deinem lokalen Zertifikat, aktualisiert <code>/Applications/backtext.app</code> und startet automatisch neu.
              </p>

              <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                <button
                  type="button"
                  id="check-updates-btn"
                  className="btn"
                  disabled={checkingUpdate || installingUpdate}
                  onClick={async () => {
                    setCheckingUpdate(true);
                    try {
                      const res = await onCheckUpdates();
                      setLocalUpdateInfo(res);
                    } finally {
                      setCheckingUpdate(false);
                    }
                  }}
                >
                  <RefreshCw size={13} />
                  {checkingUpdate ? 'Prüfe Releases…' : 'Nach Updates suchen'}
                </button>

                {updateInfo?.update_available && updateInfo.download_url && (
                  <button
                    type="button"
                    id="install-update-btn"
                    className="btn btn-primary"
                    disabled={installingUpdate}
                    onClick={async () => {
                      if (!updateInfo.download_url) return;
                      setInstallingUpdate(true);
                      try {
                        await onInstallUpdate(updateInfo.download_url, updateInfo.asset_name);
                      } finally {
                        setInstallingUpdate(false);
                      }
                    }}
                  >
                    <Download size={13} />
                    {installingUpdate
                      ? 'Installiere Update…'
                      : `v${updateInfo.latest_version} jetzt installieren & neu starten`}
                  </button>
                )}
              </div>

              {updateProgress && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
                  <div className="update-progress-track">
                    <div
                      className="update-progress-fill"
                      style={{ width: `${Math.min(100, Math.max(4, updateProgress.progress_pct))}%` }}
                    />
                  </div>
                  <div
                    className="mono-ts"
                    style={{
                      color: updateProgress.error
                        ? 'var(--accent-terracotta)'
                        : 'var(--accent-sage)',
                    }}
                  >
                    {updateProgress.progress_pct}% · {updateProgress.message}
                  </div>
                </div>
              )}

              {updateInfo && updateInfo.release_notes && !updateProgress && (
                <div style={{ fontSize: 11.8, color: 'var(--text-tertiary)', whiteSpace: 'pre-wrap' }}>
                  {updateInfo.release_notes.slice(0, 260)}
                </div>
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
