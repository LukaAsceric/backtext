import React, { useState } from 'react';
import {
  AudioWaveform,
  Briefcase,
  CheckCircle2,
  Mail,
  UserPlus,
  Pencil,
  Trash2,
  X,
} from 'lucide-react';
import type { ContextEpisode, Language, Meeting, Person } from '../types';

export interface PersonUpsertInput {
  id?: string;
  fullName: string;
  email?: string;
  company?: string;
  roleNotes?: string;
}

interface PeopleAndProjectsViewProps {
  lang: Language;
  persons: Person[];
  projects: string[];
  meetings: Meeting[];
  episodes: ContextEpisode[];
  onUpsertPerson: (input: PersonUpsertInput) => Promise<void>;
  onDeletePerson: (personId: string) => Promise<void>;
}

export const PeopleAndProjectsView: React.FC<PeopleAndProjectsViewProps> = ({
  lang,
  persons,
  projects,
  meetings,
  episodes,
  onUpsertPerson,
  onDeletePerson,
}) => {
  const [formOpen, setFormOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | undefined>(undefined);
  const [fullName, setFullName] = useState('');
  const [email, setEmail] = useState('');
  const [company, setCompany] = useState('');
  const [roleNotes, setRoleNotes] = useState('');
  const [saving, setSaving] = useState(false);

  const openCreate = () => {
    setEditingId(undefined);
    setFullName('');
    setEmail('');
    setCompany('');
    setRoleNotes('');
    setFormOpen(true);
  };

  const openEdit = (p: Person) => {
    setEditingId(p.id);
    setFullName(p.full_name);
    setEmail(p.email ?? '');
    setCompany(p.company ?? '');
    setRoleNotes(p.role_notes ?? '');
    setFormOpen(true);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!fullName.trim()) return;
    setSaving(true);
    try {
      await onUpsertPerson({
        id: editingId,
        fullName: fullName.trim(),
        email: email.trim() || undefined,
        company: company.trim() || undefined,
        roleNotes: roleNotes.trim(),
      });
      setFormOpen(false);
      setEditingId(undefined);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">
            {lang === 'de'
              ? 'Personen, Stimmprofile & Projekt-Dossiers'
              : 'People, Voiceprints & Project Dossiers'}
          </h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Verwalte Meeting-Teilnehmer und lernende 256-dim Stimmprofile (Voiceprints) für automatische Sprechererkennung.'
              : 'Manage meeting participants and learning 256-dim voiceprints for automatic speaker recognition.'}
          </p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            id="add-person-btn"
            className="btn btn-primary"
            onClick={openCreate}
          >
            <UserPlus size={14} />
            {lang === 'de' ? 'Person hinzufügen' : 'Add Contact'}
          </button>
        </div>
      </header>

      {formOpen && (
        <section className="surface-sheet">
          <div className="section-heading" style={{ marginBottom: 10 }}>
            <span>
              {editingId
                ? lang === 'de'
                  ? 'Kontakt bearbeiten'
                  : 'Edit Contact'
                : lang === 'de'
                  ? 'Neuen Kontakt anlegen'
                  : 'Create New Contact'}
            </span>
            <button type="button" className="mini-btn" onClick={() => setFormOpen(false)}>
              <X size={12} />
            </button>
          </div>
          <form
            onSubmit={handleSubmit}
            style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 10 }}
          >
            <label style={{ fontSize: 12.2, fontWeight: 600 }}>
              {lang === 'de' ? 'Vor- & Nachname *' : 'Full Name *'}
              <input
                id="person-name-input"
                className="input-field"
                style={{ marginTop: 4 }}
                value={fullName}
                onChange={(e) => setFullName(e.target.value)}
                placeholder="z. B. Tobias Richter"
                required
              />
            </label>
            <label style={{ fontSize: 12.2, fontWeight: 600 }}>
              {lang === 'de' ? 'E-Mail' : 'Email'}
              <input
                id="person-email-input"
                className="input-field"
                style={{ marginTop: 4 }}
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="name@firma.de"
              />
            </label>
            <label style={{ fontSize: 12.2, fontWeight: 600 }}>
              {lang === 'de' ? 'Unternehmen / Team' : 'Company / Team'}
              <input
                id="person-company-input"
                className="input-field"
                style={{ marginTop: 4 }}
                value={company}
                onChange={(e) => setCompany(e.target.value)}
                placeholder="z. B. Produkt & Design"
              />
            </label>
            <label style={{ fontSize: 12.2, fontWeight: 600, gridColumn: '1 / -1' }}>
              {lang === 'de' ? 'Rolle & Kontext-Notizen' : 'Role & Context Notes'}
              <input
                id="person-role-input"
                className="input-field"
                style={{ marginTop: 4 }}
                value={roleNotes}
                onChange={(e) => setRoleNotes(e.target.value)}
                placeholder={
                  lang === 'de'
                    ? 'z. B. Engineering Lead — Ansprechpartner für API-Architektur'
                    : 'e.g. Engineering Lead — API architecture owner'
                }
              />
            </label>
            <div style={{ gridColumn: '1 / -1', display: 'flex', gap: 8 }}>
              <button
                type="submit"
                id="save-person-btn"
                className="btn btn-primary"
                disabled={saving || !fullName.trim()}
              >
                {lang === 'de' ? 'Speichern' : 'Save'}
              </button>
              <button type="button" className="btn" onClick={() => setFormOpen(false)}>
                {lang === 'de' ? 'Abbrechen' : 'Cancel'}
              </button>
            </div>
          </form>
        </section>
      )}

      <div className="two-col-grid">
        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>{lang === 'de' ? 'Bekannte Personen & Voiceprints' : 'Known Contacts & Voiceprints'}</span>
            <span className="mono-ts">{persons.length}</span>
          </h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            {persons.length === 0 && (
              <div style={{ fontSize: 13, color: 'var(--text-tertiary)' }}>
                {lang === 'de'
                  ? 'Noch keine Personen angelegt. Klicke oben auf „Person hinzufügen“ oder füge Teilnehmer direkt im Meeting hinzu.'
                  : 'No contacts yet. Click Add Contact above or add participants inside a meeting.'}
              </div>
            )}
            {persons.map((p) => (
              <div key={p.id} className="transcript-row">
                <div className="transcript-meta">
                  <div>
                    <strong style={{ fontSize: 14.5 }}>{p.full_name}</strong>
                    {p.company && (
                      <span style={{ color: 'var(--text-secondary)', marginLeft: 8, fontSize: 12.5 }}>
                        · {p.company}
                      </span>
                    )}
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                    <span className="pill pill-sage">
                      <AudioWaveform size={12} />
                      {Math.round(p.voiceprint_confidence * 100)}% ({p.voiceprint_samples})
                    </span>
                    <button
                      type="button"
                      className="mini-btn"
                      title={lang === 'de' ? 'Bearbeiten' : 'Edit'}
                      onClick={() => openEdit(p)}
                    >
                      <Pencil size={11} />
                    </button>
                    <button
                      type="button"
                      className="mini-btn"
                      title={lang === 'de' ? 'Kontakt löschen' : 'Delete contact'}
                      onClick={() => onDeletePerson(p.id)}
                    >
                      <Trash2 size={11} />
                    </button>
                  </div>
                </div>
                {p.email && (
                  <div className="mono-ts">
                    <Mail size={11} style={{ verticalAlign: 'middle', marginRight: 5 }} />
                    {p.email}
                  </div>
                )}
                {p.role_notes && (
                  <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{p.role_notes}</div>
                )}
                {p.open_commitments.length > 0 && (
                  <div style={{ marginTop: 4, display: 'flex', flexDirection: 'column', gap: 4 }}>
                    <span style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--text-tertiary)' }}>
                      {lang === 'de' ? 'Offene Zusagen:' : 'Open commitments:'}
                    </span>
                    {p.open_commitments.map((c, idx) => (
                      <div key={idx} style={{ fontSize: 12.5, display: 'flex', gap: 6, alignItems: 'center' }}>
                        <CheckCircle2 size={12} style={{ color: 'var(--accent-sage)' }} />
                        <span>{c}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        </section>

        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>{lang === 'de' ? 'Aktive Projekt-Cluster' : 'Active Project Clusters'}</span>
            <span className="mono-ts">{projects.length}</span>
          </h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            {projects.map((proj) => {
              const projMeetings = meetings.filter((m) => m.project_tag === proj);
              const projEpisodes = episodes.filter((e) => e.project_tag === proj);
              return (
                <div key={proj} className="transcript-row">
                  <div className="transcript-meta">
                    <strong style={{ fontSize: 14.5 }}>
                      <Briefcase size={14} style={{ verticalAlign: 'middle', marginRight: 6 }} />
                      {proj}
                    </strong>
                    <span className="pill pill-neutral">
                      {projMeetings.length} Meetings · {projEpisodes.length} Episoden
                    </span>
                  </div>
                  <div style={{ fontSize: 12.8, color: 'var(--text-secondary)' }}>
                    {projEpisodes[0]?.summary ??
                      projMeetings[0]?.executive_summary ??
                      (lang === 'de'
                        ? 'Bereit für automatische Kontext-Zuordnung via Glossar & Fenstertitel.'
                        : 'Ready for automatic context attribution.')}
                  </div>
                </div>
              );
            })}
          </div>
        </section>
      </div>
    </div>
  );
};
