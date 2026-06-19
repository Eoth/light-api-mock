import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 15000,
  workers: 1,
  use: {
    baseURL: 'http://localhost:7342',
    headless: true,
  },
});
