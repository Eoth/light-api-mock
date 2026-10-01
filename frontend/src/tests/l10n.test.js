// Holds the translations to the rule that keeps them cheap: every sentence exists once, in English, where it is
// used; a language adds a catalogue (src/locales/<locale>.json) and nothing else.
//
// What it checks:
//   * every message given to t / tCount is a double-quoted string literal (or such literals joined by +), so it
//     can be extracted: a template string or a variable would be a sentence no catalogue can know;
//   * each catalogue translates exactly the existing messages (nothing missing, nothing left over from a sentence
//     that was reworded) and keeps their {0}, {1}… placeholders;
//   * no visible word of the interface escapes t: the components are rendered in a pseudo-locale where every
//     translated text is marked, and any other word found in the page fails the test.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { render, fireEvent, waitFor, cleanup } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeAll, afterAll, afterEach } from 'vitest';
import { format, useTranslator, setLocale } from '../lib/i18n.svelte.js';
import * as api from '../lib/api.js';

const SRC = join(__dirname, '..');

// ---------------------------------------------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------------------------------------------

function sourceFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === 'tests' || name === 'locales' ? [] : sourceFiles(path);
    return /\.(svelte|js)$/.test(name) && name !== 'i18n.svelte.js' ? [path] : [];
  });
}

/** Splits the arguments of the call whose opening parenthesis is at `open`. */
function argumentsOf(source, open) {
  const args = [];
  let depth = 0;
  let start = open + 1;
  for (let i = open + 1; i < source.length; i += 1) {
    const c = source[i];
    if (c === '"' || c === "'" || c === '`') {
      let j = i + 1;
      while (j < source.length && source[j] !== c) j += source[j] === '\\' ? 2 : 1;
      i = j;
    } else if ('([{'.includes(c)) {
      depth += 1;
    } else if (')]}'.includes(c)) {
      if (depth === 0) {
        args.push(source.slice(start, i));
        return args;
      }
      depth -= 1;
    } else if (c === ',' && depth === 0) {
      args.push(source.slice(start, i));
      start = i + 1;
    }
  }
  return args;
}

/** The message of a literal argument (double-quoted literals joined by +), or undefined for anything else. */
function messageOf(argument) {
  const text = argument.trim();
  let value = '';
  let i = 0;
  while (i < text.length) {
    if (text[i] !== '"') return undefined;
    let j = i + 1;
    while (j < text.length && text[j] !== '"') j += text[j] === '\\' ? 2 : 1;
    value += JSON.parse(text.slice(i, j + 1));
    const rest = text.slice(j + 1).match(/^\s*(\+\s*)?/);
    i = j + 1 + rest[0].length;
    if (rest[1] === undefined && i < text.length) return undefined;
  }
  return value;
}

