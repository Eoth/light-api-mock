import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 15000,
  workers: 1,
  use: {
    baseURL: 'http://localhost:7342',
    headless: true,
    // Tests read the interface in its source language; i18n.spec.js covers the French one.
    locale: 'en-US',
  },
});
