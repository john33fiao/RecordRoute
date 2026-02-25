const DEFAULT_TAURI_BACKEND_HTTP = 'http://127.0.0.1:8080';
const DEFAULT_TAURI_BACKEND_WS = 'ws://127.0.0.1:8080/ws';

function trimUrl(value: string | undefined): string {
  return value?.trim() ?? '';
}

function isTauriRuntime(): boolean {
  if (typeof window === 'undefined') return false;
  return window.location.protocol === 'tauri:' || window.location.protocol === 'asset:';
}

export function resolveApiBaseUrl(): string {
  const tauriBackendUrl = trimUrl(import.meta.env.VITE_TAURI_BACKEND_URL).replace(/\/$/, '');
  const configuredApiBase = trimUrl(import.meta.env.VITE_API_BASE_URL).replace(/\/$/, '');
  if (configuredApiBase.length > 0) return configuredApiBase;
  if (tauriBackendUrl.length > 0) return tauriBackendUrl;
  if (isTauriRuntime()) return DEFAULT_TAURI_BACKEND_HTTP;
  return '';
}

export function resolveWebSocketUrl(): string {
  const configuredWsUrl = trimUrl(import.meta.env.VITE_WS_URL);
  if (configuredWsUrl.length > 0) return configuredWsUrl;

  const tauriBackendUrl = trimUrl(import.meta.env.VITE_TAURI_BACKEND_URL).replace(/\/$/, '');
  if (tauriBackendUrl.length > 0) {
    const asWsProtocol = tauriBackendUrl.replace(/^http:/, 'ws:').replace(/^https:/, 'wss:');
    return `${asWsProtocol}/ws`;
  }

  if (isTauriRuntime()) {
    return DEFAULT_TAURI_BACKEND_WS;
  }

  if (typeof window === 'undefined') {
    return DEFAULT_TAURI_BACKEND_WS;
  }

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/ws`;
}
