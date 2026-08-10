import process from 'node:process';

import { defineConfig } from '@vben/vite-config';

// The A-factory UI talks to its co-located Agent by default. Review fixtures
// and a VM-hosted Agent remain opt-in through this environment variable.
const localViewApi =
  process.env.FUSTFS_LOCAL_REVIEW_API ?? 'http://127.0.0.1:18471';

export default defineConfig(async () => {
  return {
    application: {},
    vite: {
      build: {
        emptyOutDir: true,
        outDir: '../../../../build/web',
      },
      cacheDir: '../../../../tmp/web-vite-cache',
      server: {
        proxy: {
          '/api': {
            changeOrigin: true,
            target: localViewApi,
          },
        },
      },
    },
  };
});
