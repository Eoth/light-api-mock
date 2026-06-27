<script>
  import { buildExpr as sharedBuildExpr, fieldsToTemplate } from '../tpl-utils.js';

  let { fields = [], onUpdate = () => {} } = $props();

  let pasteInput = $state('');
  let parseError = $state('');
  let parsed = $state(false);

  const valueSources = [
    { value: 'fixed', label: 'Garder la valeur' },
    { value: 'path', label: 'Parametre URL' },
    { value: 'query', label: 'Query param' },
    { value: 'header', label: 'Header HTTP' },
    { value: 'body', label: 'Echo body' },
    { value: 'fake', label: 'Donnee fictive' },
    { value: 'uuid', label: 'UUID' },
    { value: 'now_ms', label: 'Timestamp (ms)' },
    { value: 'now_iso', label: 'Date ISO' },
    { value: 'seq', label: 'Compteur' },
    { value: 'script', label: 'Resultat script' },
  ];

  const fakeOptions = [
    'FirstName', 'LastName', 'Email', 'PhoneNumberFR', 'CompanyName',
    'StreetName', 'CityFR', 'PostcodeFR', 'Siren', 'Siret',
    'FullAddressFR', 'DatePast', 'DateFuture', 'TimestampMs',
    'BoolRandom', 'LoremSentence', 'CountryFR', 'IbanFR',
  ];

  function handleParse() {
    parseError = '';
    const text = pasteInput.trim();
    if (!text) { parseError = 'Collez un JSON valide.'; return; }
    try {
      const obj = JSON.parse(text);
      if (typeof obj !== 'object' || Array.isArray(obj)) {
        parseError = 'Le JSON doit etre un objet (pas un tableau).';
        return;
      }
      fields = objectToFields(obj);
      parsed = true;
      emit();
    } catch (e) {
      parseError = `JSON invalide : ${e.message}`;
    }
  }

  function objectToFields(obj, depth = 0) {
    return Object.entries(obj).map(([key, value]) => {
      if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
        return { key, fieldType: 'object', children: objectToFields(value, depth + 1) };
      }
      if (Array.isArray(value)) {
        if (value.length > 0 && typeof value[0] === 'object') {
          return { key, fieldType: 'array-objects', template: objectToFields(value[0], depth + 1) };
        }
        return {
          key, fieldType: 'array-values',
          items: value.map(v => ({ source: 'fixed', value: String(v), pipe: '', asNumber: typeof v === 'number' })),
        };
      }
      return {
        key, fieldType: 'value', source: 'fixed',
        value: String(value ?? ''), pipe: '',
        asNumber: typeof value === 'number' || typeof value === 'boolean',
      };
    });
  }

  function deepClone(obj) { return JSON.parse(JSON.stringify(obj)); }

  function updateField(path, prop, val) {
    const clone = deepClone(fields);
    let target = clone;
    for (let i = 0; i < path.length - 1; i++) target = target[path[i]];
    const field = target[path[path.length - 1]];
    field[prop] = val;
    if (prop === 'source' && val === 'fake') field.value = 'CompanyName';
    if (prop === 'source' && ['uuid', 'now_ms', 'now_iso', 'seq'].includes(val)) field.value = '';
    fields = clone;
    emit();
  }

  function emit() { onUpdate(fields); }

  function needsValueInput(src) {
    return ['fixed', 'path', 'query', 'header', 'body', 'script'].includes(src);
  }

  export function toTemplate() {
    return fieldsToTemplate(fields);
  }
</script>

