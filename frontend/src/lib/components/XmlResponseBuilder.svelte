<script>
  import { buildExpr as sharedBuildExpr, templateToPreview, xmlFieldsToTemplate } from '../tpl-utils.js';
  import { t, tCount } from '../i18n.svelte.js';

  let { fields = [], rootTag = 'response', onUpdate = () => {} } = $props();

  const nodeTypes = [
    { value: 'value', get label() { return t("Content"); } },
    { value: 'parent', get label() { return t("Parent node"); } },
  ];

  const valueSources = [
    { value: 'fixed', get label() { return t("Fixed value"); } },
    { value: 'path', get label() { return t("URL parameter"); } },
    { value: 'query', get label() { return t("Query param"); } },
    { value: 'header', get label() { return t("HTTP header"); } },
    { value: 'body', get label() { return t("Echo of the body (JSON pointer)"); } },
    { value: 'xpath', get label() { return t("XPath (XML/SOAP)"); } },
    { value: 'fake', get label() { return t("Fake data"); } },
    { value: 'uuid', get label() { return t("UUID"); } },
    { value: 'now_ms', get label() { return t("Timestamp (ms)"); } },
    { value: 'now_iso', get label() { return t("ISO date"); } },
    { value: 'seq', get label() { return t("Counter"); } },
    { value: 'script', get label() { return t("Script result"); } },
  ];

  const fakeOptions = [
    'FirstName', 'LastName', 'Email', 'PhoneNumberFR', 'CompanyName',
    'StreetName', 'CityFR', 'PostcodeFR', 'Siren', 'Siret',
    'FullAddressFR', 'DatePast', 'DateFuture', 'TimestampMs',
    'BoolRandom', 'LoremSentence', 'CountryFR', 'IbanFR',
  ];

  function deepClone(obj) { return JSON.parse(JSON.stringify(obj)); }

  function getByPath(root, path) {
    let current = root;
    for (const key of path) current = current[key];
    return current;
  }

  function mutate(fn) {
    const clone = deepClone(fields);
    fn(clone);
    fields = clone;
    emit();
  }

  const pipeOptions = [
    { value: '', get label() { return t("(none)"); } },
    { value: 'lower', label: 'lower' },
    { value: 'upper', label: 'upper' },
    { value: 'capitalize', label: 'capitalize' },
    { value: 'first(N)', label: 'first(N)' },
    { value: 'last(N)', label: 'last(N)' },
    { value: 'substr(start,len)', label: 'substr(start,len)' },
    { value: 'default("val")', label: 'default("val")' },
    { value: 'replace("a","b")', label: 'replace("a","b")' },
    { value: 'prepend("prefix")', label: 'prepend("prefix")' },
    { value: 'append("suffix")', label: 'append("suffix")' },
    { value: 'length', label: 'length' },
    { value: 'trim', label: 'trim' },
  ];

  function newNode() {
    return { tag: '', nodeType: 'value', source: 'fixed', value: '', pipe: '' };
  }

  function addNodeAt(path) {
    mutate(root => getByPath(root, path).push(newNode()));
  }

  function removeAt(path, idx) {
    mutate(root => getByPath(root, path).splice(idx, 1));
  }

  function moveAt(path, idx, dir) {
    const target = idx + dir;
    mutate(root => {
      const arr = getByPath(root, path);
      if (target < 0 || target >= arr.length) return;
      [arr[idx], arr[target]] = [arr[target], arr[idx]];
    });
  }

  function updateProp(path, idx, prop, val) {
    mutate(root => {
      const arr = getByPath(root, path);
      arr[idx] = { ...arr[idx], [prop]: val };
      if (prop === 'source' && val === 'fake') arr[idx].value = 'CompanyName';
      if (prop === 'source' && ['uuid','now_ms','now_iso','seq'].includes(val)) arr[idx].value = '';
    });
  }

  function changeNodeType(path, idx, newType) {
    mutate(root => {
      const arr = getByPath(root, path);
      const tag = arr[idx].tag || '';
      if (newType === 'value') {
        arr[idx] = { tag, nodeType: 'value', source: 'fixed', value: '' };
      } else if (newType === 'parent') {
        arr[idx] = { tag, nodeType: 'parent', children: [] };
      }
    });
  }

  function emit() { onUpdate(fields); }

  // 'script' needs a value input: it names the key of the script result to use.
  function needsValueInput(src) { return ['fixed','path','query','header','body','xpath','script'].includes(src); }

  function valuePlaceholder(src) {
    if (src === 'body') return t("e.g. /user/name");
    if (src === 'xpath') return t("e.g. Envelope/Body/search/Id");
    // A script result is read one flat key deep ({{script.field}}, never {{script.object.field}}): an object nested
    // under a key comes out whole, as JSON, not as fields of its own. The rule tester shows the keys a script really
    // returns.
    if (src === 'script') return t("e.g. name (empty = the whole {{script}}; one level only: test the rule to see the keys)");
    return t("value");
  }

  // Folding of parent nodes, as in JsonResponseBuilder.svelte: an in-memory Set keyed by the positional test path,
  // everything unfolded at first.
  let collapsedPaths = $state(new Set());

  function isCollapsed(testPath) { return collapsedPaths.has(testPath); }

  function toggleCollapse(testPath) {
    const next = new Set(collapsedPaths);
    if (next.has(testPath)) next.delete(testPath); else next.add(testPath);
    collapsedPaths = next;
  }

  export function toTemplate() {
    return xmlFieldsToTemplate(fields, rootTag);
  }
