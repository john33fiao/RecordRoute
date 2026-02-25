import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

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
      '/upload': 'http://localhost:8080',
      '/process': 'http://localhost:8080',
      '/history': 'http://localhost:8080',
      '/tasks': 'http://localhost:8080',
      '/search': 'http://localhost:8080',
      '/similar': 'http://localhost:8080',
      '/delete': 'http://localhost:8080',
      '/cancel': 'http://localhost:8080',
      '/models': 'http://localhost:8080',
      '/download': 'http://localhost:8080',
      '/shutdown': 'http://localhost:8080',
      '/reset': 'http://localhost:8080',
      '/update_filename': 'http://localhost:8080',
      '/update_stt_text': 'http://localhost:8080',
      '/check_existing_stt': 'http://localhost:8080',
      '/reset_summary_embedding': 'http://localhost:8080',
      '/reset_all_tasks': 'http://localhost:8080',
      '/delete_records': 'http://localhost:8080',
      '/progress': 'http://localhost:8080',
      '/file_search': 'http://localhost:8080',
      '/cache': 'http://localhost:8080',
      '/incremental_embedding': 'http://localhost:8080',
      '/segments': 'http://localhost:8080',
      '/api': 'http://localhost:8080',
      '/ws': {
        target: 'ws://localhost:8080',
        ws: true,
      },
    },
  },
});
