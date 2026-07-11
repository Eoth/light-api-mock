// Tests E2E migres vers l'infrastructure data-driven (selectors.json +
// scenario-runner.js, cf frontend/e2e/README.md et CLAUDE.md §5 points
// 53-55). Chaque test ici REMPLACE un test equivalent qui existait
// auparavant dans un fichier *.spec.js/*.spec.mjs classique (migration
// sujet 9c -- voir CLAUDE.md §6 pour la liste complete et la progression).
//
// Les scenarios eux-memes sont regroupes par domaine fonctionnel dans
// frontend/e2e/scenarios/{home,groups,rules,services}.scenarios.json
// (un fichier par domaine, un tableau de scenarios par fichier -- pas un
// fichier par scenario individuel, cf CLAUDE.md sujet 9c "regroupement par
// domaine"). `loadScenario(domainFile, scenarioName)` en extrait un seul.
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
// Lot 4 (critical-flows.spec.js Groups/Service identity + insee.spec.mjs +
// write-behind.spec.js, partiel -- cf CLAUDE.md §3 pour la clarification de
// perimetre UI-only qui a guide ce choix) :
//   - "groupe: formulaire ne demande que le nom"            <- ex critical-flows.spec.js "UI: creation form only asks for a name, code is auto-generated"
//   - "groupe: nom accentue accepte"                        <- ex critical-flows.spec.js "UI: accented/spaced group name is accepted and still produces a valid URL code"
//   - "groupe: creer plusieurs groupes a la suite"           <- ex critical-flows.spec.js "UI: creating several groups in a row never surfaces a code-collision error"
//   - "identite: suppression ne supprime pas l homonyme"     <- ex critical-flows.spec.js "supprimer un service dans un groupe ne supprime pas le service homonyme d un autre groupe"
//   - "identite: suppression sans fausse erreur"             <- ex critical-flows.spec.js "la suppression d un service n affiche pas de fausse erreur ..."
//   - "insee: service visible dans l UI"                     <- ex insee.spec.mjs "service visible in UI"
//   - "write-behind: toggle mock persiste sur disque"        <- ex write-behind.spec.js "a service mutation made through the UI survives a re-read of the on-disk config"
// Note : "l URL de test affichee en edition correspond a celle de la vue
// liste..." (critical-flows.spec.js) N'A PAS ete migre -- compare une URL
// affichee a une valeur dynamique (code de groupe) connue seulement a
// l'execution, pas modelisable avec des assertions JSON statiques sans
// nouvelle capacite de valeur dynamique dans le runner. Reste un test
// Playwright classique.
//
// Les tests d'origine sont supprimes du fichier source une fois leur
// equivalent JSON valide vert (pas de doublon testant deux fois le meme
// parcours).
import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runScenario, loadScenario } from './scenario-runner.js';

