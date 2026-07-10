import { describe, it, expect, beforeEach } from 'vitest';
import {
  isGroupExpanded,
  getExpandedGroupKeys,
  setGroupExpanded,
  toggleGroupExpanded,
  resetGroupExpansionState,
} from '../lib/group-expansion-state.svelte.js';

describe('group-expansion-state', () => {
  beforeEach(() => {
    resetGroupExpansionState();
  });

  it('demarre sans aucun groupe deplie', () => {
    expect(isGroupExpanded('team-a')).toBe(false);
    expect(getExpandedGroupKeys().size).toBe(0);
  });

  it('deplie un groupe explicitement', () => {
    setGroupExpanded('team-a', true);
    expect(isGroupExpanded('team-a')).toBe(true);
  });

  it('replie un groupe explicitement', () => {
    setGroupExpanded('team-a', true);
    setGroupExpanded('team-a', false);
    expect(isGroupExpanded('team-a')).toBe(false);
  });

  it('ne fuit pas entre plusieurs groupes distincts', () => {
    setGroupExpanded('team-a', true);
    setGroupExpanded('team-b', true);
    setGroupExpanded('team-a', false);

    expect(isGroupExpanded('team-a')).toBe(false);
    expect(isGroupExpanded('team-b')).toBe(true);
    expect(isGroupExpanded('team-c')).toBe(false);
    expect(getExpandedGroupKeys().size).toBe(1);
  });

  it('toggleGroupExpanded inverse l etat courant', () => {
    expect(isGroupExpanded('team-a')).toBe(false);
    toggleGroupExpanded('team-a');
    expect(isGroupExpanded('team-a')).toBe(true);
    toggleGroupExpanded('team-a');
    expect(isGroupExpanded('team-a')).toBe(false);
  });

  it('resetGroupExpansionState efface tous les groupes deplies', () => {
    setGroupExpanded('team-a', true);
    setGroupExpanded('team-b', true);
    resetGroupExpansionState();
    expect(getExpandedGroupKeys().size).toBe(0);
    expect(isGroupExpanded('team-a')).toBe(false);
    expect(isGroupExpanded('team-b')).toBe(false);
  });
});
