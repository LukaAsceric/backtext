import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { BacktextRealtimeEvent } from '../types';

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__);
}

export async function callRpc<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (isTauriRuntime()) {
    return await invoke<T>('backtext_rpc', { cmd, args });
  }

  const response = await fetch(`/api/invoke/${encodeURIComponent(cmd)}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(args),
  });
  const payload = await response.json();
  if (!response.ok || !payload.ok) {
    throw new Error(payload.error || `RPC ${cmd} failed`);
  }
  return payload.data as T;
}

export async function toggleNativeHoverOverlay(): Promise<boolean> {
  if (isTauriRuntime()) {
    return await invoke<boolean>('toggle_hover_overlay');
  }
  return false;
}

export async function setNativeAutostart(enabled: boolean): Promise<boolean> {
  if (isTauriRuntime()) {
    return await invoke<boolean>('set_autostart_enabled', { enabled });
  }
  return enabled;
}

/**
 * Subscribes to real-time backend events (`ChatStreamDelta`, `DaemonTelemetryUpdated`,
 * `ModelDownloadProgress`, `CalendarSynced`) over Tauri native events or HTTP SSE (`/api/events`).
 */
export function subscribeBacktextEvents(
  handler: (ev: BacktextRealtimeEvent) => void,
): () => void {
  if (isTauriRuntime()) {
    let unlistenFn: (() => void) | null = null;
    listen<BacktextRealtimeEvent>('backtext://event', (event) => {
      handler(event.payload);
    }).then((unlisten) => {
      unlistenFn = unlisten;
    });
    return () => {
      unlistenFn?.();
    };
  }

  if (typeof EventSource !== 'undefined') {
    const es = new EventSource('/api/events');
    es.onmessage = (msg) => {
      try {
        const parsed = JSON.parse(msg.data) as BacktextRealtimeEvent;
        handler(parsed);
      } catch {
        // Ignore keepalive heartbeats
      }
    };
    return () => es.close();
  }

  return () => {};
}
