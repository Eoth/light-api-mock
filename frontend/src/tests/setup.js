import '@testing-library/jest-dom/vitest';
import { setLocale } from '../lib/i18n.svelte.js';

// The unit tests run in English, the language of the code: they assert the texts written where they are used, and
// rewording a translation breaks none of them. french.test.js shows the interface in French; l10n.test.js checks the
// catalogues as a whole.
await setLocale('en');
