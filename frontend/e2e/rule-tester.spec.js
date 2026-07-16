import { test, expect } from '@playwright/test';
import { docsScreenshot } from './docs-screenshot.js';

const API = 'http://localhost:7342/api';
const BASE = 'http://localhost:7342';

// Verifie de bout en bout le testeur de regle (rejeu en lecture seule contre
// un log reel) et l'assistance de saisie path/query param :
// - une condition mal choisie (query param au lieu de path param) testee
//   contre une vraie requete du log affiche un detail explicite (pas juste
//   "ca ne matche pas") ;
// - la selection du path param est stricte (select ferme, pas de saisie
//   libre) ;
// - l'option "Parametre de chemin" est masquee sur un service a URL statique ;
// - l'autocompletion query param propose les cles vues dans le trafic reel ;
// - un corps capture tronque (> REQUEST_LOG_MAX_BODY_SIZE) declenche un
//   avertissement explicite quand la condition testee porte sur le corps,
//   et n'en declenche PAS quand aucune condition testee ne depend du corps.

function validService(name, overrides = {}) {
  return {
    name,
    listen_path: '/*',
    real_target_url: 'http://backend:8080',
    is_mocked: true,
    rewrite_directory_urls: false,
    group_name: null,
    wsdl_mode: 'auto',
    rules: [],
    ...overrides,
  };
}

async function openService(page, serviceName) {
  await page.goto('/');
  await page.waitForLoadState('networkidle');
  const group = page.locator('button[aria-expanded]').first();
  if ((await group.count()) > 0 && (await group.getAttribute('aria-expanded')) === 'false') {
    await group.click();
    await page.waitForTimeout(200);
  }
  await page.getByRole('button', { name: new RegExp(`Configurer le service ${serviceName}`) }).click();
  await page.waitForTimeout(200);
}

async function openAddRuleForm(page, serviceName) {
  await openService(page, serviceName);
  await page.getByRole('button', { name: /Ajouter une regle/ }).click();
  await page.waitForTimeout(200);
}

