import React, { useState } from 'react';
import { Trash2, Eye, Clipboard, Layers, ShieldAlert, PlusCircle } from 'lucide-react';
import type {
  ClipboardItem,
  ContextEpisode,
  ContextSnapshot,
  Language,
  ScrubberPreview,
} from '../types';
import { t } from '../lib/i18n';

interface ContextTimelineViewProps {
  lang: Language;
  episodes: ContextEpisode[];
  snapshots: ContextSnapshot[];
  clipboard: ClipboardItem[];
  onPurgeRecent: (minutes: number) => Promise<void>;
  onCaptureSample: (
    appName: string,
    windowTitle: string,
    browserUrl: string,
    visibleText: string,
    clipboardText: string,
  ) => Promise<unknown>;
  onPreviewScrubber: (
    appName: string,
    windowTitle: string,
    browserUrl: string,
    rawText: string,
  ) => Promise<ScrubberPreview>;
}

export const ContextTimelineView: React.FC<ContextTimelineViewProps> = ({
  lang,
  episodes,
  snapshots,
  clipboard,
  onPurgeRecent,
  onCaptureSample,
  onPreviewScrubber,
}) => {
  const [subTab, setSubTab] = useState<'episodes' | 'snapshots' | 'clipboard' | 'simulator'>(
    'episodes',
  );
  const [simApp, setSimApp] = useState('Cursor');
  const [simTitle, setSimTitle] = useState('auth.rs — backtext');
  const [simUrl, setSimUrl] = useState('');
  const [simText, setSimText] = useState(
    'Konfiguriere MCP Streamable HTTP Header mit Test-Key sk-ant-api03-99887766554433221100abcdef für backtext.',
  );
  const [simClip, setSimClip] = useState('');
  const [preview, setPreview] = useState<ScrubberPreview | null>(null);
  const [captureResult, setCaptureResult] = useState<string | null>(null);

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">
            {lang === 'de'
              ? 'Kontext-Gedächtnis & 3-Stufen-Filter'
              : 'Ambient Context Memory & 3-Stage Filter'}
          </h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Fenster-, App-, Tab-, Zwischenablage- und OCR-Texterfassung — automatisch bereinigt um Duplikate, Rauschen und Geheimnisse.'
              : 'Window, app, browser tab, clipboard, and Vision OCR capture — automatically scrubbed of secrets and duplicates.'}
          </p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            id="purge-15m-btn"
            className="btn"
            onClick={() => onPurgeRecent(15)}
          >
            <Trash2 size={14} />
            {t(lang, 'purge15')}
          </button>
          <button
            type="button"
            id="purge-60m-btn"
            className="btn"
            onClick={() => onPurgeRecent(60)}
          >
            <Trash2 size={14} />
            {t(lang, 'purge60')}
          </button>
        </div>
      </header>

      <div className="tab-strip" style={{ alignSelf: 'flex-start' }}>
        <button
          type="button"
          className={`tab-btn ${subTab === 'episodes' ? 'active' : ''}`}
          onClick={() => setSubTab('episodes')}
        >
          <Layers size={13} style={{ verticalAlign: 'middle', marginRight: 5 }} />
          {lang === 'de' ? 'Arbeits-Episoden' : 'Work Episodes'} ({episodes.length})
        </button>
        <button
          type="button"
          className={`tab-btn ${subTab === 'snapshots' ? 'active' : ''}`}
          onClick={() => setSubTab('snapshots')}
        >
          <Eye size={13} style={{ verticalAlign: 'middle', marginRight: 5 }} />
          {lang === 'de' ? 'Fenster & OCR' : 'Windows & OCR'} ({snapshots.length})
        </button>
        <button
          type="button"
          className={`tab-btn ${subTab === 'clipboard' ? 'active' : ''}`}
          onClick={() => setSubTab('clipboard')}
        >
          <Clipboard size={13} style={{ verticalAlign: 'middle', marginRight: 5 }} />
          {lang === 'de' ? 'Zwischenablage' : 'Safe Clipboard'} ({clipboard.length})
        </button>
        <button
          type="button"
          className={`tab-btn ${subTab === 'simulator' ? 'active' : ''}`}
          onClick={() => setSubTab('simulator')}
        >
          <ShieldAlert size={13} style={{ verticalAlign: 'middle', marginRight: 5 }} />
          {lang === 'de' ? 'Filter- & Scrubber-Labor' : 'Filter & Scrubber Lab'}
        </button>
      </div>

      {subTab === 'episodes' && (
        <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          {episodes.map((ep) => (
            <div key={ep.id} className="transcript-row">
              <div className="transcript-meta">
                <strong style={{ fontSize: 14.5 }}>{ep.title}</strong>
                <span className="mono-ts">
                  Relevanz {Math.round(ep.importance_score * 100)}% · {ep.snapshot_count} Snapshots
                </span>
              </div>
              <div style={{ color: 'var(--text-secondary)', fontSize: 13.5 }}>{ep.summary}</div>
              <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginTop: 4 }}>
                {ep.project_tag && <span className="pill pill-sage">{ep.project_tag}</span>}
                {ep.primary_apps.map((a) => (
                  <span key={a} className="pill pill-neutral">
                    {a}
                  </span>
                ))}
                {ep.urls.map((u) => (
                  <span key={u} className="mono-ts">
                    {u}
                  </span>
                ))}
              </div>
            </div>
          ))}
        </section>
      )}

      {subTab === 'snapshots' && (
        <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {snapshots.map((s) => (
            <div key={s.id} className="transcript-row">
              <div className="transcript-meta">
                <div>
                  <strong>{s.app_name}</strong> — <span>{s.window_title}</span>
                </div>
                <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                  <span className="pill pill-neutral">{s.extraction_method}</span>
                  <span className="pill pill-sage">Score {Math.round(s.relevance_score * 100)}%</span>
                  {s.redacted_secrets_count > 0 && (
                    <span className="pill pill-ochre">
                      {s.redacted_secrets_count} Secret maskiert
                    </span>
                  )}
                </div>
              </div>
              {s.browser_url && <div className="mono-ts">{s.browser_url}</div>}
              <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{s.extracted_text}</div>
            </div>
          ))}
        </section>
      )}

      {subTab === 'clipboard' && (
        <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {clipboard.map((c) => (
            <div key={c.id} className="transcript-row">
              <div className="transcript-meta">
                <strong>{c.source_app}</strong>
                <div style={{ display: 'flex', gap: 6 }}>
                  <span className="pill pill-neutral">{c.content_kind}</span>
                  <span className="mono-ts">#{c.content_hash}</span>
                </div>
              </div>
              <pre className="code-block">{c.text_preview}</pre>
            </div>
          ))}
        </section>
      )}

      {subTab === 'simulator' && (
        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>
              {lang === 'de'
                ? 'Live-Test: 3-Stufen Privacy-Scrubber & Kontexterfassung'
                : 'Live Test: 3-Stage Privacy Scrubber & Context Capture'}
            </span>
          </h2>
          <div className="two-col-grid">
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                App-Name (z. B. Cursor, Arc Browser oder 1Password zum Block-Test)
                <input
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={simApp}
                  onChange={(e) => setSimApp(e.target.value)}
                />
              </label>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Fenstertitel
                <input
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={simTitle}
                  onChange={(e) => setSimTitle(e.target.value)}
                />
              </label>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Browser-URL (optional)
                <input
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={simUrl}
                  onChange={(e) => setSimUrl(e.target.value)}
                  placeholder="https://..."
                />
              </label>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Sichtbarer Fenster-/OCR-Text (inkl. Secrets zum Redaktions-Test)
                <textarea
                  className="textarea-field"
                  rows={4}
                  style={{ marginTop: 4 }}
                  value={simText}
                  onChange={(e) => setSimText(e.target.value)}
                />
              </label>
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                Optionaler Zwischenablage-Text
                <input
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={simClip}
                  onChange={(e) => setSimClip(e.target.value)}
                  placeholder="Kopiertes Snippet oder URL…"
                />
              </label>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <button
                  type="button"
                  id="preview-scrubber-btn"
                  className="btn"
                  onClick={async () => {
                    const res = await onPreviewScrubber(simApp, simTitle, simUrl, simText);
                    setPreview(res);
                  }}
                >
                  Filter-Vorschau prüfen
                </button>
                <button
                  type="button"
                  id="capture-ambient-sample-btn"
                  className="btn btn-primary"
                  onClick={async () => {
                    const decision = await onCaptureSample(simApp, simTitle, simUrl, simText, simClip);
                    setCaptureResult(JSON.stringify(decision, null, 2));
                  }}
                >
                  <PlusCircle size={14} />
                  In Gedächtnis erfassen
                </button>
              </div>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              {preview && (
                <div className="transcript-row">
                  <div className="transcript-meta">
                    <strong>Filter-Analyse</strong>
                    <span className={`pill ${preview.blocked ? 'pill-terracotta' : 'pill-sage'}`}>
                      {preview.blocked ? 'BLOCKIERT (Stufe 1)' : 'FREIGEGEBEN'}
                    </span>
                  </div>
                  {preview.block_reason && (
                    <div style={{ color: 'var(--accent-terracotta)', fontWeight: 600 }}>
                      {preview.block_reason}
                    </div>
                  )}
                  <div style={{ fontSize: 12.5 }}>
                    Maskierte Secrets: <strong>{preview.redacted_secrets_count}</strong> · Relevanz:{' '}
                    <strong>{Math.round(preview.relevance_score * 100)}%</strong> · Projekt:{' '}
                    <strong>{preview.matched_project ?? '—'}</strong>
                  </div>
                  <pre className="code-block">{preview.scrubbed_text}</pre>
                </div>
              )}
              {captureResult && (
                <div>
                  <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 4 }}>
                    Pipeline-Entscheidung (Rust Engine):
                  </div>
                  <pre className="code-block">{captureResult}</pre>
                </div>
              )}
            </div>
          </div>
        </section>
      )}
    </div>
  );
};
