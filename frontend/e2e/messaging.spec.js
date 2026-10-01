import { test, expect } from '@playwright/test';
import { docsScreenshot } from './docs-screenshot.js';

const API = 'http://localhost:7342/api';

// Ces tests ne s'executent que contre un backend compile avec
// `--features messaging-kafka` (sinon /api/messaging/* renvoie 404 partout).
// Aucun broker Kafka reel n'est requis : le scenario passe
// par "Simuler un message" (POST /api/messaging/simulate), qui declenche le
// meme pipeline (match -> rendu -> journal) qu'un vrai message Kafka sans
// dependre d'un producteur externe — voir src/messaging/consumer.rs.
async function messagingAvailable(request) {
  const res = await request.get(`${API}/messaging/status`);
  if (!res.ok()) return false;
  const body = await res.json();
  return !!body.available;
}

function messagingService(name, overrides = {}) {
  return {
    name,
    listen_path: '/v1/*',
    real_target_url: 'http://backend:8080',
    is_mocked: true,
    rewrite_directory_urls: false,
    group_name: null,
    wsdl_mode: 'auto',
    rules: [
      {
        // method est un champ obligatoire du modele Rule partage avec le
        // HTTP (valide par validate_service contre VALID_METHODS, qui
        // n'inclut PAS "ANY") mais ignore par le matching de message (voir
        // src/messaging/matcher.rs) : n'importe quelle valeur valide convient.
        name: 'order-created',
        method: 'POST',
        sub_path: null,
        action: 'mock',
        pre_script: null,
        script: null,
        post_script: null,
        conditions: {
          all_of: [
            { source: { type: 'JsonPointer', key: '/type' }, operator: { type: 'Eq', value: 'order.created' } },
          ],
          any_of: [],
        },
        response: {
          status: 200,
          headers: [],
          body: [{ type: 'Literal', value: 'order-ack' }],
          chaos: null,
        },
      },
    ],
    ...overrides,
  };
}

test.describe('Messaging (Kafka) — journal des messages via simulation UI', () => {
  test.beforeEach(async ({ request }) => {
    const available = await messagingAvailable(request);
    test.skip(!available, 'backend not built with --features messaging-kafka');
    await request.delete(`${API}/config/reset`);
  });

  test('simuler un message matche l\'affiche dans le journal avec service/regle', async ({ page, request }) => {
    const created = await request.post(`${API}/services`, { data: messagingService('kafka-svc') });
    expect(created.ok()).toBe(true);

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await docsScreenshot(page, 'kafka-nav-button.png', '[data-testid="app-nav-messaging-button"]');
    await page.getByTitle('Kafka message log').click();
    await expect(page.getByRole('heading', { name: 'Kafka messages' })).toBeVisible();

    await page.getByLabel('Topic of the simulated message').fill('orders.in');
    await page.getByLabel('Body of the simulated message').fill('{"type":"order.created"}');
    await docsScreenshot(
      page,
      'kafka-simulation-form.png',
      '[data-testid="messaging-log-sim-topic-input"], [data-testid="messaging-log-simulate-button"]',
    );
    await page.getByRole('button', { name: 'Simulate' }).click();

    // The message log outlives a configuration reset, and lists the newest message first: on a server that already ran
    // this suite, the row of this run is the first one with its topic.
    const row = page.locator('tr', { hasText: 'orders.in' }).first();
    await expect(row).toBeVisible();
    await expect(row).toContainText('kafka-svc / order-created');
    await expect(row).toContainText('Matches');
    await expect(page.getByTestId('notification')).toBeHidden();
    await docsScreenshot(page, 'kafka-message-log.png', '[data-testid^="messaging-log-row-"]');
  });

  test('un message sans regle correspondante est journalise comme non matche', async ({ page, request }) => {
    const created = await request.post(`${API}/services`, { data: messagingService('kafka-svc-2') });
    expect(created.ok()).toBe(true);

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.getByTitle('Kafka message log').click();

    await page.getByLabel('Topic of the simulated message').fill('orders.unmatched');
    await page.getByLabel('Body of the simulated message').fill('{"type":"order.cancelled"}');
    await page.getByRole('button', { name: 'Simulate' }).click();

    const row = page.locator('tr', { hasText: 'orders.unmatched' }).first();
    await expect(row).toBeVisible();
    await expect(row).toContainText('Does not match');
  });

  test('un corps de message volumineux est journalise avec le badge "Tronque"', async ({ page, request }) => {
    test.setTimeout(30000);
    const created = await request.post(`${API}/services`, { data: messagingService('kafka-svc-3') });
    expect(created.ok()).toBe(true);

    const bigPayload = JSON.stringify({ type: 'order.created', filler: 'x'.repeat(20000) });

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.getByTitle('Kafka message log').click();

    await page.getByLabel('Topic of the simulated message').fill('orders.big');
    await page.getByLabel('Body of the simulated message').fill(bigPayload);
    await page.getByRole('button', { name: 'Simulate' }).click();

    const row = page.locator('tr', { hasText: 'orders.big' }).first();
    await expect(row).toBeVisible();
    await expect(row).toContainText('Truncated');

    // Le detail doit conserver la taille REELLE malgre la troncature de l'apercu.
    await row.locator('.btn-detail').click({ timeout: 20000 });
    const dialog = page.getByRole('dialog', { name: 'Message details' });
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText(`${bigPayload.length} bytes`);
  });

  test('simuler sans topic affiche une notification d\'erreur et ne journalise rien', async ({ page, request }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.getByTitle('Kafka message log').click();

    await page.getByRole('button', { name: 'Simulate' }).click();
    await expect(page.getByText('A topic is required to simulate a message.')).toBeVisible();
  });
});
