# Mimicway UI

A Svelte 5 single-page application (no SvelteKit), built to static files that the Mimicway binary serves. Its one runtime dependency is Svelte, whose runtime the compiler puts in the bundle; everything else in `package.json` is a build or test tool, and the release SBOM of the UI lists exactly what ships.

## Develop

```bash
npm ci
npm run dev           # http://localhost:5173, proxies /api and /runtime-config.json to a Mimicway on :7342
```

## Build and test

```bash
npm run build         # dist/, embedded by the next cargo build (or served with STATIC_DIR)
npm test              # Vitest unit tests, including the translation checks
npm run test:e2e      # Playwright, against a Mimicway running on :7342 (see e2e/README.md)
npm run docs:screenshots   # regenerates the images of docs/en/ and docs/fr/ from the end-to-end suite
```

## Layout

| Path | Role |
|---|---|
| `src/App.svelte` | Layout, navigation, import and export, reset, theme and language switches, log view |
| `src/lib/components/` | One component per screen or block: service list, card, form and detail; rule list, form and its sections (conditions, response, scripts, warnings); JSON and XML response builders, by example and in detail; rule tester; request and Kafka logs; backups; groups; traffic observation; raw TCP mocks; shared pieces (`FormField`, `ConfirmDialog`, `ToggleSwitch`, `Notification`, `Sentence`…) |
| `src/lib/api.js` | Management API client; sends the UI language as `Accept-Language` |
| `src/lib/runtime-config.js` | Reads `/runtime-config.json` at startup (`API_BASE_URL`) |
| `src/lib/i18n.svelte.js` | `t()` and `tCount()`, language detection and switch, lazy-loaded catalogues |
| `src/lib/tpl-utils.js` | The only place that converts between response templates and the builders' fields (JSON and XML), validates them and renders previews |
| `src/lib/rhai-functions.js` | Documentation and autocompletion of the script functions |
| `src/lib/service-url.js`, `path-params.js`, `format-date.js`, `hex-utils.js` | Small shared helpers |
| `src/locales/` | Translation catalogues, one file per language, keyed by the English text |
| `src/tests/` | Vitest tests, one file per component or module |
| `e2e/` | Playwright tests and scenarios |

## Response editor modes

The rule form offers five **formats**: JSON, XML, Text, Advanced template, Empty (204). JSON and XML each have two internal modes (`responseMode`) that are never shown as two buttons: a format starts *by example* (`json-paste`, `xml-paste`) and "Edit in detail" reveals the full editor (`json-guided`, `xml-guided`) on the same fields, without conversion.

| `responseMode` | What it edits |
|---|---|
| `json-paste` | A pasted JSON sample: source and pipe of each detected field |
| `json-guided` | The full JSON structure: keys, types, order, sources and pipes; produces a template |
| `xml-paste` | A pasted XML sample: source, pipe and attributes of each detected node |
| `xml-guided` | The full XML structure; produces a template |
| `text` | Free text; produces a literal body |
| `advanced` | The raw template (`{{path.siret \| first(9)}}`) |
| `empty` | No body (204) |

`Rule.response_mode` stores the mode with the rule, so the same view opens again (`RuleResponseSection.svelte`, `computeInitialEditorState()`).

| From → to | Supported | Notes |
|---|---|---|
| `json-paste` ↔ `json-guided` | Yes, lossless | Same fields, `revealDetailMode()` |
| `json-guided` → advanced | Yes, lossless | `fieldsToTemplate()` |
| advanced → JSON | When the template is a JSON object | `templateToFields()` |
| JSON → XML | Partly | Arrays of scalar values have no XML form |
| advanced ↔ text | Yes | Template fragment or literal |
| XML → JSON | No | Go through the advanced template |

## Translations

Every visible sentence goes through `t("English text")`, written once where it is used; `src/locales/fr.json` maps it to French. `src/tests/l10n.test.js` extracts every message, fails on a missing or unused catalogue entry or a lost placeholder, and renders the components in a pseudo-locale to catch any text that escapes translation. Data (names, URLs, values typed by users) carries `translate="no"`.

## Accessibility

Built to WCAG 2.1 AA (RGAA): skip link, visible labels, `aria-describedby` on hints and errors, `role="switch"` and `role="radio"` where they apply, `role="alert"` notifications, visible focus, 4.5:1 contrast in both themes, keyboard alternatives to drag and drop.