<div class="paste-builder" aria-label="Constructeur JSON par exemple">
  {#if !parsed}
    <div class="paste-zone">
      <label for="json-paste-input">Collez un exemple de reponse JSON</label>
      <textarea
        id="json-paste-input"
        bind:value={pasteInput}
        rows="6"
        class="paste-textarea"
        placeholder={'{\n  "siret": "44306184100047",\n  "nom": "ACME Corp",\n  "status": "actif"\n}'}
      ></textarea>
      {#if parseError}
        <div class="form-error" role="alert">{parseError}</div>
      {/if}
      <button type="button" class="btn btn-primary btn-sm" onclick={handleParse}>
        Analyser et variabiliser
      </button>
    </div>
  {:else}
    <div class="paste-header">
      <span class="field-hint">{fields.length} champ{fields.length !== 1 ? 's' : ''} detecte{fields.length !== 1 ? 's' : ''} — choisissez la source de chaque valeur</span>
      <button type="button" class="btn btn-outline btn-sm" onclick={() => { parsed = false; pasteInput = ''; }}>
        Recoller un JSON
      </button>
    </div>

    {#snippet renderFields(fieldList, path, depth)}
      {#each fieldList as field, idx}
        {@const currentPath = [...path, idx]}
        <div class="paste-field" style:margin-left="{depth * 1.25}rem">
          <span class="paste-key">{field.key}</span>

          {#if field.fieldType === 'object'}
            <span class="paste-type-badge">objet</span>
            {@render renderFields(field.children, [...currentPath, 'children'], depth + 1)}
          {:else if field.fieldType === 'array-values' || field.fieldType === 'array-objects'}
            <span class="paste-type-badge">tableau</span>
          {:else}
            <div class="paste-controls">
              <select value={field.source} onchange={(e) => updateField(currentPath, 'source', e.target.value)} aria-label="Source pour {field.key}">
                {#each valueSources as vs}
                  <option value={vs.value}>{vs.label}</option>
                {/each}
              </select>

              {#if field.source === 'fake'}
                <select value={field.value} onchange={(e) => updateField(currentPath, 'value', e.target.value)} aria-label="Type fictif">
                  {#each fakeOptions as fo}
                    <option value={fo}>{fo}</option>
                  {/each}
                </select>
              {:else if needsValueInput(field.source)}
                <input
                  type="text"
                  class="paste-value"
                  value={field.value}
                  oninput={(e) => updateField(currentPath, 'value', e.target.value)}
                  placeholder={field.source === 'fixed' ? 'valeur fixe' : 'nom du parametre'}
                  aria-label="Valeur pour {field.key}"
                />
              {/if}

              {#if field.source === 'fixed'}
                <span class="paste-preview-fixed">{field.value}</span>
              {:else}
                <span class="paste-preview-var">{sharedBuildExpr(field)}</span>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    {/snippet}

    {@render renderFields(fields, [], 0)}
  {/if}
</div>

<style>
  .paste-builder { display: flex; flex-direction: column; gap: 0.75rem; }

  .paste-zone { display: flex; flex-direction: column; gap: 0.5rem; }
  .paste-zone label { font-weight: 600; font-size: 0.875rem; }
  .paste-textarea {
    width: 100%; font-family: 'Cascadia Code', 'Fira Code', monospace;
    font-size: 0.8125rem; padding: 0.5rem;
    border: 2px dashed var(--color-border); border-radius: var(--radius);
    background: var(--color-bg); color: var(--color-text); resize: vertical;
    min-height: 8rem;
  }
  .paste-textarea:focus { border-color: var(--color-primary); border-style: solid; }

  .paste-header { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem; }

  .paste-field {
    display: flex; flex-direction: column; gap: 0.25rem;
    padding: 0.375rem 0; border-bottom: 1px solid var(--color-border);
  }
  .paste-field:last-child { border-bottom: none; }

  .paste-key {
    font-weight: 700; font-size: 0.875rem; color: var(--color-primary);
    font-family: monospace;
  }

  .paste-type-badge {
    display: inline-block; font-size: 0.6875rem; font-weight: 600;
    color: var(--color-text-muted); background: var(--color-bg);
    padding: 0.1rem 0.375rem; border-radius: 3px; width: fit-content;
  }

  .paste-controls {
    display: flex; gap: 0.375rem; align-items: center; flex-wrap: wrap;
  }
  .paste-controls select {
    padding: 0.25rem 0.5rem; border: 1px solid var(--color-border);
    border-radius: var(--radius); font-size: 0.8125rem;
    background: var(--color-surface); color: var(--color-text);
  }
  .paste-value {
    padding: 0.25rem 0.5rem; border: 1px solid var(--color-border);
    border-radius: var(--radius); font-size: 0.8125rem;
    background: var(--color-surface); color: var(--color-text);
    min-width: 8rem; flex: 1;
  }

  .paste-preview-fixed {
    font-size: 0.75rem; color: var(--color-text-muted); font-style: italic;
  }
  .paste-preview-var {
    font-size: 0.75rem; color: var(--color-success); font-family: monospace;
    font-weight: 600;
  }
</style>
