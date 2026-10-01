import { describe, it, expect } from 'vitest';
import { removeFolds, swapFolds } from '../lib/fold-paths.js';

describe('swapFolds', () => {
  it('swaps the folds of two fields and of what they hold', () => {
    const folded = new Set(['0', '0-children-1', '1-template-0', '2']);
    expect(swapFolds(folded, [], 0, 1)).toEqual(new Set(['1', '1-children-1', '0-template-0', '2']));
  });

  it('works inside a nested array, and leaves "10" alone when "1" moves', () => {
    const folded = new Set(['3-children-1', '3-children-10', '1']);
    expect(swapFolds(folded, [3, 'children'], 1, 2)).toEqual(new Set(['3-children-2', '3-children-10', '1']));
  });
});

describe('removeFolds', () => {
  it('drops the folds of the removed field and moves the later ones up', () => {
    const folded = new Set(['0', '1', '1-children-0', '2', '2-children-3']);
    expect(removeFolds(folded, [], 1, 3)).toEqual(new Set(['0', '1', '1-children-3']));
  });

  it('keeps the folds of other arrays as they are', () => {
    const folded = new Set(['0-children-2', '1-children-2']);
    expect(removeFolds(folded, [0, 'children'], 0, 3)).toEqual(new Set(['0-children-1', '1-children-2']));
  });
});
