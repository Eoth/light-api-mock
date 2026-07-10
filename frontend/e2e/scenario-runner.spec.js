// Tests E2E migres vers l'infrastructure data-driven (selectors.json +
// scenario-runner.js, cf frontend/e2e/README.md et CLAUDE.md §5 points
// 53-55). Chaque test ici REMPLACE un test equivalent qui existait
// auparavant dans un fichier *.spec.js/*.spec.mjs classique (migration
// sujet 9c, lot 1/N -- voir CLAUDE.md §6 pour la liste et la progression) :
//   - "creer un service"            <- ex rules.spec.mjs "ajouter un service via le formulaire"
//   - "creer une regle simple"      <- ex rules.spec.mjs "creer une nouvelle regle via le formulaire"
//   - "afficher les regles existantes" <- ex rules.spec.mjs "affiche les regles existantes"
// Les tests d'origine ont ete supprimes de rules.spec.mjs une fois leur
// equivalent JSON valide vert (pas de doublon testant deux fois le meme
// parcours) -- la suite complete reste a 81 tests executes.
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

function validRule(name, overrides = {}) {
  return {
    name,
    method: 'GET',
    sub_path: null,
    action: 'mock',
    pre_script: null,
    script: null,
    post_script: null,
    conditions: { all_of: [], any_of: [] },
    response: { status: 200, headers: [{ name: 'Content-Type', value: 'application/json' }], body: [{ type: 'Literal', value: '{"ok":true}' }], chaos: null },
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

  test('afficher les regles existantes (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('e2e-svc', {
      rules: [
        validRule('rule-alpha'),
        validRule('rule-beta', {
          conditions: { all_of: [{ source: { type: 'QueryParam', key: 'id' }, operator: { type: 'Eq', value: '42' } }], any_of: [] },
        }),
      ],
    }) });
    await runScenario(page, loadScenario('view-existing-rules.scenario.json'));
  });
});
