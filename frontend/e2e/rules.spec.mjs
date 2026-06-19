import { test, expect } from '@playwright/test';

const BASE = 'http://localhost:3000';

const svcPayload = {
  name: 'e2e-svc',
  listen_path: '/e2e/*',
  real_target_url: 'http://e2e:80',
  is_mocked: true,
  rewrite_directory_urls: false,
  rules: [
    { name: 'rule-alpha', conditions: { all_of: [], any_of: [] }, response: { status: 200, headers: [{ name: 'Content-Type', value: 'application/json' }], body: [{ type: 'Literal', value: '{"ok":true}' }], chaos: null } },
    { name: 'rule-beta', conditions: { all_of: [{ source: { type: 'QueryParam', key: 'id' }, operator: { type: 'Eq', value: '42' } }], any_of: [] }, response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'beta' }], chaos: null } },
  ],
};

test.beforeEach(async () => {
  await fetch(`${BASE}/api/services/e2e-svc`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(svcPayload) });
});

test.afterEach(async () => {
  await fetch(`${BASE}/api/services/e2e-svc`, { method: 'DELETE' });
});

async function goToServiceDetail(page) {
  await page.goto(BASE);
  await page.waitForLoadState('networkidle');
  await page.getByRole('button', { name: /Configurer/ }).first().click();
  await page.waitForTimeout(300);
}

test('affiche les regles existantes', async ({ page }) => {
  await goToServiceDetail(page);
  await expect(page.getByText('rule-alpha')).toBeVisible();
  await expect(page.getByText('rule-beta')).toBeVisible();
  await expect(page.getByText('Catch-all')).toBeVisible();
  await expect(page.getByText('1 condition')).toBeVisible();
});

test('bouton ajouter une regle fonctionne avec regles existantes', async ({ page }) => {
  await goToServiceDetail(page);
  await page.getByRole('button', { name: /Ajouter une regle/ }).click();
  await expect(page.locator('form[aria-label*="regle"]')).toBeVisible();
  await expect(page.locator('input#rule-name')).toBeVisible();
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
  await page.goto(BASE);
  await page.waitForLoadState('networkidle');
  await expect(page.getByRole('status', { name: /mock/i })).toBeVisible();
  await page.getByRole('switch').click();
  await page.waitForTimeout(500);
  await expect(page.getByRole('status', { name: /proxy/i })).toBeVisible();
});

test('recherche filtre les services', async ({ page }) => {
  await fetch(`${BASE}/api/services/other-svc`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: 'other-svc', listen_path: '/other/*', real_target_url: 'http://other:80', is_mocked: false, rules: [] }) });
  try {
    await page.goto(BASE);
    await page.waitForLoadState('networkidle');
    await expect(page.getByRole('heading', { name: 'e2e-svc' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'other-svc' })).toBeVisible();
    await page.getByPlaceholder('Rechercher').fill('e2e');
    await expect(page.getByRole('heading', { name: 'e2e-svc' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'other-svc' })).not.toBeVisible();
  } finally {
    await fetch(`${BASE}/api/services/other-svc`, { method: 'DELETE' });
  }
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

test('ajouter un service via le bouton et le formulaire', async ({ page }) => {
  await page.goto(BASE);
  await page.waitForLoadState('networkidle');
  await page.getByRole('button', { name: /Ajouter un service/ }).click();
  await expect(page.locator('form')).toBeVisible();
  await page.locator('input#svc-name').fill('new-svc');
  await page.locator('input#svc-path').clear();
  await page.locator('input#svc-path').fill('/new-svc/*');
  await page.locator('input#svc-target').clear();
  await page.locator('input#svc-target').fill('http://new-svc:80');
  await page.getByRole('button', { name: 'Ajouter' }).click();
  await page.waitForTimeout(500);
  await expect(page.getByRole('heading', { name: 'new-svc' })).toBeVisible();
  await fetch(`${BASE}/api/services/new-svc`, { method: 'DELETE' });
});
