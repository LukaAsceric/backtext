import React, { useEffect, useState } from 'react';
import { Star, Plus, Trash2, Play, LayoutTemplate, Pencil, RotateCcw, CheckCircle2, Sparkles } from 'lucide-react';
import type { Language, MeetingTemplate, Recipe } from '../types';
import type { MeetingTemplateDraft } from './MeetingWorkspace';

interface RecipesStudioViewProps {
  lang: Language;
  initialTab?: 'recipes' | 'templates';
  recipes: Recipe[];
  templates: MeetingTemplate[];
  defaultTemplateId: string;
  onSaveRecipe: (recipe: Recipe) => Promise<void>;
  onDeleteRecipe: (id: string) => Promise<void>;
  onRunRecipeInToday: (slug: string) => Promise<void>;
  onSaveTemplate: (tpl: MeetingTemplateDraft) => Promise<void>;
  onDeleteTemplate: (id: string) => Promise<void>;
  onResetTemplate: (id: string) => Promise<void>;
  onSetDefaultTemplate: (id: string) => Promise<void>;
}

const TEMPLATE_VARS = [
  '{{transcript}}',
  '{{user_notes}}',
  '{{screenshare_ocr}}',
  '{{participants}}',
  '{{active_context}}',
  '{{user_profile}}',
];

const EMPTY_TPL_DRAFT: MeetingTemplateDraft = {
  name_de: '',
  description_de: '',
  sections_prompt:
    'Erstelle eine kompakte Zusammenfassung (max. 200 Wörter):\n\n### Kurzüberblick\nMax. 2 Sätze zum Kernziel.\n\n### Kernpunkte\n- 3 bis 5 knappe Stichpunkte mit [MM:SS]-Belegen.\n\n### Entscheidungen & Nächste Schritte\nNur konkrete Beschlüsse und Aufgaben.',
};

