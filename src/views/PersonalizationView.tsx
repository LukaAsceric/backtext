import React, { useEffect, useState } from 'react';
import { Mic, Save, Plus, Trash2, Sparkles, BookOpen } from 'lucide-react';
import type {
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
  onSaveProfile: (profile: UserProfile) => Promise<void>;
  onEnrollSelfVoiceprint: () => Promise<void>;
  onAddVocabulary: (term: string, hint: string, category: string, projectTag: string) => Promise<void>;
  onDeleteVocabulary: (id: string) => Promise<void>;
  onToggleLearnedPref: (id: string, isActive: boolean) => Promise<void>;
}

export const PersonalizationView: React.FC<PersonalizationViewProps> = ({
  lang,
  profile,
  vocabulary,
  learnedPreferences,
  onSaveProfile,
  onEnrollSelfVoiceprint,
  onAddVocabulary,
  onDeleteVocabulary,
  onToggleLearnedPref,
}) => {
  const [form, setForm] = useState<UserProfile>(profile);
  const [projectsInput, setProjectsInput] = useState(profile.active_projects.join(', '));
  const [vocTerm, setVocTerm] = useState('');
  const [vocHint, setVocHint] = useState('');
  const [vocProj, setVocProj] = useState('backtext');
  const [savedBanner, setSavedBanner] = useState(false);
  const [calibrating, setCalibrating] = useState(false);

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
            {lang === 'de' ? 'Personalisierung' : 'Personalization'}
          </h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Deine Rolle, Sprache, dein Schreibstil, Stimmprofil und Fachglossar — damit Zusammenfassungen und KI-Agenten genau in deinem Stil arbeiten.'
              : 'Your identity, language, tone, voiceprint, and glossary — so summaries and AI agents reflect how you think and write.'}
          </p>
        </div>
      </header>

      <div className="two-col-grid">
        {/* Left: Personal Profile, Language & Tone */}
        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>{lang === 'de' ? 'Persönliches Profil & Sprache' : 'Identity, Language & Tone'}</span>
            {savedBanner && <span className="pill pill-sage">{lang === 'de' ? 'Gespeichert' : 'Saved'}</span>}
          </h2>
          <form onSubmit={handleProfileSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
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
              {lang === 'de' ? 'Rolle & Fokus' : 'Role & Focus'}
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
              {lang === 'de'
                ? 'Aktive Projekte (kommagetrennt — für automatische Kontext-Zuordnung)'
                : 'Active Projects (comma-separated — for automatic context tagging)'}
              <input
                className="input-field"
                style={{ marginTop: 4 }}
                value={projectsInput}
                onChange={(e) => setProjectsInput(e.target.value)}
              />
            </label>

            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              {lang === 'de' ? 'Bevorzugter Schreibstil' : 'Preferred Writing Style'}
              <input
                className="input-field"
                style={{ marginTop: 4 }}
                value={form.writing_style}
                onChange={(e) => setForm({ ...form, writing_style: e.target.value })}
              />
            </label>

            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              {lang === 'de'
                ? 'Tonfall-Anweisungen für KI & MCP-Agenten'
                : 'Tone Instructions for AI & MCP Agents'}
              <textarea
                className="textarea-field"
                rows={3}
                style={{ marginTop: 4 }}
                value={form.tone_instructions}
                onChange={(e) => setForm({ ...form, tone_instructions: e.target.value })}
              />
            </label>

            <button type="submit" id="save-profile-btn" className="btn btn-primary" style={{ alignSelf: 'flex-start' }}>
              <Save size={14} />
              {t(lang, 'saveProfile')}
            </button>
          </form>
        </section>

        {/* Right: Self-Voiceprint + Adaptive Learned Preferences */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <section className="surface-sheet">
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Mic size={15} />
                {lang === 'de' ? 'Eigenes Stimmprofil' : 'Acoustic Self-Voiceprint'}
              </span>
              <span className={`pill ${profile.has_self_voiceprint ? 'pill-sage' : 'pill-ochre'}`}>
                {profile.has_self_voiceprint
                  ? lang === 'de'
                    ? 'Kalibriert'
                    : 'Calibrated'
                  : lang === 'de'
                    ? 'Nicht kalibriert'
                    : 'Not calibrated'}
              </span>
            </h2>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginTop: 0 }}>
              {lang === 'de'
                ? 'Nimmt 5 Sekunden deiner Mikrofon-Stimme lokal auf und erstellt einen 256-dim Stimmvektor, damit deine Wortbeiträge im Meeting zuverlässig erkannt werden.'
                : 'Records 5s of your microphone voice locally to build a 256-dim speaker embedding for reliable self-attribution.'}
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
                  ? '5s Mikrofon-Stimmprobe kalibrieren'
                  : 'Calibrate 5s Mic Voiceprint'}
            </button>
          </section>

          <section className="surface-sheet">
            <h2 className="section-heading">
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
                <Sparkles size={15} />
                {lang === 'de' ? 'Gelernte Arbeitspräferenzen' : 'Learned Preferences'}
              </span>
              <span className="mono-ts">{learnedPreferences.filter((p) => p.is_active).length} aktiv</span>
            </h2>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {learnedPreferences.map((pref) => (
                <label
                  key={pref.id}
                  className="transcript-row"
                  style={{ cursor: 'pointer', opacity: pref.is_active ? 1 : 0.55 }}
                >
                  <div className="transcript-meta">
                    <span className="pill pill-neutral">{pref.category}</span>
                    <input
                      type="checkbox"
                      checked={pref.is_active}
                      onChange={(e) => onToggleLearnedPref(pref.id, e.target.checked)}
                    />
                  </div>
                  <div style={{ fontSize: 13 }}>{pref.fact_text}</div>
                </label>
              ))}
            </div>
          </section>
        </div>
      </div>

      {/* Custom Vocabulary Glossary */}
      <section className="surface-sheet">
        <h2 className="section-heading">
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
            <BookOpen size={15} />
            {lang === 'de'
              ? 'Fachglossar (Custom Vocabulary für Transkription & Kontext)'
              : 'Custom Vocabulary Glossary (for STT & Context)'}
          </span>
          <span className="mono-ts">{vocabulary.length} Einträge</span>
        </h2>

        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))',
            gap: 8,
            marginBottom: 14,
          }}
        >
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
          style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}
        >
          <input
            className="input-field"
            style={{ flex: 1, minWidth: 160 }}
            value={vocTerm}
            onChange={(e) => setVocTerm(e.target.value)}
            placeholder={lang === 'de' ? 'Begriff / Produktname' : 'Term / Product name'}
          />
          <input
            className="input-field"
            style={{ flex: 1, minWidth: 150 }}
            value={vocHint}
            onChange={(e) => setVocHint(e.target.value)}
            placeholder={lang === 'de' ? 'Aussprache-Hinweis (optional)' : 'Phonetic hint (optional)'}
          />
          <input
            className="input-field"
            style={{ width: 130 }}
            value={vocProj}
            onChange={(e) => setVocProj(e.target.value)}
            placeholder={lang === 'de' ? 'Projekt' : 'Project'}
          />
          <button type="submit" className="btn">
            <Plus size={14} />
            {lang === 'de' ? 'Hinzufügen' : 'Add'}
          </button>
        </form>
      </section>
    </div>
  );
};
