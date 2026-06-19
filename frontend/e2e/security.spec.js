import { test, expect } from '@playwright/test';

const API = 'http://localhost:7342/api';

function validService(name, overrides = {}) {
  return {
    name,
    method: 'GET',
    listen_path: '/v1/*',
    real_target_url: 'http://backend:8080',
    is_mocked: true,
    rules: [],
    ...overrides,
  };
}

test.describe('Security: route protection', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('UI is served on / even with no services', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h1')).toContainText('lightMock');
  });

  test('API rejects service with empty listen_path', async ({ request }) => {
    const res = await request.post(`${API}/services`, {
      data: validService('hijacker', { listen_path: '', is_mocked: false }),
    });
    expect(res.status()).toBe(400);
    const body = await res.json();
    expect(body.error).toBeTruthy();
  });

  test('API rejects service with listen_path "/"', async ({ request }) => {
    const res = await request.post(`${API}/services`, {
      data: validService('hijacker', { listen_path: '/' }),
    });
    expect(res.status()).toBe(400);
  });

  test('API rejects service with listen_path "/*"', async ({ request }) => {
    const res = await request.post(`${API}/services`, {
      data: validService('hijacker', { listen_path: '/*' }),
    });
    expect(res.status()).toBe(400);
  });

  test('API rejects service named "api"', async ({ request }) => {
    const res = await request.post(`${API}/services`, {
      data: validService('api'),
    });
    expect(res.status()).toBe(400);
  });

  test('UI remains accessible after creating a valid service', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('test-svc', { listen_path: '/v1/data/{id}' }),
    });
    await page.goto('/');
    await expect(page.locator('h1')).toContainText('lightMock');
  });

  test('internal API routes remain accessible with services registered', async ({ request }) => {
    await request.post(`${API}/services`, {
      data: validService('my-svc'),
    });
    const configRes = await request.get(`${API}/config`);
    expect(configRes.ok()).toBe(true);
    const servicesRes = await request.get(`${API}/services`);
    expect(servicesRes.ok()).toBe(true);
    const logsRes = await request.get(`${API}/logs`);
    expect(logsRes.ok()).toBe(true);
  });

  test('reset endpoint removes all services', async ({ request }) => {
    await request.post(`${API}/services`, {
      data: validService('svc1'),
    });
    const before = await request.get(`${API}/services`);
    const beforeData = await before.json();
    expect(beforeData.length).toBeGreaterThan(0);

    const resetRes = await request.delete(`${API}/config/reset`);
    expect(resetRes.status()).toBe(204);

    const after = await request.get(`${API}/services`);
    const afterData = await after.json();
    expect(afterData.length).toBe(0);
  });

  test('put_config rejects config with invalid service', async ({ request }) => {
    const res = await request.put(`${API}/config`, {
      data: {
        services: [validService('hijacker', { listen_path: '/' })],
      },
    });
    expect(res.status()).toBe(400);
  });
});

test.describe('Uniqueness: service_key collision', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('creating a service with a new name succeeds (201)', async ({ request }) => {
    const res = await request.post(`${API}/services`, {
      data: validService('unique-svc'),
    });
    expect(res.status()).toBe(201);
  });

  test('creating a second service with the same name is rejected (409)', async ({ request }) => {
    const first = await request.post(`${API}/services`, {
      data: validService('dup-svc'),
    });
    expect(first.status()).toBe(201);

    const second = await request.post(`${API}/services`, {
      data: validService('dup-svc'),
    });
    expect(second.status()).toBe(409);
    const body = await second.json();
    expect(body.error).toContain('existe deja');
  });

  test('updating an existing service succeeds (PUT)', async ({ request }) => {
    await request.post(`${API}/services`, {
      data: validService('edit-svc'),
    });
    const res = await request.put(`${API}/services/edit-svc`, {
      data: validService('edit-svc', { listen_path: '/v2/*' }),
    });
    expect(res.ok()).toBe(true);
  });

  test('updating a non-existent service returns 404', async ({ request }) => {
    const res = await request.put(`${API}/services/ghost`, {
      data: validService('ghost'),
    });
    expect(res.status()).toBe(404);
  });

  test('no silent overwrite via PUT on existing name', async ({ request }) => {
    await request.post(`${API}/services`, {
      data: validService('keep-me', { listen_path: '/v1/original/*' }),
    });

    const check = await request.get(`${API}/services/keep-me`);
    const svc = await check.json();
    expect(svc.listen_path).toBe('/v1/original/*');
  });
});

test.describe('Rule name uniqueness', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('rejects duplicate rule names in the same service', async ({ request }) => {
    const svc = validService('rule-test', {
      rules: [
        { name: 'dup', action: 'mock', conditions: { all_of: [], any_of: [] }, response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'a' }], chaos: null } },
        { name: 'dup', action: 'mock', conditions: { all_of: [], any_of: [] }, response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'b' }], chaos: null } },
      ],
    });
    const res = await request.post(`${API}/services`, { data: svc });
    expect(res.status()).toBe(400);
    const body = await res.json();
    expect(body.error).toContain('nom de regle');
  });

  test('accepts same rule name across different services', async ({ request }) => {
    const svc1 = validService('svc-a', {
      rules: [{ name: 'shared', action: 'mock', conditions: { all_of: [], any_of: [] }, response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'a' }], chaos: null } }],
    });
    const svc2 = validService('svc-b', {
      listen_path: '/v2/*',
      rules: [{ name: 'shared', action: 'mock', conditions: { all_of: [], any_of: [] }, response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'b' }], chaos: null } }],
    });
    const r1 = await request.post(`${API}/services`, { data: svc1 });
    expect(r1.status()).toBe(201);
    const r2 = await request.post(`${API}/services`, { data: svc2 });
    expect(r2.status()).toBe(201);
  });
});
