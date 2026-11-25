import React from 'react';
import { AudioWaveform, Briefcase, CheckCircle2, Mail } from 'lucide-react';
import type { ContextEpisode, Language, Meeting, Person } from '../types';

interface PeopleAndProjectsViewProps {
  lang: Language;
  persons: Person[];
  projects: string[];
  meetings: Meeting[];
  episodes: ContextEpisode[];
}

export const PeopleAndProjectsView: React.FC<PeopleAndProjectsViewProps> = ({
  lang,
  persons,
  projects,
  meetings,
  episodes,
}) => {
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
              ? 'Lernende 256-dim Stimmprofile (Voiceprints) erkennen bekannte Gesprächspartner automatisch ohne Meeting-Bot.'
              : 'Learning 256-dim speaker voiceprints recognize recurring contacts automatically without a meeting bot.'}
          </p>
        </div>
      </header>

      <div className="two-col-grid">
        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>{lang === 'de' ? 'Bekannte Personen & Voiceprints' : 'Known Contacts & Voiceprints'}</span>
            <span className="mono-ts">{persons.length}</span>
          </h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
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
                  <span className="pill pill-sage">
                    <AudioWaveform size={12} />
                    {Math.round(p.voiceprint_confidence * 100)}% ({p.voiceprint_samples} Proben)
                  </span>
                </div>
                {p.email && (
                  <div className="mono-ts">
                    <Mail size={11} style={{ verticalAlign: 'middle', marginRight: 5 }} />
                    {p.email}
                  </div>
                )}
                <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{p.role_notes}</div>
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
