<script>
  import { fieldsToTemplate, buildExpr as sharedBuildExpr, templateToPreview } from '../tpl-utils.js';

  let { fields = [], onUpdate = () => {} } = $props();

  const fieldTypes = [
    { value: 'value', label: 'Valeur' },
    { value: 'object', label: 'Objet' },
    { value: 'array-values', label: 'Tableau' },
    { value: 'array-objects', label: 'Tableau d\'objets' },
  ];

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
    { value: 'script', label: 'Resultat du script' },
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
    { value: '', label: '(aucun)' },
    { value: 'lower', label: 'lower — minuscules' },
    { value: 'upper', label: 'upper — majuscules' },
    { value: 'trim', label: 'trim — suppr. espaces' },
    { value: 'capitalize', label: 'capitalize — 1ere maj.' },
    { value: 'first(N)', label: 'first(N) — N premiers car.' },
    { value: 'last(N)', label: 'last(N) — N derniers car.' },
    { value: 'substr(start,len)', label: 'substr(start,len)' },
    { value: 'default("val")', label: 'default("val") — si vide' },
    { value: 'replace("a","b")', label: 'replace("a","b")' },
    { value: 'prepend("prefix")', label: 'prepend("prefix")' },
    { value: 'append("suffix")', label: 'append("suffix")' },
    { value: 'length', label: 'length — nb car.' },
  ];

  function newValueField() {
    return { key: '', fieldType: 'value', source: 'fixed', value: '', pipe: '', asNumber: false };
  }

  function addFieldAt(path) {
    mutate(root => getByPath(root, path).push(newValueField()));
  }

  function addArrayItem(path) {
    mutate(root => getByPath(root, path).push({ source: 'fixed', value: '', asNumber: false }));
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
      if (prop === 'source') {
        if (['uuid', 'now_ms', 'now_iso', 'seq'].includes(val)) arr[idx].value = '';
        if (val === 'fake') arr[idx].value = 'CompanyName';
      }
    });
  }

  function changeFieldType(path, idx, newType) {
    mutate(root => {
      const arr = getByPath(root, path);
      const old = arr[idx];
      const key = old.key || '';
      if (newType === 'value') {
        arr[idx] = { key, fieldType: 'value', source: 'fixed', value: '', asNumber: false };
      } else if (newType === 'object') {
        arr[idx] = { key, fieldType: 'object', children: [] };
      } else if (newType === 'array-values') {
        arr[idx] = { key, fieldType: 'array-values', items: [] };
      } else if (newType === 'array-objects') {
        arr[idx] = { key, fieldType: 'array-objects', template: [] };
      }
    });
  }

  function emit() { onUpdate(fields); }

  function needsValueInput(source) {
    return ['fixed', 'path', 'query', 'header', 'body'].includes(source);
  }

  function fieldPlaceholder(source) {
    switch (source) {
      case 'fixed': return 'ex: actif';
      case 'path': return 'ex: siret';
      case 'query': return 'ex: page';
      case 'header': return 'ex: x-request-id';
      case 'body': return 'ex: /user/name';
      default: return '';
    }
  }

  function buildExpr(f) { return sharedBuildExpr(f); }

  export function toTemplate() {
    return fieldsToTemplate(fields);
  }

  function previewJson() {
    try { return templateToPreview(toTemplate()); }
    catch { return '(erreur)'; }
  }
</script>

