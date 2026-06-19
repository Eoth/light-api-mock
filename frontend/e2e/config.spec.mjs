import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';

const BASE = 'http://localhost:3000';

test.beforeEach(async () => {
  await fetch(`${BASE}/api/config`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ services: [] }),
  });
});

test('bouton demo charge le service INSEE quand liste vide', async ({ page }) => {
  await page.goto(BASE);
  await page.waitForLoadState('networkidle');
  await expect(page.getByText('Aucun service configure')).toBeVisible();
  await page.getByRole('button', { name: /Charger un exemple/ }).click();
  await page.waitForTimeout(500);
  await expect(page.getByRole('heading', { name: 'insee-demo' })).toBeVisible();
});

test('demo INSEE repond avec les path params', async ({ page, request }) => {
  await page.goto(BASE);
  await page.waitForLoadState('networkidle');
  await page.getByRole('button', { name: /Charger un exemple/ }).click();
  await page.waitForTimeout(500);

  const resp = await request.get(`${BASE}/insee-demo/v4/insee/sirene/etablissements/44306184100047`);
  expect(resp.status()).toBe(200);
  const json = await resp.json();
  expect(json.siret).toBe('44306184100047');
  expect(json.siren).toBe('443061841');
  expect(json.unite_legale.denomination).toBeTruthy();
  expect(json.meta.timestamp).toBeGreaterThan(1700000000000);
});

test('export telecharge un fichier JSON valide', async ({ page }) => {
  await fetch(`${BASE}/api/services/export-test`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: 'export-test', method: 'GET', listen_path: '/exp/*', real_target_url: 'http://exp:80', is_mocked: false, rules: [] }),
  });

  await page.goto(BASE);
  await page.waitForLoadState('networkidle');

  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('button', { name: 'Export', exact: true }).click(),
  ]);

  const filePath = await download.path();
  const content = fs.readFileSync(filePath, 'utf-8');
  const config = JSON.parse(content);
  expect(config.services).toBeInstanceOf(Array);
  expect(config.services.length).toBeGreaterThan(0);
  expect(config.services.some(s => s.name === 'export-test')).toBe(true);
});

test('import charge une configuration', async ({ page }) => {
  const config = {
    services: [
      { name: 'imported-svc', method: 'GET', listen_path: '/imp/*', real_target_url: 'http://imp:80', is_mocked: true, rewrite_directory_urls: false, rules: [] },
    ],
  };
  const tmpFile = path.join(process.cwd(), 'test-import.json');
  fs.writeFileSync(tmpFile, JSON.stringify(config));

  try {
    await page.goto(BASE);
    await page.waitForLoadState('networkidle');

    const fileInput = page.locator('input[type="file"]');
    await fileInput.setInputFiles(tmpFile);
    await page.waitForTimeout(500);

    await expect(page.getByRole('heading', { name: 'imported-svc' })).toBeVisible();

    const resp = await (await fetch(`${BASE}/api/services`)).json();
    expect(resp.length).toBe(1);
    expect(resp[0].name).toBe('imported-svc');
  } finally {
    fs.unlinkSync(tmpFile);
  }
});
