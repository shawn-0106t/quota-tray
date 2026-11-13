import { defineConfig } from 'vite';
import { resolve } from 'path';

// Multi-page build: one webview window per page (main panel / settings / tray menu)
export default defineConfig({
  base: './',
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Cargo build outputs are locked/rewritten while rustc runs
      ignored: ['**/src-tauri/target/**'],
    },
  },
  build: {
    target: 'es2021',
    // Inline small assets as data URIs so no asset:// protocol is needed
    // (nothing qualifies today; the 100 KB budget stays for future icons)
    assetsInlineLimit: 100 * 1024,
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        settings: resolve(__dirname, 'settings.html'),
        skills: resolve(__dirname, 'skills.html'),
        menu: resolve(__dirname, 'menu.html'),
      },
    },
  },
});
