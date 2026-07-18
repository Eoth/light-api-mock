<script>
  // Mode "coller un exemple" JSON : edite uniquement les VALEURS (source/
  // pipe) des champs deja detectes par l'analyse de l'exemple colle -- ni
  // renommage de cle, ni ajout/suppression/reordonnancement, ni changement
  // de type (contrairement au mode guide, JsonResponseBuilder.svelte, qui
  // partage la MEME forme de Fields mais permet l'edition complete). Voir
  // CLAUDE.md, "Fusion Format x Assiste/Detail" pour la relation entre les
  // deux : RuleResponseSection.svelte affiche un bouton "Modifier en detail"
  // qui reutilise TEL QUEL le tableau `fields` de ce composant pour ouvrir
  // JsonResponseBuilder, transition sans perte puisque la structure de
  // donnees est identique (verifie en etape 0 de ce sujet).
  import { untrack } from 'svelte';
  import { buildExpr as sharedBuildExpr, fieldsToTemplate, exampleJsonToFields } from '../tpl-utils.js';

  // startParsed : seede l'etat initial `parsed` a la restauration d'une
  // regle existante (fields deja peuple par le parent depuis le template
  // persiste, cf RuleResponseSection.svelte::computeInitialEditorState) --
  // saute la zone de collage et affiche directement la liste de champs. Lu
  // UNE SEULE FOIS a la creation du composant (untrack, meme idiome que
  // RuleForm.svelte::init) : les changements ulterieurs de cette prop ne
  // doivent pas rouvrir/refermer la zone de collage a l'insu de l'utilisateur.
  let { fields = [], startParsed = false, onUpdate = () => {} } = $props();

  let pasteInput = $state('');
  let parseError = $state('');
  let parsed = $state(untrack(() => startParsed));
  let isArrayRoot = $state(false);

  // Pliage/depliage des champs 'object' (seul type imbrique reellement
  // rendu par renderFields ci-dessous -- array-values/array-objects
  // n'affichent qu'un badge "tableau" sans recursion, limitation
  // preexistante et non liee a ce correctif). Meme mecanisme que
  // XmlPasteBuilder.svelte/JsonResponseBuilder.svelte (Set en memoire, cle
  // par testPath positionnel, tout deplie par defaut, attribut `hidden`
  // jamais un {#if} -- cf CLAUDE.md point 64) : ce mode "par exemple" en
  // avait ete prive par oubli lors du sujet 25 (qui n'avait touche que les
  // vues detail JsonResponseBuilder/XmlResponseBuilder), alors que son
  // equivalent XML (XmlPasteBuilder, sujet 27) l'a des l'origine.
  let collapsedPaths = $state(new Set());

  function isCollapsed(testPath) { return collapsedPaths.has(testPath); }

  function toggleCollapse(testPath) {
    const next = new Set(collapsedPaths);
    if (next.has(testPath)) next.delete(testPath); else next.add(testPath);
    collapsedPaths = next;
  }

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

  // Pipes (retour beta-testeur : presents en mode guide, absents ici alors
  // que ce mode est juge excellent par ailleurs) -- memes options que
  // JsonResponseBuilder.svelte, dans la limite du raisonnable : uniquement
  // la liste deja existante, aucune nouvelle transformation inventee pour ce
  // sujet.
  const pipeOptions = [
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

  function handleParse() {
    parseError = '';
    const text = pasteInput.trim();
    if (!text) { parseError = 'Collez un JSON valide.'; return; }
    try {
      const data = JSON.parse(text);
      if (Array.isArray(data)) {
        if (data.length === 0) {
          parseError = 'Le tableau est vide. Collez un tableau avec au moins un element.';
          return;
        }
        isArrayRoot = true;
        fields = exampleJsonToFields(typeof data[0] === 'object' && data[0] !== null ? data[0] : { value: data[0] });
      } else if (typeof data === 'object' && data !== null) {
        isArrayRoot = false;
        fields = exampleJsonToFields(data);
      } else {
        parseError = 'Le JSON doit etre un objet ou un tableau.';
        return;
      }
      parsed = true;
      emit();
    } catch (e) {
      parseError = `JSON invalide : ${e.message}`;
    }
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
    const obj = fieldsToTemplate(fields);
    return isArrayRoot ? `[${obj}]` : obj;
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
        data-testid="json-paste-builder-textarea"
      ></textarea>
      {#if parseError}
        <div class="form-error" role="alert" data-testid="json-paste-builder-error">{parseError}</div>
      {/if}
      <button type="button" class="btn btn-primary btn-sm" onclick={handleParse} data-testid="json-paste-builder-analyze-button">
        Analyser et variabiliser
      </button>
    </div>
  {:else}
    <div class="paste-header">
      <span class="field-hint">{isArrayRoot ? 'Tableau de ' : ''}{fields.length} champ{fields.length !== 1 ? 's' : ''} detecte{fields.length !== 1 ? 's' : ''} — choisissez la source de chaque valeur</span>
      <button type="button" class="btn btn-outline btn-sm" onclick={() => { parsed = false; pasteInput = ''; }} data-testid="json-paste-builder-reset-button">
        Recoller un JSON
      </button>
    </div>

    {#snippet renderFields(fieldList, path, depth)}
      {#each fieldList as field, idx}
        {@const currentPath = [...path, idx]}
        {@const testPath = currentPath.join('-')}
        {@const isObject = field.fieldType === 'object'}
        {@const collapsed = isObject && isCollapsed(testPath)}
        <div class="paste-field" style:margin-left="{depth * 1.25}rem">
          <div class="paste-field-main">
            {#if isObject}
              <button
                type="button"
                class="btn-icon collapse-toggle"
                onclick={() => toggleCollapse(testPath)}
                aria-expanded={!collapsed}
                aria-controls="json-paste-builder-children-{testPath}"
                aria-label={collapsed ? `Deplier ${field.key || 'ce champ'}` : `Replier ${field.key || 'ce champ'}`}
                title={collapsed ? 'Deplier' : 'Replier'}
                data-testid="json-paste-builder-collapse-button-{testPath}"
              >{collapsed ? '▶' : '▼'}</button>
            {/if}
            <span class="paste-key">{field.key}</span>

            {#if isObject}
              <span class="paste-type-badge">objet</span>
              {#if collapsed}
                <span class="collapsed-indicator" data-testid="json-paste-builder-collapsed-indicator-{testPath}">({(field.children || []).length} masque{(field.children || []).length > 1 ? 's' : ''})</span>
              {/if}
            {/if}
          </div>

          {#if isObject}
            <div class="nested-block" id="json-paste-builder-children-{testPath}" hidden={collapsed}>
              {@render renderFields(field.children, [...currentPath, 'children'], depth + 1)}
            </div>
          {:else if field.fieldType === 'array-values' || field.fieldType === 'array-objects'}
            <span class="paste-type-badge">tableau</span>
          {:else}
            <div class="paste-controls">
              <select value={field.source} onchange={(e) => updateField(currentPath, 'source', e.target.value)} aria-label="Source pour {field.key}" data-testid="json-paste-builder-source-select-{currentPath.join('-')}">
                {#each valueSources as vs}
                  <option value={vs.value}>{vs.label}</option>
                {/each}
              </select>

              {#if field.source === 'fake'}
                <select value={field.value} onchange={(e) => updateField(currentPath, 'value', e.target.value)} aria-label="Type fictif" data-testid="json-paste-builder-fake-select-{currentPath.join('-')}">
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
                  data-testid="json-paste-builder-value-input-{currentPath.join('-')}"
                />
              {/if}

              {#if field.source !== 'fixed'}
                <input
                  type="text"
                  class="pipe-input"
                  value={field.pipe || ''}
                  oninput={(e) => updateField(currentPath, 'pipe', e.target.value)}
                  placeholder="ex: first(9) | upper"
                  aria-label="Pipe de transformation pour {field.key}"
                  list="dl-paste-pipes"
                  autocomplete="off"
                  data-testid="json-paste-builder-pipe-input-{currentPath.join('-')}"
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

    <datalist id="dl-paste-pipes">
      {#each pipeOptions as p}<option value={p.value}>{p.label}</option>{/each}
    </datalist>
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
    padding: 0.375rem 0; border-bottom: 1px solid var(--color-border);
  }
  .paste-field:last-child { border-bottom: none; }

  .paste-field-main { display: flex; flex-direction: column; gap: 0.25rem; align-items: flex-start; }

  .paste-key {
    font-weight: 700; font-size: 0.875rem; color: var(--color-primary);
    font-family: monospace;
  }

  .paste-type-badge {
    display: inline-block; font-size: 0.6875rem; font-weight: 600;
    color: var(--color-text-muted); background: var(--color-bg);
    padding: 0.1rem 0.375rem; border-radius: 3px; width: fit-content;
  }

  .collapse-toggle { flex-shrink: 0; }
  .btn-icon { width: 1.5rem; height: 1.5rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.7rem; cursor: pointer; }
  .btn-icon:hover { background: var(--color-bg); color: var(--color-text); }
  .collapsed-indicator { font-size: 0.75rem; color: var(--color-text-muted); font-style: italic; white-space: nowrap; }

  .nested-block { margin-top: 0.25rem; padding-left: 0.75rem; border-left: 2px solid var(--color-primary); }

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
  .pipe-input {
    min-width: 8rem; max-width: 14rem; padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border); border-radius: var(--radius);
    font-size: 0.75rem; font-family: 'Cascadia Code', 'Fira Code', monospace;
    color: var(--color-primary);
  }

  .paste-preview-fixed {
    font-size: 0.75rem; color: var(--color-text-muted); font-style: italic;
  }
  .paste-preview-var {
    font-size: 0.75rem; color: var(--color-success); font-family: monospace;
    font-weight: 600;
  }
</style>
