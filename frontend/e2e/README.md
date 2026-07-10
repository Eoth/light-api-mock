# Infrastructure E2E data-driven

Cette page documente l'infrastructure ajoutee au sujet 9b (JSON de selecteurs + runner de
scenarios). Elle vit **a cote** de la suite Playwright existante (81 tests dans les fichiers
`*.spec.js`/`*.spec.mjs` de ce dossier) sans la remplacer — la migration complete de ces tests
vers ce nouveau format est prevue pour un sujet ulterieur (9c), pas encore faite.

## Vue d'ensemble

```
frontend/e2e/
  selectors.json           <- SOURCE UNIQUE des selecteurs (voir sujet 9a, data-testid)
  scenario-runner.js        <- interpreteur JSON minimal (pas de cucumber/parseur Gherkin)
  scenario-runner.spec.js   <- fichier Playwright qui charge et rejoue les scenarios ci-dessous
  scenarios/
    create-service.scenario.json
    create-simple-rule.scenario.json
  *.spec.js / *.spec.mjs    <- suite Playwright existante (81 tests), inchangee
```

Un scenario JSON decrit une suite d'etapes ordonnees ("esprit Gherkin lisible", sans dependance
BDD). Chaque etape cible un element par un **nom logique** (`"composant.cle"`), jamais par un
selecteur en dur — le nom logique est resolu vers un vrai selecteur CSS `[data-testid=...]` via
`selectors.json`.

## Ecrire un nouveau scenario JSON

Format :

```json
{
  "scenario": "Nom lisible du scenario",
  "steps": [
    { "action": "goto", "page": "services" },
    { "action": "click", "target": "app.addServiceButton" },
    { "action": "fill", "target": "serviceForm.nameInput", "value": "mon-service" },
    { "action": "click", "target": "serviceForm.submitButton" },
    { "action": "assertVisible", "target": "serviceDetail.editButton" }
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
- Placer le fichier dans `frontend/e2e/scenarios/*.scenario.json`.
- Un scenario JSON ne fait QUE des interactions UI. La preparation de donnees (creer un service
  prealable via l'API pour tester une regle dessus, reset de la config...) reste du cote du
  fichier Playwright qui charge le scenario (`test.beforeEach`/`request.post(...)` avant
  `runScenario(...)`), pas une nouvelle action du runner — voir `scenario-runner.spec.js` pour
  un exemple (`creer une regle simple` prepare le service support via `request.post` avant de
  rejouer le scenario).

## Executer un scenario dans un test Playwright

```js
import { test } from '@playwright/test';
import { runScenario, loadScenario } from './scenario-runner.js';

test('mon scenario', async ({ page }) => {
  await runScenario(page, loadScenario('mon-scenario.scenario.json'));
});
```

`loadScenario(filename)` lit `frontend/e2e/scenarios/<filename>`. `runScenario(page, scenario)`
rejoue les etapes dans l'ordre ; si une etape echoue, l'erreur precise le nom du scenario, le
numero de l'etape et son contenu JSON complet — pas besoin de deviner quelle etape a echoue dans
un scenario long.

## Ajouter un nouveau selecteur

`selectors.json` est un objet par composant/page (cle = nom du composant en camelCase, ex.
`serviceForm`, `ruleList`, `groupManager`), chaque entree mappant un nom logique vers
`[data-testid="..."]`.

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

## Suite existante vs infrastructure data-driven

Les 81 tests Playwright existants (`*.spec.js`/`*.spec.mjs`, hors ce README) ne sont PAS
modifies par cette infrastructure : ils continuent d'utiliser leurs selecteurs actuels
(role/texte/CSS), qui restent valides. `scenario-runner.spec.js` et les 2 scenarios
d'exemple (`create-service.scenario.json`, `create-simple-rule.scenario.json`) couvrent des
parcours deja testes autrement (cf `rules.spec.mjs`), volontairement en double, pour prouver que
l'infrastructure fonctionne avant de s'engager sur une migration complete — sujet 9c, non
commence.
