<script>
  import ConditionForm from './ConditionForm.svelte';
  import JsonResponseBuilder from './JsonResponseBuilder.svelte';
  import XmlResponseBuilder from './XmlResponseBuilder.svelte';
  import ToggleSwitch from './ToggleSwitch.svelte';
  import { templateToTestJson, templateToFields, validateTemplateAsJson, validateTemplateAsXml, fieldsToTemplate, varNameToSource } from '../tpl-utils.js';

  import { untrack } from 'svelte';

  let { rule = null, existingRuleNames = [], onSave = () => {}, onCancel = () => {} } = $props();

  const init = untrack(() => rule ? JSON.parse(JSON.stringify(rule)) : null);
  let name = $state(init?.name ?? '');
  let ruleMethod = $state(init?.method ?? 'GET');
  let subPath = $state(init?.sub_path ?? '');
  let ruleAction = $state(init?.action ?? 'mock');
  let scriptEnabled = $state(!!init?.script);
  let scriptCode = $state(init?.script ?? '');

  const httpMethods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS', 'HEAD'];
  let allOf = $state(init?.conditions?.all_of ?? []);
  let anyOf = $state(init?.conditions?.any_of ?? []);
  let addingConditionTo = $state(null);

  let status = $state(init?.response?.status ?? 200);
  let respHeaders = $state(init?.response?.headers ?? []);
  let fragments = $state(init?.response?.body ?? [{ type: 'Literal', value: '' }]);
  let chaosEnabled = $state(!!init?.response?.chaos);
  let chaos = $state(init?.response?.chaos ?? { delay_ms: 0, delay_min_ms: null, delay_max_ms: null, error_rate: 0, error_status: 500 });

  let responseOpen = $state(true);
  let formError = $state('');

  function detectMode() {
    if (!init?.response?.body?.length) return 'json-guided';
    if (init.response.body.length === 1 && init.response.body[0].type === 'Template') return 'advanced';
    if (init.response.status === 204) return 'empty';
    return 'advanced';
  }
  let responseMode = $state(detectMode());

  let jsonFields = $state([]);
  let jsonBuilderRef = $state(null);
  let xmlFields = $state([]);
  let xmlBuilderRef = $state(null);
  let textContent = $state('');

  function buildFragmentsFromMode() {
    if (responseMode === 'empty') return [];
    if (responseMode === 'json-guided' && jsonBuilderRef) {
      return [{ type: 'Template', template: jsonBuilderRef.toTemplate() }];
    }
    if (responseMode === 'xml-guided' && xmlBuilderRef) {
      return [{ type: 'Template', template: xmlBuilderRef.toTemplate() }];
    }
    if (responseMode === 'text') {
      return [{ type: 'Literal', value: textContent }];
    }
    return fragments;
  }

  const fragmentTypes = [
    { value: 'Template', label: 'Template (expressions)' },
    { value: 'Literal', label: 'Texte fixe' },
    { value: 'Uuid', label: 'UUID v4' },
    { value: 'PickFrom', label: 'Choix aleatoire' },
    { value: 'FakeData', label: 'Donnee fictive' },
    { value: 'PathSegment', label: 'Segment URL (index)' },
  ];

  const fakeKinds = [
    { value: 'FirstName', label: 'Prenom' },
    { value: 'LastName', label: 'Nom de famille' },
    { value: 'Email', label: 'Adresse email' },
    { value: 'PhoneNumberFR', label: 'Telephone FR' },
    { value: 'Integer', label: 'Nombre entier' },
    { value: 'CompanyName', label: 'Nom d\'entreprise' },
    { value: 'StreetName', label: 'Nom de rue' },
    { value: 'CityFR', label: 'Ville (FR)' },
    { value: 'PostcodeFR', label: 'Code postal (FR)' },
    { value: 'Siren', label: 'SIREN (9 chiffres)' },
    { value: 'Siret', label: 'SIRET (14 chiffres)' },
    { value: 'FullAddressFR', label: 'Adresse complete (FR)' },
    { value: 'DatePast', label: 'Date passee' },
    { value: 'DateFuture', label: 'Date future' },
    { value: 'TimestampMs', label: 'Timestamp (ms)' },
    { value: 'BoolRandom', label: 'Booleen aleatoire' },
    { value: 'LoremSentence', label: 'Phrase Lorem Ipsum' },
    { value: 'CountryFR', label: 'Pays francophone' },
    { value: 'IbanFR', label: 'IBAN francais' },
  ];

  function handleSubmit(e) {
    e.preventDefault();
    formError = '';

    const trimmedName = name.trim();
    if (!trimmedName) { formError = 'Le nom de la regle est requis.'; return; }
    if (existingRuleNames.some(n => n.toLowerCase() === trimmedName.toLowerCase())) {
      formError = `Une regle avec le nom "${trimmedName}" existe deja dans ce service.`;
      return;
    }

    if (ruleAction === 'mock') {
      const validationErr = validateResponseContent();
      if (validationErr) { formError = validationErr; return; }
    }

    const finalStatus = responseMode === 'empty' ? 204 : status;
    const finalHeaders = responseMode === 'empty' ? [] : respHeaders.filter(h => h.name.trim());
    if (responseMode === 'json-guided' && !finalHeaders.some(h => h.name.toLowerCase() === 'content-type')) {
      finalHeaders.push({ name: 'Content-Type', value: 'application/json' });
    }
    if (responseMode === 'xml-guided' && !finalHeaders.some(h => h.name.toLowerCase() === 'content-type')) {
      finalHeaders.push({ name: 'Content-Type', value: 'application/xml' });
    }
    const finalBody = buildFragmentsFromMode();
    onSave({
      name: name.trim(),
      method: ruleMethod,
      sub_path: subPath.trim() || null,
      action: ruleAction,
      script: scriptEnabled && scriptCode.trim() ? scriptCode.trim() : null,
      conditions: { all_of: allOf, any_of: anyOf },
      response: {
        status: finalStatus,
        headers: finalHeaders,
        body: finalBody,
        chaos: chaosEnabled ? chaos : null,
      },
    });
  }

  function addCondition(group, condition) {
    if (group === 'all_of') allOf = [...allOf, condition];
    else anyOf = [...anyOf, condition];
    addingConditionTo = null;
  }

  function removeCondition(group, idx) {
    if (group === 'all_of') allOf = allOf.filter((_, i) => i !== idx);
    else anyOf = anyOf.filter((_, i) => i !== idx);
  }

  function conditionLabel(c) {
    const src = c.source.type === 'BodyRaw' ? 'Corps brut' : `${c.source.type}(${c.source.key})`;
    const op = c.operator.type === 'Exists' ? 'existe' : `${c.operator.type}(${c.operator.value})`;
    return `${src} ${op}`;
  }

  let pendingMode = $state(null);
  let modeKey = $state(0);

  function requestModeSwitch(newMode) {
    if (newMode === responseMode) return;
    const hasContent = currentModeHasContent();
    if (!hasContent) {
      applyModeSwitch(newMode);
      return;
    }
    const convResult = tryConvert(responseMode, newMode);
    if (convResult.ok) {
      applyModeSwitch(newMode, convResult);
      return;
    }
    pendingMode = newMode;
    pendingConvMessage = convResult.reason || '';
    modeKey++;
  }

  let pendingConvMessage = $state('');

  function confirmModeSwitch() {
    if (pendingMode) {
      applyModeSwitch(pendingMode);
      pendingMode = null;
      pendingConvMessage = '';
    }
  }

  function cancelModeSwitch() {
    pendingMode = null;
    pendingConvMessage = '';
    modeKey++;
  }

  function applyModeSwitch(newMode, convResult) {
    if (convResult?.jsonFields) jsonFields = convResult.jsonFields;
    if (convResult?.xmlFields) xmlFields = convResult.xmlFields;
    if (convResult?.textContent !== undefined) textContent = convResult.textContent;
    if (convResult?.fragments) fragments = convResult.fragments;
    responseMode = newMode;
    modeKey++;
  }

  function currentModeHasContent() {
    if (responseMode === 'json-guided') return jsonFields.length > 0;
    if (responseMode === 'xml-guided') return xmlFields.length > 0;
    if (responseMode === 'text') return textContent.trim().length > 0;
    if (responseMode === 'advanced') return fragments.some(f => {
      if (f.type === 'Template') return f.template?.trim();
      if (f.type === 'Literal') return f.value?.trim();
      return true;
    });
    return false;
  }

  function getAdvancedTemplate() {
    return fragments.map(f => {
      if (f.type === 'Literal') return f.value ?? '';
      if (f.type === 'Template') return f.template ?? '';
      return '';
    }).join('');
  }

  function tryConvert(from, to) {
    if (from === 'advanced' && to === 'text') {
      return { ok: true, textContent: getAdvancedTemplate() };
    }
    if (from === 'text' && to === 'advanced') {
      return { ok: true, fragments: [{ type: 'Literal', value: textContent }] };
    }
    if (from === 'advanced' && to === 'json-guided') {
      return tryAdvancedToJsonGuided();
    }
    if (from === 'advanced' && to === 'xml-guided') {
      return tryAdvancedToXmlGuided();
    }
    if (from === 'json-guided' && to === 'advanced') {
      if (jsonBuilderRef) {
        return { ok: true, fragments: [{ type: 'Template', template: jsonBuilderRef.toTemplate() }] };
      }
      return { ok: true };
    }
    if (from === 'xml-guided' && to === 'advanced') {
      if (xmlBuilderRef) {
        return { ok: true, fragments: [{ type: 'Template', template: xmlBuilderRef.toTemplate() }] };
      }
      return { ok: true };
    }
    if (from === 'json-guided' && to === 'xml-guided') {
      return tryJsonGuidedToXmlGuided();
    }
    if (from === 'xml-guided' && to === 'json-guided') {
      return { ok: false, reason: 'La conversion XML vers JSON guide n\'est pas supportee. Passez par le mode template avance comme intermediaire.' };
    }
    return { ok: false };
  }

  function tryAdvancedToJsonGuided() {
    const tpl = getAdvancedTemplate();
    if (!tpl.trim()) return { ok: true, jsonFields: [] };
    const jsonErr = validateTemplateAsJson(tpl);
    if (jsonErr) {
      return { ok: false, reason: `Conversion impossible : ${jsonErr}. Verifiez les accolades ({{ pour JSON literal, { pour variable).` };
    }
    try {
      const fields = templateToFields(tpl);
      return { ok: true, jsonFields: fields };
    } catch (e) {
      return { ok: false, reason: `Conversion impossible : ${e.message}` };
    }
  }

  function tryAdvancedToXmlGuided() {
    const tpl = getAdvancedTemplate();
    if (!tpl.trim()) return { ok: true, xmlFields: [] };
    const xmlErr = validateTemplateAsXml(tpl);
    if (xmlErr) {
      return { ok: false, reason: `Conversion impossible : ${xmlErr}` };
    }
    return { ok: false, reason: 'La conversion automatique XML template vers XML guide n\'est pas encore supportee. Utilisez la vue guidee pour reconstruire la structure.' };
  }

  function tryJsonGuidedToXmlGuided() {
    if (!jsonFields.length) return { ok: true, xmlFields: [] };
    try {
      const xmlF = jsonFields.filter(f => f.key?.trim()).map(f => jsonFieldToXmlNode(f));
      return { ok: true, xmlFields: xmlF };
    } catch {
      return { ok: false, reason: 'La structure JSON contient des elements incompatibles avec XML (tableaux de valeurs scalaires).' };
    }
  }

  function jsonFieldToXmlNode(f) {
    const ft = f.fieldType || 'value';
    if (ft === 'object') {
      return { tag: f.key, nodeType: 'parent', children: (f.children || []).filter(c => c.key?.trim()).map(c => jsonFieldToXmlNode(c)) };
    }
    if (ft === 'array-objects') {
      return { tag: f.key, nodeType: 'parent', children: (f.template || []).filter(c => c.key?.trim()).map(c => jsonFieldToXmlNode(c)) };
    }
    if (ft === 'array-values') {
      throw new Error('incompatible');
    }
    return { tag: f.key, nodeType: 'value', source: f.source || 'fixed', value: f.value || '' };
  }

  function validateResponseContent() {
    if (responseMode === 'json-guided' && jsonBuilderRef) {
      const err = validateTemplateAsJson(jsonBuilderRef.toTemplate());
      if (err) return `JSON guide invalide : ${err}`;
    }
    if (responseMode === 'xml-guided' && xmlBuilderRef) {
      const err = validateTemplateAsXml(xmlBuilderRef.toTemplate());
      if (err) return err;
    }
    if (responseMode === 'advanced') {
      const tpl = getAdvancedTemplate();
      const ct = respHeaders.find(h => h.name?.toLowerCase() === 'content-type')?.value?.toLowerCase() || '';
      if (ct.includes('json') && tpl.trim()) {
        const err = validateTemplateAsJson(tpl);
        if (err) return `Content-Type JSON mais template invalide : ${err}`;
      }
      if (ct.includes('xml') && tpl.trim()) {
        const xmlErr = validateTemplateAsXml(tpl);
        if (xmlErr) return `Content-Type XML mais template invalide : ${xmlErr}`;
      }
    }
    return null;
  }

  function addFragment() { fragments = [...fragments, { type: 'Literal', value: '' }]; }
  function removeFragment(idx) { fragments = fragments.filter((_, i) => i !== idx); }
  function moveFragment(idx, dir) {
    const t = idx + dir;
    if (t < 0 || t >= fragments.length) return;
    const c = [...fragments]; [c[idx], c[t]] = [c[t], c[idx]]; fragments = c;
  }
  function updateFragmentType(idx, newType) {
    const c = [...fragments];
    if (newType === 'Literal') c[idx] = { type: 'Literal', value: '' };
    else if (newType === 'Uuid') c[idx] = { type: 'Uuid' };
    else if (newType === 'PickFrom') c[idx] = { type: 'PickFrom', values: [''] };
    else if (newType === 'FakeData') c[idx] = { type: 'FakeData', kind: { type: 'FirstName' } };
    else if (newType === 'PathSegment') c[idx] = { type: 'PathSegment', index: 0 };
    else if (newType === 'Template') c[idx] = { type: 'Template', template: '' };
    fragments = c;
  }
  function updateFakeKind(idx, kindType) {
    const c = [...fragments];
    c[idx] = kindType === 'Integer'
      ? { type: 'FakeData', kind: { type: 'Integer', min: 0, max: 100 } }
      : { type: 'FakeData', kind: { type: kindType } };
    fragments = c;
  }
  function addPickValue(idx) {
    const c = [...fragments]; c[idx] = { ...c[idx], values: [...c[idx].values, ''] }; fragments = c;
  }
  function removePickValue(fi, vi) {
    const c = [...fragments]; c[fi] = { ...c[fi], values: c[fi].values.filter((_, i) => i !== vi) }; fragments = c;
  }
  const commonHeaders = [
    'Content-Type', 'Accept', 'Authorization', 'Cache-Control',
    'X-Request-Id', 'X-Correlation-Id', 'X-Forwarded-For',
    'Access-Control-Allow-Origin', 'Access-Control-Allow-Methods',
  ];

  const commonContentTypes = [
    'application/json', 'application/xml', 'text/plain', 'text/html',
    'application/x-www-form-urlencoded', 'multipart/form-data',
    'application/octet-stream', 'application/pdf',
  ];

  function addHeader() { respHeaders = [...respHeaders, { name: '', value: '' }]; }
  function removeHeader(idx) { respHeaders = respHeaders.filter((_, i) => i !== idx); }
