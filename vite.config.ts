import { defineConfig, type UserConfig } from 'vite';
import react from '@vitejs/plugin-react';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(
  async () =>
    ({
      plugins: [react()],

      // prevent vite from obscuring rust errors
      clearScreen: false,

      server: {
        port: 1420,
        strictPort: true,
        host: host || false,
        hmr: host
          ? {
              protocol: 'ws',
              host,
              port: 1421,
            }
          : undefined,
        watch: {
          // tell vite to ignore watching `src-tauri`
          ignored: ['**/src-tauri/**'],
        },
      },

      // env vars starting with TAURI_ENV_* are exposed in tauri
      envPrefix: ['VITE_', 'TAURI_ENV_'],

      build: {
        // tauri uses Chromium on Windows and WebKit on macOS/Linux
        target:
          process.env.TAURI_ENV_PLATFORM == 'windows'
            ? 'chrome105'
            : 'safari13',
        // don't minify for debug builds
        minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
        // produce sourcemaps for debug builds
        sourcemap: !!process.env.TAURI_ENV_DEBUG,
      },
    }) as UserConfig,
);
