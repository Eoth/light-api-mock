import { test, expect } from '@playwright/test';

const API = 'http://localhost:7342/api';

function validService(name, overrides = {}) {
  return {
    name,
    listen_path: '',
    real_target_url: 'http://backend:8080',
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
});

test('bouton demo charge le service quand liste vide', async ({ page }) => {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await expect(page.getByText('Aucun service configure')).toBeVisible();
  await page.getByRole('button', { name: /Charger un exemple/ }).click();
  await page.waitForTimeout(500);
  const group = page.locator('button[aria-expanded]').first();
  if (await group.getAttribute('aria-expanded') === 'false') {
    await group.click();
  }
  await expect(page.getByText('users-api').first()).toBeVisible();
});

test('demo service repond avec les path params', async ({ page, request }) => {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  await page.getByRole('button', { name: /Charger un exemple/ }).click();
  await page.waitForTimeout(500);

  const resp = await request.get('http://localhost:7342/users-api/users/42');
  expect(resp.status()).toBe(200);
  const json = await resp.json();
  expect(json.id).toBe(42);
  expect(json.name).toBeTruthy();
  expect(json.meta.timestamp).toBeGreaterThan(1700000000000);
});

test('export telecharge un fichier JSON valide', async ({ page, request }) => {
  await request.post(`${API}/services`, { data: validService('export-test') });

  await page.goto('/');
  await page.waitForLoadState('networkidle');

  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('button', { name: 'Export', exact: true }).click(),
  ]);

  const content = await (await download.createReadStream()).toArray();
  const text = Buffer.concat(content).toString('utf-8');
  const config = JSON.parse(text);
  expect(config.services).toBeInstanceOf(Array);
  expect(config.services.some(s => s.name === 'export-test')).toBe(true);
});

test('import charge une configuration via API', async ({ request }) => {
  const config = {
    services: [validService('imported-svc')],
    groups: [],
  };
  const res = await request.put(`${API}/config`, { data: config });
  expect(res.status()).toBe(200);

  const services = await (await request.get(`${API}/services`)).json();
  expect(services.length).toBe(1);
  expect(services[0].name).toBe('imported-svc');
});