</script>

<form class="rule-form" onsubmit={handleSubmit} aria-label={init ? `Modifier la regle ${init.name}` : 'Nouvelle regle'}>

  {#if formError}
    <div class="form-error" role="alert" aria-live="assertive">{formError}</div>
  {/if}

  <div class="form-field">
    <label for="rule-name">Nom de la regle</label>
    <input id="rule-name" type="text" bind:value={name} required placeholder="ex: get-siret" aria-describedby="rn-hint" />
    <span class="field-hint" id="rn-hint">Identifiant unique de cette regle dans le service</span>
  </div>

  <div class="form-row">
    <div class="form-field">
      <label for="rule-method">Methode HTTP</label>
      <select id="rule-method" bind:value={ruleMethod} aria-describedby="rule-method-hint">
        {#each httpMethods as m}
          <option value={m}>{m}</option>
        {/each}
      </select>
      <span class="field-hint" id="rule-method-hint">Methode HTTP que cette regle intercepte</span>
    </div>

    <div class="form-field">
      <label for="rule-subpath">Sous-chemin (optionnel)</label>
      <input id="rule-subpath" type="text" bind:value={subPath} placeholder="ex: /users/{'{id}'}" aria-describedby="rule-subpath-hint" />
      <span class="field-hint" id="rule-subpath-hint">Affine le matching au sein du service</span>
    </div>
  </div>

  <!-- ACTION -->
  <fieldset class="section action-section">
    <legend>Action quand cette regle matche</legend>
    <div class="action-selector">
      <label class="action-option" class:selected={ruleAction === 'mock'}>
        <input type="radio" bind:group={ruleAction} value="mock" />
        <span class="action-label">Mock</span>
        <span class="action-desc">Retourner la reponse simulee ci-dessous</span>
      </label>
      <label class="action-option" class:selected={ruleAction === 'proxy'}>
        <input type="radio" bind:group={ruleAction} value="proxy" />
        <span class="action-label">Proxy</span>
        <span class="action-desc">Forwarder vers la cible reelle du service</span>
      </label>
    </div>
  </fieldset>

  <!-- CONDITIONS -->
  <fieldset class="section">
    <legend>Conditions ET (toutes doivent correspondre)</legend>
    <p class="section-help">Sans condition, la regle matche toutes les requetes.</p>
    {#if allOf.length > 0}
      <ul class="cond-list" role="list">
        {#each allOf as cond, idx}
          <li class="cond-item">
            <span>{conditionLabel(cond)}</span>
            <button type="button" class="btn-icon btn-delete" onclick={() => removeCondition('all_of', idx)} aria-label="Supprimer">&#10005;</button>
          </li>
        {/each}
      </ul>
    {/if}
    {#if addingConditionTo === 'all_of'}
      <ConditionForm onSave={(c) => addCondition('all_of', c)} onCancel={() => addingConditionTo = null} />
    {:else}
      <button type="button" class="btn btn-sm btn-outline" onclick={() => addingConditionTo = 'all_of'}>+ Condition ET</button>
    {/if}
  </fieldset>

  <fieldset class="section">
    <legend>Conditions OU (au moins une doit correspondre)</legend>
    {#if anyOf.length > 0}
      <ul class="cond-list" role="list">
        {#each anyOf as cond, idx}
          <li class="cond-item">
            <span>{conditionLabel(cond)}</span>
            <button type="button" class="btn-icon btn-delete" onclick={() => removeCondition('any_of', idx)} aria-label="Supprimer">&#10005;</button>
          </li>
        {/each}
      </ul>
    {/if}
    {#if addingConditionTo === 'any_of'}
      <ConditionForm onSave={(c) => addCondition('any_of', c)} onCancel={() => addingConditionTo = null} />
    {:else}
      <button type="button" class="btn btn-sm btn-outline" onclick={() => addingConditionTo = 'any_of'}>+ Condition OU</button>
    {/if}
  </fieldset>

  <!-- REPONSE -->
  {#if ruleAction === 'mock'}
  <fieldset class="section section-response">
    <legend>
      <button type="button" class="legend-toggle" onclick={() => responseOpen = !responseOpen} aria-expanded={responseOpen}>
        {responseOpen ? '▼' : '▶'} Reponse mockee
      </button>
    </legend>

    {#if responseOpen}
      {#key modeKey}
      <div class="mode-selector" role="radiogroup" aria-label="Mode de reponse">
        {#each [['json-guided','JSON guide'],['xml-guided','XML guide'],['text','Texte'],['advanced','Template avance'],['empty','Vide (204)']] as [val, label]}
          <button type="button" class="mode-btn" class:mode-active={responseMode === val} onclick={() => requestModeSwitch(val)} role="radio" aria-checked={responseMode === val}>{label}</button>
        {/each}
      </div>
      {/key}

      {#if pendingMode}
        <div class="mode-warning" role="alert">
          <p>{pendingConvMessage || `Changer vers le mode "${pendingMode}" pourrait entrainer une perte de donnees.`}</p>
          <div class="mode-warning-actions">
            <button type="button" class="btn btn-sm btn-primary" onclick={confirmModeSwitch}>Changer quand meme</button>
            <button type="button" class="btn btn-sm btn-secondary" onclick={cancelModeSwitch}>Annuler</button>
          </div>
        </div>
      {/if}

      {#if responseMode !== 'empty'}
        <div class="form-row">
          <div class="form-field" style="max-width:8rem">
            <label for="resp-status">Code HTTP</label>
            <input id="resp-status" type="number" bind:value={status} min="100" max="599" />
          </div>
        </div>

        <div class="sub-section">
          <strong>En-tetes</strong>
          {#each respHeaders as hdr, idx}
            <div class="header-row">
              <input type="text" bind:value={hdr.name} placeholder="Content-Type" aria-label="Nom de l'en-tete {idx + 1}" list="dl-header-names" autocomplete="off" />
              <input type="text" bind:value={hdr.value} placeholder="application/json" aria-label="Valeur de l'en-tete {idx + 1}" list={hdr.name?.toLowerCase() === 'content-type' ? 'dl-content-types' : undefined} autocomplete="off" />
              <button type="button" class="btn-icon btn-delete" onclick={() => removeHeader(idx)} aria-label="Supprimer l'en-tete">&#10005;</button>
            </div>
          {/each}
          <datalist id="dl-header-names">
            {#each commonHeaders as h}<option value={h}></option>{/each}
          </datalist>
          <datalist id="dl-content-types">
            {#each commonContentTypes as ct}<option value={ct}></option>{/each}
          </datalist>
          <button type="button" class="btn btn-sm btn-outline" onclick={addHeader}>+ En-tete</button>
          {#if responseMode === 'json-guided'}
            <span class="field-hint">Content-Type: application/json sera ajoute automatiquement.</span>
          {/if}
        </div>
      {/if}

      {#if responseMode === 'json-guided'}
        <div class="sub-section">
          <JsonResponseBuilder bind:this={jsonBuilderRef} fields={jsonFields} onUpdate={(f) => jsonFields = f} />
        </div>

      {:else if responseMode === 'xml-guided'}
        <div class="sub-section">
          <XmlResponseBuilder bind:this={xmlBuilderRef} fields={xmlFields} onUpdate={(f) => xmlFields = f} />
        </div>

      {:else if responseMode === 'text'}
        <div class="sub-section">
          <strong>Contenu texte</strong>
          <textarea bind:value={textContent} rows="5" placeholder="Contenu de la reponse en texte brut" aria-label="Contenu texte de la reponse" class="text-area"></textarea>
        </div>

      {:else if responseMode === 'advanced'}
        <div class="sub-section">
          <strong>Corps de la reponse (fragments)</strong>
          <p class="section-help">Composez la reponse en ajoutant des blocs concatenes dans l'ordre.</p>

          {#each fragments as frag, idx}
            <div class="fragment-card">
              <div class="fragment-header">
                <span class="frag-index">{idx + 1}</span>
                <select value={frag.type} onchange={(e) => updateFragmentType(idx, e.target.value)} aria-label="Type du fragment {idx + 1}">
                  {#each fragmentTypes as ft}
                    <option value={ft.value}>{ft.label}</option>
                  {/each}
                </select>
                <div class="fragment-actions">
                  <button type="button" class="btn-icon" onclick={() => moveFragment(idx, -1)} disabled={idx === 0} aria-label="Monter" title="Monter">&#9650;</button>
                  <button type="button" class="btn-icon" onclick={() => moveFragment(idx, 1)} disabled={idx === fragments.length - 1} aria-label="Descendre" title="Descendre">&#9660;</button>
                  <button type="button" class="btn-icon btn-delete" onclick={() => removeFragment(idx)} aria-label="Supprimer" title="Supprimer">&#10005;</button>
                </div>
              </div>
              <div class="fragment-body">
                {#if frag.type === 'Literal'}
                  <textarea bind:value={frag.value} rows="2" placeholder='ex: {`{"siret":"`}' aria-label="Contenu texte"></textarea>
                {:else if frag.type === 'Uuid'}
                  <p class="frag-info">UUID v4 genere a chaque requete.</p>
                {:else if frag.type === 'PickFrom'}
                  {#each frag.values as val, vi}
                    <div class="pick-row">
                      <input type="text" bind:value={frag.values[vi]} placeholder="Valeur {vi + 1}" aria-label="Valeur {vi + 1}" />
                      <button type="button" class="btn-icon btn-delete" onclick={() => removePickValue(idx, vi)} aria-label="Supprimer">&#10005;</button>
                    </div>
                  {/each}
                  <button type="button" class="btn btn-sm btn-outline" onclick={() => addPickValue(idx)}>+ Valeur</button>
                {:else if frag.type === 'FakeData'}
                  <select value={frag.kind?.type ?? 'FirstName'} onchange={(e) => updateFakeKind(idx, e.target.value)} aria-label="Type fictif">
                    {#each fakeKinds as fk}<option value={fk.value}>{fk.label}</option>{/each}
                  </select>
                {:else if frag.type === 'PathSegment'}
                  <label class="inline-label">Position <input type="number" bind:value={frag.index} min="0" style="width:5rem" /></label>
                {:else if frag.type === 'Template'}
                  <textarea bind:value={frag.template} rows="5" class="template-textarea"
                    placeholder={`Ex: {{"siret":"{path.siret}","siren":"{path.siret | first(9)}"}}`}
                    aria-label="Template"></textarea>
                  <div class="template-help">
                    <span class="field-hint"><strong>Variables :</strong> <code>{`{path.nom}`}</code>, <code>{`{query.id}`}</code>, <code>{`{uuid}`}</code>, <code>{`{now_ms}`}</code>, <code>{`{fake.CompanyName}`}</code>, <code>{`{seq}`}</code></span>
                    <span class="field-hint"><strong>Pipes :</strong> <code>| lower</code>, <code>| upper</code>, <code>| capitalize</code>, <code>| first(N)</code>, <code>| last(N)</code>, <code>| substr(start,len)</code>, <code>| replace("a","b")</code>, <code>| prepend("x")</code>, <code>| append("x")</code>, <code>| default("val")</code>, <code>| length</code>, <code>| trim</code>. JSON : <code>{`{{`}</code> / <code>{`}}`}</code></span>
                  </div>
                {/if}
              </div>
            </div>
          {/each}
          <button type="button" class="btn btn-sm btn-outline" onclick={addFragment}>+ Ajouter un fragment</button>
        </div>

      {:else if responseMode === 'empty'}
        <p class="section-help" style="margin-top:0.5rem">La reponse sera 204 No Content, sans body.</p>
      {/if}

      <!-- CHAOS -->
      <div class="sub-section script-section">
        <ToggleSwitch label="Script personnalise" checked={scriptEnabled} onchange={(v) => scriptEnabled = v} />
        {#if scriptEnabled}
          <div class="script-editor">
            <label for="rule-script">Code Rhai</label>
            <textarea id="rule-script" bind:value={scriptCode} rows="8" class="script-textarea" placeholder={'// Exemples Rhai :\n// Retourner une valeur simple :\nlet id = request.path.id;\n`user_${id}`\n\n// Retourner un objet (accessible via {{script.champ}}) :\n#{ nom: "Alice", age: "30" }'} aria-describedby="script-hint"></textarea>
            <div class="script-help" id="script-hint">
              <p class="field-hint"><strong>Contexte disponible :</strong> <code>request.body</code> (texte), <code>request.headers</code>, <code>request.query</code>, <code>request.path</code> (maps cle/valeur)</p>
              <p class="field-hint"><strong>Resultat :</strong> La derniere expression est le retour. String → <code>{"{{script}}"}</code>. Objet <code>#{"{cle: val}"}</code> → <code>{"{{script.cle}}"}</code></p>
              <details class="script-examples">
                <summary class="field-hint">Exemples et syntaxe Rhai</summary>
                <div class="script-examples-content">
                  <p><strong>Variables :</strong> <code>let x = 42;</code> <code>let s = "hello";</code></p>
                  <p><strong>Conditions :</strong> <code>if x &gt; 10 {"{"} "grand" {"}"} else {"{"} "petit" {"}"}</code></p>
                  <p><strong>Strings :</strong> <code>s.to_upper()</code> <code>s.len()</code> <code>s.contains("el")</code> <code>s.replace("a", "b")</code></p>
                  <p><strong>Fonctions lightMock :</strong> <code>random_int(1, 5)</code> (entier aleatoire), <code>now_ms()</code> (timestamp ms)</p>
                  <p><strong>Objet retour :</strong> <code>#{"{"} cle: "val", n: random_int(1,100) {"}"}</code> → accessible via <code>{"{{script.cle}}"}</code></p>
                  <p><strong>Ratio 4/5 :</strong> <code>if random_int(1,5) &lt;= 4 {"{"} #{"{"} status: "ok" {"}"} {"}"} else {"{"} #{"{"} status: "ko" {"}"} {"}"}</code></p>
                  <p class="field-hint">Sandbox : pas d'acces fichier/reseau, 10K ops max. <a href="https://rhai.rs/book/" target="_blank" rel="noopener">Doc Rhai</a></p>
                </div>
              </details>
            </div>
          </div>
        {/if}
      </div>

      <div class="sub-section chaos-section">
        <ToggleSwitch label="Mode Chaos" checked={chaosEnabled} onchange={(v) => chaosEnabled = v} />
        {#if chaosEnabled}
          <div class="chaos-fields">
            <label>Latence fixe (ms) <input type="number" bind:value={chaos.delay_ms} min="0" max="30000" /></label>
            <label>Latence min (ms) <input type="number" bind:value={chaos.delay_min_ms} min="0" max="30000" /></label>
            <label>Latence max (ms) <input type="number" bind:value={chaos.delay_max_ms} min="0" max="30000" /></label>
            <label>Taux d'erreur (0-1) <input type="number" bind:value={chaos.error_rate} min="0" max="1" step="0.05" /></label>
            <label>Code erreur <input type="number" bind:value={chaos.error_status} min="400" max="599" /></label>
          </div>
          <span class="field-hint">Si min/max sont remplis, la latence est aleatoire dans la plage (ignore la latence fixe).</span>
        {/if}
      </div>
    {/if}
  </fieldset>
  {/if}

  <!-- ACTIONS -->
  <div class="form-actions">
    <button type="submit" class="btn btn-primary">{init ? 'Enregistrer la regle' : 'Ajouter la regle'}</button>
    <button type="button" class="btn btn-secondary" onclick={onCancel}>Annuler</button>
  </div>
</form>

<style>
  .rule-form { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius); padding: 1.25rem; }

  .action-section { border-color: var(--color-success); }
  .action-selector { display: flex; gap: 0.75rem; flex-wrap: wrap; }
  .action-option { display: flex; flex-direction: column; gap: 0.125rem; padding: 0.625rem 1rem; border: 2px solid var(--color-border); border-radius: var(--radius); cursor: pointer; min-width: 10rem; background: var(--color-bg); }
  .action-option.selected { border-color: var(--color-primary); background: var(--color-surface); }
  .action-option input { display: none; }
  .action-label { font-weight: 700; font-size: 0.9375rem; }
  .action-desc { font-size: 0.8125rem; color: var(--color-text-muted); }

  .mode-selector { display: flex; gap: 0.5rem; margin-bottom: 0.75rem; flex-wrap: wrap; }
  .mode-btn { font-size: 0.875rem; font-weight: 500; cursor: pointer; padding: 0.375rem 0.75rem; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-bg); color: var(--color-text); font-family: inherit; }
  .mode-btn:hover { border-color: var(--color-primary); }
  .mode-btn.mode-active { border-color: var(--color-primary); background: var(--color-focus); font-weight: 600; }

  .mode-warning { background: #fff3cd; border: 1px solid #ffc107; color: #664d03; padding: 0.75rem; border-radius: var(--radius); margin-bottom: 0.75rem; }
  :global([data-theme="dark"]) .mode-warning { background: #332701; border-color: #e5a50a; color: #ffe082; }
  .mode-warning p { margin: 0 0 0.5rem; font-size: 0.875rem; }
  .mode-warning-actions { display: flex; gap: 0.5rem; }

  .section { border: 1px solid var(--color-border); border-radius: var(--radius); padding: 0.75rem; margin-bottom: 1rem; }
  .section legend { font-weight: 600; font-size: 0.875rem; padding: 0 0.375rem; }
  .section-help { font-size: 0.8125rem; color: var(--color-text-muted); margin: 0 0 0.5rem; }

  .section-response { border-color: var(--color-primary); }
  .legend-toggle { background: none; border: none; font: inherit; font-weight: 600; font-size: 0.875rem; cursor: pointer; padding: 0; color: var(--color-text); }

  .sub-section { margin-top: 0.75rem; padding-top: 0.75rem; border-top: 1px solid var(--color-border); }
  .sub-section strong { display: block; margin-bottom: 0.375rem; font-size: 0.875rem; }

  .cond-list { list-style: none; padding: 0; margin: 0 0 0.5rem; }
  .cond-item { display: flex; align-items: center; justify-content: space-between; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); margin-bottom: 0.25rem; background: var(--color-bg); font-size: 0.875rem; }

  .header-row { display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.375rem; }
  .header-row input { flex: 1; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .fragment-card { border: 1px solid var(--color-border); border-radius: var(--radius); padding: 0.625rem; margin-bottom: 0.5rem; background: var(--color-bg); }
  .fragment-header { display: flex; align-items: center; gap: 0.5rem; }
  .fragment-header select { flex: 1; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }
  .fragment-actions { display: flex; gap: 0.25rem; flex-shrink: 0; }
  .fragment-body { margin-top: 0.5rem; }
  .fragment-body textarea { width: 100%; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; font-family: 'Cascadia Code', 'Fira Code', monospace; resize: vertical; }
  .fragment-body select { width: 100%; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; margin-bottom: 0.375rem; }
  .frag-index { display: inline-flex; align-items: center; justify-content: center; width: 1.5rem; height: 1.5rem; border-radius: 50%; background: var(--color-primary); color: #fff; font-size: 0.75rem; font-weight: 700; flex-shrink: 0; }
  .frag-info { font-size: 0.8125rem; color: var(--color-text-muted); font-style: italic; margin: 0; }

  .text-area { width: 100%; padding: 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; font-family: inherit; resize: vertical; }
  .template-textarea { min-height: 5rem; }
  .template-help { margin-top: 0.375rem; display: flex; flex-direction: column; gap: 0.125rem; }
  .template-help code { background: var(--color-bg); padding: 0.1rem 0.25rem; border-radius: 2px; font-size: 0.8rem; }

  .pick-row { display: flex; gap: 0.375rem; align-items: center; margin-bottom: 0.25rem; }
  .pick-row input { flex: 1; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .inline-label { display: flex; align-items: center; gap: 0.5rem; font-size: 0.875rem; font-weight: 500; margin-bottom: 0.375rem; }
  .inline-label input { padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }


  .script-section { border-top-color: var(--color-primary); }
  .script-editor { margin-top: 0.75rem; }
  .script-editor label { display: block; font-weight: 600; font-size: 0.875rem; margin-bottom: 0.25rem; }
  .script-textarea { width: 100%; font-family: 'Cascadia Code', 'Fira Code', monospace; font-size: 0.8125rem; padding: 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-bg); color: var(--color-text); resize: vertical; }
  .script-help { margin-top: 0.375rem; }
  .script-help p { margin: 0.25rem 0; }
  .script-help code { font-size: 0.8125rem; background: var(--color-bg); padding: 0.1rem 0.25rem; border-radius: 2px; }
  .script-examples { margin-top: 0.375rem; }
  .script-examples summary { cursor: pointer; color: var(--color-primary); font-size: 0.8125rem; }
  .script-examples-content { padding: 0.5rem; background: var(--color-bg); border-radius: var(--radius); margin-top: 0.25rem; font-size: 0.8125rem; }
  .script-examples-content p { margin: 0.25rem 0; }
  .script-examples-content a { color: var(--color-primary); }
  .chaos-section { border-top-color: var(--color-warning); }

  .chaos-fields { display: flex; flex-wrap: wrap; gap: 0.75rem; margin-top: 0.5rem; }
  .chaos-fields label { display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.875rem; min-width: 8rem; }
  .chaos-fields input { padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }


  .btn-icon { width: 1.75rem; height: 1.75rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.75rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }
</style>
