import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { createServer } from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Sujet "URL de l'API configurable independamment du Host du frontend" (cf
// CLAUDE.md) : avant cette passe, le frontend deduisait toujours l'URL de
// l'API de son propre Host (chemin relatif /api/...). Ca casse des que
// l'infrastructure route /api vers une origine distincte de celle qui sert
// les assets statiques (ex. Kubernetes/Gloo Edge avec un VirtualService pour
// le front et un RouteTable/Upstream separe pour le back, cas reel
// rapporte). Ce spec verifie les DEUX comportements : (1) par defaut (rien
// configure), rien ne change pour un deploiement co-localise ; (2) une fois
// API_BASE_URL configure sur le processus qui sert la SPA, le frontend
// appelle bien l'API sur l'origine configuree, meme quand elle differe de
// celle qui sert la page elle-meme.
//
// Meme cas particulier que auth-static-assets.spec.js : ce spec demarre SES
// PROPRES instances de lightMock (jamais l'instance partagee sur
// http://localhost:7342, cf frontend/e2e/README.md), car le test 2 a
// justement besoin de DEUX instances sur deux ports distincts. Necessite le
// binaire deja compile (`cargo build`, target/debug/light-mock(.exe)) et
// frontend/dist deja buildee, comme le reste de la suite E2E qui suppose un
// environnement pret.

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
    `lightMock (instance dediee a ce spec) n'a pas demarre a temps sur ${baseUrl}: ${lastError}`,
  );
}

async function spawnLightMock({ port, dataDir, extraEnv = {} }) {
  const baseUrl = `http://127.0.0.1:${port}`;
  const child = spawn(binaryPath, [], {
    cwd: repoRoot,
    env: {
      ...process.env,
      PORT: String(port),
      STATIC_DIR: staticDir,
      DATA_PATH: dataDir,
      AUTH_ENABLED: 'false',
      ...extraEnv,
    },
    stdio: 'pipe',
  });
  await waitForHealth(baseUrl);
  return { child, baseUrl };
}

test.describe('URL de l API configurable independamment du Host du frontend', () => {
  test('comportement par defaut : /runtime-config.json renvoie une URL vide et l app fonctionne (co-localise)', async ({ page }) => {
    const dataDir = mkdtempSync(path.join(tmpdir(), 'lightmock-apibase-default-'));
    const port = await getFreePort();
    let child;
    try {
      ({ child } = await spawnLightMock({ port, dataDir }));
      const baseUrl = `http://127.0.0.1:${port}`;

      const configRes = await page.request.get(`${baseUrl}/runtime-config.json`);
      expect(configRes.status()).toBe(200);
      expect((await configRes.json()).api_base_url).toBe('');

      await page.goto(baseUrl + '/');
      // Preuve que l'app a bien appele /api/... en chemin relatif (comme
      // avant cette passe) et affiche la liste de services : le titre de la
      // page d'accueil est visible une fois le chargement initial termine.
      await expect(page.getByText('lightMock')).toBeVisible();
      await expect(page.locator('[data-testid="app-add-service-button"]')).toBeVisible();
    } finally {
      if (child) child.kill();
      rmSync(dataDir, { recursive: true, force: true });
    }
  });

  test('front et back sur des origines distinctes : la SPA appelle l API sur l URL configuree via API_BASE_URL', async ({ page }) => {
    const backDataDir = mkdtempSync(path.join(tmpdir(), 'lightmock-apibase-back-'));
    const frontDataDir = mkdtempSync(path.join(tmpdir(), 'lightmock-apibase-front-'));
    const backPort = await getFreePort();
    let frontPort = await getFreePort();
    if (frontPort === backPort) frontPort = await getFreePort();

    let backChild;
    let frontChild;
    try {
      ({ child: backChild } = await spawnLightMock({ port: backPort, dataDir: backDataDir }));
      const backBaseUrl = `http://127.0.0.1:${backPort}`;

      // Sert de "back" : seede un service directement sur cette instance,
      // jamais via le front — la preuve recherchee est que le FRONT (une
      // instance/store totalement distincte) affiche des donnees qui
      // n'existent que cote back.
      const createRes = await page.request.post(`${backBaseUrl}/api/services`, {
        data: {
          name: 'cross-origin-demo',
          listen_path: '/v1/*',
          real_target_url: '',
          is_mocked: true,
          rewrite_directory_urls: false,
          group_name: null,
          wsdl_mode: 'auto',
          rules: [],
        },
      });
      expect(createRes.status()).toBe(201);

      ({ child: frontChild } = await spawnLightMock({
        port: frontPort,
        dataDir: frontDataDir,
        extraEnv: { API_BASE_URL: backBaseUrl },
      }));
      const frontBaseUrl = `http://127.0.0.1:${frontPort}`;

      const configRes = await page.request.get(`${frontBaseUrl}/runtime-config.json`);
      expect(configRes.status()).toBe(200);
      expect((await configRes.json()).api_base_url).toBe(backBaseUrl);

      // La page est chargee depuis le FRONT, mais le service affiche ne peut
      // venir que du BACK (le store du front, lui, est vide) : preuve que le
      // frontend a bien appele l'API configuree, pas son propre Host.
      await page.goto(frontBaseUrl + '/');
      // Le groupe "Sans groupe" est replie par defaut (etat non persiste
      // au-dela de la session, cf CLAUDE.md) : le deplier avant de chercher
      // la carte de service.
      await page.locator('[data-testid="service-group-header-ungrouped"]').click();
      await expect(page.locator('[data-testid="service-card-cross-origin-demo"]')).toBeVisible();

      // Le front lui-meme n'a jamais recu ce service dans SON propre store.
      const frontOwnServices = await page.request.get(`${frontBaseUrl}/api/services`);
      const frontOwnBody = await frontOwnServices.json();
      expect(frontOwnBody.find((s) => s.name === 'cross-origin-demo')).toBeUndefined();
    } finally {
      if (frontChild) frontChild.kill();
      if (backChild) backChild.kill();
      rmSync(frontDataDir, { recursive: true, force: true });
      rmSync(backDataDir, { recursive: true, force: true });
    }
  });
});
