// Tests E2E utilisant l'infrastructure data-driven (selectors.json +
// scenario-runner.js, cf frontend/e2e/README.md). Beaucoup de ces tests
// REMPLACENT un test equivalent qui pilotait auparavant l'UI directement
// dans un fichier *.spec.js/*.spec.mjs classique (rules.spec.mjs,
// critical-flows.spec.js, insee.spec.mjs, write-behind.spec.js,
// security.spec.js, group-expansion-persistence.spec.js, config.spec.mjs) --
// l'ancien test est supprime de son fichier source une fois son equivalent
// JSON valide vert, pour eviter un doublon testant deux fois le meme
// parcours. Seuls les parcours qui pilotent reellement l'UI sont candidats
// a cette migration (un test purement API-only n'a rien a gagner au format
// scenario JSON).
//
// Les scenarios eux-memes sont regroupes par domaine fonctionnel dans
// frontend/e2e/scenarios/{home,groups,rules,services}.scenarios.json (un
// fichier par domaine, un tableau de scenarios par fichier).
// `loadScenario(domainFile, scenarioName)` en extrait un seul.
//
// Exception non migree : "l'URL de test affichee en edition correspond a
// celle de la vue liste..." (critical-flows.spec.js) compare une URL
// affichee a une valeur dynamique (code de groupe) connue seulement a
// l'execution -- pas modelisable avec des assertions JSON statiques sans
// nouvelle capacite de valeur dynamique dans le runner. Reste un test
// Playwright classique.
//
// Les sections "Lot N" ci-dessous, chacune juste au-dessus des tests
// qu'elle introduit, documentent au fil de l'eau la couverture E2E ajoutee :
// detecteur de conflit de regles, service "purement mocke", pliage des
// arbres JSON/XML + repli par defaut des Options avancees pre_script/
// post_script, illustrations pour la documentation utilisateur, portage du
// mode "coller un exemple" au XML, condition XPath sur un corps SOAP,
// edition en place d'une condition de regle, restauration de la vue
// d'origine a l'edition d'une reponse, source "XPath (XML/SOAP)" du builder
// de reponse, correctifs de rendu JSON/XML, et `parse_date`.
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
  // ses 2 tests sont entierement migres ici). Contexte
  // produit conserve de ce fichier : ils verifient le niveau 1 de
  // persistance de l'etat "groupe deplie/replie" (voir
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
    // README.md) : l'ecriture disque est asynchrone (write-behind), on
    // attend que le contenu apparaisse reellement.
    await expect(async () => {
      const yaml = readConfigFromDisk();
      expect(yaml).toContain('name: write-behind-svc');
      expect(yaml).toMatch(/name: write-behind-svc\n(?:.*\n)*?\s*is_mocked: false/);
    }).toPass({ timeout: 5000 });
  });
});

// Lot 5 : couverture E2E neuve (pas une migration) pour le detecteur de
// conflit entre regles a la sauvegarde (POST /api/rule-conflicts).
// "conflict-rule-one" (GET, sans sous-chemin, sans condition —
// la regle la plus generale possible) sert de base : toute autre regle GET
// sans sous-chemin ni condition creee ensuite sur ce meme service la
// chevauche trivialement (ensembles de conditions vides identiques,
// cf MatchEngine::find_rule_conflicts).
test.describe('Runner data-driven (scenarios JSON) - lot 5 (detecteur de conflit)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('conflict-svc', { rules: [validRule('conflict-rule-one')] }),
    });
  });

  test('regle en conflit: avertissement affiche (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Une regle qui chevauche une regle existante declenche un avertissement de conflit'));
  });

  test('regle sans conflit: aucun avertissement (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Une regle qui ne chevauche aucune regle existante ne declenche aucun avertissement'));
  });

  test('regle en conflit: enregistrer quand meme fonctionne (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', "Enregistrer quand meme malgre l'avertissement de conflit fonctionne"));
  });
});

