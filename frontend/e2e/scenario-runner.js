// Petit interpreteur JSON pour des scenarios E2E "esprit Gherkin lisible"
// (une suite d'etapes dans l'ordre), sans dependance BDD lourde (pas de
// cucumber, pas de parseur Gherkin reel) : juste un objet JSON {scenario,
// steps[]} rejoue via Playwright. Les selecteurs ne sont JAMAIS en dur dans
// un scenario : `target` est un nom logique "composant.cle" resolu via
// selectors.json (SOURCE UNIQUE des selecteurs, cf CLAUDE.md et
// frontend/e2e/README.md). Voir frontend/e2e/README.md pour le format
// complet et comment ajouter un nouveau scenario/selecteur.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect } from '@playwright/test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const selectors = JSON.parse(fs.readFileSync(path.join(__dirname, 'selectors.json'), 'utf8'));

// Une SPA cote client (etat `view` dans App.svelte, pas de routeur d'URL
// reel) : seule la racine correspond a une vraie navigation navigateur.
// Les autres "pages" (logs, groupes, ...) se rejoignent par des clics
// (action `click` sur un bouton de nav), pas par `goto` -- etendre cette
// map uniquement si une vraie route serveur existe un jour pour ce nom.
const PAGE_PATHS = {
  services: '/',
};

// Resout un nom logique "composant.cle" (+ params optionnels pour les
// selecteurs a discriminant, ex. {name: "mon-service"}) vers un selecteur
// CSS concret via selectors.json. Lance une erreur explicite si le
// composant/la cle n'existe pas, ou si un placeholder {xxx} du template
// n'a pas ete fourni dans params -- ce sont des erreurs d'auteur de
// scenario, pas des echecs de test a masquer.
export function resolveTarget(target, params = {}) {
  const [component, key] = String(target).split('.');
  const group = selectors[component];
  if (!group) {
    throw new Error(`selectors.json: composant inconnu "${component}" (cible "${target}")`);
  }
  const template = group[key];
  if (!template) {
    throw new Error(`selectors.json: cle inconnue "${key}" dans le composant "${component}" (cible "${target}")`);
  }
  return template.replace(/\{(\w+)\}/g, (match, name) => {
    if (!(name in params)) {
      throw new Error(`Parametre "${name}" manquant pour resoudre la cible "${target}" (template: ${template})`);
    }
    return String(params[name]);
  });
}

async function runStep(page, step) {
  const { action, target, params, value } = step;
  switch (action) {
    case 'goto': {
      const url = PAGE_PATHS[step.page];
      if (url === undefined) {
        throw new Error(`goto: page inconnue "${step.page}" (etendre PAGE_PATHS dans scenario-runner.js si c'est une vraie route)`);
      }
      await page.goto(url);
      await page.waitForLoadState('networkidle');
      return;
    }
    case 'click': {
      await page.locator(resolveTarget(target, params)).click();
      return;
    }
    case 'fill': {
      await page.locator(resolveTarget(target, params)).fill(value);
      return;
    }
    case 'selectOption': {
      await page.locator(resolveTarget(target, params)).selectOption(value);
      return;
    }
    case 'assertVisible': {
      await expect(page.locator(resolveTarget(target, params))).toBeVisible();
      return;
    }
    case 'assertHidden': {
      await expect(page.locator(resolveTarget(target, params))).toBeHidden();
      return;
    }
    case 'assertText': {
      await expect(page.locator(resolveTarget(target, params))).toContainText(value);
      return;
    }
    default:
      throw new Error(`Action non supportee "${action}". Actions disponibles : goto, click, fill, selectOption, assertVisible, assertHidden, assertText.`);
  }
}

// Execute un scenario JSON {scenario, steps[]} dans l'ordre. Chaque etape
// echouee est re-levee avec son index et son contenu pour un diagnostic
// direct (pas besoin de deviner quelle etape a echoue dans un long
// scenario).
export async function runScenario(page, scenario) {
  for (let i = 0; i < scenario.steps.length; i++) {
    const step = scenario.steps[i];
    try {
      await runStep(page, step);
    } catch (e) {
      throw new Error(`Scenario "${scenario.scenario}", etape ${i + 1}/${scenario.steps.length} (${JSON.stringify(step)}) : ${e.message}`);
    }
  }
}

export function loadScenario(filename) {
  const p = path.join(__dirname, 'scenarios', filename);
  return JSON.parse(fs.readFileSync(p, 'utf8'));
}
