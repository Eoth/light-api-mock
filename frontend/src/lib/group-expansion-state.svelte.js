// Which service groups are expanded in ServiceList. Kept in a module rather than in the component: a module is loaded
// once per page, so the state survives leaving the list (to edit a service) and coming back, where a $state of the
// component would start over at each mount.
//
// In memory only, on purpose: a reload of the page forgets it. Keeping it across reloads would take a localStorage
// read and write here, and no change to ServiceList.svelte.
//
// The set holds group keys, never copies of services: negligible even with dozens of groups.

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

// Forgets every expanded group; tests call it so that each case starts with none.
export function resetGroupExpansionState() {
  expandedGroups = new Set();
}
