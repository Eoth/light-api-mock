# Infrastructure E2E data-driven

Cette page documente l'infrastructure ajoutee au sujet 9b (JSON de selecteurs + runner de
scenarios) et son format de regroupement par domaine (sujet 9c). Elle vit **a cote** de la
suite Playwright classique (fichiers `*.spec.js`/`*.spec.mjs` de ce dossier) sans la remplacer
entierement — seuls les parcours qui pilotent reellement l'UI sont candidats a la migration
(cf CLAUDE.md, "Migration progressive", pour l'etat exact et les tests API-only volontairement
non-candidats).

## Vue d'ensemble

```
frontend/e2e/
  selectors.json           <- SOURCE UNIQUE des selecteurs (voir sujet 9a, data-testid)
  scenario-runner.js        <- interpreteur JSON minimal (pas de cucumber/parseur Gherkin)
  scenario-runner.spec.js   <- fichier Playwright qui charge et rejoue les scenarios ci-dessous
  scenarios/
    home.scenarios.json      <- domaine "home" (accueil, service de demo)
    groups.scenarios.json    <- domaine "groups" (creation, accordeon deplie/replie)
    rules.scenarios.json     <- domaine "rules" (CRUD de regle sur un service)
    services.scenarios.json  <- domaine "services" (CRUD, recherche, toggle, identite)
  *.spec.js / *.spec.mjs    <- suite Playwright classique, non migree
  docs-screenshot.js         <- capture docs/screenshots/*.png (no-op sauf DOCS_SCREENSHOTS=1)
frontend/playwright.docs-screenshots.config.js  <- config dediee, cf "Captures d'ecran" plus bas
```

Un scenario JSON decrit une suite d'etapes ordonnees ("esprit Gherkin lisible", sans dependance
BDD). Chaque etape cible un element par un **nom logique** (`"composant.cle"`), jamais par un
selecteur en dur — le nom logique est resolu vers un vrai selecteur CSS `[data-testid=...]` via
`selectors.json`.

## Format d'un fichier de domaine (`*.scenarios.json`)

