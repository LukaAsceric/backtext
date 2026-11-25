import React, { useMemo, useState } from 'react';
import { Send, Sparkles, BookOpen } from 'lucide-react';
import type { ChatMessage, Language, Recipe } from '../types';
import { t } from '../lib/i18n';
import { FormattedMarkdown } from './FormattedMarkdown';

interface RecipeChatBarProps {
  lang: Language;
  meetingId?: string | null;
  scopeFilter?: 'single_meeting' | 'all';
  recipes: Recipe[];
  messages: ChatMessage[];
  streamingText?: string | null;
  busy: boolean;
  onExecute: (input: string, recipeSlug?: string) => Promise<void>;
}

export const RecipeChatBar: React.FC<RecipeChatBarProps> = ({
  lang,
  scopeFilter = 'all',
  recipes,
  messages,
  streamingText,
  busy,
  onExecute,
}) => {
  const [input, setInput] = useState('');
  const [showSlash, setShowSlash] = useState(false);

  const availableRecipes = useMemo(() => {
    const base =
      scopeFilter === 'single_meeting'
        ? recipes.filter((r) => r.scope === 'single_meeting' || r.is_favorite)
        : recipes;
    if (!input.startsWith('/')) return base;
    const q = input.toLowerCase();
    return base.filter(
      (r) =>
        r.slug.toLowerCase().includes(q) ||
        r.name_de.toLowerCase().includes(q.slice(1)) ||
        r.name_en.toLowerCase().includes(q.slice(1)),
    );
  }, [recipes, scopeFilter, input]);

  const quickChips = useMemo(
    () =>
      recipes
        .filter((r) =>
          scopeFilter === 'single_meeting' ? r.scope === 'single_meeting' : r.is_favorite,
        )
        .slice(0, 5),
    [recipes, scopeFilter],
  );

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || busy) return;
    const val = input.trim();
    setInput('');
    setShowSlash(false);
    await onExecute(val, val.startsWith('/') ? val.split(/\s+/)[0] : undefined);
  };

  const handleSelectRecipe = async (rec: Recipe) => {
    setShowSlash(false);
    setInput('');
    await onExecute(rec.slug, rec.slug);
  };

  return (
    <div className="chat-panel">
      <div className="section-heading" style={{ marginBottom: 4 }}>
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
          <Sparkles size={15} />
          {lang === 'de'
            ? 'KI-Assistent & Granola-Recipes (/ tippen)'
            : 'AI Assistant & Granola Recipes (type /)'}
        </span>
        <button
          type="button"
          className="mini-btn"
          id="toggle-slash-recipes-btn"
          onClick={() => setShowSlash((prev) => !prev)}
        >
          / Recipes ({recipes.length})
        </button>
      </div>

      <div className="recipe-chips-row">
        {quickChips.map((rec) => (
          <button
            key={rec.id}
            type="button"
            className="recipe-chip"
            disabled={busy}
            onClick={() => handleSelectRecipe(rec)}
            title={lang === 'de' ? rec.description_de : rec.description_en}
          >
            <span className="recipe-slug">{rec.slug}</span>
            <span>{lang === 'de' ? rec.name_de : rec.name_en}</span>
          </button>
        ))}
      </div>

      {(messages.length > 0 || (streamingText && streamingText.length > 0)) && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10, maxHeight: 360, overflowY: 'auto' }}>
          {messages.map((msg) => (
            <div key={msg.id} className={`chat-bubble ${msg.role === 'user' ? 'user' : 'assistant'}`}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--text-secondary)' }}>
                  {msg.role === 'user'
                    ? lang === 'de'
                      ? 'Du'
                      : 'You'
                    : 'backtext AI'}
                  {msg.recipe_slug && (
                    <span className="recipe-slug" style={{ marginLeft: 8 }}>
                      {msg.recipe_slug}
                    </span>
                  )}
                </span>
                <span className="mono-ts">
                  {new Date(msg.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                </span>
              </div>
              <FormattedMarkdown text={msg.content} />
              {msg.sources && msg.sources.length > 0 && (
                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginTop: 4 }}>
                  {msg.sources.map((src, idx) => (
                    <span key={`${src.ref_id}-${idx}`} className="pill pill-neutral" title={src.snippet}>
                      <BookOpen size={11} />
                      {src.kind}: {src.label.slice(0, 42)}
                    </span>
                  ))}
                </div>
              )}
            </div>
          ))}
          {streamingText && streamingText.length > 0 && (
            <div className="chat-bubble assistant">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--accent-sage)' }}>
                  backtext AI · streaming…
                </span>
              </div>
              <FormattedMarkdown text={streamingText} />
            </div>
          )}
        </div>
      )}

      <form className="chat-input-wrap" onSubmit={handleSubmit}>
        {(showSlash || input.startsWith('/')) && availableRecipes.length > 0 && (
          <div className="slash-menu" role="listbox">
            {availableRecipes.map((rec) => (
              <button
                key={rec.id}
                type="button"
                className="slash-item"
                onClick={() => handleSelectRecipe(rec)}
              >
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    <span className="recipe-slug">{rec.slug}</span>
                    <span style={{ fontWeight: 600, fontSize: 13 }}>
                      {lang === 'de' ? rec.name_de : rec.name_en}
                    </span>
                  </div>
                  <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginTop: 2 }}>
                    {lang === 'de' ? rec.description_de : rec.description_en}
                  </div>
                </div>
                <span className="pill pill-neutral">{rec.scope.replace('_', ' ')}</span>
              </button>
            ))}
          </div>
        )}

        <input
          id="recipe-chat-input"
          className="input-field"
          value={input}
          onChange={(e) => {
            setInput(e.target.value);
            if (e.target.value.startsWith('/')) setShowSlash(true);
          }}
          placeholder={t(lang, 'chatPlaceholder')}
          disabled={busy}
        />
        <button type="submit" id="recipe-chat-submit" className="btn btn-primary" disabled={busy || !input.trim()}>
          <Send size={14} />
          {busy ? '…' : t(lang, 'send')}
        </button>
      </form>
    </div>
  );
};
