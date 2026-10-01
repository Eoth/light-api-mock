# End-to-end tests

Playwright tests that drive a real Mimicway binary through a real browser. Two styles live side by side:

- **Scenarios** (`scenarios/*.scenarios.json`), replayed by `scenario-runner.js`: readable lists of UI steps that target elements by logical names, never by hard-coded selectors.
- **Classic specs** (`*.spec.js`, `*.spec.mjs`): for what scenarios do not express (API-only checks, dedicated server instances, complex assertions).

## Run

```bash
# From the repository root: a Mimicway on :7342 with an empty data directory
mkdir -p data
DATA_PATH=./data STATIC_DIR=./frontend/dist ./target/debug/mimicway &

cd frontend
npx playwright install chromium   # once
npm run test:e2e
```

The suite reads the interface in English, its source language (`playwright.config.js`); `i18n.spec.js` checks the French interface and the language switch. A few specs start their own Mimicway (authentication, API base URL); they need `cargo build` first. The Kafka specs are skipped unless the binary was built with `--features messaging-kafka`.

## Layout

```
e2e/
  selectors.json            the only place that maps logical names to [data-testid=...] selectors
  scenario-runner.js        minimal step interpreter (no Gherkin, no BDD dependency)
  scenario-runner.spec.js   loads and replays the scenarios below
  scenarios/
    home.scenarios.json       home screen, demo service
    groups.scenarios.json     group creation, collapsible groups
    rules.scenarios.json      rule creation, edition, deletion on a service
    services.scenarios.json   service creation, search, mock switch, same names in different groups
  *.spec.js / *.spec.mjs    classic specs
  docs-screenshot.js        writes docs/screenshots/*.png, only when DOCS_SCREENSHOTS is set
```

## Scenario files

One file per functional domain, holding several scenarios:

```json
{
  "domain": "services",
  "scenarios": [
    {
      "scenario": "Create a service with the form",
      "steps": [
        { "action": "goto", "page": "services" },
        { "action": "click", "target": "app.addServiceButton" },
        { "action": "fill", "target": "serviceForm.nameInput", "value": "my-service" },
        { "action": "click", "target": "serviceForm.submitButton" },
        { "action": "assertVisible", "target": "serviceDetail.editButton" }
      ]
    }
  ]
}
```

- `target` is `"component.key"`, as written in `selectors.json`.
- A repeated element (a card per service, for instance) has placeholders in its selector, filled by `params`: `{ "action": "click", "target": "serviceCard.configureButton", "params": { "name": "my-service" } }`.
- `value` is required by `fill`, `selectOption` and `assertText`.
- Actions: `goto`, `click`, `fill`, `selectOption`, `assertVisible`, `assertHidden`, `assertText`, `screenshot`. Add one only for a real need: the runner stays minimal.
- `goto` takes a `page`, not a URL: the UI has no router, so only `"services"` (the root) is mapped; other views are reached by clicking their navigation button (`app.navLogsButton`, `app.navGroupsButton`…).
- Scenarios only interact with the UI. Data setup (creating a service through the API, resetting the configuration) belongs to the spec that replays the scenario (`test.beforeEach`, `request.post(...)`), as in `scenario-runner.spec.js`.

Add a scenario to the domain file it belongs to, with a name unique in that file; a new domain is a deliberate choice, listed above. Replay it from a spec:

```js
import { test } from '@playwright/test';
import { runScenario, loadScenario } from './scenario-runner.js';

test('create a service', async ({ page }) => {
  await runScenario(page, loadScenario('services.scenarios.json', 'Create a service with the form'));
});
```

`loadScenario(file, name)` fails clearly when the file or the name does not exist; `loadDomain(file)` returns a whole domain. When a step fails, the error names the scenario, the step number and the step itself.

## Selectors

`selectors.json` groups logical names by component (`serviceForm`, `ruleList`, `groupManager`…), each mapped to `[data-testid="..."]`. To add one:

1. Make sure the component has the `data-testid` (convention `{component-kebab}-{element-role}[-{discriminant}]`); add it to the Svelte component first if needed. Never register a selector that matches nothing.
2. Add the entry under its component with a camelCase key (`nameInput`, `submitButton`).
3. For a repeated element, use a placeholder named like the `params` key that fills it: `"card": "[data-testid=\"service-card-{name}\"]"`.

No selector is written anywhere else. To compare `selectors.json` with the code:

```bash
grep -rhoE 'data-testid="[^"]*"' frontend/src --include="*.svelte" | sort -u
```

## Documentation screenshots

The images of `docs/` are taken by this suite, so they follow the interface instead of going stale.

- `docsScreenshot(page, file)` (`docs-screenshot.js`) does nothing unless `DOCS_SCREENSHOTS` is set: the standard run takes no screenshot and pays nothing.
- In a scenario, a `{ "action": "screenshot", "file": "name.png" }` step captures the page at that point; in a spec, `await docsScreenshot(page, 'name.png')` does the same.
- `playwright.docs-screenshots.config.js` sets `DOCS_SCREENSHOTS` and runs only the files that take screenshots.

```bash
npm run docs:screenshots   # from frontend/, with Mimicway running on :7342
```

Images are written to `docs/screenshots/` under the names the pages reference; `node scripts/check-doc-links.mjs` (run by CI) fails when a page references an image that does not exist. The three Kafka images need a binary built with `--features messaging-kafka`. States that no test reaches yet have no image; covering them is roadmap item R5.
