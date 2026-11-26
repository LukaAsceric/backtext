import React, { useEffect, useState } from 'react';
import { EyeOff, X, Monitor } from 'lucide-react';
import type { ChatMessage, ContextSnapshot, Language, Recipe } from '../types';
import { hideNativeHoverOverlay, subscribeHoverShown } from '../lib/ipc';
import { RecipeChatBar } from './RecipeChatBar';

interface HoverModalProps {
  open: boolean;
  standaloneWindow?: boolean;
  lang: Language;
  frontSnapshot: ContextSnapshot | undefined;
  recipes: Recipe[];
  messages: ChatMessage[];
  streamingText?: string | null;
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
  streamingText,
  busy,
  onClose,
  onExecute,
}) => {
  const [focusSignal, setFocusSignal] = useState(0);

  // Always auto-focus the Hover chat input whenever the modal opens, window gains focus, or Tauri reveals the hover overlay
  useEffect(() => {
    if (open || standaloneWindow) {
      setFocusSignal((n) => n + 1);
    }
  }, [open, standaloneWindow]);

  useEffect(() => {
    if (!open && !standaloneWindow) return;
    const bumpFocus = () => setFocusSignal((n) => n + 1);
    window.addEventListener('focus', bumpFocus);
    const unsub = subscribeHoverShown(bumpFocus);
    return () => {
      window.removeEventListener('focus', bumpFocus);
      unsub();
    };
  }, [open, standaloneWindow]);

  if (!open && !standaloneWindow) return null;

  const handleDismiss = async () => {
    if (standaloneWindow) {
      await hideNativeHoverOverlay();
    } else {
      onClose();
    }
  };

  const dialog = (
    <div
      className="hover-dialog"
      style={standaloneWindow ? { width: '100%', height: '100vh', borderRadius: 0 } : undefined}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <strong>backtext Hover</strong>
          <span className="pill pill-sage" title="NSWindowSharingNone aktiv">
            <EyeOff size={11} />
            {lang === 'de' ? 'Unsichtbar bei Screen-Sharing' : 'Hidden from Screen Share'}
          </span>
        </div>
        <button type="button" className="mini-btn" onClick={handleDismiss}>
          <X size={13} /> ESC
        </button>
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
        autoFocus
        focusSignal={focusSignal}
        recipes={recipes}
        messages={messages}
        streamingText={streamingText}
        busy={busy}
        onExecute={onExecute}
        onEscape={handleDismiss}
      />
    </div>
  );

  if (standaloneWindow) {
    return dialog;
  }

  return (
    <div className="hover-backdrop" onClick={handleDismiss}>
      <div onClick={(e) => e.stopPropagation()}>{dialog}</div>
    </div>
  );
};
