// Config Playwright dediee a la regeneration des captures d'ecran de docs/.
// Volontairement SEPAREE de playwright.config.js (utilise par
// `npm run test:e2e`) : positionner DOCS_SCREENSHOTS ici, dans le module de
// config (execute par Node avant le chargement des fichiers de test, donc
// fonctionne a l'identique sous PowerShell/cmd/bash sans syntaxe shell
// specifique), garantit que la suite standard ne prend jamais de capture --
// `docs-screenshot.js` est un no-op tant que cette variable n'est pas
// positionnee. Ne cible que les fichiers ou des etapes/appels
// `docsScreenshot()`/`{"action":"screenshot"}` ont ete ajoutes (voir
// frontend/e2e/README.md) ; les autres fichiers de la suite ne produisent
// aucune capture et n'ont pas besoin d'etre executes ici.
import { defineConfig } from '@playwright/test';

process.env.DOCS_SCREENSHOTS = '1';

export default defineConfig({
  testDir: './e2e',
  testMatch: [
    'scenario-runner.spec.js',
    'backups.spec.js',
    'rule-tester.spec.js',
    'messaging.spec.js',
    'config.spec.mjs',
    'rhai-autocomplete.spec.js',
  ],
  timeout: 15000,
  workers: 1,
  use: {
    baseURL: 'http://localhost:7342',
    headless: true,
  },
});
