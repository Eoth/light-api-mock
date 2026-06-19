<script>
  let { fields = [], rootTag = 'response', onUpdate = () => {} } = $props();

  const nodeTypes = [
    { value: 'value', label: 'Contenu' },
    { value: 'parent', label: 'Noeud parent' },
  ];

  const valueSources = [
    { value: 'fixed', label: 'Valeur fixe' },
    { value: 'path', label: 'Parametre URL' },
    { value: 'query', label: 'Query param' },
    { value: 'header', label: 'Header HTTP' },
    { value: 'body', label: 'Echo body' },
    { value: 'fake', label: 'Donnee fictive' },
    { value: 'uuid', label: 'UUID' },
    { value: 'now_ms', label: 'Timestamp (ms)' },
    { value: 'now_iso', label: 'Date ISO' },
    { value: 'seq', label: 'Compteur' },
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

  function newNode() {
    return { tag: '', nodeType: 'value', source: 'fixed', value: '' };
  }

  function addNodeAt(path) {
    mutate(root => getByPath(root, path).push(newNode()));
  }

  function removeAt(path, idx) {
    mutate(root => getByPath(root, path).splice(idx, 1));
  }

  function moveAt(path, idx, dir) {
    const t = idx + dir;
    mutate(root => {
      const arr = getByPath(root, path);
      if (t < 0 || t >= arr.length) return;
      [arr[idx], arr[t]] = [arr[t], arr[idx]];
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

  function needsValueInput(src) { return ['fixed','path','query','header','body'].includes(src); }

  function buildExpr(f) {
    switch (f.source) {
      case 'fixed': return f.value;
      case 'path': return `{path.${f.value}}`;
      case 'query': return `{query.${f.value}}`;
      case 'header': return `{header.${f.value}}`;
      case 'body': return `{body.${f.value}}`;
      case 'fake': return `{fake.${f.value}}`;
      case 'uuid': return '{uuid}';
      case 'now_ms': return '{now_ms}';
      case 'now_iso': return '{now_iso}';
      case 'seq': return '{seq}';
      default: return f.value;
    }
  }

  function nodeToXml(field) {
    const t = field.tag?.trim();
    if (!t) return '';
    const nt = field.nodeType || 'value';
    if (nt === 'parent') {
      const inner = (field.children || []).filter(c => c.tag?.trim()).map(c => nodeToXml(c)).join('');
      return `<${t}>${inner}</${t}>`;
    }
    return `<${t}>${buildExpr(field)}</${t}>`;
  }

  export function toTemplate() {
    const inner = fields.filter(f => f.tag?.trim()).map(f => nodeToXml(f)).join('');
    return `<${rootTag}>${inner}</${rootTag}>`;
  }
</script>

<div class="xml-builder" aria-label="Constructeur de reponse XML">
  <div class="builder-header">
    <strong>Noeuds XML</strong>
    <label class="inline-label">Tag racine : <input type="text" bind:value={rootTag} class="root-input" /></label>
  </div>

  {#snippet renderValueControls(field, path, idx)}
    <select value={field.source} onchange={(e) => updateProp(path, idx, 'source', e.target.value)} aria-label="Source">
      {#each valueSources as vs}<option value={vs.value}>{vs.label}</option>{/each}
    </select>
    {#if field.source === 'fake'}
      <select value={field.value} onchange={(e) => updateProp(path, idx, 'value', e.target.value)} aria-label="Type fictif">
        {#each fakeOptions as fo}<option value={fo}>{fo}</option>{/each}
      </select>
    {:else if needsValueInput(field.source)}
      <input type="text" class="value-input" value={field.value} oninput={(e) => updateProp(path, idx, 'value', e.target.value)} placeholder="valeur" aria-label="Valeur" />
    {/if}
  {/snippet}

  {#snippet renderNodes(nodeList, path, depth)}
    {#each nodeList as field, idx}
      {@const nt = field.nodeType || 'value'}
      <div class="field-row" style:margin-left="{depth * 1.25}rem">
        <div class="field-main">
          <input type="text" class="tag-input" value={field.tag} oninput={(e) => updateProp(path, idx, 'tag', e.target.value)} placeholder="tag" aria-label="Tag XML" />
          <select class="type-select" value={nt} onchange={(e) => changeNodeType(path, idx, e.target.value)} aria-label="Type de noeud">
            {#each nodeTypes as t}<option value={t.value}>{t.label}</option>{/each}
          </select>
          {#if nt === 'value'}
            {@render renderValueControls(field, path, idx)}
          {/if}
          <div class="field-actions">
            <button type="button" class="btn-icon" onclick={() => moveAt(path, idx, -1)} disabled={idx === 0} aria-label="Monter" title="Monter">&#9650;</button>
            <button type="button" class="btn-icon" onclick={() => moveAt(path, idx, 1)} disabled={idx === nodeList.length - 1} aria-label="Descendre" title="Descendre">&#9660;</button>
            <button type="button" class="btn-icon btn-delete" onclick={() => removeAt(path, idx)} aria-label="Supprimer">&#10005;</button>
          </div>
        </div>
        {#if nt === 'parent'}
          <div class="nested-block">
            {@render renderNodes(field.children || [], [...path, idx, 'children'], depth + 1)}
            <button type="button" class="btn btn-xs btn-outline" onclick={() => addNodeAt([...path, idx, 'children'])}>+ Sous-noeud</button>
          </div>
        {/if}
      </div>
    {/each}
  {/snippet}

  {@render renderNodes(fields, [], 0)}

  <button type="button" class="btn btn-sm btn-outline" onclick={() => addNodeAt([])}>+ Ajouter un noeud</button>

  {#if fields.length > 0}
    <details class="preview-section">
      <summary>Apercu XML</summary>
      <code class="preview-code">{toTemplate()}</code>
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

  .btn { cursor: pointer; } .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; }
  .btn-xs { padding: 0.15rem 0.5rem; font-size: 0.75rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; }
  .btn-outline { background: var(--color-surface); color: var(--color-text-muted); border-color: var(--color-border); }
  .btn-outline:hover { background: var(--color-bg); color: var(--color-text); }

  .preview-section { margin-top: 0.5rem; }
  .preview-section summary { font-size: 0.8125rem; cursor: pointer; color: var(--color-text-muted); }
  .preview-code { display: block; margin-top: 0.25rem; padding: 0.5rem; background: var(--color-bg); border-radius: var(--radius); font-size: 0.75rem; word-break: break-all; white-space: pre-wrap; }
</style>