</script>

<div class="xml-builder" aria-label={t("XML response builder")}>
  <div class="builder-header">
    <strong>{t("XML nodes")}</strong>
    <label class="inline-label">{t("Root tag:")} <input type="text" bind:value={rootTag} class="root-input" data-testid="xml-builder-root-tag-input" /></label>
  </div>

  {#snippet renderValueControls(field, path, idx)}
    {@const testPath = [...path, idx].join('-')}
    <select value={field.source} onchange={(e) => updateProp(path, idx, 'source', e.target.value)} aria-label={t("Source")} data-testid="xml-builder-source-select-{testPath}">
      {#each valueSources as vs}<option value={vs.value}>{vs.label}</option>{/each}
    </select>
    {#if field.source === 'fake'}
      <select value={field.value} onchange={(e) => updateProp(path, idx, 'value', e.target.value)} aria-label={t("Kind of fake data")} data-testid="xml-builder-fake-select-{testPath}">
        {#each fakeOptions as fo}<option value={fo}>{fo}</option>{/each}
      </select>
    {:else if needsValueInput(field.source)}
      <input type="text" class="value-input" value={field.value} oninput={(e) => updateProp(path, idx, 'value', e.target.value)} placeholder={valuePlaceholder(field.source)} aria-label={t("Value")} data-testid="xml-builder-value-input-{testPath}" />
    {/if}
    {#if field.source !== 'fixed'}
      <input type="text" class="pipe-input" value={field.pipe || ''} oninput={(e) => updateProp(path, idx, 'pipe', e.target.value)} placeholder={t("e.g. {0}", "lower | first(5)")} aria-label={t("Pipe")} list="dl-xml-pipes" autocomplete="off" data-testid="xml-builder-pipe-input-{testPath}" />
    {/if}
  {/snippet}

  {#snippet renderNodes(nodeList, path, depth)}
    {#each nodeList as field, idx}
      {@const nt = field.nodeType || 'value'}
      {@const testPath = [...path, idx].join('-')}
      {@const collapsed = nt === 'parent' && isCollapsed(testPath)}
      <div class="field-row" style:margin-left="{depth * 1.25}rem">
        <div class="field-main">
          {#if nt === 'parent'}
            <button
              type="button"
              class="btn-icon collapse-toggle"
              onclick={() => toggleCollapse(testPath)}
              aria-expanded={!collapsed}
              aria-controls="xml-builder-children-{testPath}"
              aria-label={collapsed ? t("Expand {0}", field.tag || t("this node")) : t("Collapse {0}", field.tag || t("this node"))}
              title={collapsed ? t("Expand") : t("Collapse")}
              data-testid="xml-builder-collapse-button-{testPath}"
            >{collapsed ? '▶' : '▼'}</button>
          {/if}
          <input type="text" class="tag-input" value={field.tag} oninput={(e) => updateProp(path, idx, 'tag', e.target.value)} placeholder={t("tag")} aria-label={t("XML tag")} data-testid="xml-builder-tag-input-{testPath}" />
          <select class="type-select" value={nt} onchange={(e) => changeNodeType(path, idx, e.target.value)} aria-label={t("Node type")} data-testid="xml-builder-type-select-{testPath}">
            {#each nodeTypes as option}<option value={option.value}>{option.label}</option>{/each}
          </select>
          {#if nt === 'value'}
            {@render renderValueControls(field, path, idx)}
          {/if}
          {#if collapsed}
            <span class="collapsed-indicator" data-testid="xml-builder-collapsed-indicator-{testPath}">{tCount((field.children || []).length, "({0} hidden item)", "({0} hidden items)")}</span>
          {/if}
          <div class="field-actions">
            <button type="button" class="btn-icon" onclick={() => moveAt(path, idx, -1)} disabled={idx === 0} aria-label={t("Move up")} title={t("Move up")} data-testid="xml-builder-moveup-button-{testPath}">&#9650;</button>
            <button type="button" class="btn-icon" onclick={() => moveAt(path, idx, 1)} disabled={idx === nodeList.length - 1} aria-label={t("Move down")} title={t("Move down")} data-testid="xml-builder-movedown-button-{testPath}">&#9660;</button>
            <button type="button" class="btn-icon btn-delete" onclick={() => removeAt(path, idx)} aria-label={t("Delete")} data-testid="xml-builder-delete-button-{testPath}">&#10005;</button>
          </div>
        </div>
        {#if nt === 'parent'}
          <div class="nested-block" id="xml-builder-children-{testPath}" hidden={collapsed}>
            {@render renderNodes(field.children || [], [...path, idx, 'children'], depth + 1)}
            <button type="button" class="btn btn-xs btn-outline" onclick={() => addNodeAt([...path, idx, 'children'])} data-testid="xml-builder-add-subnode-button-{testPath}">{t("+ Child node")}</button>
          </div>
        {/if}
      </div>
    {/each}
  {/snippet}

  {@render renderNodes(fields, [], 0)}

  <datalist id="dl-xml-pipes">
    {#each pipeOptions.filter(p => p.value) as p}<option value={p.value}>{p.label}</option>{/each}
  </datalist>

  <button type="button" class="btn btn-sm btn-outline" onclick={() => addNodeAt([])} data-testid="xml-builder-add-node-button">{t("+ Add a node")}</button>

  {#if fields.length > 0}
    <details class="preview-section">
      <summary>{t("Template preview")}</summary>
      <code class="preview-code">{toTemplate()}</code>
    </details>
    <details class="preview-section">
      <summary>{t("Readable XML preview")}</summary>
      <code class="preview-code preview-readable">{templateToPreview(toTemplate())}</code>
    </details>
  {/if}
</div>

<style>
  .xml-builder { display: flex; flex-direction: column; gap: 0.5rem; }
  .builder-header { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem; }
  .builder-header strong { font-size: 0.9rem; }
  .inline-label { display: flex; align-items: center; gap: 0.375rem; font-size: 0.8125rem; }
  .root-input { width: 8rem; padding: 0.25rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }

  .field-row { padding: 0.375rem; background: var(--color-bg); border: 1px solid var(--color-border); border-radius: var(--radius); margin-bottom: 0.25rem; }
  .field-main { display: flex; gap: 0.375rem; align-items: center; flex-wrap: wrap; }
  .tag-input { width: 7rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; font-weight: 600; }
  .type-select { padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; min-width: 5rem; background: var(--color-surface); }
  .value-input { flex: 1; min-width: 6rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }
  select { padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }

  .field-actions { display: flex; gap: 0.2rem; margin-left: auto; }
  .btn-icon { width: 1.5rem; height: 1.5rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.7rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }

  .nested-block { margin-top: 0.375rem; padding-left: 0.75rem; border-left: 2px solid var(--color-primary); }
  .collapse-toggle { flex-shrink: 0; }
  .collapsed-indicator { font-size: 0.75rem; color: var(--color-text-muted); font-style: italic; white-space: nowrap; }

  .btn-xs { padding: 0.15rem 0.5rem; font-size: 0.75rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; }

  .preview-section { margin-top: 0.5rem; }
  .preview-section summary { font-size: 0.8125rem; cursor: pointer; color: var(--color-text-muted); }
  .preview-code { display: block; margin-top: 0.25rem; padding: 0.5rem; background: var(--color-bg); border-radius: var(--radius); font-size: 0.75rem; word-break: break-all; white-space: pre-wrap; }
  .preview-readable { color: var(--color-primary); }
  .pipe-input { min-width: 8rem; max-width: 14rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.75rem; font-family: 'Cascadia Code', 'Fira Code', monospace; color: var(--color-primary); }
</style>
