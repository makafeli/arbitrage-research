import { defineConfig } from 'vite';

export default defineConfig({
  server: { host: '127.0.0.1', strictPort: true, proxy: { '/v1': { target: 'http://127.0.0.1:8080', changeOrigin: false } } },
  preview: { host: '127.0.0.1', strictPort: true },
  build: { target: 'es2022' },
});