const MESSAGE_ARGUMENTS = { t: [0], tCount: [1, 2] };
const messages = new Map();
const unextractable = [];
for (const file of sourceFiles(SRC)) {
  const text = readFileSync(file, 'utf8');
  const call = /(^|[^.\w$])(tCount|t)\(/g;
  let match;
  while ((match = call.exec(text)) !== null) {
    const open = match.index + match[0].length - 1;
    const args = argumentsOf(text, open);
    const line = text.slice(0, match.index).split('\n').length;
    for (const index of MESSAGE_ARGUMENTS[match[2]]) {
      const message = args[index] === undefined ? undefined : messageOf(args[index]);
      if (message === undefined) {
        unextractable.push(`${relative(SRC, file)}:${line} ${match[2]}(${(args[index] ?? '').trim().slice(0, 50)})`);
      } else if (!messages.has(message)) {
        messages.set(message, `${relative(SRC, file)}:${line}`);
      }
    }
  }
}

const placeholders = (text) => (text.match(/\{\d+\}/g) ?? []).sort().join(' ');

const catalogues = Object.fromEntries(
  readdirSync(join(SRC, 'locales'))
    .filter((name) => name.endsWith('.json'))
    .map((name) => [name, JSON.parse(readFileSync(join(SRC, 'locales', name), 'utf8'))])
);

describe('translations', () => {
  it('extracts every message as a literal', () => {
    expect(unextractable).toEqual([]);
    expect(messages.size).toBeGreaterThan(500);
  });

  for (const [name, catalogue] of Object.entries(catalogues)) {
    it(`${name} translates exactly the messages of the code`, () => {
      const missing = [...messages.keys()].filter((m) => !(m in catalogue));
      const extra = Object.keys(catalogue).filter((m) => !messages.has(m));
      expect(missing).toEqual([]);
      expect(extra).toEqual([]);
    });

    it(`${name} keeps the placeholders of every message`, () => {
      const broken = Object.entries(catalogue)
        .filter(([source, target]) => !target.trim() || placeholders(source) !== placeholders(target))
        .map(([source, target]) => `${source} => ${target}`);
      expect(broken).toEqual([]);
    });
  }
});

// ---------------------------------------------------------------------------------------------------------------
// Pseudo-locale rendering
// ---------------------------------------------------------------------------------------------------------------

vi.mock('../lib/api.js', () => ({
  getAuthStatus: vi.fn(),
  validateToken: vi.fn(),
  login: vi.fn(),
  getMessagingStatus: vi.fn(),
  getMessagingLogs: vi.fn(),
  simulateMessage: vi.fn(),
  getTcpStatus: vi.fn(),
  getTcpServices: vi.fn(),
  createTcpService: vi.fn(),
  updateTcpService: vi.fn(),
  deleteTcpService: vi.fn(),
  getServices: vi.fn(),
  getConfig: vi.fn(),
  putConfig: vi.fn(),
  toggleService: vi.fn(),
  createService: vi.fn(),
  updateService: vi.fn(),
  deleteService: vi.fn(),
  reorderRules: vi.fn(),
  pingService: vi.fn(),
  resetConfig: vi.fn(),
  getGroups: vi.fn(),
  createGroup: vi.fn(),
  deleteGroup: vi.fn(),
  updateGroupMembers: vi.fn(),
  getBackups: vi.fn(),
  restoreBackup: vi.fn(),
  getLogs: vi.fn(),
  validateScript: vi.fn(),
  testRule: vi.fn(),
  checkRuleConflicts: vi.fn(),
  observeService: vi.fn(),
  unobserveService: vi.fn(),
  getObservationStatus: vi.fn(),
  getServiceSuggestions: vi.fn(),
}));

// Words that are the same in every language: product name, protocol and data-format identifiers, and examples of
// code. Anything else visible must come from t.
const UNTRANSLATED = new Set(['lightMock', 'GET', 'POST', 'mock', 'proxy', 'no-rule', 'Content-Type', 'application/json',
  'orders.in', '/user/role', 'Envelope/Body/id', 'English', 'Français']);

const marked = (text) => `⟦${text}⟧`;

// Code, data marked translate="no" (names, URLs, template expressions) and form values are not interface text.
const NOT_INTERFACE_TEXT = 'code, pre, textarea, datalist, script, style, kbd, [translate="no"]';

/** `text` without its marked (translated) parts; markers nest when a translation is a value of another one. */
function unmarked(text) {
  let rest = text;
  for (let previous = ''; previous !== rest; ) {
    previous = rest;
    rest = rest.replace(/⟦[^⟦⟧]*⟧/g, ' ');
  }
  return rest.trim();
}

function untranslatedWords(container) {
  const found = [];
  const check = (text, where) => {
    const rest = unmarked(text);
    if (!rest || UNTRANSLATED.has(rest)) return;
    for (const token of rest.split(/\s+/)) {
      if (/[A-Za-zÀ-ÿ]{2,}/.test(token) && !UNTRANSLATED.has(token)) found.push(`${where}: "${rest}"`);
    }
  };
  // The text of each element is checked as a whole, its code children standing for placeholders: a translated
  // sentence may be split around them (see Sentence.svelte).
  for (const element of [container, ...container.querySelectorAll('*')]) {
    if (element.closest(NOT_INTERFACE_TEXT)) continue;
    // An option showing its own value is an identifier (an HTTP method, a fake-data kind).
    if (element.tagName === 'OPTION' && element.textContent.trim() === element.value) continue;
    const SEPARATOR = '\u0000';
    const own = [...element.childNodes]
      .map((n) => {
        if (n.nodeType === Node.TEXT_NODE) return n.textContent;
        if (n.nodeType !== Node.ELEMENT_NODE) return ''; // the comments Svelte uses as anchors
        return n.matches(NOT_INTERFACE_TEXT) ? ' ' : SEPARATOR;
      })
      .join('');
    for (const piece of own.split(SEPARATOR)) check(piece, `<${element.tagName.toLowerCase()}>`);
  }
  for (const element of container.querySelectorAll('[title], [placeholder], [aria-label], [alt]')) {
    if (element.closest('[translate="no"]')) continue;
    for (const attribute of ['title', 'placeholder', 'aria-label', 'alt']) {
      const value = element.getAttribute(attribute);
      if (value) check(value, `${attribute} of <${element.tagName.toLowerCase()}>`);
    }
  }
  return [...new Set(found)];
}

const group = { name: 'g1', code: 'c0d3e', admins: ['u1'], members: ['u2'] };
const proxied = { name: 'a1', listen_path: '/v1/{n}', real_target_url: 'http://10.0.0.1:8080', is_mocked: false, rewrite_directory_urls: false, group_name: 'g1', wsdl_mode: 'auto', rules: [] };
const fullRule = {
  name: 'r1', method: 'POST', sub_path: '/x1', action: 'mock', pre_script: '1', script: '2', post_script: '3', response_mode: 'advanced',
  conditions: { all_of: [{ source: { type: 'QueryParam', key: 'q' }, operator: { type: 'Eq', value: '1' } }], any_of: [{ source: { type: 'BodyRaw' }, operator: { type: 'Exists' } }] },
  response: {
    status: 200, headers: [{ name: 'Content-Type', value: 'application/json' }],
    body: [
      { type: 'Template', template: '{}' }, { type: 'Literal', value: '1' }, { type: 'Uuid' },
      { type: 'PickFrom', values: ['1', '2'] }, { type: 'FakeData', kind: { type: 'FirstName' } }, { type: 'PathSegment', index: 1 },
    ],
    chaos: { delay_ms: 1, delay_min_ms: null, delay_max_ms: null, error_rate: 0.1, error_status: 500 },
  },
};
const mocked = { ...proxied, name: 'b2', is_mocked: true, group_name: null, rules: [fullRule] };
const captured = { remaining_path: '/x1', path_params: {}, query_params: { q: '1' }, headers: {}, body: '{}', body_truncated: true, content_type: null };
const logs = [
  { timestamp: 1, service_name: 'b2', group_name: null, method: 'POST', path: '/b2/x1', mode: 'mock', rule_matched: 'r1', target_url: null, status: 200, captured },
  { timestamp: 2, service_name: 'a1', group_name: 'g1', method: 'GET', path: '/c0d3e/a1/x2', mode: 'proxy', rule_matched: null, target_url: 'http://10.0.0.1:8080/x2', status: 502, captured: null },
];

async function expectFullyTranslated(component, props = {}, interact = async () => {}) {
  const { container } = render(component, { props });
  await interact(container);
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(untranslatedWords(container)).toEqual([]);
}

const click = (container, testId) => fireEvent.click(container.querySelector(`[data-testid="${testId}"]`));

describe('pseudo-locale: no visible word escapes t', () => {
  beforeAll(() => {
    window.matchMedia = window.matchMedia || vi.fn().mockReturnValue({ matches: false });
    useTranslator((message, args) => marked(format(message, args)));
    api.getAuthStatus.mockResolvedValue({ enabled: false, show_reset_button: true });
    api.getMessagingStatus.mockResolvedValue({ available: true });
    api.getTcpStatus.mockResolvedValue([{ name: 't1', listen_port: 9000, listening: true, error: null }]);
    api.getTcpServices.mockResolvedValue([{ name: 't1', listen_port: 9000, rules: [{ name: 'r1', matcher: { type: 'Any' }, response_hex: '' }] }]);
    api.getServices.mockResolvedValue([proxied, mocked]);
    api.getGroups.mockResolvedValue([group]);
    api.getBackups.mockResolvedValue([{ filename: 'f1.yaml', created_at_ms: 1, size_bytes: 2048, protected: true }]);
    api.getLogs.mockResolvedValue(logs);
    api.getMessagingLogs.mockResolvedValue([{ timestamp: 1, direction: 'out', topic: 't1', service_name: 'b2', rule_matched: 'r1', matched: true, body_preview: '{}', body_truncated: true, body_size_bytes: 9 }]);
    api.getObservationStatus.mockResolvedValue([{ group_name: 'g1', service_name: 'a1' }]);
    api.getServiceSuggestions.mockResolvedValue([
      { outcome: 'Conditional', rules: [{ method: 'GET', sub_path: '/x2', condition: { source: { type: 'QueryParam', key: 'q' }, operator: { type: 'Eq', value: '1' } }, response: { status: 200, headers: [], body: [{ type: 'Literal', value: '1' }] }, sample_count: 3 }] },
      { outcome: 'Unconditional', rule: { method: 'GET', sub_path: '/x3', condition: null, response: { status: 404, headers: [], body: [] }, sample_count: 3 } },
      { outcome: 'VarianceUnexplained', sample_count: 4, response_class_count: 2 },
    ]);
    api.testRule.mockResolvedValue({
      overall_matched: false, method_matches: true, sub_path_matches: false, body_truncated: true,
      all_of: [{ condition: fullRule.conditions.all_of[0], matched: false, found_value: null, hint: null }],
      any_of: [{ condition: fullRule.conditions.any_of[0], matched: true, found_value: '1', hint: null }],
      script_errors: [{ slot: 'pre_script', message: '1' }], script_results: [{ slot: 'script', value: '1', fields: { k1: '2' } }],
    });
  });

  afterEach(() => cleanup());

  afterAll(() => {
    useTranslator(null);
    setLocale('fr');
  });

  it('the application shell, its list and its dialogs', async () => {
    const App = (await import('../App.svelte')).default;
    const { container } = render(App);
    await waitFor(() => expect(container.querySelector('[data-testid="app-add-service-button"]')).not.toBeNull());
    await fireEvent.click(container.querySelector('[data-testid="service-group-header-g1"]'));
    await fireEvent.click(container.querySelector('[data-testid="service-group-header-ungrouped"]'));
    expect(untranslatedWords(container)).toEqual([]);
  });

  it('the service screens', async () => {
    const ServiceForm = (await import('../lib/components/ServiceForm.svelte')).default;
    const ServiceDetail = (await import('../lib/components/ServiceDetail.svelte')).default;
    await expectFullyTranslated(ServiceForm, { availableGroups: [group] });
    await expectFullyTranslated(ServiceDetail, { service: proxied, availableGroups: [group] }, async (c) => {
      await waitFor(() => expect(c.querySelector('[data-testid="observation-refresh-suggestions-button-a1"]')).not.toBeNull());
      await click(c, 'observation-refresh-suggestions-button-a1');
      await waitFor(() => expect(c.querySelector('[data-testid="observation-suggestion-a1-0-0"]')).not.toBeNull());
    });
    await expectFullyTranslated(ServiceDetail, { service: mocked });
  });

  it('the rule editor with every block open', async () => {
    const RuleForm = (await import('../lib/components/RuleForm.svelte')).default;
    await expectFullyTranslated(RuleForm, { rule: fullRule, serviceName: 'b2', listenPath: '/v1/{n}', existingRules: [] }, async (c) => {
      await click(c, 'rule-form-advanced-options-toggle-button');
      await waitFor(() => expect(c.querySelector('[data-testid="rule-tester-log-select"]')).not.toBeNull());
      const select = c.querySelector('[data-testid="rule-tester-log-select"]');
      select.value = '0';
      await fireEvent.change(select);
      await click(c, 'rule-tester-test-button');
      await waitFor(() => expect(c.querySelector('[data-testid="rule-tester-result"]')).not.toBeNull());
      await click(c, 'rule-form-add-condition-allof-button');
    });
  });

  it('the response builders', async () => {
    const JsonResponseBuilder = (await import('../lib/components/JsonResponseBuilder.svelte')).default;
    const JsonPasteBuilder = (await import('../lib/components/JsonPasteBuilder.svelte')).default;
    const XmlResponseBuilder = (await import('../lib/components/XmlResponseBuilder.svelte')).default;
    const XmlPasteBuilder = (await import('../lib/components/XmlPasteBuilder.svelte')).default;
    const jsonFields = [
      { key: 'k1', fieldType: 'value', source: 'fake', value: 'FirstName', pipe: '', asNumber: false },
      { key: 'k2', fieldType: 'object', children: [{ key: 'k3', fieldType: 'value', source: 'path', value: 'n', pipe: '', asNumber: false }] },
      { key: 'k4', fieldType: 'array-values', items: [{ source: 'fixed', value: '1', asNumber: true }] },
      { key: 'k5', fieldType: 'array-objects', template: [] },
    ];
    await expectFullyTranslated(JsonResponseBuilder, { fields: jsonFields });
    await expectFullyTranslated(JsonPasteBuilder, { fields: jsonFields.slice(0, 2), startParsed: true });
    const xmlFields = [
      { tag: 'k1', nodeType: 'value', source: 'query', value: 'q', pipe: '', attributes: [{ name: 'k6', source: 'fixed', value: '1' }] },
      { tag: 'k2', nodeType: 'parent', children: [{ tag: 'k3', nodeType: 'value', source: 'fake', value: 'FirstName', pipe: '' }] },
    ];
    await expectFullyTranslated(XmlResponseBuilder, { fields: xmlFields, rootTag: 'k0' });
    await expectFullyTranslated(XmlPasteBuilder, { fields: xmlFields, rootTag: 'k0', rootAttributes: [], startParsed: true });
  });

  it('the logs, groups, backups, TCP and sign-in screens', async () => {
    const RequestLog = (await import('../lib/components/RequestLog.svelte')).default;
    const MessagingLog = (await import('../lib/components/MessagingLog.svelte')).default;
    const GroupManager = (await import('../lib/components/GroupManager.svelte')).default;
    const BackupManager = (await import('../lib/components/BackupManager.svelte')).default;
    const TcpServiceManager = (await import('../lib/components/TcpServiceManager.svelte')).default;
    const LoginForm = (await import('../lib/components/LoginForm.svelte')).default;
    await expectFullyTranslated(RequestLog, {}, async (c) => {
      await waitFor(() => expect(c.querySelector('[data-testid="request-log-detail-button-0"]')).not.toBeNull());
      await click(c, 'request-log-detail-button-0');
    });
    await expectFullyTranslated(MessagingLog, {}, async (c) => {
      await waitFor(() => expect(c.querySelector('[data-testid="messaging-log-detail-button-0"]')).not.toBeNull());
      await click(c, 'messaging-log-detail-button-0');
    });
    await expectFullyTranslated(GroupManager, { services: [proxied, mocked], authEnabled: true }, async (c) => {
      await waitFor(() => expect(c.querySelector('[data-testid="group-manager-manage-button-g1"]')).not.toBeNull());
      await click(c, 'group-manager-manage-button-g1');
      await click(c, 'group-manager-new-group-button');
    });
    await expectFullyTranslated(BackupManager, {}, async (c) => {
      await waitFor(() => expect(c.querySelector('[data-testid="backup-manager-item-f1.yaml"]')).not.toBeNull());
    });
    await expectFullyTranslated(TcpServiceManager, {}, async (c) => {
      await waitFor(() => expect(c.querySelector('[data-testid="tcp-manager-item-t1"]')).not.toBeNull());
    });
    await expectFullyTranslated(TcpServiceManager, {}, async (c) => {
      await waitFor(() => expect(c.querySelector('[data-testid="tcp-manager-add-button"]')).not.toBeNull());
      await click(c, 'tcp-manager-add-button');
    });
    await expectFullyTranslated(LoginForm);
  });
});
