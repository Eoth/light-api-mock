// Source unique de verite pour la construction de l'URL de test d'un
// service (meme principe que tpl-utils.js pour le format template : cette
// logique ne doit jamais etre dupliquee/recopiee dans un composant). Deux
// points d'usage : ServiceCard.svelte (vue liste) et ServiceForm.svelte
// (ajout/edition) — les deux doivent afficher la meme URL pour un meme
// service, prefixee par le code du groupe (5 caracteres) quand il en a un.
export function buildServiceTestUrl({ name, listenPath = '', groupCode = '', baseUrl = '' }) {
  const n = (name || '').trim() || '...';
  const prefix = groupCode ? `/${groupCode}/${n}` : `/${n}`;
  const p = (listenPath || '').trim();
  const path = p ? (p.startsWith('/') ? p : '/' + p) : '/*';
  return `${baseUrl}${prefix}${path}`;
}
