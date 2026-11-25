import React, { useEffect, useState } from 'react';
import {
  Mic,
  Save,
  Plus,
  Trash2,
  Cpu,
  KeyRound,
  Download,
  Calendar,
  RefreshCw,
  CheckCircle2,
  Lock,
  Sparkles,
} from 'lucide-react';
import type {
  AiProviderSettings,
  CalendarFeedConfig,
  DaemonTelemetry,
  Language,
  LearnedPreference,
  ProviderTestResult,
  UpdateCheckResult,
  UserProfile,
  VocabularyEntry,
  WhisperModelInfo,
} from '../types';
import { t } from '../lib/i18n';

interface PersonalizationViewProps {
  lang: Language;
  profile: UserProfile;
  telemetry: DaemonTelemetry;
  vocabulary: VocabularyEntry[];
  learnedPreferences: LearnedPreference[];
  aiSettings: AiProviderSettings;
  whisperModels: WhisperModelInfo[];
  ollamaModels: string[];
  calendarFeeds: CalendarFeedConfig[];
  onSaveProfile: (profile: UserProfile) => Promise<void>;
  onEnrollSelfVoiceprint: () => Promise<void>;
  onAddVocabulary: (term: string, hint: string, category: string, projectTag: string) => Promise<void>;
  onDeleteVocabulary: (id: string) => Promise<void>;
  onToggleLearnedPref: (id: string, isActive: boolean) => Promise<void>;
  onSaveAiSettings: (settings: AiProviderSettings) => Promise<void>;
  onSetApiKey: (provider: string, apiKey: string) => Promise<void>;
  onDeleteApiKey: (provider: string) => Promise<void>;
  onTestProvider: (provider: string) => Promise<ProviderTestResult>;
  onDownloadWhisperModel: (modelId: string) => Promise<void>;
  onAddCalendarFeed: (name: string, icsUrl: string) => Promise<void>;
  onDeleteCalendarFeed: (id: string) => Promise<void>;
  onSyncCalendars: () => Promise<void>;
  onToggleAutostart: (enabled: boolean) => Promise<void>;
  onCheckUpdates: () => Promise<UpdateCheckResult>;
}

const KEY_PROVIDERS = [
  { id: 'anthropic', label: 'Anthropic Claude', field: 'anthropic_api_key_set', placeholder: 'sk-ant-api03-…' },
  { id: 'openai', label: 'OpenAI (GPT & Whisper)', field: 'openai_api_key_set', placeholder: 'sk-proj-…' },
  { id: 'gemini', label: 'Google Gemini', field: 'gemini_api_key_set', placeholder: 'AIza…' },
  { id: 'deepgram', label: 'Deepgram Nova-3 (STT + Diarisierung)', field: 'deepgram_api_key_set', placeholder: 'dg_…' },
  { id: 'groq', label: 'Groq Whisper (Ultra-Fast STT)', field: 'groq_api_key_set', placeholder: 'gsk_…' },
] as const;

