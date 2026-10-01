// The path parameter names ({name} or :name) of a URL pattern taken alone. The server's matcher
// (`normalize_colon_syntax`, `match_path` in src/engine/matcher.rs) learns them only by matching a real request path;
// this module reads the same segments from the pattern alone, so that the path parameter picker of
// ConditionForm.svelte is filled before any request exists.
//
// RuleForm.svelte combines the parameters of the service's listen_path and of the edited rule's sub_path.

/**
 * The path parameter names of a URL pattern (`{name}` or `:name`), in order of first appearance, without duplicates.
 * The `*` wildcard is not one.
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
 * The path parameter names of several patterns (e.g. the service's listen_path and the rule's sub_path), in order of
 * first appearance, without duplicates.
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
