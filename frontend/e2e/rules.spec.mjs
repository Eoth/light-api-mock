import { test, expect } from '@playwright/test';

const API = 'http://localhost:7342/api';

function validService(name, overrides = {}) {
  return {
    name,
    listen_path: '/e2e/*',
    real_target_url: 'http://e2e:80',
    is_mocked: true,
    rewrite_directory_urls: false,
    group_name: null,
    wsdl_mode: 'auto',
    rules: [],
    ...overrides,
  };
}

function validRule(name, overrides = {}) {
  return {
    name,
    method: 'GET',
    sub_path: null,
    action: 'mock',
    script: null,
    conditions: { all_of: [], any_of: [] },
    response: { status: 200, headers: [{ name: 'Content-Type', value: 'application/json' }], body: [{ type: 'Literal', value: '{"ok":true}' }], chaos: null },
    ...overrides,
  };
}

const svcPayload = validService('e2e-svc', {
  rules: [
    validRule('rule-alpha'),
    validRule('rule-beta', {
      conditions: { all_of: [{ source: { type: 'QueryParam', key: 'id' }, operator: { type: 'Eq', value: '42' } }], any_of: [] },
    }),
  ],
});

test.beforeEach(async ({ request }) => {
  await request.delete(`${API}/config/reset`);
  await request.post(`${API}/services`, { data: svcPayload });
});

async function goToServiceDetail(page) {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  const group = page.locator('button[aria-expanded]').first();
  if (await group.getAttribute('aria-expanded') === 'false') {
    await group.click();
    await page.waitForTimeout(200);
  }
  await page.getByRole('button', { name: /Configurer/ }).first().click();
  await page.waitForTimeout(300);
}

test('affiche les regles existantes', async ({ page }) => {
  await goToServiceDetail(page);
  await expect(page.getByText('rule-alpha')).toBeVisible();
  await expect(page.getByText('rule-beta')).toBeVisible();
});

test('bouton ajouter une regle fonctionne avec regles existantes', async ({ page }) => {
  await goToServiceDetail(page);
  await page.getByRole('button', { name: /Ajouter une regle/ }).click();
  await expect(page.locator('form[aria-label*="regle"]')).toBeVisible();
});

test('bouton modifier (crayon) ouvre le formulaire', async ({ page }) => {
  await goToServiceDetail(page);
  await page.locator('button[title="Modifier"]').first().click();
  await expect(page.locator('form[aria-label*="regle"]')).toBeVisible();
  const nameInput = page.locator('input#rule-name');
  await expect(nameInput).toHaveValue('rule-alpha');
});

test('bouton supprimer retire la regle', async ({ page }) => {
  await goToServiceDetail(page);
  await page.locator('button[title="Supprimer"]').first().click();
  await page.waitForTimeout(500);
  await expect(page.getByText('rule-alpha')).not.toBeVisible();
  await expect(page.getByText('rule-beta')).toBeVisible();
});

test('toggle mock/proxy fonctionne', async ({ page }) => {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await page.getByRole('switch').first().click();
  await page.waitForTimeout(500);
});

test('recherche filtre les services', async ({ page, request }) => {
  await request.post(`${API}/services`, { data: validService('other-svc', { listen_path: '/other/*' }) });
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await page.getByPlaceholder(/Rechercher/).fill('xyz-unique');
  await page.waitForTimeout(500);
  await expect(page.getByText(/Aucun service ne correspond/)).toBeVisible();
});

test('annuler le formulaire de regle revient a la liste', async ({ page }) => {
  await goToServiceDetail(page);
  await page.getByRole('button', { name: /Ajouter une regle/ }).click();
  await expect(page.locator('form[aria-label*="regle"]')).toBeVisible();
  await page.getByRole('button', { name: 'Annuler' }).click();
  await expect(page.getByText('rule-alpha')).toBeVisible();
});

test('creer une nouvelle regle via le formulaire', async ({ page }) => {
  await goToServiceDetail(page);
  await page.getByRole('button', { name: /Ajouter une regle/ }).click();
  await page.locator('input#rule-name').fill('new-rule');
  await page.getByRole('button', { name: /Ajouter la regle/ }).click();
  await page.waitForTimeout(500);
  await expect(page.getByText('new-rule', { exact: true })).toBeVisible();
});

test('ajouter un service via le formulaire', async ({ page }) => {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await page.getByRole('button', { name: /Ajouter un service/ }).click();
  await page.locator('input#svc-name').fill('new-svc');
  await page.locator('input#svc-target').clear();
  await page.locator('input#svc-target').fill('http://new-svc:80');
  await page.getByRole('button', { name: 'Ajouter' }).click();
  await page.waitForTimeout(500);
  await expect(page.getByRole('heading', { name: 'new-svc' })).toBeVisible();
});
