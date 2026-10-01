// Screenshots of the user guide, taken by the end-to-end suite so that they follow the interface instead of going
// stale. The guide exists once per language, in docs/<language>/ with the same file names, so each call writes the
// same state once per language: the page is captured in English (the language the suite runs in), switched to French
// with the interface's own language selector, captured again, then switched back so that the test goes on in English.
// The switch does not reload the page, so both images show exactly the same state, and the production bundle needs no
// test hook.
//
// Disabled by default: it only captures when DOCS_SCREENSHOTS is set, which playwright.docs-screenshots.config.js
// does and playwright.config.js never does, so the standard run takes no screenshot and pays nothing.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DOCS_DIR = path.join(__dirname, '..', '..', 'docs');
const LANGUAGE_SELECT = '[data-testid="app-language-select"]';
const NOTIFICATION = '[data-testid="notification"]';
// The language of the suite first; the others are captured by switching to them.
const LANGUAGES = [
  { code: 'en', locale: 'en-US' },
  { code: 'fr', locale: 'fr-FR' },
];

export const DOCS_SCREENSHOTS_ENABLED = !!process.env.DOCS_SCREENSHOTS;

function target(language, filename) {
  const dir = path.join(DOCS_DIR, language, 'screenshots');
  fs.mkdirSync(dir, { recursive: true });
  return path.join(dir, filename);
}

// Transitions (a group's chevron, a button turning from primary to outline) are run to their end, and the pointer is
// moved off the content: otherwise an image may catch a transition half-way, or a button hovered in one language only
// (the texts, hence the layout, differ), and the two images of a pair would not show the same state.
async function capture(page, language, filename) {
  await page.mouse.move(0, 0);
  await page.screenshot({ path: target(language, filename), animations: 'disabled' });
}

// Whether an element shows in the captured viewport (an element scrolled out of it does not appear in the image).
async function inViewport(page, selector) {
  const element = page.locator(selector);
  if (!(await element.isVisible())) return false;
  const box = await element.boundingBox();
  const { height } = page.viewportSize();
  return box !== null && box.y + box.height > 0 && box.y < height;
}

async function interfaceLanguage(page) {
  return page.evaluate(() => document.documentElement.lang);
}

async function switchLanguage(page, code) {
  // The selector's own change handler, as a user's choice triggers it, but without selectOption(), which may scroll the
  // navigation bar into view or move the focus: the screen must keep its scroll position and its open popups.
  await page.locator(LANGUAGE_SELECT).evaluate((select, value) => {
    select.value = value;
    select.dispatchEvent(new Event('change', { bubbles: true }));
  }, code);
  // The catalogue is fetched on first use; `lang` changes once it is applied.
  await page.waitForFunction((expected) => document.documentElement.lang === expected, code);
  // Two frames: the re-rendered texts are laid out and painted before the capture.
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}

// A screen without the navigation bar (the login screen) has no language selector: it is opened again at the same URL
// in a browser set to the other language, which is how a visitor of that language first sees it. Only valid for a
// screen that opening its URL reproduces; `waitFor` is the selector that shows it is ready.
async function captureReopened(page, language, filename, waitFor) {
  const context = await page.context().browser().newContext({ locale: language.locale, viewport: page.viewportSize() });
  try {
    const other = await context.newPage();
    await other.goto(page.url());
    await other.waitForFunction((expected) => document.documentElement.lang === expected, language.code);
    await other.locator(waitFor).waitFor();
    await other.waitForLoadState('networkidle');
    await capture(other, language.code, filename);
  } finally {
    await context.close();
  }
}

/**
 * Writes docs/<language>/screenshots/<filename> for each language of the guide, when DOCS_SCREENSHOTS is set.
 *
 * Texts written before the switch keep their language, so they are refused or redone rather than captured as they are:
 * - a notification holds a sentence chosen when it appeared: a visible one is an error (wait until it closes);
 * - a text worded by the server follows the language of the request that fetched it: `options.afterSwitch(page)` runs
 *   after every switch (back to English included) to fetch it again, as the user would by repeating the action.
 * `options.reopenWaitingFor`: for a screen without the language selector, the selector to wait for once the screen is
 * opened again in the other languages (see captureReopened); without it, a missing selector is an error.
 */
export async function docsScreenshot(page, filename, options = {}) {
  if (!DOCS_SCREENSHOTS_ENABLED) return;
  const [source, ...others] = LANGUAGES;
  const shown = await interfaceLanguage(page);
  if (shown !== source.code) {
    throw new Error(`docsScreenshot(${filename}): the interface is in "${shown}", expected "${source.code}"`);
  }
  if (await inViewport(page, NOTIFICATION)) {
    throw new Error(`docsScreenshot(${filename}): a notification is in view and would keep its language; wait for it to close`);
  }
  await capture(page, source.code, filename);

  const hasSelector = (await page.locator(LANGUAGE_SELECT).count()) > 0;
  if (!hasSelector && !options.reopenWaitingFor) {
    throw new Error(`docsScreenshot(${filename}): no language selector on this screen; pass reopenWaitingFor`);
  }
  const switchTo = async (code) => {
    await switchLanguage(page, code);
    if (options.afterSwitch) {
      await options.afterSwitch(page);
      await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    }
  };
  for (const language of others) {
    if (hasSelector) {
      await switchTo(language.code);
      await capture(page, language.code, filename);
    } else {
      await captureReopened(page, language, filename, options.reopenWaitingFor);
    }
  }
  if (hasSelector) await switchTo(source.code);
}
