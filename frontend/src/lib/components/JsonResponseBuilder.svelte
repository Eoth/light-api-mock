<script>
  let { fields = [], onUpdate = () => {} } = $props();

  const valueSources = [
    { value: 'fixed', label: 'Valeur fixe' },
    { value: 'path', label: 'Parametre URL {param}' },
    { value: 'query', label: 'Query param' },
    { value: 'header', label: 'Header HTTP' },
    { value: 'body', label: 'Echo body (JSON pointer)' },
    { value: 'fake', label: 'Donnee fictive' },
    { value: 'uuid', label: 'UUID' },
    { value: 'now_ms', label: 'Timestamp (ms)' },
    { value: 'now_iso', label: 'Date ISO' },
    { value: 'seq', label: 'Compteur sequentiel' },
  ];

  const fakeOptions = [
    'FirstName', 'LastName', 'Email', 'PhoneNumberFR', 'CompanyName',
    'StreetName', 'CityFR', 'PostcodeFR', 'Siren', 'Siret',
    'FullAddressFR', 'DatePast', 'DateFuture', 'TimestampMs',
  ];

  function addField() {
    fields = [...fields, { key: '', source: 'fixed', value: '', asNumber: false }];
    emit();
  }

  function removeField(idx) {
    fields = fields.filter((_, i) => i !== idx);
    emit();
  }

  function moveField(idx, dir) {
    const t = idx + dir;
    if (t < 0 || t >= fields.length) return;
    const c = [...fields]; [c[idx], c[t]] = [c[t], c[idx]]; fields = c;
    emit();
  }

  function updateField(idx, prop, val) {
    const c = [...fields];
    c[idx] = { ...c[idx], [prop]: val };
    if (prop === 'source') {
      if (['uuid', 'now_ms', 'now_iso', 'seq'].includes(val)) c[idx].value = '';
      if (val === 'fake') c[idx].value = 'CompanyName';
    }
    fields = c;
    emit();
  }

  function emit() {
    onUpdate(fields);
  }

  function needsValueInput(source) {
    return ['fixed', 'path', 'query', 'header', 'body'].includes(source);
  }

  function fieldPlaceholder(source) {
    switch (source) {
      case 'fixed': return 'Valeur fixe (ex: actif)';
      case 'path': return 'Nom du param (ex: siret)';
      case 'query': return 'Nom du query param (ex: page)';
      case 'header': return 'Nom du header (ex: x-request-id)';
      case 'body': return 'JSON pointer (ex: /user/name)';
      default: return '';
    }
  }

  export function toTemplate() {
    if (fields.length === 0) return '{{}}';
    const parts = fields
      .filter(f => f.key.trim())
      .map(f => {
        const k = f.key.trim();
        const expr = buildExpr(f);
        if (f.asNumber) return `"${k}":${expr}`;
        return `"${k}":"${expr}"`;
      });
    return `{{${parts.join(',')}}}`;
  }

  function buildExpr(f) {
    switch (f.source) {
      case 'fixed': return f.asNumber ? f.value : f.value;
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

<div class="json-builder" aria-label="Constructeur de reponse JSON">
  <div class="builder-header">
    <strong>Champs de la reponse JSON</strong>
    <span class="field-hint">Chaque ligne = une cle du JSON. Choisissez la source de la valeur.</span>
  </div>

  {#if fields.length === 0}
    <p class="empty-msg">Aucun champ. Cliquez "+ Ajouter un champ" pour commencer.</p>
  {/if}

  {#each fields as field, idx}
    <div class="field-row">
      <input
        type="text"
        class="key-input"
        value={field.key}
        oninput={(e) => updateField(idx, 'key', e.target.value)}
        placeholder="cle (ex: siret)"
        aria-label="Nom de la cle {idx + 1}"
      />
      <select
        value={field.source}
        onchange={(e) => updateField(idx, 'source', e.target.value)}
        aria-label="Source de la valeur {idx + 1}"
      >
        {#each valueSources as vs}
          <option value={vs.value}>{vs.label}</option>
        {/each}
      </select>
      {#if field.source === 'fake'}
        <select
          value={field.value}
          onchange={(e) => updateField(idx, 'value', e.target.value)}
          aria-label="Type de donnee fictive {idx + 1}"
        >
          {#each fakeOptions as fo}
            <option value={fo}>{fo}</option>
          {/each}
        </select>
      {:else if needsValueInput(field.source)}
        <input
          type="text"
          class="value-input"
          value={field.value}
          oninput={(e) => updateField(idx, 'value', e.target.value)}
          placeholder={fieldPlaceholder(field.source)}
          aria-label="Valeur {idx + 1}"
        />
      {/if}
      <label class="number-toggle" title="Rendre sans guillemets (nombre JSON)">
        <input type="checkbox" checked={field.asNumber} onchange={(e) => updateField(idx, 'asNumber', e.target.checked)} />
        <span class="number-label">#</span>
      </label>
      <div class="field-actions">
        <button type="button" class="btn-icon" onclick={() => moveField(idx, -1)} disabled={idx === 0} aria-label="Monter" title="Monter">&#9650;</button>
        <button type="button" class="btn-icon" onclick={() => moveField(idx, 1)} disabled={idx === fields.length - 1} aria-label="Descendre" title="Descendre">&#9660;</button>
        <button type="button" class="btn-icon btn-delete" onclick={() => removeField(idx)} aria-label="Supprimer le champ {field.key || idx + 1}">&#10005;</button>
      </div>
    </div>
  {/each}

  <button type="button" class="btn btn-sm btn-outline" onclick={addField}>+ Ajouter un champ</button>

  {#if fields.length > 0}
    <details class="preview-section">
      <summary>Apercu du template genere</summary>
      <code class="preview-code">{toTemplate()}</code>
    </details>
  {/if}
</div>

<style>
  .json-builder { display: flex; flex-direction: column; gap: 0.5rem; }
  .builder-header strong { font-size: 0.9rem; }
  .empty-msg { color: var(--color-text-muted); font-style: italic; font-size: 0.875rem; margin: 0.25rem 0; }

  .field-row {
    display: flex; gap: 0.375rem; align-items: center; flex-wrap: wrap;
    padding: 0.375rem; background: var(--color-bg); border: 1px solid var(--color-border);
    border-radius: var(--radius);
  }

  .key-input { width: 8rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; font-weight: 600; }
  .value-input { flex: 1; min-width: 8rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }
  select { padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }

  .number-toggle { display: flex; align-items: center; gap: 0.2rem; cursor: pointer; }
  .number-toggle input { width: 1rem; height: 1rem; }
  .number-label { font-size: 0.75rem; font-weight: 700; color: var(--color-text-muted); }

  .field-actions { display: flex; gap: 0.2rem; }
  .btn-icon { width: 1.5rem; height: 1.5rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.7rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }

  .btn { cursor: pointer; } .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; }
  .btn-outline { background: var(--color-surface); color: var(--color-text-muted); border-color: var(--color-border); }
  .btn-outline:hover { background: var(--color-bg); color: var(--color-text); }

  .preview-section { margin-top: 0.5rem; }
  .preview-section summary { font-size: 0.8125rem; cursor: pointer; color: var(--color-text-muted); }
  .preview-code { display: block; margin-top: 0.25rem; padding: 0.5rem; background: var(--color-bg); border-radius: var(--radius); font-size: 0.75rem; word-break: break-all; white-space: pre-wrap; }
  .field-hint { display: block; font-size: 0.75rem; color: var(--color-text-muted); }
</style>
