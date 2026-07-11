import { test, expect } from '@playwright/test';
import { docsScreenshot } from './docs-screenshot.js';

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

test.beforeEach(async ({ request }) => {
  await request.delete(`${API}/config/reset`);
  await request.post(`${API}/services`, { data: validService('autocomplete-svc') });
});

async function openAddRuleForm(page) {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  const group = page.locator('button[aria-expanded]').first();
  if (await group.getAttribute('aria-expanded') === 'false') {
    await group.click();
    await page.waitForTimeout(200);
  }
  await page.getByRole('button', { name: /Configurer/ }).first().click();
  await page.waitForTimeout(300);
  await page.getByRole('button', { name: /Ajouter une regle/ }).click();
}

test('autocompletion : la selection au clic insere la fonction avec ses parametres', async ({ page }) => {
  await openAddRuleForm(page);
  await page.locator('input#rule-name').fill('ac-rule-click');
  await page.getByRole('switch', { name: 'Script personnalise' }).click();

  const scriptField = page.locator('#rule-script');
  await scriptField.click();
  await scriptField.type('seed');

  const option = page.getByRole('option', { name: /seeded_pick/ });
  await expect(option).toBeVisible();
  await docsScreenshot(page, 'rhai-autocompletion.png');
  await option.click();

  await expect(scriptField).toHaveValue('seeded_pick(seed, ["a", "b"])');
  await expect(page.getByRole('listbox')).not.toBeVisible();
});

test('autocompletion : navigation clavier (fleches + Entree) insere la fonction active', async ({ page }) => {
  await openAddRuleForm(page);
  await page.locator('input#rule-name').fill('ac-rule-kbd');
  await page.getByRole('switch', { name: 'Script personnalise' }).click();

  const scriptField = page.locator('#rule-script');
  await scriptField.click();
  await scriptField.type('date_n');

  await expect(page.getByRole('listbox')).toBeVisible();
  await scriptField.press('Enter');

  await expect(scriptField).toHaveValue('date_now("iso")');
});

test('autocompletion : Echap ferme la liste sans rien inserer', async ({ page }) => {
  await openAddRuleForm(page);
  await page.locator('input#rule-name').fill('ac-rule-esc');
  await page.getByRole('switch', { name: 'Script personnalise' }).click();

  const scriptField = page.locator('#rule-script');
  await scriptField.click();
  await scriptField.type('uuid');
  await expect(page.getByRole('listbox')).toBeVisible();

  await scriptField.press('Escape');
  await expect(page.getByRole('listbox')).not.toBeVisible();
  await expect(scriptField).toHaveValue('uuid');
});

test('autocompletion : Ctrl+Espace ouvre la liste complete sans prefixe tape', async ({ page }) => {
  await openAddRuleForm(page);
  await page.locator('input#rule-name').fill('ac-rule-ctrlspace');
  await page.getByRole('switch', { name: 'Script personnalise' }).click();

  const scriptField = page.locator('#rule-script');
  await scriptField.click();
  await scriptField.press('Control+ ');

  const options = page.getByRole('option');
  await expect(options).not.toHaveCount(0);
  await expect(await options.count()).toBeGreaterThan(5);
});

test('la fonction inseree via autocompletion est bien enregistree telle quelle', async ({ page, request }) => {
  await openAddRuleForm(page);
  await page.locator('input#rule-name').fill('ac-rule-persist');
  await page.getByRole('switch', { name: 'Script personnalise' }).click();

  const scriptField = page.locator('#rule-script');
  await scriptField.click();
  await scriptField.type('seeded_int');
  const option = page.getByRole('option', { name: /^seeded_int/ });
  await option.click();
  // Les parametres suggeres sont deja selectionnes : la frappe les remplace.
  await page.keyboard.type('request.path.id, 0, 10');

  await page.getByRole('button', { name: /Ajouter la regle/ }).click();
  await page.waitForTimeout(500);

  const res = await request.get(`${API}/services/autocomplete-svc`);
  const svc = await res.json();
  const rule = svc.rules.find((r) => r.name === 'ac-rule-persist');
  expect(rule).toBeTruthy();
  expect(rule.script).toBe('seeded_int(request.path.id, 0, 10)');
});
