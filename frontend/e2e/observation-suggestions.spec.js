import { test, expect } from '@playwright/test';
import { docsScreenshot } from './docs-screenshot.js';
import http from 'node:http';

const API = 'http://localhost:7342/api';

// Fausse cible reelle qui varie sa reponse selon ?id= -- exerce le piege
// central du chantier (deux appels au meme endpoint, reponses legitimement
// differentes) plutot qu'un cas trivial ou tout endpoint repond pareil.
// Content-Length explicite : Node envoie "chunked" par defaut sans ca, ce qui
// desactiverait silencieusement la capture cote lightMock (voir
// ProxyClient::forward_with_capture, src/engine/proxy.rs).
function startFakeTarget() {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const url = new URL(req.url, 'http://x');
      const found = url.searchParams.get('id') === '1';
      const body = found ? '{"found":true}' : '{"found":false}';
      res.writeHead(found ? 200 : 404, {
        'content-type': 'application/json',
        'content-length': Buffer.byteLength(body),
      });
      res.end(body);
    });
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

function proxyService(name, targetPort) {
  return {
    name,
    listen_path: '/*',
    real_target_url: `http://127.0.0.1:${targetPort}`,
    is_mocked: false,
    rewrite_directory_urls: false,
    group_name: null,
    wsdl_mode: 'auto',
    rules: [],
  };
}

test.describe('Observation de trafic proxy et suggestions de regles', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('observer un service, generer du trafic reel variable, obtenir puis sauvegarder une suggestion', async ({
    page,
    request,
  }) => {
    test.setTimeout(30000);
    const target = await startFakeTarget();
    const targetPort = target.address().port;

    try {
      const created = await request.post(`${API}/services`, {
        data: proxyService('orders-proxy', targetPort),
      });
      expect(created.ok()).toBe(true);

      await page.goto('/');
      await page.waitForLoadState('networkidle');
      await page.getByText('Sans groupe').click();
      await page.getByTestId('service-card-configure-button-orders-proxy').click();

      const panel = page.getByTestId('observation-panel-orders-proxy');
      await expect(panel).toBeVisible();
      const toggleBtn = page.getByTestId('observation-toggle-button-orders-proxy');
      await expect(toggleBtn).toHaveText('Observer ce service');
      await docsScreenshot(page, 'observation-panneau-inactif.png');

      await toggleBtn.click();
      await expect(toggleBtn).toHaveText("Arrêter d'observer");
      await docsScreenshot(page, 'observation-panneau-actif.png');

      // Vrai trafic proxifie, alterne id=1/id=2 -- 3 appels de chaque cote,
      // au-dela du seuil minimal avant qu'une suggestion soit calculee.
      for (let i = 0; i < 3; i++) {
        const r1 = await request.get('http://localhost:7342/orders-proxy/orders?id=1');
        expect(r1.status()).toBe(200);
        const r2 = await request.get('http://localhost:7342/orders-proxy/orders?id=2');
        expect(r2.status()).toBe(404);
      }

      await page.getByTestId('observation-refresh-suggestions-button-orders-proxy').click();
      const firstSuggestionCard = page.getByText('si Paramètre de requête "id" = "1"');
      await expect(firstSuggestionCard).toBeVisible();
      await expect(page.getByText('si Paramètre de requête "id" = "2"')).toBeVisible();
      await firstSuggestionCard.scrollIntoViewIfNeeded();
      await docsScreenshot(page, 'observation-suggestions-liste.png');

      // "Utiliser cette suggestion" doit pre-remplir le formulaire de regle
      // existant (methode/sous-chemin/condition), pas creer la regle
      // directement -- l'utilisateur reste maitre de la sauvegarde.
      await page.getByTestId('observation-use-suggestion-orders-proxy-0-0').click();
      await expect(page.getByTestId('rule-form-method-select')).toHaveValue('GET');
      await expect(page.getByTestId('rule-form-subpath-input')).toHaveValue('/orders');
      await docsScreenshot(page, 'observation-suggestion-formulaire-pre-rempli.png');

      await page.getByTestId('rule-form-name-input').fill('id-1-found');
      await page.getByTestId('rule-form-submit-button').click();

      await expect(page.getByTestId('rule-list-item-id-1-found')).toBeVisible();
      const rules = await request.get(`${API}/services/orders-proxy`);
      const savedRule = (await rules.json()).rules.find((r) => r.name === 'id-1-found');
      expect(savedRule.conditions.all_of).toEqual([
        { source: { type: 'QueryParam', key: 'id' }, operator: { type: 'Eq', value: '1' } },
      ]);
      expect(savedRule.response.status).toBe(200);
    } finally {
      target.close();
    }
  });

  test('une variance sans champ discriminant fiable ne propose aucune regle', async ({ page, request }) => {
    test.setTimeout(30000);
    // Cible qui alterne sa reponse SANS rapport avec un champ de la requete
    // (meme appel exact, reponses differentes) -- doit rester sans
    // suggestion actionnable plutot que de figer la premiere reponse vue.
    let counter = 0;
    const target = await new Promise((resolve) => {
      const server = http.createServer((req, res) => {
        counter += 1;
        const body = `{"n":${counter}}`;
        res.writeHead(200, { 'content-type': 'application/json', 'content-length': Buffer.byteLength(body) });
        res.end(body);
      });
      server.listen(0, '127.0.0.1', () => resolve(server));
    });
    const targetPort = target.address().port;

    try {
      const created = await request.post(`${API}/services`, {
        data: proxyService('flaky-proxy', targetPort),
      });
      expect(created.ok()).toBe(true);

      await request.post(`${API}/services/flaky-proxy/observe`);
      for (let i = 0; i < 3; i++) {
        await request.get('http://localhost:7342/flaky-proxy/status');
      }

      await page.goto('/');
      await page.waitForLoadState('networkidle');
      await page.getByText('Sans groupe').click();
      await page.getByTestId('service-card-configure-button-flaky-proxy').click();
      await page.getByTestId('observation-refresh-suggestions-button-flaky-proxy').click();

      await expect(page.getByText(/Réponses variables observées/)).toBeVisible();
      await expect(page.getByTestId('observation-suggestion-flaky-proxy-0-0')).toHaveCount(0);
    } finally {
      target.close();
    }
  });
});
