import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  Send,
  Sparkles,
  BookOpen,
  ChevronDown,
  ChevronUp,
  Copy,
  Check,
  Slash,
  Mic,
  Square,
  FilePlus2,
  X,
  FolderOpen,
} from 'lucide-react';
import type { ChatMessage, DictationEntry, Language, Recipe, TranscriptSegment } from '../types';
import { callRpc } from '../lib/ipc';
import { t } from '../lib/i18n';
import { FormattedMarkdown } from './FormattedMarkdown';

interface RecipeChatBarProps {
  lang: Language;
  meetingId?: string | null;
  scopeFilter?: 'single_meeting' | 'all';
  scopeChip?: { label: string; onClear: () => void } | null;
  compact?: boolean;
  autoFocus?: boolean;
  focusSignal?: number;
  recipes: Recipe[];
  messages: ChatMessage[];
  transcript?: TranscriptSegment[];
  onJumpToTranscript?: (segmentId: string) => void;
  onOpenMeeting?: (meetingId: string) => void;
  onApplyToNotes?: (markdown: string) => Promise<void>;
  streamingText?: string | null;
  busy: boolean;
  onExecute: (input: string, recipeSlug?: string) => Promise<void>;
  onEscape?: () => void;
}

export const RecipeChatBar: React.FC<RecipeChatBarProps> = ({
  lang,
  scopeFilter = 'all',
  scopeChip = null,
  compact = false,
  autoFocus = false,
  focusSignal = 0,
  recipes,
  messages,
  transcript,
  onJumpToTranscript,
  onOpenMeeting,
  onApplyToNotes,
  streamingText,
  busy,
  onExecute,
  onEscape,
}) => {
  const [input, setInput] = useState('');
  const [showSlash, setShowSlash] = useState(false);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [appliedId, setAppliedId] = useState<string | null>(null);
  const [dictating, setDictating] = useState(false);
  const [dictationBusy, setDictationBusy] = useState(false);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const hasHistory = messages.length > 0 || Boolean(streamingText && streamingText.length > 0);

  useEffect(() => {
    if (autoFocus || focusSignal > 0) {
      const raf = window.requestAnimationFrame(() => {
        inputRef.current?.focus();
      });
      return () => window.cancelAnimationFrame(raf);
    }
  }, [autoFocus, focusSignal]);

  useEffect(() => {
    if (streamingText && streamingText.length > 0) {
      setDrawerOpen(true);
    }
  }, [streamingText]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages.length, streamingText, drawerOpen]);

  const availableRecipes = useMemo(() => {
    const base =
      scopeFilter === 'single_meeting'
        ? recipes.filter((r) => r.scope === 'single_meeting' || r.is_favorite)
        : recipes;
    if (!input.startsWith('/')) return base;
    const firstToken = input.split(/\s+/)[0].toLowerCase();
    const query = firstToken.slice(1);
    if (!query) return base;
    return base.filter(
      (r) =>
        r.slug.toLowerCase().includes(firstToken) ||
        r.name_de.toLowerCase().includes(query) ||
        r.name_en.toLowerCase().includes(query),
    );
  }, [recipes, scopeFilter, input]);

  // Keep selected index valid when filtered list changes
  useEffect(() => {
    setSelectedIdx(0);
  }, [input, availableRecipes.length, showSlash]);

  const isSlashMenuVisible =
    (showSlash || (input.startsWith('/') && !input.includes(' '))) &&
    availableRecipes.length > 0;

  const quickChips = useMemo(
    () =>
      recipes
        .filter((r) =>
          scopeFilter === 'single_meeting' ? r.scope === 'single_meeting' : r.is_favorite,
        )
        .slice(0, compact ? 4 : 5),
    [recipes, scopeFilter, compact],
  );

  const runPrompt = async (rawValue: string, explicitSlug?: string) => {
    const val = rawValue.trim();
    if (!val || busy) return;
    setInput('');
    setShowSlash(false);
    setDrawerOpen(true);
    const slug = explicitSlug ?? (val.startsWith('/') ? val.split(/\s+/)[0] : undefined);
    await onExecute(val, slug);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || busy) return;
    if (isSlashMenuVisible && input.startsWith('/') && !input.includes(' ')) {
      const exact = availableRecipes.find(
        (r) => r.slug.toLowerCase() === input.trim().toLowerCase(),
      );
      const chosen = exact ?? availableRecipes[selectedIdx] ?? availableRecipes[0];
      if (chosen) {
        await runPrompt(chosen.slug, chosen.slug);
        return;
      }
    }
    await runPrompt(input);
  };

  const handleSelectRecipe = async (rec: Recipe) => {
    await runPrompt(rec.slug, rec.slug);
  };

  const handleCopyMessage = async (msg: ChatMessage) => {
    try {
      await navigator.clipboard.writeText(msg.content);
      setCopiedId(msg.id);
      window.setTimeout(() => setCopiedId((prev) => (prev === msg.id ? null : prev)), 1600);
    } catch {
      // ignore clipboard errors in restricted environments
    }
  };

  const handleApplyMessageToNotes = async (msg: ChatMessage) => {
    if (!onApplyToNotes) return;
    await onApplyToNotes(msg.content);
    setAppliedId(msg.id);
    window.setTimeout(() => setAppliedId((prev) => (prev === msg.id ? null : prev)), 1800);
  };

  const handleToggleChatMic = async () => {
    if (dictationBusy) return;
    setDictationBusy(true);
    try {
      const res = await callRpc<{ active: boolean; entry?: DictationEntry }>('toggle_voice_typing', {
        mode: 'chat',
      });
      setDictating(res.active);
      if (!res.active && res.entry?.cleaned_text) {
        setInput((prev) => {
          const base = prev.trim();
          return base ? `${base} ${res.entry!.cleaned_text}` : res.entry!.cleaned_text;
        });
        window.requestAnimationFrame(() => inputRef.current?.focus());
      }
    } catch {
      setDictating(false);
    } finally {
      setDictationBusy(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (isSlashMenuVisible) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIdx((prev) => (prev + 1) % availableRecipes.length);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIdx((prev) => (prev - 1 + availableRecipes.length) % availableRecipes.length);
        return;
      }
      if (e.key === 'Tab') {
        e.preventDefault();
        const chosen = availableRecipes[selectedIdx] ?? availableRecipes[0];
        if (chosen) {
          setInput(`${chosen.slug} `);
          setShowSlash(false);
          window.requestAnimationFrame(() => {
            if (inputRef.current) {
              const len = chosen.slug.length + 1;
              inputRef.current.setSelectionRange(len, len);
            }
          });
        }
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        setShowSlash(false);
        return;
      }
    } else if (e.key === 'Escape') {
      if (onEscape) {
        e.preventDefault();
        e.stopPropagation();
        onEscape();
      }
    }
  };

  const showMessages = compact ? hasHistory && drawerOpen : hasHistory;

  return (
    <div className={compact ? 'chat-panel chat-panel-compact' : 'chat-panel'}>
      {!compact && (
        <div className="section-heading" style={{ marginBottom: 2 }}>
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
            <Sparkles size={15} style={{ color: 'var(--accent-sage)' }} />
            {lang === 'de'
              ? 'KI-Assistent & Granola-Recipes (/ + Tab)'
              : 'AI Assistant & Granola Recipes (/ + Tab)'}
          </span>
          <span className="mono-ts">{recipes.length} Recipes</span>
        </div>
      )}

      {scopeChip && (
        <div className="chat-scope-banner" id="active-chat-scope-chip">
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <FolderOpen size={13} style={{ color: 'var(--accent-sage)' }} />
            <strong>{lang === 'de' ? 'Kontext-Fokus:' : 'Chat Scope:'}</strong>
            <span>{scopeChip.label}</span>
          </span>
          <button
            type="button"
            className="mini-btn"
            onClick={scopeChip.onClear}
            style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
          >
            <X size={11} />
            {lang === 'de' ? 'Aufheben' : 'Clear'}
          </button>
        </div>
      )}

      {showMessages && (
        <div ref={scrollRef} className={`chat-thread-scroll ${compact ? 'compact' : ''}`}>
          {messages.map((msg) => {
            const isUser = msg.role === 'user';
            return (
              <div
                key={msg.id}
                className={`chat-bubble ${isUser ? 'user' : 'assistant'}`}
              >
                <div className="chat-bubble-header">
                  <span className="chat-bubble-author">
                    {!isUser && <Sparkles size={11} style={{ color: 'var(--accent-sage)' }} />}
                    <span>
                      {isUser
                        ? lang === 'de'
                          ? 'Du'
                          : 'You'
                        : 'backtext AI'}
                    </span>
                    {msg.recipe_slug && (
                      <span className="recipe-slug">{msg.recipe_slug}</span>
                    )}
                  </span>
                  <div style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                    <span className="mono-ts">
                      {new Date(msg.created_at).toLocaleTimeString([], {
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                    </span>
                    {!isUser && onApplyToNotes && (
                      <button
                        type="button"
                        className="chat-apply-btn"
                        onClick={() => handleApplyMessageToNotes(msg)}
                        title={
                          lang === 'de'
                            ? 'Antwort direkt in KI-Notizen übernehmen'
                            : 'Apply answer to AI Notes'
                        }
                      >
                        {appliedId === msg.id ? <Check size={11} /> : <FilePlus2 size={11} />}
                        <span>
                          {appliedId === msg.id
                            ? lang === 'de'
                              ? 'Übernommen'
                              : 'Applied'
                            : lang === 'de'
                              ? 'In KI-Notizen'
                              : 'Apply to Notes'}
                        </span>
                      </button>
                    )}
                    {!isUser && (
                      <button
                        type="button"
                        className="chat-copy-btn"
                        onClick={() => handleCopyMessage(msg)}
                        title={lang === 'de' ? 'Antwort kopieren' : 'Copy answer'}
                      >
                        {copiedId === msg.id ? <Check size={12} /> : <Copy size={12} />}
                      </button>
                    )}
                  </div>
                </div>
                <FormattedMarkdown
                  text={msg.content}
                  transcript={transcript}
                  onJumpToTranscript={onJumpToTranscript}
                />
                {msg.sources && msg.sources.length > 0 && (
                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginTop: 4 }}>
                    {msg.sources.map((src, idx) => {
                      const canOpenMeeting = src.kind === 'meeting' && Boolean(onOpenMeeting);
                      return canOpenMeeting ? (
                        <button
                          key={`${src.ref_id}-${idx}`}
                          type="button"
                          className="pill pill-sage"
                          style={{ cursor: 'pointer', border: '1px solid rgba(47, 107, 83, 0.24)' }}
                          title={src.snippet}
                          onClick={() => onOpenMeeting?.(src.ref_id)}
                        >
                          <BookOpen size={11} />
                          {src.label.slice(0, 42)} ↗
                        </button>
                      ) : (
                        <span
                          key={`${src.ref_id}-${idx}`}
                          className="pill pill-neutral"
                          title={src.snippet}
                        >
                          <BookOpen size={11} />
                          {src.kind}: {src.label.slice(0, 42)}
                        </span>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
          {streamingText && streamingText.length > 0 && (
            <div className="chat-bubble assistant streaming">
              <div className="chat-bubble-header">
                <span className="chat-bubble-author" style={{ color: 'var(--accent-sage)' }}>
                  <Sparkles size={11} />
                  backtext AI · streaming…
                </span>
              </div>
              <FormattedMarkdown
                text={streamingText}
                transcript={transcript}
                onJumpToTranscript={onJumpToTranscript}
              />
            </div>
          )}
        </div>
      )}

      {/* Quick Recipe Pills Strip + Compact History Toggle */}
      <div className="chat-quick-strip">
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

        {compact && hasHistory && (
          <button
            type="button"
            className="mini-btn"
            onClick={() => setDrawerOpen((prev) => !prev)}
            style={{ display: 'inline-flex', alignItems: 'center', gap: 4, flexShrink: 0 }}
          >
            {drawerOpen ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
            Chat ({messages.length})
          </button>
        )}
      </div>

      {/* Elevated Pill Chat Composer with Slash Menu, Tab Autocomplete & Voice Dictation Mic */}
      <form className="chat-input-wrap" onSubmit={handleSubmit}>
        {isSlashMenuVisible && (
          <div className="slash-menu" role="listbox" aria-label="Recipes">
            <div className="slash-menu-list">
              {availableRecipes.map((rec, idx) => (
                <button
                  key={rec.id}
                  type="button"
                  role="option"
                  aria-selected={idx === selectedIdx}
                  className={`slash-item ${idx === selectedIdx ? 'active' : ''}`}
                  onMouseEnter={() => setSelectedIdx(idx)}
                  onClick={() => handleSelectRecipe(rec)}
                >
                  <div style={{ minWidth: 0 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                      <span className="recipe-slug">{rec.slug}</span>
                      <span style={{ fontWeight: 600, fontSize: 13 }}>
                        {lang === 'de' ? rec.name_de : rec.name_en}
                      </span>
                    </div>
                    <div
                      style={{
                        fontSize: 12,
                        color: 'var(--text-secondary)',
                        marginTop: 2,
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {lang === 'de' ? rec.description_de : rec.description_en}
                    </div>
                  </div>
                  <span className="pill pill-neutral" style={{ flexShrink: 0 }}>
                    {idx === selectedIdx ? 'Tab ↹' : rec.scope.replace('_', ' ')}
                  </span>
                </button>
              ))}
            </div>
            <div className="slash-menu-footer">
              <span>↑↓ {lang === 'de' ? 'Wählen' : 'Select'}</span>
              <span>Tab {lang === 'de' ? 'Vervollständigen' : 'Autocomplete'}</span>
              <span>Enter {lang === 'de' ? 'Ausführen' : 'Run'}</span>
              <span>Esc {lang === 'de' ? 'Schließen' : 'Close'}</span>
            </div>
          </div>
        )}

        <div className="chat-composer">
          <button
            type="button"
            id="toggle-slash-recipes-btn"
            className={`composer-slash-btn ${isSlashMenuVisible ? 'active' : ''}`}
            title={lang === 'de' ? 'Recipes öffnen (/ tippen, Tab zum Vervollständigen)' : 'Open Recipes (type /, Tab to complete)'}
            onClick={() => {
              setShowSlash((prev) => !prev);
              inputRef.current?.focus();
            }}
          >
            <Slash size={13} />
          </button>

          <input
            ref={inputRef}
            id="recipe-chat-input"
            className="composer-input"
            value={input}
            onChange={(e) => {
              const v = e.target.value;
              setInput(v);
              if (v.startsWith('/') && !v.includes(' ')) {
                setShowSlash(true);
              } else if (!v.startsWith('/')) {
                setShowSlash(false);
              }
            }}
            onKeyDown={handleKeyDown}
            placeholder={
              dictating
                ? lang === 'de'
                  ? 'Diktat läuft — erneut auf Mikrofon klicken zum Einfügen…'
                  : 'Listening — click mic again to insert…'
                : scopeChip
                  ? lang === 'de'
                    ? `Frage über „${scopeChip.label}“ stellen…`
                    : `Ask across "${scopeChip.label}"…`
                  : compact
                    ? lang === 'de'
                      ? 'Frage zum Meeting, /rewrite zum Umschreiben oder / Recipe…'
                      : 'Ask about this meeting, /rewrite to edit notes, or / Recipe…'
                    : t(lang, 'chatPlaceholder')
            }
            disabled={busy}
            autoComplete="off"
          />

          <button
            type="button"
            id="chat-voice-mic-btn"
            className={`composer-mic-btn ${dictating ? 'active' : ''}`}
            disabled={busy || dictationBusy}
            onClick={handleToggleChatMic}
            title={
              dictating
                ? lang === 'de'
                  ? 'Diktat beenden & Text einfügen'
                  : 'Stop dictation & insert text'
                : lang === 'de'
                  ? 'Per Sprache diktieren (Voice Typing)'
                  : 'Dictate prompt with Voice Typing'
            }
          >
            {dictating ? <Square size={12} /> : <Mic size={14} />}
          </button>

          <button
            type="submit"
            id="recipe-chat-submit"
            className="composer-send-btn"
            disabled={busy || !input.trim()}
            title={t(lang, 'send')}
          >
            <Send size={14} />
          </button>
        </div>
      </form>
    </div>
  );
};
