// setup.js refuses the French catalogue to every test file but french.test.js and l10n.test.js. This file is one of
// the others: switching to French must fail here, and no other test file may reach the catalogues on disk instead.
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, it, expect } from 'vitest';
import { getLocale, setLocale } from '../lib/i18n.svelte.js';
import { MAY_LOAD_FRENCH, REFUSAL } from './helpers/french-catalogue.js';

describe('the French catalogue in the other test files', () => {
  it('cannot be loaded: switching to French fails and the interface stays in English', async () => {
    await expect(setLocale('fr')).rejects.toThrow(REFUSAL);
    expect(getLocale()).toBe('en');
  });

  it('is not read from the catalogues folder either', () => {
    // setup.js names the catalogue to refuse it, and this file to check the others.
    const allowed = new Set([...MAY_LOAD_FRENCH, 'setup.js', 'french-catalogue-guard.test.js']);
    const readers = readdirSync(__dirname, { recursive: true })
      .map((name) => name.replace(/\\/g, '/'))
      .filter((name) => /\.(js|svelte)$/.test(name) && !allowed.has(name))
      .filter((name) => /\blocales\b/.test(readFileSync(join(__dirname, name), 'utf8')));
    expect(readers).toEqual([]);
  });
});