export const RecipesStudioView: React.FC<RecipesStudioViewProps> = ({
  lang,
  initialTab = 'recipes',
  recipes,
  templates,
  defaultTemplateId,
  onSaveRecipe,
  onDeleteRecipe,
  onRunRecipeInToday,
  onSaveTemplate,
  onDeleteTemplate,
  onResetTemplate,
  onSetDefaultTemplate,
}) => {
  const [studioTab, setStudioTab] = useState<'recipes' | 'templates'>(initialTab);
  const [scopeFilter, setScopeFilter] = useState<'all' | Recipe['scope']>('all');
  const [slug, setSlug] = useState('/custom-brief');
  const [nameDe, setNameDe] = useState('');
  const [descDe, setDescDe] = useState('');
  const [scope, setScope] = useState<Recipe['scope']>('single_meeting');
  const [promptTemplate, setPromptTemplate] = useState(
    'Analysiere {{transcript}} und {{user_notes}} im Hinblick auf konkrete Architekturentscheidungen und offene Risiken.',
  );

  const [editingTpl, setEditingTpl] = useState<MeetingTemplateDraft>(EMPTY_TPL_DRAFT);
  const [editingBuiltin, setEditingBuiltin] = useState(false);
  const [tplSavedBanner, setTplSavedBanner] = useState<string | null>(null);

  useEffect(() => {
    setStudioTab(initialTab);
  }, [initialTab]);

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

  const handleSelectTemplateForEdit = (tpl: MeetingTemplate) => {
    setEditingTpl({
      id: tpl.id,
      name_de: tpl.name_de,
      name_en: tpl.name_en,
      description_de: tpl.description_de,
      description_en: tpl.description_en,
      icon_key: tpl.icon_key,
      sections_prompt: tpl.sections_prompt,
    });
    setEditingBuiltin(tpl.is_builtin);
    setTplSavedBanner(null);
  };

  const handleSaveTemplateForm = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editingTpl.name_de.trim() || !editingTpl.sections_prompt.trim()) return;
    await onSaveTemplate(editingTpl);
    setTplSavedBanner(
      lang === 'de'
        ? `Template „${editingTpl.name_de}“ gespeichert.`
        : `Template "${editingTpl.name_de}" saved.`,
    );
    if (!editingTpl.id) {
      setEditingTpl(EMPTY_TPL_DRAFT);
      setEditingBuiltin(false);
    }
    window.setTimeout(() => setTplSavedBanner(null), 2600);
  };

  return (
    <div className="page-container">
      <header className="page-header">
        <div>
          <h1 className="page-title">
            {lang === 'de' ? 'Recipes & Templates Studio' : 'Recipes & Templates Studio'}
          </h1>
          <p className="page-subtitle">
            {lang === 'de'
              ? 'Verwalte deine Meeting-Zusammenfassungstemplates (inkl. „Allgemeines Meeting“) sowie wiederverwendbare /-Recipes für Chat & MCP.'
              : 'Manage your meeting summary templates (including built-ins) and reusable / recipes for chat and MCP.'}
          </p>
        </div>

        <div className="tab-strip">
          <button
            type="button"
            id="studio-tab-recipes"
            className={`tab-btn ${studioTab === 'recipes' ? 'active' : ''}`}
            onClick={() => setStudioTab('recipes')}
          >
            <Sparkles size={13} style={{ marginRight: 5, verticalAlign: 'middle' }} />
            {lang === 'de' ? `KI-Recipes (${recipes.length})` : `AI Recipes (${recipes.length})`}
          </button>
          <button
            type="button"
            id="studio-tab-templates"
            className={`tab-btn ${studioTab === 'templates' ? 'active' : ''}`}
            onClick={() => setStudioTab('templates')}
          >
            <LayoutTemplate size={13} style={{ marginRight: 5, verticalAlign: 'middle' }} />
            {lang === 'de'
              ? `Meeting-Templates (${templates.length})`
              : `Meeting Templates (${templates.length})`}
          </button>
        </div>
      </header>

      {/* TAB 1: Slash Recipes */}
      {studioTab === 'recipes' && (
        <>
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
        </>
      )}

      {/* TAB 2: Dedicated Meeting Summary Templates Manager */}
      {studioTab === 'templates' && (
        <div className="two-col-grid" style={{ alignItems: 'start' }}>
          <section
            className="surface-sheet"
            id="meeting-templates-list"
            style={{ display: 'flex', flexDirection: 'column', gap: 10 }}
          >
            <div className="section-heading" style={{ marginBottom: 2 }}>
              <span>
                {lang === 'de' ? 'Zusammenfassungs-Templates' : 'Summary Templates'}
              </span>
              <button
                type="button"
                id="new-template-reset-form-btn"
                className="mini-btn active-choice"
                onClick={() => {
                  setEditingTpl(EMPTY_TPL_DRAFT);
                  setEditingBuiltin(false);
                }}
              >
                <Plus size={11} style={{ marginRight: 3, verticalAlign: 'middle' }} />
                {lang === 'de' ? 'Neues Template' : 'New Template'}
              </button>
            </div>
            <p style={{ margin: 0, fontSize: 12.2, color: 'var(--text-secondary)' }}>
              {lang === 'de'
                ? 'Alle Templates — auch „Allgemeines Meeting“ — lassen sich direkt anpassen. Das als Standard markierte Template wird automatisch für neue Meetings verwendet.'
                : 'Every template — including built-ins — can be edited directly. The default template is applied automatically to new meetings.'}
            </p>

            {templates.map((tpl) => {
              const isDefault = defaultTemplateId === tpl.id;
              const isEditing = editingTpl.id === tpl.id;
              return (
                <div
                  key={tpl.id}
                  id={`template-card-${tpl.id}`}
                  className="transcript-row"
                  style={{
                    borderColor: isEditing ? 'var(--accent-sage)' : undefined,
                    background: isEditing ? 'var(--accent-sage-soft)' : undefined,
                  }}
                >
                  <div className="transcript-meta">
                    <div style={{ display: 'flex', alignItems: 'center', gap: 7, flexWrap: 'wrap' }}>
                      <strong style={{ fontSize: 13.5 }}>
                        {lang === 'de' ? tpl.name_de : tpl.name_en}
                      </strong>
                      {isDefault && (
                        <span className="pill pill-sage">
                          <CheckCircle2 size={10} />
                          {lang === 'de' ? 'Standard' : 'Default'}
                        </span>
                      )}
                      <span className="pill pill-neutral">
                        {tpl.is_builtin
                          ? lang === 'de'
                            ? 'Vorlage'
                            : 'Built-in'
                          : lang === 'de'
                            ? 'Eigenes'
                            : 'Custom'}
                      </span>
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 5, flexWrap: 'wrap' }}>
                      <button
                        type="button"
                        id={`edit-template-btn-${tpl.id}`}
                        className="mini-btn"
                        onClick={() => handleSelectTemplateForEdit(tpl)}
                      >
                        <Pencil size={11} style={{ marginRight: 3, verticalAlign: 'middle' }} />
                        {lang === 'de' ? 'Bearbeiten' : 'Edit'}
                      </button>
                      {!isDefault && (
                        <button
                          type="button"
                          id={`set-default-template-btn-${tpl.id}`}
                          className="mini-btn"
                          onClick={() => onSetDefaultTemplate(tpl.id)}
                        >
                          {lang === 'de' ? 'Als Standard' : 'Set Default'}
                        </button>
                      )}
                      {tpl.is_builtin ? (
                        <button
                          type="button"
                          id={`reset-template-btn-${tpl.id}`}
                          className="mini-btn"
                          title={lang === 'de' ? 'Auf Standard-Prompt zurücksetzen' : 'Reset to default prompt'}
                          onClick={async () => {
                            await onResetTemplate(tpl.id);
                            if (editingTpl.id === tpl.id) {
                              setEditingTpl(EMPTY_TPL_DRAFT);
                              setEditingBuiltin(false);
                            }
                          }}
                        >
                          <RotateCcw size={11} />
                        </button>
                      ) : (
                        <button
                          type="button"
                          id={`delete-template-btn-${tpl.id}`}
                          className="mini-btn"
                          title={lang === 'de' ? 'Löschen' : 'Delete'}
                          onClick={async () => {
                            await onDeleteTemplate(tpl.id);
                            if (editingTpl.id === tpl.id) {
                              setEditingTpl(EMPTY_TPL_DRAFT);
                              setEditingBuiltin(false);
                            }
                          }}
                        >
                          <Trash2 size={11} />
                        </button>
                      )}
                    </div>
                  </div>
                  <div style={{ fontSize: 12.4, color: 'var(--text-secondary)' }}>
                    {lang === 'de' ? tpl.description_de : tpl.description_en}
                  </div>
                  <pre className="code-block" style={{ fontSize: 11.4, maxHeight: 112, overflowY: 'auto' }}>
                    {tpl.sections_prompt}
                  </pre>
                </div>
              );
            })}
          </section>

          <section className="surface-sheet" id="meeting-template-editor">
            <h2 className="section-heading">
              <span>
                {editingTpl.id
                  ? lang === 'de'
                    ? `Template bearbeiten: ${editingTpl.name_de}`
                    : `Edit Template: ${editingTpl.name_de}`
                  : lang === 'de'
                    ? 'Neues Meeting-Template erstellen'
                    : 'Create Meeting Template'}
              </span>
              {tplSavedBanner && <span className="pill pill-sage">{tplSavedBanner}</span>}
            </h2>

            <form
              onSubmit={handleSaveTemplateForm}
              style={{ display: 'flex', flexDirection: 'column', gap: 11 }}
            >
              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                {lang === 'de' ? 'Name *' : 'Name *'}
                <input
                  id="studio-template-name-input"
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={editingTpl.name_de}
                  onChange={(e) => setEditingTpl({ ...editingTpl, name_de: e.target.value })}
                  placeholder={lang === 'de' ? 'z. B. Kompaktes Status-Update' : 'e.g. Compact Status Sync'}
                  required
                />
              </label>

              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                {lang === 'de' ? 'Kurzbeschreibung' : 'Short Description'}
                <input
                  id="studio-template-desc-input"
                  className="input-field"
                  style={{ marginTop: 4 }}
                  value={editingTpl.description_de}
                  onChange={(e) => setEditingTpl({ ...editingTpl, description_de: e.target.value })}
                  placeholder={
                    lang === 'de'
                      ? 'Wofür soll dieses Template genutzt werden?'
                      : 'When should this template be used?'
                  }
                />
              </label>

              <label style={{ fontSize: 12.5, fontWeight: 600 }}>
                {lang === 'de'
                  ? 'Gliederung & Anweisungen für die KI-Zusammenfassung *'
                  : 'Sections & AI Summary Instructions *'}
                <textarea
                  id="studio-template-prompt-input"
                  className="textarea-field"
                  style={{ marginTop: 4, minHeight: 190, fontFamily: 'var(--font-mono)', fontSize: 12 }}
                  value={editingTpl.sections_prompt}
                  onChange={(e) =>
                    setEditingTpl({ ...editingTpl, sections_prompt: e.target.value })
                  }
                  required
                />
              </label>

              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <button type="submit" id="studio-save-template-btn" className="btn btn-primary">
                  <LayoutTemplate size={14} />
                  {editingTpl.id
                    ? lang === 'de'
                      ? 'Änderungen speichern'
                      : 'Save Changes'
                    : lang === 'de'
                      ? 'Template erstellen'
                      : 'Create Template'}
                </button>
                {editingTpl.id && editingBuiltin && (
                  <button
                    type="button"
                    id="studio-reset-editing-template-btn"
                    className="btn"
                    onClick={async () => {
                      if (!editingTpl.id) return;
                      await onResetTemplate(editingTpl.id);
                      setEditingTpl(EMPTY_TPL_DRAFT);
                      setEditingBuiltin(false);
                    }}
                  >
                    <RotateCcw size={13} />
                    {lang === 'de' ? 'Auf Werkseinstellung zurücksetzen' : 'Reset to Built-in Default'}
                  </button>
                )}
                {editingTpl.id && (
                  <button
                    type="button"
                    className="btn"
                    onClick={() => {
                      setEditingTpl(EMPTY_TPL_DRAFT);
                      setEditingBuiltin(false);
                    }}
                  >
                    {lang === 'de' ? 'Abbrechen' : 'Cancel'}
                  </button>
                )}
              </div>
            </form>
          </section>
        </div>
      )}
    </div>
  );
};