test.describe('Testeur de regle : condition mal choisie contre une vraie requete', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('detecte et explique un path param saisi comme query param', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('tester-svc', { listen_path: '/{id}/*' }),
    });

    // Requete reelle capturee (aucune regle encore definie -> "no-rule", mais
    // le detail de la requete est quand meme retenu).
    const captured = await request.get(`${BASE}/tester-svc/42/details`);
    expect(captured.status()).toBe(404);

    await openAddRuleForm(page, 'tester-svc');
    await page.locator('input#rule-name').fill('mauvais-choix');

    await page.getByRole('button', { name: '+ Condition ET' }).click();
    await page.locator('#cond-source').selectOption('QueryParam');
    await page.locator('#cond-key').fill('id');
    await page.locator('#cond-val').fill('42');
    await page.getByRole('button', { name: 'Valider' }).click();

    // RequestLog n'est jamais purge par /config/reset (seule la config
    // services/regles l'est) : selectionner la premiere occurrence (la plus
    // recente, cf recent() cote backend) plutot que de supposer un compte
    // exact d'entrees, pour rester robuste si un run precedent a laisse une
    // entree homonyme dans le journal.
    const logSelect = page.locator('#rule-tester-log');
    await expect(logSelect).toBeVisible();
    const matchingOption = logSelect.locator('option', { hasText: 'tester-svc/42/details' }).first();
    await expect(matchingOption).toBeAttached();
    const optionValue = await matchingOption.getAttribute('value');
    await logSelect.selectOption(optionValue);

    await page.getByRole('button', { name: /Tester contre cette requête/ }).click();

    await expect(page.getByText(/ne matcherait pas cette requête/)).toBeVisible();
    await expect(page.getByText(/valeur trouvée : absente/)).toBeVisible();
    await expect(page.getByText(/present comme parametre de chemin/)).toBeVisible();
    await docsScreenshot(page, 'testeur-regle-hint.png');
  });

  test('la selection du path param est stricte (select ferme, pas de saisie libre)', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('strict-svc', { listen_path: '/{orderId}/*' }),
    });

    await openAddRuleForm(page, 'strict-svc');
    await page.getByRole('button', { name: '+ Condition ET' }).click();
    await page.locator('#cond-source').selectOption('PathParam');

    const keyField = page.locator('#cond-key');
    await expect(keyField).toHaveJSProperty('tagName', 'SELECT');
    const optionValues = await keyField.locator('option').evaluateAll((opts) => opts.map((o) => o.value));
    expect(optionValues.filter(Boolean)).toEqual(['orderId']);
  });

  test('masque l\'option "Parametre de chemin" sur un service a URL statique', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('static-svc', { listen_path: '/fixed/path' }),
    });

    await openAddRuleForm(page, 'static-svc');
    await page.getByRole('button', { name: '+ Condition ET' }).click();

    const sourceSelect = page.locator('#cond-source');
    const optionValues = await sourceSelect.locator('option').evaluateAll((opts) => opts.map((o) => o.value));
    expect(optionValues).not.toContain('PathParam');
  });

  test('autocompletion query param propose les cles vues dans le trafic reel', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('autocomplete-qp-svc', { listen_path: '/*' }),
    });

    await request.get(`${BASE}/autocomplete-qp-svc/anything?customerRef=abc123`);

    await openAddRuleForm(page, 'autocomplete-qp-svc');
    await page.getByRole('button', { name: '+ Condition ET' }).click();
    await page.locator('#cond-source').selectOption('QueryParam');

    const keyField = page.locator('#cond-key');
    const datalistId = await keyField.getAttribute('list');
    expect(datalistId).toBeTruthy();
    const suggestions = await page
      .locator(`#${datalistId} option`)
      .evaluateAll((opts) => opts.map((o) => o.value));
    expect(suggestions).toContain('customerRef');

    // Non contraignant : une valeur inedite reste saisissable.
    await keyField.fill('brandNewParam');
    await expect(keyField).toHaveValue('brandNewParam');
  });

  test('avertit quand le corps capture est tronque et la condition testee porte sur le corps', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('truncation-svc', { listen_path: '/*' }),
    });

    // Corps > REQUEST_LOG_MAX_BODY_SIZE (16 Ko par defaut) pour declencher
    // une troncature reelle dans RequestLog.
    const largeBody = 'x'.repeat(20000);
    await request.post(`${BASE}/truncation-svc/anything`, {
      data: largeBody,
      headers: { 'content-type': 'text/plain' },
    });

    await openAddRuleForm(page, 'truncation-svc');
    await page.locator('input#rule-name').fill('body-based-rule');

    // BodyRaw : source basee sur le corps entier, sensible a la troncature.
    await page.getByRole('button', { name: '+ Condition ET' }).click();
    await page.locator('#cond-source').selectOption('BodyRaw');
    await page.locator('#cond-op').selectOption('Exists');
    await page.getByRole('button', { name: 'Valider' }).click();

    const logSelect = page.locator('#rule-tester-log');
    await expect(logSelect).toBeVisible();
    const matchingOption = logSelect.locator('option', { hasText: 'truncation-svc/anything' }).first();
    await expect(matchingOption).toBeAttached();
    const optionValue = await matchingOption.getAttribute('value');
    await logSelect.selectOption(optionValue);

    await page.getByRole('button', { name: /Tester contre cette requête/ }).click();

    await expect(page.getByText(/corps de cette requête a été tronqué/)).toBeVisible();
  });

  test('n\'avertit PAS de troncature quand aucune condition testee ne porte sur le corps', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('truncation-noop-svc', { listen_path: '/*' }),
    });

    const largeBody = 'x'.repeat(20000);
    await request.post(`${BASE}/truncation-noop-svc/anything?foo=bar`, {
      data: largeBody,
      headers: { 'content-type': 'text/plain' },
    });

    await openAddRuleForm(page, 'truncation-noop-svc');
    await page.locator('input#rule-name').fill('query-only-rule');
    // La requete capturee est un POST : aligner la methode de la regle pour
    // que method_matches soit vrai, sinon "overall_matched" serait faux pour
    // une raison sans rapport avec le corps/la troncature teste ici.
    await page.locator('#rule-method').selectOption('POST');

    // QueryParam : ne depend jamais du corps, donc la troncature du corps
    // capture n'a aucune incidence sur ce test -- pas d'avertissement attendu.
    await page.getByRole('button', { name: '+ Condition ET' }).click();
    await page.locator('#cond-source').selectOption('QueryParam');
    await page.locator('#cond-key').fill('foo');
    await page.locator('#cond-val').fill('bar');
    await page.getByRole('button', { name: 'Valider' }).click();

    const logSelect = page.locator('#rule-tester-log');
    await expect(logSelect).toBeVisible();
    const matchingOption = logSelect.locator('option', { hasText: 'truncation-noop-svc/anything' }).first();
    await expect(matchingOption).toBeAttached();
    const optionValue = await matchingOption.getAttribute('value');
    await logSelect.selectOption(optionValue);

    await page.getByRole('button', { name: /Tester contre cette requête/ }).click();

    await expect(page.getByText(/matcherait cette requête/)).toBeVisible();
    await expect(page.getByText(/corps de cette requête a été tronqué/)).not.toBeVisible();
  });
});

