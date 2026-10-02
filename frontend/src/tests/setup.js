import '@testing-library/jest-dom/vitest';
import { expect, vi } from 'vitest';
import { setLocale } from '../lib/i18n.svelte.js';

// The unit tests run in English, the language of the code: they assert the texts written where they are used, and
// rewording a translation breaks none of them. french.test.js shows the interface in French; l10n.test.js checks the
// catalogues as a whole.
await setLocale('en');

// Any other test file that loads the French catalogue fails on its first use: switching to French there would assert
// French texts again. The helper is imported here, not at the top, because vi.mock runs before the imports.
vi.mock('../locales/fr.json', async (importOriginal) => {
  const { mayLoadFrench, REFUSAL } = await import('./helpers/french-catalogue.js');
  if (mayLoadFrench(expect.getState().testPath)) return importOriginal();
  return {
    get default() {
      throw new Error(REFUSAL);
    },
  };
});