// Lot 6 : couverture E2E neuve (pas une migration) pour le service
// "purement mocke" (real_target_url vide).
test.describe('Runner data-driven (scenarios JSON) - lot 6 (service purement mocke)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
  });

  test('creer un service purement mocke (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('services.scenarios.json', 'Creer un service purement mocke via le formulaire'));
  });

  test('decocher purement mocke reaffiche la cible sans perte de regles (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('purely-mocked-existing', { real_target_url: '', rules: [validRule('existing-rule')] }),
    });
    await runScenario(page, loadScenario('services.scenarios.json', 'Decocher purement mocke reaffiche la cible sans perte des regles'));
  });

  test('action Proxy absente pour une regle d un service purement mocke (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('purely-mocked-for-rule', { real_target_url: '' }),
    });
    await runScenario(page, loadScenario('rules.scenarios.json', "L'action Proxy est absente pour une regle d'un service purement mocke"));
  });

  test('bascule a posteriori avec regle Proxy existante avertit sans bloquer (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('svc-with-proxy-rule', { rules: [validRule('legacy-proxy-rule', { action: 'proxy' })] }),
    });
    await runScenario(page, loadScenario('services.scenarios.json', 'Bascule a posteriori vers purement mocke avec une regle Proxy existante affiche un avertissement'));
  });

  test('modifier une regle proxy heritee avertit avant de persister le changement vers mock (scenario JSON)', async ({ page, request }) => {
    await request.post(`${API}/services`, {
      data: validService('purely-mocked-stale-proxy', {
        real_target_url: '',
        rules: [validRule('stale-proxy-rule', { action: 'proxy' })],
      }),
    });
    await runScenario(page, loadScenario('rules.scenarios.json', 'Modifier une regle proxy heritee sur un service purement mocke affiche un avertissement avant sauvegarde'));

    // Verification hors runner (pas une interaction UI) : le clic sur
    // "Enregistrer quand meme" a bien persiste le changement reel
    // proxy -> mock, pas seulement fait disparaitre l'avertissement a
    // l'ecran.
    const resp = await request.get(`${API}/services/purely-mocked-stale-proxy`);
    const body = await resp.json();
    const rule = body.rules.find((r) => r.name === 'stale-proxy-rule');
    expect(rule.action).toBe('mock');
    expect(rule.sub_path).toBe('/updated');
  });
});

// Lot 7 : couverture E2E neuve (pas une migration) pour le pliage des
// arbres JSON/XML et le repli par defaut des Options avancees
// (pre_script/post_script). Seul le pliage JSON est couvert ici en E2E (le
// mecanisme XML est strictement identique --
// meme composant de pliage, meme attribut `hidden` -- deja verifie en
// profondeur par XmlResponseBuilder.test.js ; dupliquer un parcours UI
// quasi identique en E2E n'aurait ajoute aucune garantie supplementaire).
test.describe('Runner data-driven (scenarios JSON) - lot 7 (pliage JSON + options avancees)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('fold-adv-svc', {
        rules: [validRule('fold-adv-existing-rule', { post_script: '"deja configure"' })],
      }),
    });
  });

  test('options avancees repliees par defaut pour une nouvelle regle (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Options avancees repliees par defaut pour une nouvelle regle'));
  });

  test('options avancees s ouvre automatiquement si post_script deja rempli (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Options avancees s ouvre automatiquement si une regle existante a deja du post_script'));
  });

  test('saisir du contenu dans le pre-script survit au pliage/depliage des options avancees (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Saisir du contenu dans le pre-script puis plier/deplier les options avancees ne perd rien'));

    // Verification hors runner : le contenu saisi dans le pre-script avant
    // le pliage a bien ete persiste (preuve reelle, pas seulement que le
    // champ redevient visible a l'ecran).
    const resp = await request.get(`${API}/services/fold-adv-svc`);
    const body = await resp.json();
    const rule = body.rules.find((r) => r.name === 'fold-adv-rule');
    expect(rule.pre_script).toBe('"greeting"');
  });

  test('replier un noeud JSON imbrique ne perd pas son contenu (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Replier un noeud JSON imbrique masque ses sous-champs sans perdre leur contenu'));

    // Verification hors runner : le sous-champ saisi pendant que le noeud
    // parent etait replie (puis redeplie) a bien ete persiste dans le
    // template genere.
    const resp = await request.get(`${API}/services/fold-adv-svc`);
    const body = await resp.json();
    const rule = body.rules.find((r) => r.name === 'fold-json-rule');
    expect(rule.response.body[0].template).toContain('"parent"');
    expect(rule.response.body[0].template).toContain('"child"');
    expect(rule.response.body[0].template).toContain('hello');
  });
});

