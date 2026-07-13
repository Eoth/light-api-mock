import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { createServer } from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Cas particulier de la suite E2E : contrairement a tous les autres fichiers
// *.spec.js/*.spec.mjs (qui ciblent le backend PARTAGE deja demarre sur
// http://localhost:7342 avec AUTH_ENABLED=false, cf frontend/e2e/README.md),
// ce spec demarre SA PROPRE instance de lightMock, sur un port dedie, avec
// AUTH_ENABLED=true. Impossible de reutiliser l'instance partagee pour ce
// besoin : activer l'auth dessus casserait tous les 80+ autres tests de la
// suite, qui supposent tous AUTH_ENABLED=false (cf CLAUDE.md, "l'environnement
// E2E tourne avec AUTH_ENABLED=false"). Necessite le binaire deja compile
// (`cargo build`, target/debug/light-mock(.exe)) et frontend/dist deja
// buildee (memes prerequis que `npm run build`/`cargo build`) -- pas de build
// automatique dans ce spec, comme le reste de la suite E2E qui suppose deja
// un environnement pret.
//
// Contexte du bug corrige : auth_middleware (src/auth/middleware.rs)
// n'exemptait auparavant que 4 routes /api/auth/*, jamais les assets
// statiques de la SPA (index.html, bundle JS/CSS, favicon) servis par
// ServeDir (fallback_service, server/mod.rs::build_router). Avec
// AUTH_ENABLED=true, un navigateur sans token ne pouvait donc meme pas
// charger la page qui affiche l'ecran de connexion (LoginForm.svelte) --
// probleme de poule et l'oeuf. Corrige en exemptant precisement ces assets
// (is_static_asset_route, src/server/validation.rs), jamais /api/*.

const repoRoot = path.resolve(fileURLToPath(new URL('.', import.meta.url)), '..', '..');
const binaryPath = path.join(
  repoRoot,
  'target',
  'debug',
  process.platform === 'win32' ? 'light-mock.exe' : 'light-mock',
);
const staticDir = path.join(repoRoot, 'frontend', 'dist');

function getFreePort() {
  return new Promise((resolve, reject) => {
    const srv = createServer();
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
    srv.on('error', reject);
  });
}

async function waitForHealth(baseUrl, timeoutMs = 15000) {
  const start = Date.now();
  let lastError;
  while (Date.now() - start < timeoutMs) {
    try {
      const res = await fetch(`${baseUrl}/api/health`);
      if (res.ok) return;
    } catch (e) {
      lastError = e;
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw new Error(
    `lightMock (instance auth-enabled dediee a ce spec) n'a pas demarre a temps sur ${baseUrl}: ${lastError}`,
  );
}

test.describe('Auth: assets statiques de la SPA accessibles sans token (AUTH_ENABLED=true)', () => {
  let child;
  let baseUrl;
  let dataDir;

  test.beforeAll(async () => {
    const port = await getFreePort();
    baseUrl = `http://127.0.0.1:${port}`;
    dataDir = mkdtempSync(path.join(tmpdir(), 'lightmock-auth-e2e-'));

    child = spawn(binaryPath, [], {
      cwd: repoRoot,
      env: {
        ...process.env,
        PORT: String(port),
        STATIC_DIR: staticDir,
        DATA_PATH: dataDir,
        AUTH_ENABLED: 'true',
        // Keycloak factice : jamais reellement contacte par ces tests (aucun
        // login n'est exerce ici) -- juste assez pour satisfaire la garde de
        // demarrage AuthConfig::from_env() (panique si vide quand enabled).
        KEYCLOAK_URL: 'http://127.0.0.1:1',
        KEYCLOAK_REALM: 'test-realm',
        KEYCLOAK_CLIENT_ID: 'lightmock',
        SUPER_ADMINS: '',
      },
      stdio: 'pipe',
    });

    await waitForHealth(baseUrl);
  });

  test.afterAll(() => {
    if (child) {
      child.kill();
    }
    if (dataDir) {
      rmSync(dataDir, { recursive: true, force: true });
    }
  });

  test('la page d accueil se charge sans token et affiche l ecran de connexion', async ({ page }) => {
    const response = await page.goto(baseUrl + '/');
    expect(response.status()).toBe(200);
    // Preuve que index.html ET le bundle JS/CSS ont bien charge (pas juste le
    // HTML brut) : le frontend a demarre, interroge /api/auth/status (deja
    // exempte), constate enabled=true et affiche LoginForm.svelte plutot que
    // la liste des services.
    await expect(page.locator('[data-testid="login-form-username-input"]')).toBeVisible();
    await expect(page.locator('[data-testid="login-form-password-input"]')).toBeVisible();
  });

  test('un fichier du bundle assets/ reel se charge sans token', async ({ request }) => {
    const assetFiles = readdirSync(path.join(staticDir, 'assets'));
    const realFile = assetFiles.find((f) => f.endsWith('.js')) || assetFiles[0];
    const res = await request.get(`${baseUrl}/assets/${realFile}`);
    expect(res.status()).toBe(200);
  });

  test('un appel a une route API protegee sans token echoue toujours (401)', async ({ request }) => {
    // Regression cible : le bypass des assets statiques ne doit RIEN
    // assouplir cote API -- une route protegee arbitraire (hors les 4 deja
    // exemptees) doit continuer d'exiger un token.
    const res = await request.get(`${baseUrl}/api/services`);
    expect(res.status()).toBe(401);
    const body = await res.json();
    expect(body.error).toBe('Token manquant');
  });
});
