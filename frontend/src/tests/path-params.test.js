import { describe, it, expect } from 'vitest';
import { extractPathParamNames, combinePathParamNames } from '../lib/path-params.js';

describe('extractPathParamNames', () => {
  it('extrait les noms entre accolades', () => {
    expect(extractPathParamNames('/orders/{id}/items/{itemId}')).toEqual(['id', 'itemId']);
  });

  it('extrait les noms en syntaxe deux-points', () => {
    expect(extractPathParamNames('/orders/:id/items/:itemId')).toEqual(['id', 'itemId']);
  });

  it('ignore le wildcard *', () => {
    expect(extractPathParamNames('/orders/*')).toEqual([]);
  });

  it('deduplique les noms repetes', () => {
    expect(extractPathParamNames('/a/{id}/b/{id}')).toEqual(['id']);
  });

  it('retourne un tableau vide pour un pattern statique', () => {
    expect(extractPathParamNames('/orders/list')).toEqual([]);
  });

  it('retourne un tableau vide pour une valeur vide ou nulle', () => {
    expect(extractPathParamNames('')).toEqual([]);
    expect(extractPathParamNames(null)).toEqual([]);
    expect(extractPathParamNames(undefined)).toEqual([]);
  });
});

describe('combinePathParamNames', () => {
  it('combine plusieurs patterns sans doublon, ordre de premiere apparition', () => {
    expect(combinePathParamNames(['/orders/{id}', '/{id}/sub/{subId}'])).toEqual(['id', 'subId']);
  });

  it('gere des patterns vides melanges a des patterns valides', () => {
    expect(combinePathParamNames(['', '/users/{userId}'])).toEqual(['userId']);
  });

  it('retourne un tableau vide si aucun pattern ne contient de param', () => {
    expect(combinePathParamNames(['/a/b', '/c/d'])).toEqual([]);
  });
});