// Lot 8 : couverture E2E neuve (pas une migration) illustrant, pour la doc
// utilisateur (docs/regles-de-matching.md), qu'un meme service peut deja
// repondre differemment selon le header SOAPAction via deux regles
// independantes (chacune avec sa propre condition Header/SOAPAction) --
// aucune fonctionnalite nouvelle, juste la capture des deux ecrans de
// configuration de condition.
test.describe('Runner data-driven (scenarios JSON) - lot 8 (doc SOAPAction)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, { data: validService('soap-routing-demo') });
  });

  test('deux regles routees par SOAPAction sur le meme service (scenario JSON)', async ({ page }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Deux regles sur le meme service routees par le header SOAPAction (illustration doc)'));
  });
});

// Lot 9 : couverture E2E neuve (pas une migration) illustrant, pour la doc
// utilisateur (docs/scripts-rhai.md), le pattern "la requete contient une
// liste d'objets, la reponse doit contenir le meme nombre d'elements
// construits par position" via parse_json/to_json (script Rhai). Seul
// l'exemple JSON est illustre en UI (l'exemple XML/SOAP equivalent est deja
// verifie bout-en-bout par les tests d'integration Rust dans
// src/server/intercept.rs -- dupliquer un parcours UI quasi identique en
// E2E n'aurait ajoute aucune garantie supplementaire, cf §3 sobriete des
// tests du projet).
test.describe('Runner data-driven (scenarios JSON) - lot 9 (repetition JSON/XML)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('repeat-pattern-svc', { listen_path: '/calcul', real_target_url: '' }),
    });
  });

  test('configurer une regle de repetition JSON via l UI produit bien N elements de reponse (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Configurer une regle qui repete un element de reponse par element de la requete (illustration doc)'));

    // Verification hors runner (pas une interaction UI) : la regle
    // configuree via le formulaire produit reellement le comportement
    // documente -- un vrai appel HTTP avec 2 lignes doit renvoyer 2
    // elements, chacun construit a partir de la ligne correspondante.
    const resp = await request.post('http://localhost:7342/repeat-pattern-svc/calcul', {
      data: { lines: [{ sku: 'REF-001', qty: 3 }, { sku: 'REF-002', qty: 1 }] },
    });
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.count).toBe(2);
    expect(body.lines).toHaveLength(2);
    expect(body.lines[0].sku).toBe('REF-001');
    expect(body.lines[0].qty).toBe(3);
    expect(body.lines[1].sku).toBe('REF-002');
    expect(body.lines[1].qty).toBe(1);
    expect(body.lines[0].lineTotal).toBe(body.lines[0].unitPrice * 3);
  });
});

