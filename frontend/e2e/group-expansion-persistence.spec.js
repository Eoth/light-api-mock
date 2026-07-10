import { test, expect } from '@playwright/test';

const API = 'http://localhost:7342/api';

// Verifie le niveau 1 de persistance de l'etat "groupe deplie/replie" (cf
// CLAUDE.md, group-expansion-state.svelte.js) : l'etat doit survivre a une
// navigation vers l'edition d'un service et retour (store partage hors du
// cycle de vie de ServiceList.svelte), mais PAS a un rechargement complet de
// la page (F5) -- limite volontaire, verifiee explicitement ci-dessous pour
// ne pas la confondre avec une regression future.

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

async function expandGroup(page, groupName) {
  const header = page.locator('.group-header', { hasText: groupName });
  if (await header.getAttribute('aria-expanded') === 'false') {
    await header.click();
  }
}

test.describe('Persistance de l etat deplie/replie des groupes', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('un groupe deplie reste visible apres retour depuis l edition d un service', async ({ page, request }) => {
    await request.post(`${API}/groups`, { data: { name: 'persist-grp', code: '', admins: [], members: [] } });
    await request.post(`${API}/services`, {
      data: validService('persist-svc', { group_name: 'persist-grp' }),
    });

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await expandGroup(page, 'persist-grp');

    const card = page.locator('.service-card', { hasText: 'persist-svc' });
    await expect(card).toBeVisible();

    await card.getByRole('button', { name: 'Configurer le service persist-svc' }).click();
    await expect(page.getByRole('heading', { name: 'persist-svc', exact: true })).toBeVisible();

    await page.getByRole('button', { name: '← Retour' }).click();

    // Retour a la liste : le groupe doit rester deplie sans reclic manuel.
    const header = page.locator('.group-header', { hasText: 'persist-grp' });
    await expect(header).toHaveAttribute('aria-expanded', 'true');
    await expect(page.locator('.service-card', { hasText: 'persist-svc' })).toBeVisible();
  });

  test('un rechargement complet de la page (F5) reinitialise l etat deplie', async ({ page, request }) => {
    await request.post(`${API}/groups`, { data: { name: 'reload-grp', code: '', admins: [], members: [] } });
    await request.post(`${API}/services`, {
      data: validService('reload-svc', { group_name: 'reload-grp' }),
    });

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await expandGroup(page, 'reload-grp');
    await expect(page.locator('.service-card', { hasText: 'reload-svc' })).toBeVisible();

    await page.reload();
    await page.waitForLoadState('networkidle');

    // Comportement voulu (niveau 1, pas une regression) : F5 reinitialise
    // l'etat en memoire, le groupe redemarre replie.
    const header = page.locator('.group-header', { hasText: 'reload-grp' });
    await expect(header).toHaveAttribute('aria-expanded', 'false');
    await expect(page.locator('.service-card', { hasText: 'reload-svc' })).not.toBeVisible();
  });
});
