<script>
  import ConditionForm from './ConditionForm.svelte';
  import JsonResponseBuilder from './JsonResponseBuilder.svelte';
  import XmlResponseBuilder from './XmlResponseBuilder.svelte';

  let { rule = null, onSave = () => {}, onCancel = () => {} } = $props();

  function deepCopy(obj) { return JSON.parse(JSON.stringify(obj)); }

  const initial = rule ? deepCopy(rule) : null;
  let name = $state(initial?.name ?? '');
  let allOf = $state(initial?.conditions?.all_of ?? []);
  let anyOf = $state(initial?.conditions?.any_of ?? []);
  let addingConditionTo = $state(null);

  let status = $state(initial?.response?.status ?? 200);
  let respHeaders = $state(initial?.response?.headers ?? []);
  let fragments = $state(initial?.response?.body ?? [{ type: 'Literal', value: '' }]);
  let chaosEnabled = $state(!!initial?.response?.chaos);
  let chaos = $state(initial?.response?.chaos ?? { delay_ms: 0, delay_min_ms: null, delay_max_ms: null, error_rate: 0, error_status: 500 });

  let responseOpen = $state(true);

  function detectMode() {
    if (!initial?.response?.body?.length) return 'json-guided';
    if (initial.response.body.length === 1 && initial.response.body[0].type === 'Template') return 'advanced';
    if (initial.response.status === 204) return 'empty';
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
  ];

  function handleSubmit(e) {
    e.preventDefault();
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
  function addHeader() { respHeaders = [...respHeaders, { name: '', value: '' }]; }
  function removeHeader(idx) { respHeaders = respHeaders.filter((_, i) => i !== idx); }
</script>

<form class="rule-form" onsubmit={handleSubmit} aria-label={initial ? `Modifier la regle ${initial.name}` : 'Nouvelle regle'}>

  <div class="form-field">
    <label for="rule-name">Nom de la regle</label>
    <input id="rule-name" type="text" bind:value={name} required placeholder="ex: get-siret" aria-describedby="rn-hint" />
    <span class="field-hint" id="rn-hint">Identifiant unique de cette regle dans le service</span>
  </div>

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
  <fieldset class="section section-response">
    <legend>
      <button type="button" class="legend-toggle" onclick={() => responseOpen = !responseOpen} aria-expanded={responseOpen}>
        {responseOpen ? '▼' : '▶'} Reponse mockee
      </button>
    </legend>

    {#if responseOpen}
      <div class="mode-selector">
        <label class="mode-option"><input type="radio" bind:group={responseMode} value="json-guided" /> JSON guide</label>
        <label class="mode-option"><input type="radio" bind:group={responseMode} value="xml-guided" /> XML guide</label>
        <label class="mode-option"><input type="radio" bind:group={responseMode} value="text" /> Texte</label>
        <label class="mode-option"><input type="radio" bind:group={responseMode} value="advanced" /> Template avance</label>
        <label class="mode-option"><input type="radio" bind:group={responseMode} value="empty" /> Vide (204)</label>
      </div>

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
              <input type="text" bind:value={hdr.name} placeholder="Content-Type" aria-label="Nom de l'en-tete {idx + 1}" />
              <input type="text" bind:value={hdr.value} placeholder="application/json" aria-label="Valeur de l'en-tete {idx + 1}" />
              <button type="button" class="btn-icon btn-delete" onclick={() => removeHeader(idx)} aria-label="Supprimer l'en-tete">&#10005;</button>
            </div>
          {/each}
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
                    <span class="field-hint"><strong>Pipes :</strong> <code>| lower</code>, <code>| upper</code>, <code>| first(N)</code>, <code>| default("val")</code>. JSON : <code>{`{{`}</code> / <code>{`}}`}</code></span>
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
      <div class="sub-section chaos-section">
        <div class="chaos-toggle">
          <strong>Mode Chaos</strong>
          <button type="button" role="switch" aria-checked={chaosEnabled} aria-label="Activer le mode chaos" class="toggle-switch" class:active={chaosEnabled} onclick={() => chaosEnabled = !chaosEnabled}>
            <span class="toggle-knob"></span>
          </button>
          <span aria-live="polite">{chaosEnabled ? 'Actif' : 'Inactif'}</span>
        </div>
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

  <!-- ACTIONS -->
  <div class="form-actions">
    <button type="submit" class="btn btn-primary">{initial ? 'Enregistrer la regle' : 'Ajouter la regle'}</button>
    <button type="button" class="btn btn-secondary" onclick={onCancel}>Annuler</button>
  </div>
</form>

<style>
  .rule-form { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius); padding: 1.25rem; }

  .mode-selector { display: flex; gap: 1rem; margin-bottom: 0.75rem; flex-wrap: wrap; }
  .mode-option { display: flex; align-items: center; gap: 0.375rem; font-size: 0.875rem; font-weight: 500; cursor: pointer; padding: 0.375rem 0.75rem; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-bg); }
  .mode-option:has(input:checked) { border-color: var(--color-primary); background: #e8f0fe; }
  .mode-option input { margin: 0; }
  .form-field { margin-bottom: 1rem; }
  .form-field label { display: block; font-weight: 600; margin-bottom: 0.25rem; }
  .form-field input, .form-field select { width: 100%; padding: 0.5rem 0.75rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 1rem; font-family: inherit; }
  .field-hint { display: block; font-size: 0.8125rem; color: var(--color-text-muted); margin-top: 0.25rem; }

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

  .int-range { display: flex; gap: 0.75rem; margin-top: 0.375rem; }
  .int-range label { display: flex; align-items: center; gap: 0.375rem; font-size: 0.875rem; }
  .int-range input { width: 5rem; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .path-help { margin-top: 0.375rem; }
  .path-example { border-collapse: collapse; font-size: 0.8125rem; margin-bottom: 0.25rem; }
  .path-example th, .path-example td { border: 1px solid var(--color-border); padding: 0.25rem 0.5rem; text-align: center; }
  .path-example th { background: var(--color-bg); font-weight: 600; }
  .path-example .hl { background: #d4edda; font-weight: 600; }

  .chaos-section { border-top-color: var(--color-warning); }
  .chaos-toggle { display: flex; align-items: center; gap: 0.75rem; }
  .chaos-fields { display: flex; flex-wrap: wrap; gap: 0.75rem; margin-top: 0.5rem; }
  .chaos-fields label { display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.875rem; min-width: 8rem; }
  .chaos-fields input { padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .toggle-switch { position: relative; width: 44px; height: 24px; border-radius: 12px; border: 2px solid var(--color-border); background: var(--color-border); padding: 0; cursor: pointer; transition: background-color 0.2s; }
  .toggle-switch.active { background: var(--color-warning); border-color: var(--color-warning); }
  .toggle-knob { position: absolute; top: 2px; left: 2px; width: 16px; height: 16px; border-radius: 50%; background: #fff; box-shadow: 0 1px 2px rgba(0,0,0,0.2); transition: transform 0.2s; }
  .toggle-switch.active .toggle-knob { transform: translateX(20px); }

  .form-actions { display: flex; gap: 0.75rem; margin-top: 1rem; }
  .form-row { display: flex; gap: 0.75rem; flex-wrap: wrap; }

  .btn { padding: 0.5rem 1.25rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; font-size: 0.9375rem; cursor: pointer; }
  .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; }
  .btn-primary { background: var(--color-primary); color: #fff; }
  .btn-primary:hover { background: var(--color-primary-hover); }
  .btn-secondary { background: var(--color-surface); color: var(--color-text); border-color: var(--color-border); }
  .btn-secondary:hover { background: var(--color-bg); }
  .btn-outline { background: var(--color-surface); color: var(--color-text-muted); border-color: var(--color-border); }
  .btn-outline:hover { background: var(--color-bg); color: var(--color-text); }
  .btn-icon { width: 1.75rem; height: 1.75rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.75rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }
</style>
