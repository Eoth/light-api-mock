<script>
  // Mode "coller un exemple" pour XML (sujet portage JSON->XML). Miroir
  // fonctionnel de JsonPasteBuilder.svelte (paste -> exampleXmlToFields ->
  // assignation de source champ par champ, jamais de renommage/ajout/
  // suppression de structure ici -- pour ces retouches, repasser par le
  // mode "XML guide" habituel, cf docs/reponses-et-templates.md) mais avec
  // DEUX ajouts volontaires par rapport a la parite stricte JSON :
  //  - navigation par fil d'Ariane (focusPath) + chevrons de pliage
  //    (collapsedPaths, meme mecanisme `hidden` que XmlResponseBuilder.svelte
  //    -- jamais un {#if} qui demonterait le contenu, cf CLAUDE.md point 64) :
  //    un XML colle (typiquement une enveloppe SOAP) est generalement bien
  //    plus imbrique qu'un JSON REST plat, un rendu recursif entierement a
  //    plat (comme JsonPasteBuilder) serait illisible. Ces deux mecanismes
  //    sont repris tels quels de JsonResponseBuilder.svelte/
  //    XmlResponseBuilder.svelte plutot que reinventes.
  //  - edition des ATTRIBUTS XML (absents du modele JSON) : chaque noeud
  //    (racine incluse) peut porter des attributs, chacun avec sa propre
  //    source (fixe/variable), meme mecanisme de source que le contenu
  //    texte. Les noms d'attribut restent en lecture seule (comme les cles
  //    JSON en mode paste) -- seule la source de la valeur est editable ici.
  //
  // Namespaces XML : les prefixes ("soap:Envelope") et attributs xmlns/
  // xmlns:* sont preserves tels quels comme texte litteral (aucune
  // resolution semantique) -- cf tpl-utils.js::exampleXmlToFields pour le
  // detail de cette limite assumee.
  import { untrack } from 'svelte';
  import { buildExpr as sharedBuildExpr, xmlFieldsToTemplate, exampleXmlToFields } from '../tpl-utils.js';

  // startParsed/initialRootTag/initialRootAttributes : seedent l'etat a la
  // restauration d'une regle existante (fields/rootTag/rootAttributes deja
  // reconstruits par le parent depuis le template persiste via
  // templateToXmlFields, cf RuleResponseSection.svelte). Lus UNE SEULE FOIS
  // a la creation (untrack), memes principes que JsonPasteBuilder.svelte.
  let {
    fields = [],
    startParsed = false,
    rootTag: initialRootTag = 'response',
    rootAttributes: initialRootAttributes = [],
    onUpdate = () => {},
  } = $props();

  let pasteInput = $state('');
  let parseError = $state('');
  let parsed = $state(untrack(() => startParsed));
  let rootTag = $state(untrack(() => initialRootTag));
  let rootAttributes = $state(untrack(() => initialRootAttributes));

  const valueSources = [
    { value: 'fixed', label: 'Garder la valeur' },
    { value: 'path', label: 'Parametre URL' },
    { value: 'query', label: 'Query param' },
    { value: 'header', label: 'Header HTTP' },
    { value: 'body', label: 'Echo body (JSON pointer)' },
    { value: 'xpath', label: 'XPath (XML/SOAP)' },
    { value: 'fake', label: 'Donnee fictive' },
    { value: 'uuid', label: 'UUID' },
    { value: 'now_ms', label: 'Timestamp (ms)' },
    { value: 'now_iso', label: 'Date ISO' },
    { value: 'seq', label: 'Compteur' },
    { value: 'script', label: 'Resultat du script' },
  ];

  const fakeOptions = [
    'FirstName', 'LastName', 'Email', 'PhoneNumberFR', 'CompanyName',
    'StreetName', 'CityFR', 'PostcodeFR', 'Siren', 'Siret',
    'FullAddressFR', 'DatePast', 'DateFuture', 'TimestampMs',
    'BoolRandom', 'LoremSentence', 'CountryFR', 'IbanFR',
  ];

  // Pipes (retour beta-testeur, cf JsonPasteBuilder.svelte) : uniquement sur
  // le CONTENU d'un noeud valeur (memes options que XmlResponseBuilder.svelte
  // guide) -- pas sur les attributs, qui n'ont deja aucun equivalent dans le
  // mode guide (l'edition d'attributs est une capacite propre au mode
  // "par exemple", cf CLAUDE.md).
  const pipeOptions = [
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

  function deepClone(obj) { return JSON.parse(JSON.stringify(obj)); }

  function getByPath(root, path) {
    let current = root;
    for (const key of path) current = current[key];
    return current;
  }

  function getByPathSafe(root, path) {
    let current = root;
    for (const key of path) {
      if (current == null) return undefined;
      current = current[key];
    }
    return current;
  }

  // focusPath ne navigue que dans 'children' (pas d'equivalent 'items'/
  // 'template' comme JsonResponseBuilder : un noeud XML n'a qu'une seule
  // forme d'imbrication possible). Reset si le chemin ne pointe plus vers
  // rien de valide (meme garde que JsonResponseBuilder).
  let focusPath = $state([]);
  let collapsedPaths = $state(new Set());

  $effect(() => {
    if (focusPath.length > 0 && getByPathSafe(fields, focusPath) === undefined) {
      focusPath = [];
    }
  });

  let focusedFields = $derived(getByPathSafe(fields, focusPath) ?? []);

  function isCollapsed(testPath) { return collapsedPaths.has(testPath); }

  function toggleCollapse(testPath) {
    const next = new Set(collapsedPaths);
    if (next.has(testPath)) next.delete(testPath); else next.add(testPath);
    collapsedPaths = next;
  }

  function breadcrumbTrail(path) {
    const trail = [{ label: 'racine', path: [] }];
    let current = fields;
    for (let i = 0; i < path.length; i += 2) {
      const idx = path[i];
      const field = current?.[idx];
      if (!field) break;
      trail.push({ label: field.tag?.trim() || `#${idx + 1}`, path: path.slice(0, i + 2) });
      current = field.children;
    }
    return trail;
  }

  let breadcrumb = $derived(breadcrumbTrail(focusPath));

  function handleParse() {
    parseError = '';
    const text = pasteInput.trim();
    if (!text) { parseError = 'Collez un XML valide.'; return; }
    try {
      const result = exampleXmlToFields(text);
      rootTag = result.rootTag;
      rootAttributes = result.rootAttributes;
      fields = result.fields;
      focusPath = [];
      collapsedPaths = new Set();
      parsed = true;
      emit();
    } catch (e) {
      parseError = e.message;
    }
  }

  function resetPaste() {
    parsed = false;
    pasteInput = '';
    parseError = '';
    focusPath = [];
    collapsedPaths = new Set();
  }

  function mutate(fn) {
    const clone = deepClone(fields);
    fn(clone);
    fields = clone;
    emit();
  }

  function applySourceDefaults(node, prop, val) {
    if (prop === 'source' && val === 'fake') node.value = 'CompanyName';
    if (prop === 'source' && ['uuid', 'now_ms', 'now_iso', 'seq'].includes(val)) node.value = '';
  }

  function updateNodeProp(path, idx, prop, val) {
    mutate(root => {
      const arr = getByPath(root, path);
      const node = { ...arr[idx], [prop]: val };
      applySourceDefaults(node, prop, val);
      arr[idx] = node;
    });
  }

  function updateAttrProp(path, idx, attrIdx, prop, val) {
    mutate(root => {
      const arr = getByPath(root, path);
      const node = { ...arr[idx] };
      const attrs = [...(node.attributes || [])];
      const attr = { ...attrs[attrIdx], [prop]: val };
      applySourceDefaults(attr, prop, val);
      attrs[attrIdx] = attr;
      node.attributes = attrs;
      arr[idx] = node;
    });
  }

  function updateRootAttrProp(attrIdx, prop, val) {
    const clone = deepClone(rootAttributes);
    const attr = { ...clone[attrIdx], [prop]: val };
    applySourceDefaults(attr, prop, val);
    clone[attrIdx] = attr;
    rootAttributes = clone;
    emit();
  }

  function emit() { onUpdate(fields); }

  function needsValueInput(src) {
    return ['fixed', 'path', 'query', 'header', 'body', 'xpath', 'script'].includes(src);
  }

  function valuePlaceholder(src) {
    if (src === 'fixed') return 'valeur fixe';
    if (src === 'xpath') return 'ex: Envelope/Body/recherche/Siret';
    return 'nom du parametre';
  }

  function buildExpr(f) { return sharedBuildExpr(f); }

  export function toTemplate() {
    return xmlFieldsToTemplate(fields, rootTag, rootAttributes);
  }
</script>

<div class="paste-builder" aria-label="Constructeur XML par exemple">
  {#if !parsed}
    <div class="paste-zone">
      <label for="xml-paste-input">Collez un exemple de reponse XML</label>
      <textarea
        id="xml-paste-input"
        bind:value={pasteInput}
        rows="6"
        class="paste-textarea"
        placeholder={'<response>\n  <siret>44306184100047</siret>\n  <nom>ACME Corp</nom>\n</response>'}
        data-testid="xml-paste-builder-textarea"
      ></textarea>
      {#if parseError}
        <div class="form-error" role="alert" data-testid="xml-paste-builder-error">{parseError}</div>
      {/if}
      <button type="button" class="btn btn-primary btn-sm" onclick={handleParse} data-testid="xml-paste-builder-analyze-button">
        Analyser et variabiliser
      </button>
    </div>
  {:else}
    <div class="paste-header">
      <span class="field-hint">Racine <code>&lt;{rootTag}&gt;</code>, {fields.length} noeud{fields.length !== 1 ? 's' : ''} racine detecte{fields.length !== 1 ? 's' : ''} — choisissez la source de chaque valeur</span>
      <button type="button" class="btn btn-outline btn-sm" onclick={resetPaste} data-testid="xml-paste-builder-reset-button">
        Recoller un XML
      </button>
    </div>

    {#snippet renderAttributes(attributes, testPathPrefix, onAttrUpdate)}
      {#if attributes && attributes.length > 0}
        <div class="paste-attrs">
          <span class="paste-attrs-label">Attributs :</span>
          {#each attributes as attr, aidx}
            <div class="paste-attr-row">
              <span class="paste-attr-name">@{attr.name}</span>
              <select
                value={attr.source}
                onchange={(e) => onAttrUpdate(aidx, 'source', e.target.value)}
                aria-label="Source pour l'attribut {attr.name} ({testPathPrefix})"
                data-testid="xml-paste-builder-attr-source-select-{testPathPrefix}-{aidx}"
              >
                {#each valueSources as vs}<option value={vs.value}>{vs.label}</option>{/each}
              </select>
              {#if attr.source === 'fake'}
                <select
                  value={attr.value}
                  onchange={(e) => onAttrUpdate(aidx, 'value', e.target.value)}
                  aria-label="Type fictif pour l'attribut {attr.name} ({testPathPrefix})"
                  data-testid="xml-paste-builder-attr-fake-select-{testPathPrefix}-{aidx}"
                >
                  {#each fakeOptions as fo}<option value={fo}>{fo}</option>{/each}
                </select>
              {:else if needsValueInput(attr.source)}
                <input
                  type="text"
                  class="paste-value"
                  value={attr.value}
                  oninput={(e) => onAttrUpdate(aidx, 'value', e.target.value)}
                  placeholder={valuePlaceholder(attr.source)}
                  aria-label="Valeur pour l'attribut {attr.name} ({testPathPrefix})"
                  data-testid="xml-paste-builder-attr-value-input-{testPathPrefix}-{aidx}"
                />
              {/if}
              {#if attr.source === 'fixed'}
                <span class="paste-preview-fixed">{attr.value}</span>
              {:else}
                <span class="paste-preview-var">{buildExpr(attr)}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/snippet}

    {@render renderAttributes(rootAttributes, 'root', (aidx, prop, val) => updateRootAttrProp(aidx, prop, val))}

    {#if focusPath.length > 0}
      <nav class="data-breadcrumb" aria-label="Chemin des donnees">
        <ol>
          {#each breadcrumb as segment, i}
            <li aria-current={i === breadcrumb.length - 1 ? 'page' : undefined}>
              {#if i === breadcrumb.length - 1}
                <span>{segment.label}</span>
              {:else}
                <button type="button" class="breadcrumb-link" onclick={() => focusPath = segment.path} data-testid="xml-paste-builder-breadcrumb-link-{i}">{segment.label}</button>
              {/if}
            </li>
          {/each}
        </ol>
      </nav>
    {/if}

    {#snippet renderNodes(nodeList, path, depth)}
      {#each nodeList as field, idx}
        {@const nt = field.nodeType || 'value'}
        {@const testPath = [...path, idx].join('-')}
        {@const hasChildren = nt === 'parent'}
        {@const collapsed = hasChildren && isCollapsed(testPath)}
        <div class="paste-field" style:margin-left="{depth * 1.25}rem">
          <div class="paste-field-main">
            {#if hasChildren}
              <button
                type="button"
                class="btn-icon collapse-toggle"
                onclick={() => toggleCollapse(testPath)}
                aria-expanded={!collapsed}
                aria-controls="xml-paste-builder-children-{testPath}"
                aria-label={collapsed ? `Deplier ${field.tag || 'ce noeud'}` : `Replier ${field.tag || 'ce noeud'}`}
                title={collapsed ? 'Deplier' : 'Replier'}
                data-testid="xml-paste-builder-collapse-button-{testPath}"
              >{collapsed ? '▶' : '▼'}</button>
            {/if}
            <span class="paste-key">{field.tag}</span>
            {#if hasChildren}
              <span class="paste-type-badge">noeud parent ({(field.children || []).length})</span>
              {#if collapsed}
                <span class="collapsed-indicator" data-testid="xml-paste-builder-collapsed-indicator-{testPath}">({(field.children || []).length} masque{(field.children || []).length > 1 ? 's' : ''})</span>
              {/if}
              <button
                type="button"
                class="btn-icon"
                onclick={() => focusPath = [...path, idx, 'children']}
                aria-label="Naviguer dans {field.tag || 'ce noeud'}"
                title="Naviguer dans ce noeud"
                data-testid="xml-paste-builder-navigate-button-{testPath}"
              >&#8594;</button>
            {:else}
              <div class="paste-controls">
                <select value={field.source} onchange={(e) => updateNodeProp(path, idx, 'source', e.target.value)} aria-label="Source pour {field.tag}" data-testid="xml-paste-builder-source-select-{testPath}">
                  {#each valueSources as vs}<option value={vs.value}>{vs.label}</option>{/each}
                </select>
                {#if field.source === 'fake'}
                  <select value={field.value} onchange={(e) => updateNodeProp(path, idx, 'value', e.target.value)} aria-label="Type fictif pour {field.tag}" data-testid="xml-paste-builder-fake-select-{testPath}">
                    {#each fakeOptions as fo}<option value={fo}>{fo}</option>{/each}
                  </select>
                {:else if needsValueInput(field.source)}
                  <input
                    type="text"
                    class="paste-value"
                    value={field.value}
                    oninput={(e) => updateNodeProp(path, idx, 'value', e.target.value)}
                    placeholder={valuePlaceholder(field.source)}
                    aria-label="Valeur pour {field.tag}"
                    data-testid="xml-paste-builder-value-input-{testPath}"
                  />
                {/if}
                {#if field.source !== 'fixed'}
                  <input
                    type="text"
                    class="pipe-input"
                    value={field.pipe || ''}
                    oninput={(e) => updateNodeProp(path, idx, 'pipe', e.target.value)}
                    placeholder="ex: lower | first(5)"
                    aria-label="Pipe de transformation pour {field.tag}"
                    list="dl-xml-paste-pipes"
                    autocomplete="off"
                    data-testid="xml-paste-builder-pipe-input-{testPath}"
                  />
                {/if}
                {#if field.source === 'fixed'}
                  <span class="paste-preview-fixed">{field.value}</span>
                {:else}
                  <span class="paste-preview-var">{buildExpr(field)}</span>
                {/if}
              </div>
            {/if}
          </div>

          {@render renderAttributes(field.attributes, testPath, (aidx, prop, val) => updateAttrProp(path, idx, aidx, prop, val))}

          {#if hasChildren}
            <div class="nested-block" id="xml-paste-builder-children-{testPath}" hidden={collapsed}>
              {@render renderNodes(field.children || [], [...path, idx, 'children'], depth + 1)}
            </div>
          {/if}
        </div>
      {/each}
    {/snippet}

    {@render renderNodes(focusedFields, focusPath, 0)}

    <datalist id="dl-xml-paste-pipes">
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

  .paste-attrs {
    display: flex; flex-direction: column; gap: 0.25rem;
    margin: 0.25rem 0 0.375rem; padding: 0.375rem 0.5rem;
    background: var(--color-bg); border-radius: var(--radius);
    border: 1px dashed var(--color-border);
  }
  .paste-attrs-label { font-size: 0.6875rem; font-weight: 600; color: var(--color-text-muted); text-transform: uppercase; }
  .paste-attr-row { display: flex; gap: 0.375rem; align-items: center; flex-wrap: wrap; }
  .paste-attr-name { font-family: monospace; font-size: 0.8125rem; color: var(--color-text-muted); }
  .paste-attr-row select { padding: 0.25rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; background: var(--color-surface); color: var(--color-text); }

  .collapse-toggle { flex-shrink: 0; }
  .btn-icon { width: 1.5rem; height: 1.5rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.7rem; cursor: pointer; }
  .btn-icon:hover { background: var(--color-bg); color: var(--color-text); }
  .collapsed-indicator { font-size: 0.75rem; color: var(--color-text-muted); font-style: italic; white-space: nowrap; }

  .nested-block { margin-top: 0.25rem; padding-left: 0.75rem; border-left: 2px solid var(--color-primary); }

  .data-breadcrumb { margin: 0.25rem 0; }
  .data-breadcrumb ol { list-style: none; display: flex; align-items: center; gap: 0.375rem; flex-wrap: wrap; margin: 0; padding: 0; font-size: 0.8125rem; }
  .data-breadcrumb li { display: flex; align-items: center; gap: 0.375rem; color: var(--color-text-muted); }
  .data-breadcrumb li:not(:last-child)::after { content: ">"; color: var(--color-border); }
  .data-breadcrumb li[aria-current="page"] { color: var(--color-text); font-weight: 600; }
  .breadcrumb-link { background: none; border: none; padding: 0; color: var(--color-primary); cursor: pointer; font: inherit; text-decoration: underline; text-underline-offset: 2px; }
  .breadcrumb-link:hover { color: var(--color-primary-hover); }
</style>