export const PersonalizationView: React.FC<PersonalizationViewProps> = ({
  lang,
  profile,
  telemetry,
  vocabulary,
  learnedPreferences,
  aiSettings,
  whisperModels,
  ollamaModels,
  calendarFeeds,
  onSaveProfile,
  onEnrollSelfVoiceprint,
  onAddVocabulary,
  onDeleteVocabulary,
  onToggleLearnedPref,
  onSaveAiSettings,
  onSetApiKey,
  onDeleteApiKey,
  onTestProvider,
  onDownloadWhisperModel,
  onAddCalendarFeed,
  onDeleteCalendarFeed,
  onSyncCalendars,
  onToggleAutostart,
  onCheckUpdates,
}) => {
  const [form, setForm] = useState<UserProfile>(profile);
  const [projectsInput, setProjectsInput] = useState(profile.active_projects.join(', '));
  const [aiForm, setAiForm] = useState<AiProviderSettings>(aiSettings);
  const [vocTerm, setVocTerm] = useState('');
  const [vocHint, setVocHint] = useState('');
  const [vocProj, setVocProj] = useState('backtext');
  const [savedBanner, setSavedBanner] = useState(false);

  // Keychain API key draft inputs
  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [testResults, setTestResults] = useState<Record<string, ProviderTestResult>>({});
  const [calibrating, setCalibrating] = useState(false);
  const [downloadingModelId, setDownloadingModelId] = useState<string | null>(null);

  // Calendar ICS feed inputs
  const [feedName, setFeedName] = useState('');
  const [feedUrl, setFeedUrl] = useState('');

  // Update checker
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);

  useEffect(() => {
    setForm(profile);
    setProjectsInput(profile.active_projects.join(', '));
  }, [profile]);

  useEffect(() => {
    setAiForm(aiSettings);
  }, [aiSettings]);

  const handleProfileSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const projects = projectsInput
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
    await onSaveProfile({ ...form, active_projects: projects });
    setSavedBanner(true);
    setTimeout(() => setSavedBanner(false), 1800);
  };

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">
            {lang === 'de'
              ? 'Personalisierung, KI-Modelle & macOS System'
              : 'Personalization, AI Models & macOS System'}
          </h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Verwalte dein Nutzerprofil, echtes Stimmprofil, macOS Keychain API-Keys, lokale Whisper-Modelle, Kalender-Feeds und Login-Autostart.'
              : 'Configure user profile, acoustic voiceprint, macOS Keychain API keys, local Whisper models, calendar feeds, and login autostart.'}
          </p>
        </div>
      </header>

      {/* Row 1: Profile & Voiceprint + System Polish */}
      <div className="two-col-grid">
        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>{lang === 'de' ? 'Persönliches Profil & Sprache' : 'Identity, Language & Tone'}</span>
            {savedBanner && <span className="pill pill-sage">Gespeichert</span>}
          </h2>
          <form onSubmit={handleProfileSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 11 }}>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Name
                <input
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={form.display_name}
                  onChange={(e) => setForm({ ...form, display_name: e.target.value })}
                />
              </label>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Organisation
                <input
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={form.organization}
                  onChange={(e) => setForm({ ...form, organization: e.target.value })}
                />
              </label>
            </div>

            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              Rolle & Fokus
              <input
                className="input-field"
                style={{ marginTop: 4 }}
                value={form.role}
                onChange={(e) => setForm({ ...form, role: e.target.value })}
              />
            </label>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                {t(lang, 'languageLabel')}
                <select
                  id="ui-language-select"
                  className="select-field"
                  style={{ marginTop: 4 }}
                  value={form.ui_language}
                  onChange={(e) => setForm({ ...form, ui_language: e.target.value as Language })}
                >
                  <option value="de">Deutsch (Standard)</option>
                  <option value="en">English</option>
                </select>
              </label>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                {t(lang, 'outputLangLabel')}
                <select
                  className="select-field"
                  style={{ marginTop: 4 }}
                  value={form.preferred_output_language}
                  onChange={(e) =>
                    setForm({ ...form, preferred_output_language: e.target.value as Language })
                  }
                >
                  <option value="de">Deutsch</option>
                  <option value="en">English</option>
                </select>
              </label>
            </div>

            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              Aktive Projekte (kommagetrennt — für automatische Kontext-Zuordnung)
              <input
                className="input-field"
                style={{ marginTop: 4 }}
                value={projectsInput}
                onChange={(e) => setProjectsInput(e.target.value)}
              />
            </label>

            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              Bevorzugter Schreibstil
              <input
                className="input-field"
                style={{ marginTop: 4 }}
                value={form.writing_style}
                onChange={(e) => setForm({ ...form, writing_style: e.target.value })}
              />
            </label>

            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              Tonfall-Anweisungen für KI & MCP-Agenten
              <textarea
                className="textarea-field"
                rows={3}
                style={{ marginTop: 4 }}
                value={form.tone_instructions}
                onChange={(e) => setForm({ ...form, tone_instructions: e.target.value })}
              />
            </label>

            <label style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13 }}>
              <input
                type="checkbox"
                checked={form.auto_record_meetings}
                onChange={(e) => setForm({ ...form, auto_record_meetings: e.target.checked })}
              />
              Meetings bei erkanntem Audio-/Kalender-Signal automatisch starten
            </label>

            <label style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13 }}>
              <input
                type="checkbox"
                checked={form.consent_reminder_enabled}
                onChange={(e) => setForm({ ...form, consent_reminder_enabled: e.target.checked })}
              />
              Einwilligungs-Erinnerung (Consent-Hinweis) zu Gesprächsbeginn anzeigen
            </label>

            <button type="submit" id="save-profile-btn" className="btn btn-primary" style={{ alignSelf: 'flex-start' }}>
              <Save size={14} />
              {t(lang, 'saveProfile')}
            </button>
          </form>
        </section>

        {/* Right Column: Acoustic Voiceprint + macOS Autostart/Encryption/Updates */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <section className="surface-sheet">
            <h2 className="section-heading">
              <span>{lang === 'de' ? 'Akustisches Stimmprofil (MFCC & Pitch)' : 'Acoustic Self-Voiceprint'}</span>
              <span className={`pill ${profile.has_self_voiceprint ? 'pill-sage' : 'pill-ochre'}`}>
                {profile.has_self_voiceprint ? 'Kalibriert (256-dim)' : 'Nicht kalibriert'}
              </span>
            </h2>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginTop: 0 }}>
              {lang === 'de'
                ? 'Nimmt 5 Sekunden deiner echten Mikrofon-Stimme auf und berechnet lokal einen 256-dim MFCC-/Pitch-Stimmvektor zur sicheren Erkennung.'
                : 'Records 5s of your microphone voice locally to build a 256-dim MFCC/pitch speaker embedding.'}
            </p>
            <button
              type="button"
              id="enroll-voiceprint-btn"
              className="btn btn-sage"
              disabled={calibrating}
              onClick={async () => {
                setCalibrating(true);
                try {
                  await onEnrollSelfVoiceprint();
                } finally {
                  setCalibrating(false);
                }
              }}
            >
              <Mic size={14} />
              {calibrating
                ? lang === 'de'
                  ? 'Nehme 5s Stimmprobe auf…'
                  : 'Recording 5s voice sample…'
                : lang === 'de'
                  ? 'Echte 5s Mikrofon-Stimmprobe kalibrieren'
                  : 'Calibrate Real 5s Mic Voiceprint'}
            </button>
          </section>

          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 11 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Lock size={15} />
                {lang === 'de' ? 'macOS System, Sicherheit & Updates' : 'macOS System, Security & Updates'}
              </span>
              <span className="pill pill-sage">v0.2.0</span>
            </h2>

            <div className="transcript-row">
              <div className="transcript-meta">
                <strong>Datenbank-Verschlüsselung (At-Rest)</strong>
                <span className={`pill ${telemetry.db_encrypted ? 'pill-sage' : 'pill-ochre'}`}>
                  {telemetry.db_encrypted ? 'SQLCipher AES-256 aktiv' : 'SQLite WAL'}
                </span>
              </div>
              <div className="mono-ts">Schlüssel-Tresor: {telemetry.keychain_backend}</div>
            </div>

            <div className="transcript-row">
              <div className="transcript-meta">
                <strong>Systemweiter Hover-Shortcut</strong>
                <span className="kbd">{telemetry.global_shortcut}</span>
              </div>
              <div style={{ fontSize: 12.2, color: 'var(--text-secondary)' }}>
                Springt über jeder macOS-App auf (unsichtbar bei Screen-Sharing via NSWindowSharingNone).
              </div>
            </div>

            <label
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '9px 11px',
                borderRadius: 6,
                background: 'var(--bg-surface-subtle)',
                cursor: 'pointer',
              }}
            >
              <span style={{ fontSize: 13 }}>
                <strong>Autostart beim macOS-Login</strong> (LaunchAgent im Hintergrund)
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
                {checkingUpdate ? 'Prüfe GitHub Releases…' : 'Nach Updates suchen'}
              </button>
              {updateInfo && (
                <span className={`pill ${updateInfo.update_available ? 'pill-terracotta' : 'pill-sage'}`}>
                  {updateInfo.update_available
                    ? `Update v${updateInfo.latest_version} verfügbar`
                    : `Aktuell (v${updateInfo.current_version})`}
                </span>
              )}
            </div>
          </section>
        </div>
      </div>

      {/* Row 2: Phase 5 Keychain API Key Vault + Phase 4 Multi-LLM / Embeddings / Whisper Manager */}
      <div className="two-col-grid">
        <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <h2 className="section-heading">
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
              <KeyRound size={15} />
              {lang === 'de' ? 'API-Schlüssel-Tresor (macOS Keychain)' : 'API Key Vault (macOS Keychain)'}
            </span>
            <span className="pill pill-sage">Live Streaming</span>
          </h2>
          <p style={{ margin: 0, fontSize: 12.5, color: 'var(--text-secondary)' }}>
            Schlüssel werden sicher im {telemetry.keychain_backend} abgelegt und niemals im Klartext an das UI zurückgegeben.
          </p>

          {KEY_PROVIDERS.map((prov) => {
            const isSet = Boolean(aiForm[prov.field]);
            const testRes = testResults[prov.id];
            return (
              <div key={prov.id} className="transcript-row" style={{ gap: 7 }}>
                <div className="transcript-meta">
                  <strong style={{ fontSize: 13 }}>{prov.label}</strong>
                  <span className={`pill ${isSet ? 'pill-sage' : 'pill-neutral'}`}>
                    {isSet ? 'Im Tresor gespeichert' : 'Nicht hinterlegt'}
                  </span>
                </div>
                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                  <input
                    type="password"
                    className="input-field"
                    style={{ flex: 1, minWidth: 160, fontFamily: 'var(--font-mono)', fontSize: 12 }}
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
                  {isSet && (prov.id === 'anthropic' || prov.id === 'openai' || prov.id === 'gemini') && (
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

        {/* Right: LLM Routing, Neural Embeddings & Local Apple Metal Whisper Models */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Cpu size={15} />
                {lang === 'de' ? 'KI-Router, Ollama & Neuronale Embeddings' : 'AI Router, Ollama & Neural Embeddings'}
              </span>
            </h2>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                LLM-Provider (Streaming)
                <select
                  className="select-field"
                  style={{ marginTop: 4 }}
                  value={aiForm.llm_provider}
                  onChange={(e) => setAiForm({ ...aiForm, llm_provider: e.target.value })}
                >
                  <option value="hybrid_auto">Hybrid Auto (Claude → OpenAI → Gemini → Ollama)</option>
                  <option value="anthropic">Anthropic Claude (Streaming)</option>
                  <option value="openai">OpenAI GPT (Streaming)</option>
                  <option value="gemini">Google Gemini (Streaming)</option>
                  <option value="local_ollama">100% Lokal via Ollama</option>
                </select>
              </label>

              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                LLM-Modellname
                <input
                  className="input-field"
                  style={{ marginTop: 4, fontFamily: 'var(--font-mono)', fontSize: 12 }}
                  value={aiForm.llm_model}
                  onChange={(e) => setAiForm({ ...aiForm, llm_model: e.target.value })}
                  list="ollama-models-datalist"
                />
                <datalist id="ollama-models-datalist">
                  <option value="claude-sonnet-4-20250514" />
                  <option value="gpt-4o-mini" />
                  <option value="gemini-2.5-flash" />
                  {ollamaModels.map((m) => (
                    <option key={m} value={m} />
                  ))}
                </datalist>
              </label>
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Spracherkennung (STT)
                <select
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
                  <option value="ollama">Ollama Lokal (nomic-embed-text / bge-m3)</option>
                  <option value="openai">OpenAI text-embedding-3-small</option>
                </select>
              </label>
            </div>

            <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
              <label style={{ fontSize: 12.5, fontWeight: 600, flex: 1 }}>
                Ollama Endpoint ({ollamaModels.length} lokale Modelle erkannt)
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
                  const res = await onTestProvider('ollama');
                  setTestResults({ ...testResults, ollama: res });
                }}
              >
                Ollama testen
              </button>
              <button
                type="button"
                id="save-ai-settings-btn"
                className="btn btn-primary"
                onClick={() => onSaveAiSettings(aiForm)}
              >
                Speichern
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
          </section>

          {/* Local Whisper GGML Model Manager */}
          <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Sparkles size={15} />
                Lokale Whisper-Modelle (Metal / CoreML)
              </span>
              <span className={`pill ${telemetry.whisper_cli_detected ? 'pill-sage' : 'pill-ochre'}`}>
                {telemetry.whisper_cli_detected ? 'whisper-cli erkannt' : 'brew install whisper-cpp'}
              </span>
            </h2>
            {whisperModels.map((m) => (
              <div key={m.id} className="transcript-row">
                <div className="transcript-meta">
                  <div>
                    <strong style={{ fontSize: 13 }}>{m.name}</strong>
                    <div className="mono-ts">
                      {m.size_mb} MB · {m.language_support}
                    </div>
                  </div>
                  {m.installed ? (
                    <span className="pill pill-sage">
                      <CheckCircle2 size={11} />
                      Installiert
                    </span>
                  ) : (
                    <button
                      type="button"
                      className="btn"
                      disabled={downloadingModelId !== null}
                      onClick={async () => {
                        setDownloadingModelId(m.id);
                        try {
                          await onDownloadWhisperModel(m.id);
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
            ))}
          </section>
        </div>
      </div>

      {/* Row 3: Calendar EventKit + ICS Feeds */}
      <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        <div className="section-heading" style={{ marginBottom: 0 }}>
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
            <Calendar size={15} />
            {lang === 'de'
              ? 'Kalender-Synchronisation (macOS EventKit + externe ICS-Feeds)'
              : 'Calendar Sync (macOS EventKit + External ICS Feeds)'}
          </span>
          <button type="button" className="btn" onClick={onSyncCalendars}>
            <RefreshCw size={13} />
            Jetzt synchronisieren
          </button>
        </div>
        <p style={{ margin: 0, fontSize: 12.5, color: 'var(--text-secondary)' }}>
          macOS Apple Kalender wird nativ über EventKit gelesen. Zusätzlich kannst du hier geheime `.ics`-Links (z. B. Google Calendar oder Outlook) für automatische Vorbereitungs-Briefings einbinden.
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
          style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}
        >
          <input
            className="input-field"
            style={{ width: 200 }}
            value={feedName}
            onChange={(e) => setFeedName(e.target.value)}
            placeholder="Kalendername (z. B. Work)"
          />
          <input
            className="input-field"
            style={{ flex: 1, minWidth: 240, fontFamily: 'var(--font-mono)', fontSize: 12 }}
            value={feedUrl}
            onChange={(e) => setFeedUrl(e.target.value)}
            placeholder="https://calendar.google.com/calendar/ical/.../basic.ics"
          />
          <button type="submit" className="btn">
            <Plus size={14} />
            ICS-Feed verbinden
          </button>
        </form>
      </section>

      {/* Row 4: Custom Vocabulary & Adaptive Learned Preferences */}
      <div className="two-col-grid">
        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>
              {lang === 'de'
                ? 'Fachglossar (Custom Vocabulary für STT & Kontext)'
                : 'Custom Vocabulary Glossary'}
            </span>
          </h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 7, marginBottom: 12 }}>
            {vocabulary.map((v) => (
              <div
                key={v.id}
                className="transcript-row"
                style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}
              >
                <div>
                  <strong>{v.term}</strong>
                  {v.phonetic_hint && <span className="mono-ts"> · „{v.phonetic_hint}“</span>}
                  {v.project_tag && (
                    <span className="pill pill-neutral" style={{ marginLeft: 8 }}>
                      {v.project_tag}
                    </span>
                  )}
                </div>
                <button type="button" className="mini-btn" onClick={() => onDeleteVocabulary(v.id)}>
                  <Trash2 size={12} />
                </button>
              </div>
            ))}
          </div>
          <form
            onSubmit={async (e) => {
              e.preventDefault();
              if (!vocTerm.trim()) return;
              await onAddVocabulary(vocTerm, vocHint, 'Produkt / Fachbegriff', vocProj);
              setVocTerm('');
              setVocHint('');
            }}
            style={{ display: 'flex', gap: 7, flexWrap: 'wrap' }}
          >
            <input
              className="input-field"
              style={{ flex: 1, minWidth: 130 }}
              value={vocTerm}
              onChange={(e) => setVocTerm(e.target.value)}
              placeholder="Begriff / Produktname"
            />
            <input
              className="input-field"
              style={{ flex: 1, minWidth: 120 }}
              value={vocHint}
              onChange={(e) => setVocHint(e.target.value)}
              placeholder="Aussprache-Hinweis"
            />
            <input
              className="input-field"
              style={{ width: 115 }}
              value={vocProj}
              onChange={(e) => setVocProj(e.target.value)}
              placeholder="Projekt"
            />
            <button type="submit" className="btn">
              <Plus size={14} />
              Hinzufügen
            </button>
          </form>
        </section>

        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>
              {lang === 'de'
                ? 'Gelernte Arbeitspräferenzen (Transparentes Memory)'
                : 'Adaptive Learned Preferences'}
            </span>
          </h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 9 }}>
            {learnedPreferences.map((pref) => (
              <label
                key={pref.id}
                className="transcript-row"
                style={{ cursor: 'pointer', opacity: pref.is_active ? 1 : 0.55 }}
              >
                <div className="transcript-meta">
                  <span className="pill pill-sage">{pref.category}</span>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    <span className="mono-ts">{Math.round(pref.confidence * 100)}%</span>
                    <input
                      type="checkbox"
                      checked={pref.is_active}
                      onChange={(e) => onToggleLearnedPref(pref.id, e.target.checked)}
                    />
                  </div>
                </div>
                <div style={{ fontSize: 13 }}>{pref.fact_text}</div>
                {pref.source_ref && <div className="mono-ts">{pref.source_ref}</div>}
              </label>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
};
