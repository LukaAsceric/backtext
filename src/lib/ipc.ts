import { invoke } from '@tauri-apps/api/core';

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
