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
