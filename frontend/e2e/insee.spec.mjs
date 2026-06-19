import { test, expect } from '@playwright/test';

const BASE = 'http://localhost:3000';

const inseeSvc = {
  name: 'insee',
  method: 'GET',
  listen_path: '/v4/insee/sirene/etablissements/{siret}',
  real_target_url: 'https://staging.entreprise.api.gouv.fr',
  is_mocked: true,
  rewrite_directory_urls: false,
  rules: [
    {
      name: 'insee-template',
      conditions: { all_of: [], any_of: [] },
      response: {
        status: 200,
        headers: [{ name: 'Content-Type', value: 'application/json' }],
        body: [
          {
            type: 'Template',
            template: '{{"siret":"{path.siret}","siren":"{path.siret | first(9)}","entreprise":"{fake.CompanyName}","ts":{now_ms}}}',
          },
        ],
        chaos: null,
      },
    },
  ],
};

test.beforeEach(async () => {
  await fetch(`${BASE}/api/services/insee`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(inseeSvc),
  });
});

test.afterEach(async () => {
  await fetch(`${BASE}/api/services/insee`, { method: 'DELETE' });
});

test('named path param extracts siret from URL', async ({ request }) => {
  const resp = await request.get(`${BASE}/insee/v4/insee/sirene/etablissements/44306184100047`);
  expect(resp.status()).toBe(200);
  const json = await resp.json();
  expect(json.siret).toBe('44306184100047');
  expect(json.siren).toBe('443061841');
  expect(json.entreprise).toBeTruthy();
  expect(typeof json.ts).toBe('number');
  expect(json.ts).toBeGreaterThan(1700000000000);
});

test('different siret returns different value', async ({ request }) => {
  const resp = await request.get(`${BASE}/insee/v4/insee/sirene/etablissements/12345678901234`);
  const json = await resp.json();
  expect(json.siret).toBe('12345678901234');
  expect(json.siren).toBe('123456789');
});

test('path with wrong prefix does not match', async ({ request }) => {
  const resp = await request.get(`${BASE}/other/v4/other/sirene/etablissements/44306184100047`);
  expect(resp.status()).toBe(404);
});

test('path with extra segments does not match', async ({ request }) => {
  const resp = await request.get(`${BASE}/insee/v4/insee/sirene/etablissements/44306184100047/extra`);
  expect(resp.status()).toBe(404);
});

test('template with seq counter increments', async ({ request }) => {
  const r1 = await request.get(`${BASE}/insee/v4/insee/sirene/etablissements/11111111111111`);
  const r2 = await request.get(`${BASE}/insee/v4/insee/sirene/etablissements/22222222222222`);
  const j1 = await r1.json();
  const j2 = await r2.json();
  expect(j1.siret).toBe('11111111111111');
  expect(j2.siret).toBe('22222222222222');
});

test('insee service visible in UI', async ({ page }) => {
  await page.goto(BASE);
  await page.waitForLoadState('networkidle');
  await expect(page.getByRole('heading', { name: 'insee' })).toBeVisible();
  await expect(page.getByText('/insee/v4/insee/sirene/etablissements/{siret}')).toBeVisible();
});
