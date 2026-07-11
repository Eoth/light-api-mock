// Etat d'affichage UI (groupes de services deplies/replies dans ServiceList).
// Volontairement en dehors du cycle de vie de ServiceList.svelte : ce module
// n'est charge/initialise qu'une seule fois par le navigateur, donc l'etat
// qu'il porte survit au demontage/remontage du composant (navigation vers
// l'edition d'un service puis retour a la liste), contrairement a un $state
// local au composant qui serait recree a chaque montage.
//
// Niveau 1 assume : cet etat est un Set en memoire uniquement,
// jamais ecrit dans localStorage/sessionStorage. Un rechargement complet de
// la page (F5) recharge ce module a zero et reinitialise donc l'etat -- c'est
// le comportement voulu, pas une limitation a corriger. Un futur "niveau 2"
// (persistance across F5) etendrait ce fichier avec une lecture/ecriture
// localStorage, sans toucher a ServiceList.svelte.
//
// Cout ressources : le contenu est une poignee de cles de groupe (des
// chaines), jamais une copie des services eux-memes -- negligeable en
// memoire/CPU meme avec des dizaines de groupes.

let expandedGroups = $state(new Set());

export function isGroupExpanded(key) {
  return expandedGroups.has(key);
}

export function getExpandedGroupKeys() {
  return expandedGroups;
}

export function setGroupExpanded(key, value) {
  if (expandedGroups.has(key) === value) return;
  const next = new Set(expandedGroups);
  if (value) {
    next.add(key);
  } else {
    next.delete(key);
  }
  expandedGroups = next;
}

export function toggleGroupExpanded(key) {
  setGroupExpanded(key, !expandedGroups.has(key));
}

// Reinitialise l'etat. Utilise par les tests (isolation entre cas) ; peut
// aussi servir a un futur flux de deconnexion s'il faut purger l'UI.
export function resetGroupExpansionState() {
  expandedGroups = new Set();
}
