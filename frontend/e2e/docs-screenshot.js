// Capture de captures d'ecran pour docs/ (sujet 13b) a partir de la suite E2E
// existante (sujet 13a a pose les marqueurs <!-- SCREENSHOT: ... --> dans
// docs/*.md). Reutilise par scenario-runner.js (action "screenshot" dans les
// scenarios JSON) et par les fichiers *.spec.js/.mjs classiques qui capturent
// un etat non modelisable en scenario JSON pur.
//
// Desactive par defaut (no-op) : ne prend une vraie capture que si
// DOCS_SCREENSHOTS est positionne (fait par playwright.docs-screenshots.config.js,
// jamais par playwright.config.js standard) -- voir frontend/e2e/README.md
// pour ne jamais ralentir/declencher la suite E2E standard (`npm run test:e2e`).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SCREENSHOTS_DIR = path.join(__dirname, '..', '..', 'docs', 'screenshots');

export const DOCS_SCREENSHOTS_ENABLED = !!process.env.DOCS_SCREENSHOTS;

// Prend une capture nommee `filename` (ex. "accueil-liste-services.png") dans
// docs/screenshots/ si DOCS_SCREENSHOTS_ENABLED, sinon ne fait rien (simple
// verification booleenne, cout negligeable sur la suite standard).
export async function docsScreenshot(page, filename) {
  if (!DOCS_SCREENSHOTS_ENABLED) return;
  fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });
  await page.screenshot({ path: path.join(SCREENSHOTS_DIR, filename) });
}