// Lot 10 : couverture E2E neuve (pas une migration) pour le portage du mode
// "coller un exemple" au XML (XmlPasteBuilder.svelte, miroir de
// JsonPasteBuilder.svelte avec breadcrumb/pliage/attributs XML en plus).
// Colle un exemple XML imbrique
// (enveloppe avec un attribut de namespace), navigue dans le noeud enfant
// via le fil d'Ariane, transforme une valeur en variable de path param, puis
// verifie hors runner qu'une vraie requete HTTP produit bien le XML attendu
// (valeur substituee + attribut/contenu fixe preserves) -- pas seulement que
// le formulaire se soumet sans erreur.
test.describe('Runner data-driven (scenarios JSON) - lot 10 (XML par exemple)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('xml-paste-demo', { listen_path: '/quote/{siret}' }),
    });
  });

  test('configurer une regle via le mode XML par exemple produit le XML attendu (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Configurer une regle via le mode XML par exemple (coller un exemple SOAP)'));

    // Verification hors runner : la regle configuree visuellement (paste +
    // navigation breadcrumb + assignation source=path sur un champ imbrique)
    // produit reellement, a l'execution, un XML ou le siret colle a
    // l'origine (fixe) a bien ete remplace par le path param de la vraie
    // requete, tandis que le nom (jamais reassigne) et l'attribut de
    // namespace de la racine (jamais touche) restent preserves tels quels.
    const resp = await request.get('http://localhost:7342/xml-paste-demo/quote/12345678901234');
    expect(resp.status()).toBe(200);
    const xml = await resp.text();
    expect(xml).toContain('<devisResponse xmlns:x="urn:test">');
    expect(xml).toContain('<nom>ACME Corp</nom>');
    expect(xml).toContain('<siret>12345678901234</siret>');
    expect(xml).not.toContain('00000000000000');
  });
});

// Lot 11 : couverture E2E neuve (pas une migration), comble un trou constate
// a l'audit documentaire de docs/ -- le mode "exemple d'abord" JSON
// (JsonPasteBuilder.svelte) n'avait jamais ete illustre par une capture,
// contrairement a sa variante XML (lot 10). Colle un exemple JSON plat,
// verifie la detection des champs, puis reassigne un champ en parametre de
// chemin et verifie hors runner qu'une vraie requete HTTP produit bien la valeur
// substituee -- pas seulement que le formulaire se soumet sans erreur.
test.describe('Runner data-driven (scenarios JSON) - lot 11 (JSON par exemple)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('json-paste-demo', { listen_path: '/entreprise/{siret}' }),
    });
  });

  test('configurer une regle via le mode JSON par exemple produit le JSON attendu (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Configurer une regle via le mode JSON par exemple (coller un exemple)'));

    const resp = await request.get('http://localhost:7342/json-paste-demo/entreprise/44306184100047');
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.siret).toBe('44306184100047');
    expect(body.nom).toBe('ACME Corp');
    expect(body.actif).toBe(true);
  });
});

// Lot 12 : couverture E2E neuve (pas une migration) -- configure via l'UI
// une condition XPath sur un XML SOAP namespace (Envelope/Body/recherche)
// et un script d'extraction
// (parse_xml_items) qui reinjecte le Siret de la requete dans la reponse.
// Verifie hors runner (vraie requete HTTP avec un corps SOAP realiste,
// Header non-autoferme sibling de Body) que la condition matche bien et que
// le Siret extrait se retrouve dans la reponse -- pas seulement que le
// formulaire se soumet sans erreur.
test.describe('Runner data-driven (scenarios JSON) - lot 12 (XPath SOAP + extraction)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('soap-extraction-demo', { listen_path: '/service', real_target_url: '' }),
    });
  });

  test('configurer une condition XPath SOAP + extraction via l UI produit bien le Siret dans la reponse (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Condition XPath sur un XML SOAP namespace + extraction d une valeur vers la reponse (illustration doc)'));

    // Verification hors runner : requete SOAP realiste, avec un
    // <Header></Header> non-autoferme sibling de <Body> (structure qui
    // declenchait le bug corrige de walk_xml) -- la condition XPath doit
    // matcher malgre le Header, et le Siret de la requete doit se retrouver
    // tel quel dans la reponse.
    const resp = await request.post('http://localhost:7342/soap-extraction-demo/service', {
      headers: { 'Content-Type': 'text/xml' },
      data: '<SOAP:Envelope><SOAP-ENV:Header></SOAP-ENV:Header><SOAP-ENV:Body><ns3:recherche><ns3:Nom>Test</ns3:Nom><ns3:Siret>98765432109876</ns3:Siret></ns3:recherche></SOAP-ENV:Body></SOAP:Envelope>',
    });
    expect(resp.status()).toBe(200);
    const xml = await resp.text();
    expect(xml).toContain('<siret>98765432109876</siret>');
  });
});

