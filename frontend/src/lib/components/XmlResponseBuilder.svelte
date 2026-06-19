<script>
  let { fields = [], rootTag = 'response', onUpdate = () => {} } = $props();

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
  ];

  function addField() { fields = [...fields, { tag: '', source: 'fixed', value: '' }]; emit(); }
  function removeField(idx) { fields = fields.filter((_, i) => i !== idx); emit(); }
  function updateField(idx, prop, val) {
    const c = [...fields]; c[idx] = { ...c[idx], [prop]: val };
    if (prop === 'source' && val === 'fake') c[idx].value = 'CompanyName';
    if (prop === 'source' && ['uuid','now_ms','now_iso','seq'].includes(val)) c[idx].value = '';
    fields = c; emit();
  }
  function emit() { onUpdate(fields); }

  function needsValueInput(src) { return ['fixed','path','query','header','body'].includes(src); }

  export function toTemplate() {
    const inner = fields.filter(f => f.tag.trim()).map(f => {
      const t = f.tag.trim();
      const v = buildExpr(f);
      return `<${t}>${v}</${t}>`;
    }).join('');
    return `<${rootTag}>${inner}</${rootTag}>`;
  }

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
</script>

<div class="xml-builder" aria-label="Constructeur de reponse XML">
  <div class="builder-header">
    <strong>Noeuds XML</strong>
    <label class="inline-label">Tag racine : <input type="text" bind:value={rootTag} class="root-input" /></label>
  </div>

  {#each fields as field, idx}
    <div class="field-row">
      <input type="text" class="tag-input" value={field.tag} oninput={(e) => updateField(idx, 'tag', e.target.value)} placeholder="tag (ex: siret)" aria-label="Tag XML {idx + 1}" />
      <select value={field.source} onchange={(e) => updateField(idx, 'source', e.target.value)} aria-label="Source {idx + 1}">
        {#each valueSources as vs}<option value={vs.value}>{vs.label}</option>{/each}
      </select>
      {#if field.source === 'fake'}
        <select value={field.value} onchange={(e) => updateField(idx, 'value', e.target.value)} aria-label="Type fictif {idx + 1}">
          {#each fakeOptions as fo}<option value={fo}>{fo}</option>{/each}
        </select>
      {:else if needsValueInput(field.source)}
        <input type="text" class="value-input" value={field.value} oninput={(e) => updateField(idx, 'value', e.target.value)} placeholder="valeur" aria-label="Valeur {idx + 1}" />
      {/if}
      <button type="button" class="btn-icon btn-delete" onclick={() => removeField(idx)} aria-label="Supprimer">&#10005;</button>
    </div>
  {/each}

  <button type="button" class="btn btn-sm btn-outline" onclick={addField}>+ Ajouter un noeud</button>

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

  .field-row { display: flex; gap: 0.375rem; align-items: center; flex-wrap: wrap; padding: 0.375rem; background: var(--color-bg); border: 1px solid var(--color-border); border-radius: var(--radius); }
  .tag-input { width: 7rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; font-weight: 600; }
  .value-input { flex: 1; min-width: 6rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }
  select { padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }

  .btn-icon { width: 1.5rem; height: 1.5rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.7rem; cursor: pointer; }
  .btn-icon.btn-delete:hover { color: var(--color-danger); border-color: var(--color-danger); }
  .btn { cursor: pointer; } .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; }
  .btn-outline { background: var(--color-surface); color: var(--color-text-muted); border-color: var(--color-border); }

  .preview-section { margin-top: 0.5rem; }
  .preview-section summary { font-size: 0.8125rem; cursor: pointer; color: var(--color-text-muted); }
  .preview-code { display: block; margin-top: 0.25rem; padding: 0.5rem; background: var(--color-bg); border-radius: var(--radius); font-size: 0.75rem; word-break: break-all; white-space: pre-wrap; }
</style>