const API = 'http://localhost:7342/api';
const dataDir = process.env.DATA_PATH || resolve(process.cwd(), '../data');
const CONFIG_FILE = resolve(dataDir, 'mock-config.yaml');
function readConfigFromDisk() {
  return readFileSync(CONFIG_FILE, 'utf-8');
}

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
    await runScenario(page, loadScenario('services.scenarios.json', 'Creer un service via le formulaire'));
  });

  test('creer une regle simple (scenario JSON)', async ({ page, request }) => {
    // Prealable hors runner : le scenario ne couvre que le parcours UI de
    // creation de regle, pas la creation du service support -- reste
    // dans le champ des actions minimales (goto/click/fill/assert...),
    // pas de nouvelle action "apiRequest" ajoutee par anticipation.
    await request.post(`${API}/services`, { data: validService('scenario-rule-svc') });
    await runScenario(page, loadScenario('rules.scenarios.json', 'Creer une regle simple sur un service existant'));
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
    await runScenario(page, loadScenario('rules.scenarios.json', "Afficher les regles existantes d'un service"));
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
    await runScenario(page, loadScenario('home.scenarios.json', 'Charger le service de demo depuis la liste vide'));
  });

  test('page d accueil affiche le titre (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('home.scenarios.json', "La page d'accueil se charge avec le titre lightMock"));
  });

  test('liste affiche un service cree via l API (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('ui-test-svc') });
    await runScenario(page, loadScenario('services.scenarios.json', "La liste affiche un service cree via l'API dans son groupe"));
  });

  test('page groupes accessible depuis la nav (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('groups.scenarios.json', 'La page Groupes est accessible depuis la nav principale'));
  });

  test('groupe deplie persiste apres retour d edition (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/groups`, { data: { name: 'persist-grp', code: '', admins: [], members: [] } });
    await request.post(`${API}/services`, { data: validService('persist-svc', { group_name: 'persist-grp' }) });
    await runScenario(page, loadScenario('groups.scenarios.json', "Un groupe deplie reste visible apres retour depuis l'edition d'un service"));
  });

  test('groupe deplie reinitialise apres rechargement (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/groups`, { data: { name: 'reload-grp', code: '', admins: [], members: [] } });
    await request.post(`${API}/services`, { data: validService('reload-svc', { group_name: 'reload-grp' }) });
    await runScenario(page, loadScenario('groups.scenarios.json', "Un rechargement complet de la page reinitialise l'etat deplie d'un groupe"));
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
    await runScenario(page, loadScenario('rules.scenarios.json', 'Le bouton Ajouter une regle fonctionne quand des regles existent deja'));
  });

  test('regle: bouton modifier ouvre le formulaire (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('rules.scenarios.json', 'Le bouton Modifier (crayon) ouvre le formulaire de la regle'));
  });

  test('regle: bouton supprimer retire la regle (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('rules.scenarios.json', 'Le bouton Supprimer retire la regle de la liste'));
  });

  test('service: toggle mock/proxy fonctionne (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('services.scenarios.json', "Le toggle mock/proxy d'un service fonctionne"));
  });

  test('liste: recherche filtre les services (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await request.post(`${API}/services`, { data: validService('other-svc') });
    await runScenario(page, loadScenario('services.scenarios.json', 'La recherche filtre les services et affiche un message si aucun resultat'));
  });

  test('regle: annuler le formulaire revient a la liste (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: ruleTestService('e2e-svc') });
    await runScenario(page, loadScenario('rules.scenarios.json', 'Annuler le formulaire de regle revient a la liste'));
  });

  test('UI servie sans aucun service (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('home.scenarios.json', "La page d'accueil se charge avec le titre lightMock"));
  });

  test('UI accessible apres creation d un service (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('security-svc') });
    await runScenario(page, loadScenario('home.scenarios.json', "La page d'accueil se charge avec le titre lightMock"));
  });
});

test.describe('Runner data-driven (scenarios JSON) - lot 4', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('groupe: formulaire ne demande que le nom (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('groups.scenarios.json', "Le formulaire de creation de groupe ne demande qu'un nom, le code est auto-genere"));
  });

  test('groupe: nom accentue accepte (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('groups.scenarios.json', 'Un nom de groupe accentue/espace est accepte'));
  });

  test('groupe: creer plusieurs groupes a la suite (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('groups.scenarios.json', 'Creer plusieurs groupes a la suite ne bloque jamais sur une collision de code'));
  });

  test('identite: suppression ne supprime pas l homonyme (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/groups`, { data: { name: 'ambig-grp-a', code: '', admins: [], members: [] } });
    await request.post(`${API}/groups`, { data: { name: 'ambig-grp-b', code: '', admins: [], members: [] } });
    await request.post(`${API}/services`, { data: validService('ambig-svc', { group_name: 'ambig-grp-a' }) });
    await request.post(`${API}/services`, { data: validService('ambig-svc', { group_name: 'ambig-grp-b' }) });

    await runScenario(page, loadScenario('services.scenarios.json', "Supprimer un service dans un groupe ne supprime pas son homonyme d'un autre groupe"));

    await expect(async () => {
      const stillB = await request.get(`${API}/groups/ambig-grp-b/services/ambig-svc`);
      expect(stillB.status()).toBe(200);
      const goneA = await request.get(`${API}/groups/ambig-grp-a/services/ambig-svc`);
      expect(goneA.status()).toBe(404);
    }).toPass();
  });

  test('identite: suppression sans fausse erreur (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('no-crash-svc') });
    await runScenario(page, loadScenario('services.scenarios.json', "Supprimer un service n'affiche pas de fausse erreur apres le succes"));
  });

  test('insee: service visible dans l UI (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('tpl-test', { listen_path: '/items/{id}' }) });
    await runScenario(page, loadScenario('services.scenarios.json', 'Le service mocke type INSEE est visible dans la liste UI'));
  });

  test('write-behind: toggle mock persiste sur disque (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, { data: validService('write-behind-svc') });
    await runScenario(page, loadScenario('services.scenarios.json', "Basculer le mode mock/proxy d'un service via l'UI"));

    // Assertion filesystem hors runner (pas une interaction UI, cf
    // README.md) : l'ecriture disque est asynchrone (write-behind, cf
    // CLAUDE.md), on attend que le contenu apparaisse reellement.
    await expect(async () => {
      const yaml = readConfigFromDisk();
      expect(yaml).toContain('name: write-behind-svc');
      expect(yaml).toMatch(/name: write-behind-svc\n(?:.*\n)*?\s*is_mocked: false/);
    }).toPass({ timeout: 5000 });
  });
});
