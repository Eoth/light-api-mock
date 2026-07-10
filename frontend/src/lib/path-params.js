// Source unique de verite pour extraire les noms de path params ({name} ou
// :name) d'un pattern d'URL PRIS ISOLEMENT (pas de requete reelle en face) —
// meme principe que tpl-utils.js/service-url.js. Miroir cote frontend de
// `normalize_colon_syntax`/`match_path` (src/engine/matcher.rs) : ces
// fonctions Rust n'extraient les noms qu'en comparant a un VRAI chemin de
// requete ; ce module fait la meme analyse de segments mais sur le pattern
// seul, pour peupler le selecteur strict de path param (ConditionForm.svelte)
// avant meme qu'une requete existe.
//
// Utilise pour combiner les params du listen_path du service ET du sub_path
// de la regle en cours d'edition (RuleForm.svelte).

/**
 * Extrait, dans l'ordre de premiere apparition et sans doublon, les noms de
 * path params d'un pattern d'URL (`{name}` ou `:name`). Ignore le wildcard `*`.
 * @param {string} pattern
 * @returns {string[]}
 */
export function extractPathParamNames(pattern) {
  if (!pattern) return [];
  const names = [];
  const seen = new Set();
  const segments = pattern.split('/').filter((s) => s.length > 0);
  for (const seg of segments) {
    let name = null;
    if (seg.startsWith('{') && seg.endsWith('}') && seg.length > 2) {
      name = seg.slice(1, -1);
    } else if (seg.startsWith(':') && seg.length > 1) {
      name = seg.slice(1);
    }
    if (name && !seen.has(name)) {
      seen.add(name);
      names.push(name);
    }
  }
  return names;
}

/**
 * Combine les path params de plusieurs patterns (ex: listen_path du service +
 * sub_path de la regle), dedupliques, ordre de premiere apparition.
 * @param {string[]} patterns
 * @returns {string[]}
 */
export function combinePathParamNames(patterns) {
  const names = [];
  const seen = new Set();
  for (const pattern of patterns) {
    for (const name of extractPathParamNames(pattern)) {
      if (!seen.has(name)) {
        seen.add(name);
        names.push(name);
      }
    }
  }
  return names;
}
