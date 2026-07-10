// Preuve de fonctionnement de l'infrastructure data-driven (selectors.json +
// scenario-runner.js, cf frontend/e2e/README.md) : rejoue les scenarios JSON
// de frontend/e2e/scenarios/ via runScenario(). Couvre volontairement deux
// parcours deja testes autrement dans la suite existante (rules.spec.mjs :
// "ajouter un service via le formulaire" / "creer une nouvelle regle via le
// formulaire") pour comparaison directe -- ces tests existants ne sont PAS
// modifies ni remplaces dans cette passe (migration complete prevue au
// sujet 9c).
import { test } from '@playwright/test';
import { runScenario, loadScenario } from './scenario-runner.js';

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

test.describe('Runner data-driven (scenarios JSON)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('creer un service (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('create-service.scenario.json'));
  });

  test('creer une regle simple (scenario JSON)', async ({ page, request }) => {
    // Prealable hors runner : le scenario ne couvre que le parcours UI de
    // creation de regle, pas la creation du service support -- reste
    // dans le champ des actions minimales (goto/click/fill/assert...),
    // pas de nouvelle action "apiRequest" ajoutee par anticipation.
    await request.post(`${API}/services`, { data: validService('scenario-rule-svc') });
    await runScenario(page, loadScenario('create-simple-rule.scenario.json'));
  });
});
