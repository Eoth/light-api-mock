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
    pre_script: null,
    script: null,
    post_script: null,
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

// "affiche les regles existantes" migre vers frontend/e2e/scenario-runner.spec.js
// (scenario JSON view-existing-rules.scenario.json) -- cf CLAUDE.md §6, sujet 9c lot 1.

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
  const group = page.locator('button[aria-expanded="false"]').first();
  await group.click();
  await page.waitForTimeout(200);
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

// "creer une nouvelle regle via le formulaire" migre vers frontend/e2e/scenario-runner.spec.js
// (scenario JSON create-simple-rule.scenario.json) -- cf CLAUDE.md §6, sujet 9c lot 1.

// "ajouter un service via le formulaire" migre vers frontend/e2e/scenario-runner.spec.js
// (scenario JSON create-service.scenario.json) -- cf CLAUDE.md §6, sujet 9c lot 1.
