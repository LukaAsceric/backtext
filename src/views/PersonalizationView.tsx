import React, { useEffect, useState } from 'react';
import { Mic, Save, Plus, Trash2, Cpu } from 'lucide-react';
import type {
  AiProviderSettings,
  Language,
  LearnedPreference,
  UserProfile,
  VocabularyEntry,
} from '../types';
import { t } from '../lib/i18n';

interface PersonalizationViewProps {
  lang: Language;
  profile: UserProfile;
  vocabulary: VocabularyEntry[];
  learnedPreferences: LearnedPreference[];
  aiSettings: AiProviderSettings;
  onSaveProfile: (profile: UserProfile) => Promise<void>;
  onEnrollSelfVoiceprint: (phrase: string) => Promise<void>;
  onAddVocabulary: (term: string, hint: string, category: string, projectTag: string) => Promise<void>;
  onDeleteVocabulary: (id: string) => Promise<void>;
  onToggleLearnedPref: (id: string, isActive: boolean) => Promise<void>;
  onSaveAiSettings: (settings: AiProviderSettings) => Promise<void>;
}

export const PersonalizationView: React.FC<PersonalizationViewProps> = ({
  lang,
  profile,
  vocabulary,
  learnedPreferences,
  aiSettings,
  onSaveProfile,
  onEnrollSelfVoiceprint,
  onAddVocabulary,
  onDeleteVocabulary,
  onToggleLearnedPref,
  onSaveAiSettings,
}) => {
  const [form, setForm] = useState<UserProfile>(profile);
  const [projectsInput, setProjectsInput] = useState(profile.active_projects.join(', '));
  const [aiForm, setAiForm] = useState<AiProviderSettings>(aiSettings);
  const [vocTerm, setVocTerm] = useState('');
  const [vocHint, setVocHint] = useState('');
  const [vocProj, setVocProj] = useState('backtext');
  const [savedBanner, setSavedBanner] = useState(false);

  useEffect(() => {
    setForm(profile);
    setProjectsInput(profile.active_projects.join(', '));
  }, [profile]);

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
              ? 'Nutzerpersonalisierung, Stimmprofil & KI-Modelle'
              : 'User Personalization, Self-Voiceprint & AI Models'}
          </h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Dein Profil, Fachglossar und Arbeitsstil steuern sowohl Zusammenfassungen als auch das `get_user_profile`-Dossier deiner MCP-Agenten.'
              : 'Your profile, custom glossary, and working style shape summaries and the MCP `get_user_profile` agent dossier.'}
          </p>
        </div>
      </header>

      <div className="two-col-grid">
        {/* Identity, Language & Style */}
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

        {/* Self-Voiceprint & Hybrid AI Provider Settings */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <section className="surface-sheet">
            <h2 className="section-heading">
              <span>{lang === 'de' ? 'Eigenes Stimmprofil (Self-Voiceprint)' : 'Self-Voiceprint'}</span>
              <span className={`pill ${profile.has_self_voiceprint ? 'pill-sage' : 'pill-ochre'}`}>
                {profile.has_self_voiceprint ? 'Kalibriert' : 'Noch nicht kalibriert'}
              </span>
            </h2>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginTop: 0 }}>
              {lang === 'de'
                ? 'Neben der dedizierten Mikrofon-Spur hilft dein Stimmprofil dabei, deine Stimme auch in Präsenz-Meetings am selben Raummikrofon eindeutig zuzuordnen.'
                : 'Helps identify your voice even during in-person meetings on a shared room microphone.'}
            </p>
            <button
              type="button"
              id="enroll-voiceprint-btn"
              className="btn"
              onClick={() =>
                onEnrollSelfVoiceprint(
                  `Hallo, ich bin ${profile.display_name} und nutze backtext für ruhigen Hintergrund-Kontext.`,
                )
              }
            >
              <Mic size={14} />
              {lang === 'de' ? '15s Stimmprobe jetzt kalibrieren' : 'Calibrate Voiceprint Sample'}
            </button>
          </section>

          <section className="surface-sheet">
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Cpu size={15} />
                {lang === 'de' ? 'Hybrid-KI & Audio-Provider' : 'Hybrid AI & Speech Providers'}
              </span>
            </h2>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Spracherkennung (STT)
                <select
                  className="select-field"
                  style={{ marginTop: 4 }}
                  value={aiForm.stt_provider}
                  onChange={(e) => setAiForm({ ...aiForm, stt_provider: e.target.value })}
                >
                  <option value="local_whisper">Lokal: Whisper Large-v3-Turbo (CoreML / Metal)</option>
                  <option value="deepgram">Cloud: Deepgram Nova-3 Streaming</option>
                  <option value="openai_audio">Cloud: OpenAI GPT-4o Transcribe</option>
                </select>
              </label>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                LLM-Engine für Summaries & Recipes
                <select
                  className="select-field"
                  style={{ marginTop: 4 }}
                  value={aiForm.llm_provider}
                  onChange={(e) => setAiForm({ ...aiForm, llm_provider: e.target.value })}
                >
                  <option value="hybrid_auto">Hybrid Auto (Lokales Ollama / Cloud API Fallback)</option>
                  <option value="local_ollama">100% Lokal via Ollama</option>
                  <option value="openai">OpenAI API (OPENAI_API_KEY)</option>
                </select>
              </label>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Lokale Ollama-URL
                <input
                  className="input-field"
                  style={{ marginTop: 4, fontFamily: 'var(--font-mono)' }}
                  value={aiForm.ollama_url}
                  onChange={(e) => setAiForm({ ...aiForm, ollama_url: e.target.value })}
                />
              </label>
              <button
                type="button"
                className="btn"
                style={{ alignSelf: 'flex-start' }}
                onClick={() => onSaveAiSettings(aiForm)}
              >
                KI-Einstellungen sichern
              </button>
            </div>
          </section>
        </div>
      </div>

      {/* Custom Vocabulary & Adaptive Learned Preferences */}
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
