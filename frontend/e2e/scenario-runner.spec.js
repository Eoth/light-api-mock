// Tests E2E migres vers l'infrastructure data-driven (selectors.json +
// scenario-runner.js, cf frontend/e2e/README.md et CLAUDE.md §5 points
// 53-55). Chaque test ici REMPLACE un test equivalent qui existait
// auparavant dans un fichier *.spec.js/*.spec.mjs classique (migration
// sujet 9c -- voir CLAUDE.md §6 pour la liste complete et la progression) :
//
// Lot 1 :
//   - "creer un service"               <- ex rules.spec.mjs "ajouter un service via le formulaire"
//   - "creer une regle simple"         <- ex rules.spec.mjs "creer une nouvelle regle via le formulaire"
//   - "afficher les regles existantes" <- ex rules.spec.mjs "affiche les regles existantes"
//
// Lot 2 :
//   - "charge le service de demo"                    <- ex config.spec.mjs "bouton demo charge le service quand liste vide"
//   - "page d accueil affiche le titre"               <- ex critical-flows.spec.js "homepage loads with breadcrumb navigation"
//   - "liste affiche un service cree via l API"       <- ex critical-flows.spec.js "service list shows created services in group"
//   - "page groupes accessible depuis la nav"         <- ex critical-flows.spec.js "groups page is accessible to all"
//   - "groupe deplie persiste apres retour d edition" <- ex group-expansion-persistence.spec.js "un groupe deplie reste visible apres retour depuis l edition d un service"
//   - "groupe deplie reinitialise apres rechargement" <- ex group-expansion-persistence.spec.js "un rechargement complet de la page (F5) reinitialise l etat deplie"
//
// Lot 3 (rules.spec.mjs migre integralement -> fichier supprime + 2 tests
// security.spec.js reutilisant homepage-loads.scenario.json) :
//   - "regle: bouton ajouter fonctionne avec regles existantes" <- ex rules.spec.mjs "bouton ajouter une regle fonctionne avec regles existantes"
//   - "regle: bouton modifier ouvre le formulaire"               <- ex rules.spec.mjs "bouton modifier (crayon) ouvre le formulaire"
//   - "regle: bouton supprimer retire la regle"                  <- ex rules.spec.mjs "bouton supprimer retire la regle"
//   - "service: toggle mock/proxy fonctionne"                    <- ex rules.spec.mjs "toggle mock/proxy fonctionne"
//   - "liste: recherche filtre les services"                     <- ex rules.spec.mjs "recherche filtre les services"
//   - "regle: annuler le formulaire revient a la liste"          <- ex rules.spec.mjs "annuler le formulaire de regle revient a la liste"
//   - "UI servie sans aucun service (scenario JSON)"              <- ex security.spec.js "UI is served on / even with no services"
//   - "UI accessible apres creation d un service (scenario JSON)" <- ex security.spec.js "UI remains accessible after creating a valid service"
//
// Les tests d'origine sont supprimes du fichier source une fois leur
// equivalent JSON valide vert (pas de doublon testant deux fois le meme
// parcours).
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

test.describe('Runner data-driven (scenarios JSON) - lot 2', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  // Les 2 tests "groupe deplie ..." ci-dessous remplacent l'integralite de
  // l'ancien frontend/e2e/group-expansion-persistence.spec.js (supprime,
  // ses 2 tests sont entierement migres ici -- cf CLAUDE.md §6). Contexte
  // produit conserve de ce fichier : ils verifient le niveau 1 de
  // persistance de l'etat "groupe deplie/replie" (cf CLAUDE.md,
  // group-expansion-state.svelte.js) -- l'etat doit survivre a une
  // navigation vers l'edition d'un service et retour (store partage hors
  // du cycle de vie de ServiceList.svelte), mais PAS a un rechargement
  // complet de la page (F5), limite volontaire.

  test('charge le service de demo (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('load-demo-service.scenario.json'));
  });

  test('page d accueil affiche le titre (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('homepage-loads.scenario.json'));
  });

  test('liste affiche un service cree via l API (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('ui-test-svc') });
    await runScenario(page, loadScenario('service-list-shows-created-service.scenario.json'));
  });

  test('page groupes accessible depuis la nav (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('groups-page-accessible.scenario.json'));
  });

  test('groupe deplie persiste apres retour d edition (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/groups`, { data: { name: 'persist-grp', code: '', admins: [], members: [] } });
    await request.post(`${API}/services`, { data: validService('persist-svc', { group_name: 'persist-grp' }) });
    await runScenario(page, loadScenario('group-expansion-persists-after-edit.scenario.json'));
  });

  test('groupe deplie reinitialise apres rechargement (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/groups`, { data: { name: 'reload-grp', code: '', admins: [], members: [] } });
    await request.post(`${API}/services`, { data: validService('reload-svc', { group_name: 'reload-grp' }) });
    await runScenario(page, loadScenario('group-expansion-resets-after-reload.scenario.json'));
  });
});

// frontend/e2e/rules.spec.mjs a ete SUPPRIME entierement au lot 3 (comme
// group-expansion-persistence.spec.js au lot 2) : ses 6 derniers tests
// (les 3 premiers etaient deja migres au lot 1) sont tous migres ici, un
// fichier source vide de tests n'avait plus de raison d'exister.
//
// Fixture partagee lot 3 : un service avec 2 regles (rule-alpha, rule-beta),
// identique a l'ancien svcPayload de rules.spec.mjs.
function ruleTestService(name) {
  return validService(name, {
    rules: [
      validRule('rule-alpha'),
      validRule('rule-beta', {
        conditions: { all_of: [{ source: { type: 'QueryParam', key: 'id' }, operator: { type: 'Eq', value: '42' } }], any_of: [] },
      }),
    ],
  });
}

test.describe('Runner data-driven (scenarios JSON) - lot 3', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('regle: bouton ajouter fonctionne avec regles existantes (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('rule-form-add-with-existing-rules.scenario.json'));
  });

  test('regle: bouton modifier ouvre le formulaire (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('rule-edit-opens-form.scenario.json'));
  });

  test('regle: bouton supprimer retire la regle (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('rule-delete-removes-rule.scenario.json'));
  });

  test('service: toggle mock/proxy fonctionne (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('service-toggle-mock-proxy.scenario.json'));
  });

  test('liste: recherche filtre les services (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await request.post(`${API}/services`, { data: validService('other-svc') });
    await runScenario(page, loadScenario('search-filters-services.scenario.json'));
  });

  test('regle: annuler le formulaire revient a la liste (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('rule-form-cancel-returns-to-list.scenario.json'));
  });

  test('UI servie sans aucun service (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('homepage-loads.scenario.json'));
  });

  test('UI accessible apres creation d un service (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('security-svc') });
    await runScenario(page, loadScenario('homepage-loads.scenario.json'));
  });
});
