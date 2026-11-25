import React from 'react';
import { EyeOff, X, Monitor } from 'lucide-react';
import type { ChatMessage, ContextSnapshot, Language, Recipe } from '../types';
import { RecipeChatBar } from './RecipeChatBar';

interface HoverModalProps {
  open: boolean;
  standaloneWindow?: boolean;
  lang: Language;
  frontSnapshot: ContextSnapshot | undefined;
  recipes: Recipe[];
  messages: ChatMessage[];
  busy: boolean;
  onClose: () => void;
  onExecute: (input: string, recipeSlug?: string) => Promise<void>;
}

export const HoverModal: React.FC<HoverModalProps> = ({
  open,
  standaloneWindow = false,
  lang,
  frontSnapshot,
  recipes,
  messages,
  busy,
  onClose,
  onExecute,
}) => {
  if (!open && !standaloneWindow) return null;

  const dialog = (
    <div className="hover-dialog" style={standaloneWindow ? { width: '100%', height: '100vh', borderRadius: 0 } : undefined}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <strong>backtext Hover</strong>
          <span className="pill pill-sage" title="NSWindowSharingNone aktiv">
            <EyeOff size={11} />
            {lang === 'de' ? 'Unsichtbar bei Screen-Sharing' : 'Hidden from Screen Share'}
          </span>
        </div>
        {!standaloneWindow && (
          <button type="button" className="mini-btn" onClick={onClose}>
            <X size={13} /> ESC
          </button>
        )}
      </div>

      {frontSnapshot && (
        <div
          style={{
            padding: '8px 11px',
            borderRadius: 6,
            background: 'var(--bg-surface-subtle)',
            border: '1px solid var(--border-subtle)',
            fontSize: 12.2,
            color: 'var(--text-secondary)',
            display: 'flex',
            alignItems: 'center',
            gap: 8,
          }}
        >
          <Monitor size={13} />
          <span>
            <strong>Front-App-Kontext aktiv:</strong> {frontSnapshot.app_name} —{' '}
            {frontSnapshot.window_title}
          </span>
        </div>
      )}

      <RecipeChatBar
        lang={lang}
        scopeFilter="all"
        recipes={recipes}
        messages={messages}
        busy={busy}
        onExecute={onExecute}
      />
    </div>
  );

  if (standaloneWindow) {
    return dialog;
  }

  return (
    <div className="hover-backdrop" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()}>{dialog}</div>
    </div>
  );
};