**Un fichier par domaine fonctionnel, PAS un fichier par scenario individuel** (revu au sujet
9c — l'ancien format "un scenario = un fichier" a produit ~24 petits fichiers difficiles a
retrouver a l'echelle ; la lecon a ete tiree). Chaque fichier de domaine contient un objet
`{domain, scenarios: [...]}` :

```json
{
  "domain": "services",
  "scenarios": [
    {
      "scenario": "Creer un service via le formulaire",
      "steps": [
        { "action": "goto", "page": "services" },
        { "action": "click", "target": "app.addServiceButton" },
        { "action": "fill", "target": "serviceForm.nameInput", "value": "mon-service" },
        { "action": "click", "target": "serviceForm.submitButton" },
        { "action": "assertVisible", "target": "serviceDetail.editButton" }
      ]
    },
    {
      "scenario": "Un autre scenario du meme domaine",
      "steps": [ ... ]
    }
  ]
}
```

- `target` est toujours `"composant.cle"`, ou `composant`/`cle` sont les cles telles
  qu'elles apparaissent dans `selectors.json` (ex. `serviceForm.nameInput`).
- Si le selecteur cible comporte un discriminant (element repete dans une boucle, ex. une carte
  de service par nom), le template dans `selectors.json` contient un ou plusieurs placeholders
  `{nom}` — fournir leurs valeurs via `"params"` :
  ```json
  { "action": "click", "target": "serviceCard.configureButton", "params": { "name": "mon-service" } }
  ```
- `value` est requis pour `fill`/`selectOption`/`assertText` (texte a saisir, valeur d'option,
  texte attendu).
- Actions disponibles (`action`) : `goto`, `click`, `fill`, `selectOption`, `assertVisible`,
  `assertText`, `assertHidden`. **Ne pas ajouter de nouvelle action sans besoin concret** — le
  but est un interpreteur minimal, pas un DSL complet.
  - `goto` prend `"page"` (pas d'URL en dur) : c'est une SPA cote client (etat `view` dans
    `App.svelte`, pas de routeur d'URL reel), donc seule `"services"` (racine `/`) est mappee
    aujourd'hui dans `PAGE_PATHS` (`scenario-runner.js`). Les autres vues (logs, groupes,
    sauvegardes...) se rejoignent par un `click` sur le bouton de nav correspondant
    (`app.navLogsButton`, `app.navGroupsButton`, ...), pas par `goto`.
- Un scenario JSON ne fait QUE des interactions UI. La preparation de donnees (creer un service
  prealable via l'API pour tester une regle dessus, reset de la config...) reste du cote du
  fichier Playwright qui charge le scenario (`test.beforeEach`/`request.post(...)` avant
  `runScenario(...)`), pas une nouvelle action du runner — voir `scenario-runner.spec.js` pour
  un exemple (`creer une regle simple` prepare le service support via `request.post` avant de
  rejouer le scenario).

## Ajouter un nouveau scenario

**Ajouter au fichier de domaine existant qui correspond, ne JAMAIS creer un nouveau fichier par
scenario.** Choisir le domaine par ce que le parcours teste fonctionnellement (pas par le
fichier `*.spec.js` d'origine) :

- `home.scenarios.json` : accueil, service de demo.
- `groups.scenarios.json` : creation de groupe, page Groupes, accordeon deplie/replie.
- `rules.scenarios.json` : CRUD de regle (ajout/modification/suppression/annulation) sur un
  service existant.
- `services.scenarios.json` : CRUD de service, recherche, toggle mock/proxy, identite
  inter-groupes.

Si aucun domaine existant ne convient a un nouveau lot de migration (sujet 9c, lots futurs),
c'est une decision explicite a documenter dans CLAUDE.md — ne pas trancher silencieusement en
ajoutant un 5e fichier sans mettre a jour cette liste.

Ajouter l'entree `{scenario, steps}` dans le tableau `scenarios` du fichier de domaine choisi,
avec un nom de scenario (`scenario`) unique DANS ce fichier — `loadScenario(filename,
scenarioName)` le recherche par ce nom exact.

## Executer un scenario dans un test Playwright

```js
import { test } from '@playwright/test';
import { runScenario, loadScenario } from './scenario-runner.js';

test('mon scenario', async ({ page }) => {
  await runScenario(page, loadScenario('services.scenarios.json', 'Creer un service via le formulaire'));
});
```

`loadScenario(filename, scenarioName)` lit `frontend/e2e/scenarios/<filename>` (un fichier de
domaine) et en extrait le scenario dont le champ `scenario` correspond exactement a
`scenarioName` — erreur explicite si le fichier ou le nom n'existe pas. `loadDomain(filename)`
charge le fichier de domaine complet (`{domain, scenarios: [...]}`) sans filtrer, utile pour
un test qui voudrait rejouer tous les scenarios d'un domaine (aucun test actuel ne le fait,
mais l'API le permet). `runScenario(page, scenario)` rejoue les etapes d'UN scenario dans
l'ordre ; si une etape echoue, l'erreur precise le nom du scenario, le numero de l'etape et son
contenu JSON complet — pas besoin de deviner quelle etape a echoue dans un scenario long.

Un meme scenario peut etre reutilise par plusieurs tests Playwright quand seule la PREPARATION
de donnees differe (ex. le scenario "La page d'accueil se charge avec le titre lightMock" est
rejoue par 3 tests distincts dans `scenario-runner.spec.js`, chacun avec un `beforeEach`/prealable
different) — verifier avant de creer un nouveau scenario si un existant du meme domaine ne
convient pas deja tel quel.

## Ajouter un nouveau selecteur

`selectors.json` est un objet par composant/page (cle = nom du composant en camelCase, ex.
`serviceForm`, `ruleList`, `groupManager`), chaque entree mappant un nom logique vers
`[data-testid="..."]`. **Non concerne par le regroupement par domaine ci-dessus** — un seul
fichier `selectors.json`, quel que soit le nombre de fichiers de domaine.

1. Verifier que le `data-testid` existe deja sur le composant (cf sujet 9a, convention
   documentee dans `CLAUDE.md` §3/§5 point 52 : `{composant-kebab}-{role-element}[-{discriminant}]`).
   S'il n'existe pas encore, l'ajouter au composant Svelte d'abord (en suivant la meme
   convention), PUIS l'enregistrer ici — ne jamais inventer un selecteur qui ne correspond a
   rien dans le DOM.
2. Ajouter l'entree dans le bon groupe de `selectors.json`, avec une cle logique en camelCase
   (ex. `nameInput`, `submitButton`, `deleteButton`).
3. Pour un element repete (discriminant dans le `data-testid`), utiliser un placeholder
   `{nomDuParametre}` dans le template — le nom du placeholder est libre, il doit juste
   correspondre a la cle utilisee dans `"params"` des scenarios qui l'utilisent :
   ```json
   "serviceCard": {
     "card": "[data-testid=\"service-card-{name}\"]"
   }
   ```
4. `selectors.json` reste la SEULE source de verite : ne jamais ecrire un selecteur CSS en dur
   dans un scenario JSON ou dans `scenario-runner.spec.js`.

## Verifier la coherence selectors.json / code source

Aucun script de generation automatique n'est fourni dans cette passe (le fichier est tenu a jour
a la main, meme principe que `frontend/src/lib/rhai-functions.js` vis-a-vis de `script.rs` — la
coherence humaine est la garde-fou, pas une CI dediee). En cas de doute sur un decalage entre
`selectors.json` et les `data-testid` reels, une recherche simple suffit :

```bash
grep -rhoE 'data-testid="[^"]*"' frontend/src --include="*.svelte" | sort -u
```

## Captures d'écran pour docs/ (sujet 13b)

`docs/` (sujet 13a) contient des marqueurs `<!-- SCREENSHOT: ... -->` remplacés par de vraies
images générées à partir de la suite E2E existante, pour qu'une capture reste à jour
automatiquement au lieu de se périmer au premier changement d'UI.

- **`docs-screenshot.js`** exporte `docsScreenshot(page, filename)` : no-op tant que la variable
  d'environnement `DOCS_SCREENSHOTS` n'est pas positionnée (donc **zéro coût sur la suite E2E
  standard**, `npm run test:e2e`), sinon écrit `docs/screenshots/<filename>.png`.
- Dans un scénario JSON (`scenarios/*.scenarios.json`), une étape
  `{ "action": "screenshot", "file": "nom.png" }` déclenche une capture au point exact du
  parcours — ajoutée comme n'importe quelle autre étape, entre deux étapes déjà existantes.
  Ne JAMAIS ajouter une capture en créant un nouveau parcours UI seulement pour l'illustrer
  (cf CLAUDE.md, "Captures d'écran de documentation") — seuls des points déjà traversés par un
  scénario existant sont capturés.
- Dans un fichier `*.spec.js`/`*.spec.mjs` classique, un appel direct
  `await docsScreenshot(page, 'nom.png');` est inséré entre deux lignes de test déjà existantes
  (jamais en ajoutant une interaction UI supplémentaire) — voir `backups.spec.js`,
  `rule-tester.spec.js`, `messaging.spec.js`, `config.spec.mjs`, `rhai-autocomplete.spec.js`.
- **`playwright.docs-screenshots.config.js`** (racine `frontend/`) est une config Playwright
  SÉPARÉE de `playwright.config.js` : elle positionne `DOCS_SCREENSHOTS=1` (dans le module de
  config, donc valable identiquement sous PowerShell/cmd/bash sans syntaxe shell spécifique) et
  restreint `testMatch` aux seuls fichiers contenant des captures — les autres fichiers de la
  suite n'ont besoin d'aucune capture et ne sont pas exécutés par cette commande.

### Régénérer les captures

```bash
npm run docs:screenshots
```

(depuis `frontend/`, backend lightMock déjà démarré sur `http://localhost:7342` — même prérequis
que `npm run test:e2e`). Les images sont écrites dans `docs/screenshots/`, sous les noms déjà
référencés par les `docs/*.md` — regénérer écrase les fichiers existants, aucune étape manuelle
supplémentaire n'est nécessaire après coup.

Le bouton "Messages Kafka" et les captures qui en dépendent (`messaging-bouton-nav.png`,
`messaging-journal-statuts.png`, `messaging-formulaire-simulation.png`) ne sont produits que
contre un binaire compilé avec `--features messaging-kafka` (sinon `messaging.spec.js` est
`test.skip`, cf CLAUDE.md) — régénérer contre un tel binaire si ces 3 images manquent.

### Captures manquantes (à faire manuellement)

Certains marqueurs `docs/*.md` n'ont aucun scénario E2E existant capable de les produire (état
très spécifique non couvert par un test actuel) — plutôt que de complexifier la suite de tests
pour un besoin purement illustratif, ces cas sont documentés dans le `.md` concerné par une note
`*(Capture manquante — ...)*` expliquant pourquoi, et listés dans CLAUDE.md. Ne pas créer de
nouveau test E2E dans le seul but de produire une de ces captures sans un besoin de test réel
sous-jacent.

## Suite existante vs infrastructure data-driven

De nombreux tests Playwright restent des fichiers `*.spec.js`/`*.spec.mjs` classiques
(role/texte/CSS), en particulier tous les tests **API-only** (aucune interaction `page`,
uniquement `request.get/post/put/delete` avec assertions sur le code HTTP/JSON) : migrer un
test API-only vers `scenario-runner.js` produirait un scenario JSON vide de sens (l'outil ne
fait QUE des interactions UI) et casserait la coherence du format — ce ne sont pas des
candidats de migration, decision assumee (cf CLAUDE.md, "Migration progressive", pour le detail
et la liste des tests restants).
