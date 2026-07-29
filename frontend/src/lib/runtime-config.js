// Configuration chargee au demarrage du frontend, AVANT le montage de l'app
// (voir main.js) et avant tout appel a api.js. Permet de configurer l'URL de
// base de l'API independamment du Host sur lequel la SPA elle-meme est
// chargee — necessaire des que l'infrastructure route /api vers une origine
// distincte de celle qui sert les assets statiques (ex. Kubernetes/Gloo Edge
// avec un VirtualService pour le front et un RouteTable/Upstream separe pour
// le back). Voir CLAUDE.md pour le detail de la decision (config au niveau
// du conteneur plutot qu'au build, comme AuthConfig cote backend).
//
// Servi par le backend a /runtime-config.json (PAS sous /api : doit rester
// joignable meme quand /api est route separement par l'infrastructure — ce
// fichier est ce qui indique au frontend ou se trouve l'API, donc il doit
// arriver par le MEME chemin que index.html/le bundle JS).
//
// Valeur par defaut '' si absent/injoignable/invalide : preserve le
// comportement historique (URL relative /api/..., derivee du Host courant
// par le navigateur) — aucune rupture pour les deploiements co-localises
// actuels qui fonctionnent deja sans configuration.
let apiBaseUrl = '';

export async function loadRuntimeConfig() {
  try {
    const res = await fetch('/runtime-config.json');
    if (res.ok) {
      const data = await res.json();
      apiBaseUrl = (data.api_base_url || '').trim().replace(/\/+$/, '');
    }
  } catch {
    // Backend injoignable ou reponse invalide : on garde le defaut '' (URL
    // relative). Jamais bloquant pour le demarrage de l'app.
  }
}

export function getApiBaseUrl() {
  return apiBaseUrl;
}

// Reserve aux tests : reinitialise l'etat module entre deux cas, puisque
// `apiBaseUrl` vit au niveau du module (comme group-expansion-state.svelte.js
// et auth.svelte.js), pas dans un composant demonte/remonte entre les tests.
export function _resetForTests() {
  apiBaseUrl = '';
}