<div class="json-builder" aria-label="Constructeur de reponse JSON">
  <div class="builder-header">
    <strong>Champs de la reponse JSON</strong>
    <span class="field-hint">Construisez la structure JSON : valeurs, objets imbriques, tableaux.</span>
  </div>

  {#if fields.length === 0}
    <p class="empty-msg">Aucun champ. Cliquez "+ Ajouter un champ" pour commencer.</p>
  {/if}

  {#snippet renderValueControls(field, path, idx)}
    <select
      value={field.source}
      onchange={(e) => updateProp(path, idx, 'source', e.target.value)}
      aria-label="Source de la valeur"
    >
      {#each valueSources as vs}
        <option value={vs.value}>{vs.label}</option>
      {/each}
    </select>
    {#if field.source === 'fake'}
      <select
        value={field.value}
        onchange={(e) => updateProp(path, idx, 'value', e.target.value)}
        aria-label="Type de donnee fictive"
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
        oninput={(e) => updateProp(path, idx, 'value', e.target.value)}
        placeholder={fieldPlaceholder(field.source)}
        aria-label="Valeur"
      />
    {/if}
    {#if field.source !== 'fixed'}
      <input
        type="text"
        class="pipe-input"
        value={field.pipe || ''}
        oninput={(e) => updateProp(path, idx, 'pipe', e.target.value)}
        placeholder="ex: first(9) | upper"
        aria-label="Pipe de transformation"
        list="dl-pipes"
        autocomplete="off"
      />
    {/if}
    <label class="number-toggle" title="Rendre sans guillemets (nombre JSON)">
      <input type="checkbox" checked={field.asNumber} onchange={(e) => updateProp(path, idx, 'asNumber', e.target.checked)} />
      <span class="number-label">#</span>
    </label>
  {/snippet}

  {#snippet renderFields(fieldList, path, depth)}
    {#each fieldList as field, idx}
      {@const ft = field.fieldType || 'value'}
      <div class="field-row" style:margin-left="{depth * 1.25}rem">
        <div class="field-main">
          <input
            type="text"
            class="key-input"
            value={field.key}
            oninput={(e) => updateProp(path, idx, 'key', e.target.value)}
            placeholder="cle"
            aria-label="Nom de la cle"
          />
          <select
            class="type-select"
            value={ft}
            onchange={(e) => changeFieldType(path, idx, e.target.value)}
            aria-label="Type de champ"
          >
            {#each fieldTypes as t}
              <option value={t.value}>{t.label}</option>
            {/each}
          </select>
          {#if ft === 'value'}
            {@render renderValueControls(field, path, idx)}
          {/if}
          <div class="field-actions">
            <button type="button" class="btn-icon" onclick={() => moveAt(path, idx, -1)} disabled={idx === 0} aria-label="Monter" title="Monter">&#9650;</button>
            <button type="button" class="btn-icon" onclick={() => moveAt(path, idx, 1)} disabled={idx === fieldList.length - 1} aria-label="Descendre" title="Descendre">&#9660;</button>
            <button type="button" class="btn-icon btn-delete" onclick={() => removeAt(path, idx)} aria-label="Supprimer le champ {field.key || idx + 1}">&#10005;</button>
          </div>
        </div>

        {#if ft === 'object'}
          <div class="nested-block">
            {@render renderFields(field.children || [], [...path, idx, 'children'], depth + 1)}
            <button type="button" class="btn btn-xs btn-outline" onclick={() => addFieldAt([...path, idx, 'children'])}>+ Sous-champ</button>
          </div>
        {:else if ft === 'array-values'}
          <div class="nested-block">
            {#each (field.items || []) as item, iidx}
              <div class="array-item">
                <span class="item-index">{iidx + 1}</span>
                {@render renderValueControls(item, [...path, idx, 'items'], iidx)}
                <button type="button" class="btn-icon btn-delete" onclick={() => removeAt([...path, idx, 'items'], iidx)} aria-label="Supprimer l'element {iidx + 1}">&#10005;</button>
              </div>
            {/each}
            <button type="button" class="btn btn-xs btn-outline" onclick={() => addArrayItem([...path, idx, 'items'])}>+ Element</button>
          </div>
        {:else if ft === 'array-objects'}
          <div class="nested-block">
            <span class="nested-hint">Schema d'un element du tableau :</span>
            {@render renderFields(field.template || [], [...path, idx, 'template'], depth + 1)}
            <button type="button" class="btn btn-xs btn-outline" onclick={() => addFieldAt([...path, idx, 'template'])}>+ Champ</button>
          </div>
        {/if}
      </div>
    {/each}
  {/snippet}

  {@render renderFields(fields, [], 0)}

  <button type="button" class="btn btn-sm btn-outline" onclick={() => addFieldAt([])}>+ Ajouter un champ</button>

  <datalist id="dl-pipes">
    {#each pipeOptions.filter(p => p.value) as p}<option value={p.value}>{p.label}</option>{/each}
  </datalist>

  {#if fields.length > 0}
    <details class="preview-section">
      <summary>Apercu du template genere</summary>
      <code class="preview-code">{toTemplate()}</code>
    </details>
    <details class="preview-section">
      <summary>Apercu JSON lisible</summary>
      <code class="preview-code preview-readable">{previewJson()}</code>
    </details>
  {/if}
</div>

<style>
  .json-builder { display: flex; flex-direction: column; gap: 0.5rem; }
  .builder-header strong { font-size: 0.9rem; }
  .empty-msg { color: var(--color-text-muted); font-style: italic; font-size: 0.875rem; margin: 0.25rem 0; }

  .field-row {
    padding: 0.375rem; background: var(--color-bg); border: 1px solid var(--color-border);
    border-radius: var(--radius); margin-bottom: 0.25rem;
  }

  .field-main {
    display: flex; gap: 0.375rem; align-items: center; flex-wrap: wrap;
  }

  .key-input { width: 7rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; font-weight: 600; }
  .type-select { padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; min-width: 5rem; background: var(--color-surface); }
  .value-input { flex: 1; min-width: 6rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }
  select { padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; }

  .pipe-input { min-width: 8rem; max-width: 14rem; padding: 0.3rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.75rem; font-family: 'Cascadia Code', 'Fira Code', monospace; color: var(--color-primary); }
  .number-toggle { display: flex; align-items: center; gap: 0.2rem; cursor: pointer; }
  .number-toggle input { width: 1rem; height: 1rem; }
  .number-label { font-size: 0.75rem; font-weight: 700; color: var(--color-text-muted); }

  .field-actions { display: flex; gap: 0.2rem; margin-left: auto; }
  .btn-icon { width: 1.5rem; height: 1.5rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.7rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }

  .nested-block { margin-top: 0.375rem; padding-left: 0.75rem; border-left: 2px solid var(--color-primary); }
  .nested-hint { font-size: 0.75rem; color: var(--color-text-muted); font-style: italic; display: block; margin-bottom: 0.25rem; }

  .array-item { display: flex; gap: 0.375rem; align-items: center; flex-wrap: wrap; padding: 0.25rem 0; }
  .item-index { display: inline-flex; align-items: center; justify-content: center; width: 1.25rem; height: 1.25rem; border-radius: 50%; background: var(--color-text-muted); color: #fff; font-size: 0.65rem; font-weight: 700; flex-shrink: 0; }

  .btn-xs { padding: 0.15rem 0.5rem; font-size: 0.75rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; }

  .preview-section { margin-top: 0.375rem; }
  .preview-section summary { font-size: 0.8125rem; cursor: pointer; color: var(--color-text-muted); }
  .preview-code { display: block; margin-top: 0.25rem; padding: 0.5rem; background: var(--color-bg); border-radius: var(--radius); font-size: 0.75rem; word-break: break-all; white-space: pre-wrap; }
  .preview-readable { color: var(--color-primary); }
</style>