// Visibilite des erreurs d'execution de script (cf CLAUDE.md, "Visibilite
// des erreurs de script") : avant cette extension, /api/rule-test ne
// rejouait que le matching, jamais les scripts — un script casse (fonction
// Rhai inexistante) restait invisible du testeur, exactement comme en
// production (soft-fail + log serveur uniquement). Ces 2 tests couvrent le
// cas d'erreur ET le cas nominal (map/lookup correct) demandes en E2E.
test.describe('Testeur de regle : execution des scripts', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('un script en erreur a l\'execution affiche un message clair, pas un echec silencieux', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('script-error-svc', { listen_path: '/*' }),
    });
    await request.get(`${BASE}/script-error-svc/anything`);

    await openAddRuleForm(page, 'script-error-svc');
    await page.locator('input#rule-name').fill('script-casse');

    // Active le bloc "Script personnalisé" et y saisit un appel de fonction
    // Rhai INEXISTANTE — la classe d'erreur diagnostiquee comme cause
    // racine (validate() ne la detecte pas, seule une vraie execution le
    // peut).
    await page.getByRole('switch', { name: /Script personnalis/ }).click();
    await page.locator('#rule-script').fill('totally_undefined_fn(1, 2)');

    const logSelect = page.locator('#rule-tester-log');
    await expect(logSelect).toBeVisible();
    const matchingOption = logSelect.locator('option', { hasText: 'script-error-svc/anything' }).first();
    await expect(matchingOption).toBeAttached();
    await logSelect.selectOption(await matchingOption.getAttribute('value'));

    await page.getByRole('button', { name: /Tester contre cette requête/ }).click();

    await expect(page.getByText(/matcherait cette requête/)).toBeVisible();
    const errorBanner = page.getByTestId('rule-tester-script-errors');
    await expect(errorBanner).toBeVisible();
    await expect(errorBanner).toContainText('totally_undefined_fn');
    await expect(page.getByTestId('rule-tester-script-error-script')).toBeVisible();
    await docsScreenshot(page, 'testeur-regle-erreur-script.png');
  });

  test('un script de correspondance (map/lookup) correct ne produit aucune erreur', async ({ page, request }) => {
    // listen_path avec {name} : le path param est deja extrait au niveau
    // SERVICE (cf CLAUDE.md, capture du detail de requete), donc disponible
    // via request.path.name des la capture, sans meme avoir besoin d'un
    // sous-chemin de regle.
    await request.post(`${API}/services`, {
      data: validService('lookup-e2e-svc', { listen_path: '/lookup/{name}' }),
    });
    const captured = await request.get(`${BASE}/lookup-e2e-svc/lookup/billing`);
    expect(captured.status()).toBe(404); // pas encore de regle -> no-rule, mais capture quand meme

    await openAddRuleForm(page, 'lookup-e2e-svc');
    await page.locator('input#rule-name').fill('lookup-service');

    // Meme script, meme syntaxe verifiee, que le test d'integration backend
    // (map_lookup_by_path_param_returns_correct_target_and_falls_back_for_unknown_key,
    // src/server/intercept.rs) et que l'exemple documente dans
    // docs/scripts-rhai.md.
    await page.getByRole('switch', { name: /Script personnalis/ }).click();
    await page.locator('#rule-script').fill(
      'let mapping = #{ "billing": "svc-billing-042", "orders": "svc-orders-017" };\n' +
      'let name = request.path.name;\n' +
      'if mapping.contains(name) { #{ id: mapping[name], found: "true" } } else { #{ id: "unknown", found: "false" } }'
    );

    const logSelect = page.locator('#rule-tester-log');
    await expect(logSelect).toBeVisible();
    const matchingOption = logSelect.locator('option', { hasText: 'lookup-e2e-svc/lookup/billing' }).first();
    await expect(matchingOption).toBeAttached();
    await logSelect.selectOption(await matchingOption.getAttribute('value'));

    await page.getByRole('button', { name: /Tester contre cette requête/ }).click();

    await expect(page.getByText(/matcherait cette requête/)).toBeVisible();
    await expect(page.getByTestId('rule-tester-script-errors')).not.toBeVisible();
  });
});
