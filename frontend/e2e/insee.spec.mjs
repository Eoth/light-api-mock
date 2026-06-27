import { test, expect } from '@playwright/test';

const API = 'http://localhost:7342/api';

const templateSvc = {
  name: 'tpl-test',
  listen_path: '/items/{id}',
  real_target_url: 'http://backend:8080',
  is_mocked: true,
  rewrite_directory_urls: false,
  group_name: null,
  wsdl_mode: 'auto',
  rules: [
    {
      name: 'tpl-rule',
      method: 'GET',
      sub_path: null,
      action: 'mock',
      script: null,
      conditions: { all_of: [], any_of: [] },
      response: {
        status: 200,
        headers: [{ name: 'Content-Type', value: 'application/json' }],
        body: [
          {
            type: 'Template',
            template: '{"id":"{{path.id}}","short":"{{path.id | first(3)}}","company":"{{fake.CompanyName}}","ts":{{now_ms}},"seq":{{seq}}}',
          },
        ],
        chaos: null,
      },
    },
  ],
};

test.beforeEach(async ({ request }) => {
  await request.delete(`${API}/config/reset`);
  await request.post(`${API}/services`, { data: templateSvc });
});

test('path param extracts id from URL', async ({ request }) => {
  const resp = await request.get('http://localhost:7342/tpl-test/items/44306184100047');
  expect(resp.status()).toBe(200);
  const json = await resp.json();
  expect(json.id).toBe('44306184100047');
  expect(json.short).toBe('443');
  expect(json.company).toBeTruthy();
  expect(json.ts).toBeGreaterThan(1700000000000);
});

test('different id returns different value', async ({ request }) => {
  const resp = await request.get('http://localhost:7342/tpl-test/items/12345678901234');
  const json = await resp.json();
  expect(json.id).toBe('12345678901234');
  expect(json.short).toBe('123');
});

test('path with wrong prefix does not match', async ({ request }) => {
  const resp = await request.get('http://localhost:7342/other/items/12345');
  expect(resp.status()).toBe(404);
});

test('path with extra segments does not match', async ({ request }) => {
  const resp = await request.get('http://localhost:7342/tpl-test/items/123/extra');
  expect(resp.status()).toBe(404);
});

test('seq counter increments', async ({ request }) => {
  const r1 = await request.get('http://localhost:7342/tpl-test/items/aaa');
  const r2 = await request.get('http://localhost:7342/tpl-test/items/bbb');
  const j1 = await r1.json();
  const j2 = await r2.json();
  expect(j2.seq).toBeGreaterThan(j1.seq);
});

test('service visible in UI', async ({ page }) => {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  const group = page.locator('button[aria-expanded]').first();
  if (await group.getAttribute('aria-expanded') === 'false') {
    await group.click();
  }
  await expect(page.getByText('tpl-test').first()).toBeVisible();
});
