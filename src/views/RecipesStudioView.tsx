import React, { useState } from 'react';
import { Star, Plus, Trash2, Play } from 'lucide-react';
import type { Language, Recipe } from '../types';

interface RecipesStudioViewProps {
  lang: Language;
  recipes: Recipe[];
  onSaveRecipe: (recipe: Recipe) => Promise<void>;
  onDeleteRecipe: (id: string) => Promise<void>;
  onRunRecipeInToday: (slug: string) => Promise<void>;
}

const TEMPLATE_VARS = [
  '{{transcript}}',
  '{{user_notes}}',
  '{{screenshare_ocr}}',
  '{{participants}}',
  '{{active_context}}',
  '{{user_profile}}',
];

export const RecipesStudioView: React.FC<RecipesStudioViewProps> = ({
  lang,
  recipes,
  onSaveRecipe,
  onDeleteRecipe,
  onRunRecipeInToday,
}) => {
  const [scopeFilter, setScopeFilter] = useState<'all' | Recipe['scope']>('all');
  const [slug, setSlug] = useState('/custom-brief');
  const [nameDe, setNameDe] = useState('');
  const [descDe, setDescDe] = useState('');
  const [scope, setScope] = useState<Recipe['scope']>('single_meeting');
  const [promptTemplate, setPromptTemplate] = useState(
    'Analysiere {{transcript}} und {{user_notes}} im Hinblick auf konkrete Architekturentscheidungen und offene Risiken.',
  );

  const visible = recipes.filter((r) => scopeFilter === 'all' || r.scope === scopeFilter);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!slug.trim() || !nameDe.trim() || !promptTemplate.trim()) return;
    await onSaveRecipe({
      id: `rec_${Date.now()}`,
      slug: slug.startsWith('/') ? slug.trim() : `/${slug.trim()}`,
      name_de: nameDe.trim(),
      name_en: nameDe.trim(),
      description_de: descDe.trim() || nameDe.trim(),
      description_en: descDe.trim() || nameDe.trim(),
      icon_key: 'sparkles',
      scope,
      prompt_template: promptTemplate,
      preferred_model: null,
      is_builtin: false,
      is_favorite: true,
      usage_count: 0,
    });
    setNameDe('');
    setDescDe('');
  };

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">Recipes Studio</h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Wiederverwendbare Prompt-Rezepte im Granola-Stil — aufrufbar per / im KI-Chat sowie direkt als MCP-Prompts in Claude & Cursor.'
              : 'Reusable Granola-style prompt recipes — trigger via / in chat or directly as MCP Prompts in Claude & Cursor.'}
          </p>
        </div>
      </header>

      <div className="tab-strip" style={{ alignSelf: 'flex-start' }}>
        {(['all', 'single_meeting', 'multi_meeting', 'ambient_workspace'] as const).map((sc) => (
          <button
            key={sc}
            type="button"
            className={`tab-btn ${scopeFilter === sc ? 'active' : ''}`}
            onClick={() => setScopeFilter(sc)}
          >
            {sc === 'all' ? (lang === 'de' ? 'Alle Recipes' : 'All Recipes') : sc.replace('_', ' ')}
          </button>
        ))}
      </div>

      <div className="two-col-grid">
        <section className="surface-sheet" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {visible.map((rec) => (
            <div key={rec.id} className="transcript-row">
              <div className="transcript-meta">
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span className="recipe-slug">{rec.slug}</span>
                  <strong>{lang === 'de' ? rec.name_de : rec.name_en}</strong>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <span className="pill pill-neutral">{rec.scope.replace('_', ' ')}</span>
                  <button
                    type="button"
                    className="mini-btn"
                    title="Favorit umschalten"
                    onClick={() => onSaveRecipe({ ...rec, is_favorite: !rec.is_favorite })}
                  >
                    <Star
                      size={12}
                      fill={rec.is_favorite ? 'currentColor' : 'none'}
                      style={{ color: rec.is_favorite ? 'var(--accent-ochre)' : undefined }}
                    />
                  </button>
                  <button
                    type="button"
                    className="mini-btn"
                    onClick={() => onRunRecipeInToday(rec.slug)}
                  >
                    <Play size={11} style={{ verticalAlign: 'middle', marginRight: 3 }} />
                    Run
                  </button>
                  {!rec.is_builtin && (
                    <button
                      type="button"
                      className="mini-btn"
                      onClick={() => onDeleteRecipe(rec.id)}
                    >
                      <Trash2 size={11} />
                    </button>
                  )}
                </div>
              </div>
              <div style={{ fontSize: 12.8, color: 'var(--text-secondary)' }}>
                {lang === 'de' ? rec.description_de : rec.description_en}
              </div>
              <pre className="code-block" style={{ fontSize: 11.5 }}>
                {rec.prompt_template}
              </pre>
            </div>
          ))}
        </section>

        <section className="surface-sheet">
          <h2 className="section-heading">
            <span>{lang === 'de' ? 'Neues Recipe erstellen' : 'Create Custom Recipe'}</span>
          </h2>
          <form onSubmit={handleCreate} style={{ display: 'flex', flexDirection: 'column', gap: 11 }}>
            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              Slash-Shortcut
              <input
                className="input-field"
                style={{ marginTop: 4, fontFamily: 'var(--font-mono)' }}
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
                placeholder="/mein-recipe"
              />
            </label>
            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              Titel
              <input
                className="input-field"
                style={{ marginTop: 4 }}
                value={nameDe}
                onChange={(e) => setNameDe(e.target.value)}
                placeholder="z. B. Architektur-Entscheidungsprotokoll (ADR)"
              />
            </label>
            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              Kurzbeschreibung
              <input
                className="input-field"
                style={{ marginTop: 4 }}
                value={descDe}
                onChange={(e) => setDescDe(e.target.value)}
                placeholder="Wofür wird dieses Recipe genutzt?"
              />
            </label>
            <label style={{ fontSize: 12.5, fontWeight: 600 }}>
              Verfügbar in (Scope)
              <select
                className="select-field"
                style={{ marginTop: 4 }}
                value={scope}
                onChange={(e) => setScope(e.target.value as Recipe['scope'])}
              >
                <option value="single_meeting">Single Meeting (Live & Post-Call)</option>
                <option value="multi_meeting">Multi-Meeting (Personen & Wochen-Synthese)</option>
                <option value="ambient_workspace">Ambient Workspace (Fenster + Meetings)</option>
              </select>
            </label>
            <div>
              <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 5 }}>
                Platzhalter per Klick einfügen:
              </div>
              <div style={{ display: 'flex', gap: 5, flexWrap: 'wrap', marginBottom: 8 }}>
                {TEMPLATE_VARS.map((v) => (
                  <button
                    key={v}
                    type="button"
                    className="mini-btn"
                    style={{ fontFamily: 'var(--font-mono)' }}
                    onClick={() => setPromptTemplate((prev) => `${prev} ${v}`)}
                  >
                    {v}
                  </button>
                ))}
              </div>
              <textarea
                className="textarea-field"
                rows={5}
                value={promptTemplate}
                onChange={(e) => setPromptTemplate(e.target.value)}
              />
            </div>
            <button type="submit" id="create-recipe-submit" className="btn btn-primary">
              <Plus size={14} />
              {lang === 'de' ? 'Recipe speichern & für MCP aktivieren' : 'Save Recipe & Expose via MCP'}
            </button>
          </form>
        </section>
      </div>
    </div>
  );
};
