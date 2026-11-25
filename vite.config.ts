import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: '0.0.0.0',
    port: 5173,
    strictPort: true,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:39421',
        changeOrigin: true,
      },
      '/mcp': {
        target: 'http://127.0.0.1:39421',
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
  },
});
