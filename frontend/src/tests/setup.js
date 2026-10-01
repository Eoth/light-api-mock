import '@testing-library/jest-dom/vitest';
import { setLocale } from '../lib/i18n.svelte.js';

// The component tests were written against the French interface: they run in French, which also checks the French
// catalogue line by line. English and the catalogues as a whole are covered by l10n.test.js.
await setLocale('fr');
