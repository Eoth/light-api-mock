import { test, expect } from '@playwright/test';

// Service "purement mocke" (real_target_url vide, sujet 22, cf CLAUDE.md
// §3). Ce fichier ne pilote jamais l'UI (que des `request.get/post`,
// assertions sur le code HTTP/le corps de reponse) : un test API-only n'a
// pas sa place dans un scenario JSON data-driven (aucune etape UI reelle a
// modeliser) -- meme convention que les tests API-only deja presents dans
// critical-flows.spec.js/insee.spec.mjs/security.spec.js, cf
// frontend/e2e/README.md.
const BASE = 'http://localhost:7342';
const API = `${BASE}/api`;

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

test.describe('Service purement mocke : comportement reseau', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('une requete sans regle correspondante renvoie un 404 explicite mentionnant l\'absence de cible', async ({ request }) => {
    await request.post(`${API}/services`, {
      data: validService('nocible-nomatch', { real_target_url: '' }),
    });

    const resp = await request.get(`${BASE}/nocible-nomatch/anything`);
    expect(resp.status()).toBe(404);
    const body = await resp.text();
    expect(body).toContain('purement mock');
  });

  test('une requete sans regle correspondante sur un service AVEC cible reste generique (pas de mention de cible absente)', async ({ request }) => {
    await request.post(`${API}/services`, {
      data: validService('avecible-nomatch'),
    });

    const resp = await request.get(`${BASE}/avecible-nomatch/anything`);
    expect(resp.status()).toBe(404);
    const body = await resp.text();
    expect(body).not.toContain('purement mock');
  });

  test('un service purement mocke avec une regle qui matche continue de repondre normalement', async ({ request }) => {
    await request.post(`${API}/services`, {
      data: validService('nocible-avecregle', {
        real_target_url: '',
        rules: [{
          name: 'ok-rule',
          method: 'GET',
          sub_path: null,
          action: 'mock',
          pre_script: null,
          script: null,
          post_script: null,
          conditions: { all_of: [], any_of: [] },
          response: { status: 200, headers: [], body: [{ type: 'Literal', value: 'ok' }], chaos: null },
        }],
      }),
    });

    const resp = await request.get(`${BASE}/nocible-avecregle/anything`);
    expect(resp.status()).toBe(200);
    expect(await resp.text()).toBe('ok');
  });
});
