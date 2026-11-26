import React, { useEffect, useState } from 'react';
import { Mic, Square, CheckCircle2, Loader2 } from 'lucide-react';
import type { VoiceTypingStatePayload } from '../types';
import { callRpc, subscribeBacktextEvents } from '../lib/ipc';

/**
 * Global floating non-activating Dictation HUD window (`index.html?window=dictation`).
 * Floats transparently above all macOS apps (Word, Safari, Slack, Cursor) without stealing
 * keyboard focus so the user always sees live dictation status and can stop with a click or hotkey.
 */
export const DictationHud: React.FC = () => {
  const [state, setState] = useState<VoiceTypingStatePayload>({
    active: true,
    mode: 'system',
    status: 'Diktat aktiv — spricht…',
    text: null,
    target_app: null,
  });
  const [shortcutLabel, setShortcutLabel] = useState('⌥⇧ D');

  useEffect(() => {
    document.documentElement.classList.add('hud-window');
    document.body.classList.add('hud-window');

    callRpc<{ voice_typing_shortcut?: string }>('get_daemon_telemetry')
      .then((tel) => {
        if (tel?.voice_typing_shortcut) {
          setShortcutLabel(tel.voice_typing_shortcut);
        }
      })
      .catch(() => {});

    const unsub = subscribeBacktextEvents((ev) => {
      if (ev.event === 'voice_typing_state_changed') {
        const payload = ev.payload as VoiceTypingStatePayload;
        if (payload.mode === 'system') {
          setState(payload);
        }
      } else if (ev.event === 'daemon_telemetry_updated') {
        const tel = ev.payload as { voice_typing_shortcut?: string };
        if (tel?.voice_typing_shortcut) {
          setShortcutLabel(tel.voice_typing_shortcut);
        }
      }
    });

    return () => {
      unsub();
      document.documentElement.classList.remove('hud-window');
      document.body.classList.remove('hud-window');
    };
  }, []);

  const isProcessing = !state.active && state.status.includes('Transkribiert');
  const isDone = !state.active && !isProcessing;

  const handleStop = async () => {
    try {
      await callRpc('toggle_voice_typing', { mode: 'system' });
    } catch {
      // ignore
    }
  };

  return (
    <div className="dictation-hud-root" id="global-dictation-hud">
      <div
        className={`dictation-hud-pill ${
          state.active ? 'recording' : isProcessing ? 'processing' : 'done'
        }`}
      >
        {state.active ? (
          <span className="dictation-hud-dot" />
        ) : isProcessing ? (
          <Loader2 size={14} className="dictation-hud-spin" />
        ) : (
          <CheckCircle2 size={14} style={{ color: '#5FD19B', flexShrink: 0 }} />
        )}

        <Mic
          size={14}
          style={{
            color: state.active ? '#FF8B6A' : 'rgba(255,255,255,0.72)',
            flexShrink: 0,
          }}
        />

        <div className="dictation-hud-copy">
          <div className="dictation-hud-status" title={state.status}>
            {state.status}
          </div>
          {isDone && state.text && (
            <div className="dictation-hud-preview" title={state.text}>
              {state.text}
            </div>
          )}
        </div>

        {state.active ? (
          <button
            type="button"
            id="hud-stop-dictation-btn"
            className="dictation-hud-stop-btn"
            onClick={handleStop}
            title={`Diktat beenden (${shortcutLabel})`}
          >
            <Square size={9} fill="currentColor" />
            <span>Stop</span>
          </button>
        ) : (
          <span className="dictation-hud-kbd">{shortcutLabel}</span>
        )}
      </div>
    </div>
  );
};