// Lot 13 : couverture E2E neuve (pas une migration), pour l'edition en place
// d'une condition de regle (jusqu'ici il fallait supprimer puis recreer).
// Cree une regle avec une condition QueryParam, l'edite en cliquant dessus
// (bascule vers Header, nouvelle cle, nouvelle valeur), sauvegarde, puis
// verifie hors runner via de vraies requetes HTTP que le MATCHING refletebien
// la nouvelle condition (et plus l'ancienne) -- pas seulement que le
// formulaire affiche le nouveau libelle.
test.describe('Runner data-driven (scenarios JSON) - lot 13 (edition en place d une condition)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('edit-condition-svc', { listen_path: '/service', real_target_url: '' }),
    });
  });

  test('editer une condition existante en place change reellement le comportement de matching (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', "Modifier une condition existante d'une regle (edition en place)"));

    // Verification hors runner : la nouvelle condition (Header X-Mode=new-value)
    // doit matcher...
    const withNewHeader = await request.post('http://localhost:7342/edit-condition-svc/service', {
      headers: { 'X-Mode': 'new-value' },
    });
    expect(withNewHeader.status()).toBe(200);

    // ... alors que l'ANCIENNE condition (query param mode=legacy), qui a ete
    // remplacee et non simplement complementee, ne doit plus matcher du tout
    // (service purement mocke : aucune regle ne correspond -> 404).
    const withOldQueryParam = await request.post('http://localhost:7342/edit-condition-svc/service?mode=legacy');
    expect(withOldQueryParam.status()).toBe(404);

    // ... et l'ancienne VALEUR sur la nouvelle cle ne doit pas non plus
    // matcher, pour ecarter un faux positif ou seule la cle aurait change.
    const withWrongValue = await request.post('http://localhost:7342/edit-condition-svc/service', {
      headers: { 'X-Mode': 'legacy' },
    });
    expect(withWrongValue.status()).toBe(404);
  });
});

// Lot 14 : couverture E2E neuve (pas une migration), pour la restauration
// de la vue d'origine a l'edition d'une reponse. Avant cette passe, editer
// une regle construite via n'importe quel mode structure
// (JSON/XML, par exemple/guide) atterrissait TOUJOURS en "Template avance",
// meme heuristique retour 1 corrigee par Rule.response_mode (backend) +
// RuleResponseSection.svelte::computeInitialEditorState(). Verifie aussi le
// retour 2 (pipes desormais disponibles en mode "par exemple") et, de facto,
// le retour 3 (les boutons de Format fusionnes JSON/XML + reveal "Modifier en
// detail" sont le seul chemin desormais disponible pour atteindre ces vues).
test.describe('Runner data-driven (scenarios JSON) - lot 14 (restauration de la vue d origine + pipes en mode par exemple)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('view-restore-json', { listen_path: '/echo/{siret}' }),
    });
    await request.post(`${API}/services`, {
      data: validService('view-restore-json-detail'),
    });
    await request.post(`${API}/services`, {
      data: validService('view-restore-xml', { listen_path: '/echo/{siret}' }),
    });
  });

  test('editer une regle JSON par exemple restaure la vue assistee, pipe applique (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Editer une regle JSON par exemple restaure la vue assistee avec le pipe applique'));

    // Le scenario reouvre la regle et change le pipe upper -> lower AVANT de
    // resauvegarder : verifie hors runner que ce changement, fait depuis la
    // vue restauree (pas depuis un "template avance" reconstruit a vide),
    // est reellement celui qui a ete persiste.
    const resp = await request.get('http://localhost:7342/view-restore-json/echo/abc123');
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.siret).toBe('abc123');
    expect(body.note).toBe('bonjour');
  });

  test('editer une regle JSON en detail restaure la vue detaillee, pas le template avance (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Editer une regle JSON en detail restaure la vue detaillee, pas le template avance'));
  });

  test('editer une regle XML par exemple restaure la vue assistee, pipe applique (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Editer une regle XML par exemple restaure la vue assistee avec le pipe applique'));

    const resp = await request.get('http://localhost:7342/view-restore-xml/echo/abc123');
    expect(resp.status()).toBe(200);
    const xml = await resp.text();
    expect(xml).toContain('<siret>ABC123</siret>');
    expect(xml).toContain('<note>bonjour</note>');
  });
});

