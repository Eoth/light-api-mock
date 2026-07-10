import { test, expect } from '@playwright/test';

const API = 'http://localhost:7342/api';

function validService(name, overrides = {}) {
  return {
    name,
    listen_path: '/v1/*',
    real_target_url: 'http://backend:8080',
    is_mocked: true,
    rewrite_directory_urls: false,
    group_name: null,
    wsdl_mode: 'auto',
    rules: [],
    ...overrides,
  };
}

async function backupFilenames(request) {
  const res = await request.get(`${API}/config/backups`);
  const backups = await res.json();
  return new Set(backups.map((b) => b.filename));
}

// Identifie le fichier de backup cree par une ecriture precise en comparant
// l'ensemble des noms de fichiers avant/apres (difference d'ensembles) —
// evite toute hypothese fragile sur le tri ou les timestamps, meme si
// d'autres tests de la suite ont deja rempli backups/ auparavant.
async function newBackupFilename(request, action) {
  const before = await backupFilenames(request);
  await action();
  const after = await backupFilenames(request);
  const added = [...after].filter((f) => !before.has(f));
  expect(added.length).toBe(1);
  return added[0];
}

test.describe('Config backups & restore', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('GET /api/config/backups returns metadata only (no YAML content)', async ({ request }) => {
    await request.post(`${API}/services`, { data: validService('meta-svc') });
    const res = await request.get(`${API}/config/backups`);
    expect(res.ok()).toBe(true);
    const backups = await res.json();
    expect(Array.isArray(backups)).toBe(true);
    expect(backups.length).toBeGreaterThan(0);
    for (const b of backups) {
      expect(typeof b.filename).toBe('string');
      expect(typeof b.protected).toBe('boolean');
      expect(typeof b.size_bytes).toBe('number');
      expect(typeof b.created_at_ms).toBe('number');
    }
  });

  test('restore rejects path traversal filename (400)', async ({ request }) => {
    const res = await request.post(`${API}/config/restore/${encodeURIComponent('../mock-config.yaml')}`);
    expect(res.status()).toBe(400);
  });

  test('restore rejects unknown filename (404)', async ({ request }) => {
    const res = await request.post(`${API}/config/restore/mock-config-9999999999999-000042.yaml`);
    expect(res.status()).toBe(404);
  });

  // NB: le refus "sans droits admin" (403 via require_super_admin) n'est pas
  // testable en E2E dans cet environnement : le serveur de test tourne avec
  // AUTH_ENABLED=false (par defaut), auquel cas AuthUser::anonymous() a
  // is_super_admin=true et require_super_admin() passe toujours — il n'existe
  // pas de session "utilisateur non-admin" reelle a produire sans backend
  // Keycloak (meme limitation deja acceptee pour /api/config/reset, cf
  // CLAUDE.md). Le garde-fou lui-meme est couvert par un test unitaire Rust
  // (server::api::tests::require_super_admin_rejects_non_admin) et le
  // comportement UI en cas de 403 par un test Vitest simulant le rejet
  // (BackupManager.test.js : "notifie une erreur quand le backend refuse").

  test('UI restore flow: click through to a restored config', async ({ page, request }) => {
    const targetFilename = await newBackupFilename(request, () =>
      request.post(`${API}/services`, { data: validService('restore-target-svc') })
    );

    // Une seconde ecriture modifie l'etat courant : elle doit disparaitre une
    // fois la restauration effectuee vers le backup capture ci-dessus (etat
    // "aucun service").
    await request.post(`${API}/services`, { data: validService('restore-decoy-svc') });

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.getByText('Sauvegardes', { exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Sauvegardes de configuration' })).toBeVisible();

    const row = page.locator('.backup-card', { hasText: targetFilename });
    await expect(row).toBeVisible();
    await row.getByText('Restaurer').click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.locator('#confirm-keyword-input').fill('RESTAURER');
    await dialog.getByRole('button', { name: 'Restaurer' }).click();

    await expect(dialog).not.toBeVisible();

    await expect(async () => {
      const res = await request.get(`${API}/services`);
      const services = await res.json();
      expect(services.length).toBe(0);
    }).toPass();
  });

  test('UI restore flow: cancelling the confirmation does not change the config', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('cancel-flow-svc') });

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.getByText('Sauvegardes', { exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Sauvegardes de configuration' })).toBeVisible();

    const anyRow = page.locator('.backup-card').first();
    await expect(anyRow).toBeVisible();
    await anyRow.getByText('Restaurer').click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByText('Annuler').click();
    await expect(dialog).not.toBeVisible();

    const res = await request.get(`${API}/services`);
    const services = await res.json();
    expect(services.some((s) => s.name === 'cancel-flow-svc')).toBe(true);
  });
});
