import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

function normalizeHttpTarget(url: string): string {
  return url.replace(/\/$/, '');
}

function resolveDevProxyHttpTarget(): string {
  const fromApiBase = process.env.VITE_API_BASE_URL?.trim();
  if (fromApiBase && fromApiBase.length > 0) {
    return normalizeHttpTarget(fromApiBase);
  }

  const fromTauriBackend = process.env.VITE_TAURI_BACKEND_URL?.trim();
  if (fromTauriBackend && fromTauriBackend.length > 0) {
    return normalizeHttpTarget(fromTauriBackend);
  }

  return 'http://localhost:8080';
}

function resolveDevProxyWsTarget(httpTarget: string): string {
  return httpTarget.replace(/^http:/, 'ws:').replace(/^https:/, 'wss:');
}

const devProxyHttpTarget = resolveDevProxyHttpTarget();
const devProxyWsTarget = resolveDevProxyWsTarget(devProxyHttpTarget);

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    port: 3000,
    proxy: {
      '/upload': devProxyHttpTarget,
      '/process': devProxyHttpTarget,
      '/history': devProxyHttpTarget,
      '/tasks': devProxyHttpTarget,
      '/search': devProxyHttpTarget,
      '/similar': devProxyHttpTarget,
      '/delete': devProxyHttpTarget,
      '/cancel': devProxyHttpTarget,
      '/models': devProxyHttpTarget,
      '/download': devProxyHttpTarget,
      '/shutdown': devProxyHttpTarget,
      '/reset': devProxyHttpTarget,
      '/update_filename': devProxyHttpTarget,
      '/update_stt_text': devProxyHttpTarget,
      '/check_existing_stt': devProxyHttpTarget,
      '/reset_summary_embedding': devProxyHttpTarget,
      '/reset_all_tasks': devProxyHttpTarget,
      '/delete_records': devProxyHttpTarget,
      '/progress': devProxyHttpTarget,
      '/file_search': devProxyHttpTarget,
      '/cache': devProxyHttpTarget,
      '/incremental_embedding': devProxyHttpTarget,
      '/segments': devProxyHttpTarget,
      '/api': devProxyHttpTarget,
      '/ws': {
        target: devProxyWsTarget,
        ws: true,
      },
    },
  },
});