// Lot 15 : couverture E2E neuve (pas une migration), pour la source "XPath
// (XML/SOAP)" ajoutee au builder de reponse XML (guide + par exemple). Avant
// cette passe, la seule option d'extraction depuis le corps de requete
// disponible dans ce menu etait "Echo body" (JSON pointer, non fonctionnel
// pour un corps XML/SOAP -- silencieusement vide) ; extraire une valeur XML
// necessitait un script Rhai complet (parse_xml_items). Configure via l'UI
// reelle une regle dont la reponse XML guidee reinjecte une valeur XPath du
// corps SOAP (avec un pipe substr pour ne garder que les 9 premiers
// caracteres), verifie hors runner via une vraie requete SOAP (avec un
// Header non-autoferme sibling de Body, structure qui declenchait auparavant
// un bug de matching XPath desormais corrige) que la valeur extraite et
// tronquee se retrouve dans la reponse.
test.describe('Runner data-driven (scenarios JSON) - lot 15 (source XPath dans le builder XML)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('xpath-echo-demo', { listen_path: '/service', real_target_url: '' }),
    });
  });

  test('la source XPath du builder XML guide extrait et tronque une valeur du corps SOAP (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Extraire une valeur du corps SOAP via la source XPath du builder XML guide'));

    const resp = await request.post('http://localhost:7342/xpath-echo-demo/service', {
      headers: { 'Content-Type': 'text/xml' },
      data: '<SOAP:Envelope><SOAP-ENV:Header></SOAP-ENV:Header><SOAP-ENV:Body><ns3:recherche><ns3:Siret>98765432109876</ns3:Siret></ns3:recherche></SOAP-ENV:Body></SOAP:Envelope>',
    });
    expect(resp.status()).toBe(200);
    const xml = await resp.text();
    expect(xml).toContain('<siret>987654321</siret>');
  });
});

// Lot 16 (4 tests, couverture neuve) : un scenario par symptome corrige dans
// le rendu des reponses JSON/XML. Chacun verifie explicitement le cas qui
// echouait avant le correctif, pas seulement que le formulaire se soumet
// sans erreur.
test.describe('Runner data-driven (scenarios JSON) - lot 16 (diagnostic reponse JSON/XML)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('json-fold-demo', { listen_path: '/service', real_target_url: '' }),
    });
    await request.post(`${API}/services`, {
      data: validService('advanced-to-xml-demo', { listen_path: '/service', real_target_url: '' }),
    });
    await request.post(`${API}/services`, {
      data: validService('back-to-paste-demo', { listen_path: '/service', real_target_url: '' }),
    });
    await request.post(`${API}/services`, {
      data: validService('script-value-detail-demo', { listen_path: '/service', real_target_url: '' }),
    });
  });

  test('symptome 1 : replier un champ objet en vue JSON par exemple ne perd aucune donnee (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Replier un champ objet en vue JSON par exemple ne perd aucune donnee'));

    // Avant le correctif, ce chevron de pliage n'existait pas du tout dans
    // cette vue (JsonPasteBuilder.svelte) -- le scenario ci-dessus l'exerce
    // deja explicitement (repli/depli en cours de route) ; on verifie ici
    // que ni le champ non touche (nom) ni le champ modifie APRES un cycle
    // repli/depli (siret) n'ont ete perdus ou corrompus par le pliage.
    const resp = await request.post('http://localhost:7342/json-fold-demo/service');
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body).toEqual({ client: { nom: 'ACME', siret: '12345678901234' } });
  });

  test('symptome 2 : un template XML valide en mode avance se convertit vers XML sans echouer (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Convertir un template avance XML valide vers le format XML sans avertissement'));

    // Avant le correctif, cette conversion echouait TOUJOURS (meme pour un
    // XML valide) -- le scenario n'aurait jamais pu depasser les deux
    // assertVisible sur la vue "par exemple" XML (elles auraient trouve la
    // banniere d'avertissement a la place). Verifie ici que le contenu
    // ET l'attribut de racine ont ete correctement repris jusqu'a la
    // sauvegarde (pas seulement que le formulaire s'est soumis).
    const resp = await request.post('http://localhost:7342/advanced-to-xml-demo/service');
    expect(resp.status()).toBe(200);
    const xml = await resp.text();
    expect(xml).toBe('<devisResponse ver="1"><nom>ACME</nom></devisResponse>');
  });

  test('symptome 3 : revenir a la vue par exemple depuis le detail JSON preserve le contenu (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Revenir a la vue par exemple depuis le detail JSON preserve le contenu'));

    // Avant le correctif, aucun bouton retour n'existait -- le scenario
    // n'aurait jamais pu cliquer dessus. Verifie ici que le champ ajoute EN
    // DETAIL (siret) ET le champ d'origine collé (nom) sont bien tous deux
    // persistes apres l'aller-retour detail -> par exemple -> sauvegarde.
    const resp = await request.post('http://localhost:7342/back-to-paste-demo/service');
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body).toEqual({ nom: 'ACME', siret: '12345678901234' });
  });

  test('symptome 4 : la source Resultat du script reste utilisable en vue JSON detail (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'La source Resultat du script reste utilisable en vue JSON detail'));

    // Avant le correctif, le champ de saisie de la cle du script etait
    // masque : impossible de preciser QUELLE cle du resultat de script
    // utiliser, la regle n'aurait donc jamais pu produire {{script.nom}}.
    const resp = await request.post('http://localhost:7342/script-value-detail-demo/service');
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body).toEqual({ nom: 'ACME Corp' });
  });
});

// Lot 17 (1 test, couverture neuve) : `parse_date`, sens inverse de
// date_now/date_past/date_future. Configure via l'UI reelle
// une regle dont le script appelle parse_date(request.query.date, "dd/MM/yyyy")
// et verifie hors runner qu'une vraie requete HTTP renvoie bien la date
// saisie convertie en millisecondes depuis epoch -- pas seulement que le
// formulaire se soumet sans erreur.
test.describe('Runner data-driven (scenarios JSON) - lot 17 (parse_date)', () => {
  test.beforeEach(async ({ request }) => {
    await request.delete(`${API}/config/reset`);
    await request.post(`${API}/services`, {
      data: validService('parse-date-demo', { listen_path: '/convert', real_target_url: '' }),
    });
  });

  test('configurer une regle avec parse_date via l UI produit bien la date en millisecondes (scenario JSON)', async ({ page, request }) => {
    await runScenario(page, loadScenario('rules.scenarios.json', 'Convertir une date saisie dans un format personnalise en millisecondes via parse_date (illustration doc)'));

    const resp = await request.get('http://localhost:7342/parse-date-demo/convert?date=15/03/2026');
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    // 15/03/2026 00:00:00 UTC, cf src/engine/script.rs::parse_date_iso_pattern_date_only.
    expect(body.ms).toBe(1773532800000);
  });
});
