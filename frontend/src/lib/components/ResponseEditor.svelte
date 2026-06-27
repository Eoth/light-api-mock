<script>
  let { response = null, onSave = () => {}, onCancel = () => {} } = $props();

  const initial = response;
  let status = $state(initial?.status ?? 200);
  let headers = $state(initial?.headers ? initial.headers.map(h => ({ ...h })) : []);
  let fragments = $state(initial?.body ? initial.body.map(f => structuredClone(f)) : []);
  let chaos = $state(initial?.chaos ? { ...initial.chaos } : null);
  let chaosEnabled = $state(!!initial?.chaos);

  const fragmentTypes = [
    { value: 'Literal', label: 'Texte fixe', hint: 'Du texte brut injecté tel quel dans la réponse' },
    { value: 'Uuid', label: 'UUID v4', hint: 'Un identifiant unique généré à chaque requête' },
    { value: 'PickFrom', label: 'Choix aléatoire', hint: 'Pioche une valeur au hasard dans la liste' },
    { value: 'FakeData', label: 'Donnée fictive', hint: 'Génère un prénom, nom, email, téléphone ou nombre' },
    { value: 'PathSegment', label: 'Segment du chemin (URL)', hint: 'Récupère une partie de l\'URL appelée (ex: le siret dans /api/insee/{siret})' },
  ];

  const fakeKinds = [
    { value: 'FirstName', label: 'Prénom' },
    { value: 'LastName', label: 'Nom de famille' },
    { value: 'Email', label: 'Adresse email' },
    { value: 'PhoneNumberFR', label: 'Téléphone FR' },
    { value: 'Integer', label: 'Nombre entier' },
  ];

  function addFragment() {
    fragments = [...fragments, { type: 'Literal', value: '' }];
  }

  function removeFragment(idx) {
    fragments = fragments.filter((_, i) => i !== idx);
  }

  function moveFragment(idx, dir) {
    const target = idx + dir;
    if (target < 0 || target >= fragments.length) return;
    const copy = [...fragments];
    [copy[idx], copy[target]] = [copy[target], copy[idx]];
    fragments = copy;
  }

  function updateFragmentType(idx, newType) {
    const copy = [...fragments];
    if (newType === 'Literal') copy[idx] = { type: 'Literal', value: '' };
    else if (newType === 'Uuid') copy[idx] = { type: 'Uuid' };
    else if (newType === 'PickFrom') copy[idx] = { type: 'PickFrom', values: [''] };
    else if (newType === 'FakeData') copy[idx] = { type: 'FakeData', kind: { type: 'FirstName' } };
    else if (newType === 'PathSegment') copy[idx] = { type: 'PathSegment', index: 0 };
    fragments = copy;
  }

  function updateFakeKind(idx, kindType) {
    const copy = [...fragments];
    if (kindType === 'Integer') {
      copy[idx] = { type: 'FakeData', kind: { type: 'Integer', min: 0, max: 100 } };
    } else {
      copy[idx] = { type: 'FakeData', kind: { type: kindType } };
    }
    fragments = copy;
  }

  function addPickValue(idx) {
    const copy = [...fragments];
    copy[idx] = { ...copy[idx], values: [...copy[idx].values, ''] };
    fragments = copy;
  }

  function removePickValue(fragIdx, valIdx) {
    const copy = [...fragments];
    copy[fragIdx] = { ...copy[fragIdx], values: copy[fragIdx].values.filter((_, i) => i !== valIdx) };
    fragments = copy;
  }

  function addHeader() {
    headers = [...headers, { name: '', value: '' }];
  }

  function removeHeader(idx) {
    headers = headers.filter((_, i) => i !== idx);
  }

  function toggleChaos() {
    chaosEnabled = !chaosEnabled;
    if (chaosEnabled && !chaos) {
      chaos = { delay_ms: 0, error_rate: 0, error_status: 500 };
    }
  }

  function handleSubmit(e) {
    e.preventDefault();
    onSave({
      status,
      headers: headers.filter(h => h.name.trim()),
      body: fragments,
      chaos: chaosEnabled ? chaos : null,
    });
  }
</script>

<form class="response-editor" onsubmit={handleSubmit} aria-label="Éditeur de réponse mockée">

  <div class="form-field">
    <label for="resp-status">Code HTTP de réponse</label>
    <input id="resp-status" type="number" bind:value={status} min="100" max="599" />
  </div>

  <fieldset class="editor-section">
    <legend>En-têtes de réponse</legend>
    {#each headers as header, idx}
      <div class="header-row">
        <div class="form-field">
          <label for="hdr-name-{idx}">Nom</label>
          <input id="hdr-name-{idx}" type="text" bind:value={header.name} placeholder="Content-Type" />
        </div>
        <div class="form-field">
          <label for="hdr-val-{idx}">Valeur</label>
          <input id="hdr-val-{idx}" type="text" bind:value={header.value} placeholder="application/json" />
        </div>
        <button type="button" class="btn-icon btn-delete" onclick={() => removeHeader(idx)} aria-label="Supprimer l'en-tête {header.name || idx + 1}">&#10005;</button>
      </div>
    {/each}
    <button type="button" class="btn btn-sm btn-secondary" onclick={addHeader}>+ En-tête</button>
  </fieldset>

  <fieldset class="editor-section">
    <legend>Corps de la réponse (fragments)</legend>
    <p class="section-hint" id="frag-hint">Composez le corps en ajoutant des blocs. Ils seront concaténés dans l'ordre.</p>

    {#each fragments as frag, idx}
      <div class="fragment-card" aria-label="Fragment {idx + 1}">
        <div class="fragment-header">
          <span class="frag-index">{idx + 1}</span>
          <div class="form-field">
            <label for="frag-type-{idx}">Type</label>
            <select id="frag-type-{idx}" value={frag.type} onchange={(e) => updateFragmentType(idx, e.target.value)}>
              {#each fragmentTypes as ft}
                <option value={ft.value}>{ft.label}</option>
              {/each}
            </select>
          </div>
          <div class="fragment-actions">
            <button type="button" class="btn-icon" onclick={() => moveFragment(idx, -1)} disabled={idx === 0} aria-label="Monter le fragment {idx + 1}" title="Monter">&#9650;</button>
            <button type="button" class="btn-icon" onclick={() => moveFragment(idx, 1)} disabled={idx === fragments.length - 1} aria-label="Descendre le fragment {idx + 1}" title="Descendre">&#9660;</button>
            <button type="button" class="btn-icon btn-delete" onclick={() => removeFragment(idx)} aria-label="Supprimer le fragment {idx + 1}" title="Supprimer">&#10005;</button>
          </div>
        </div>

        <div class="fragment-body">
          {#if frag.type === 'Literal'}
            <div class="form-field">
              <label for="frag-val-{idx}">Contenu texte</label>
              <textarea id="frag-val-{idx}" bind:value={frag.value} rows="3" placeholder='ex: {`{"siret": "`}'  aria-describedby="lit-hint-{idx}"></textarea>
              <span class="field-hint" id="lit-hint-{idx}">Ce texte sera injecte tel quel dans la reponse. Pour du JSON, ecrivez un morceau du JSON ici puis utilisez d'autres fragments pour les valeurs dynamiques.</span>
            </div>

          {:else if frag.type === 'Uuid'}
            <p class="frag-info">Un identifiant unique (UUID v4) sera genere automatiquement a chaque requete. Exemple : <code>3f2504e0-4f89-11d3-9a0c-0305e82c3301</code></p>

          {:else if frag.type === 'PickFrom'}
            <div class="pick-values">
              <span id="pick-label-{idx}" class="pick-label">Valeurs possibles (une sera choisie aléatoirement)</span>
              {#each frag.values as val, vi}
                <div class="pick-row" role="group" aria-labelledby="pick-label-{idx}">
                  <input type="text" bind:value={frag.values[vi]} placeholder="Valeur {vi + 1}" aria-label="Valeur {vi + 1}" />
                  <button type="button" class="btn-icon btn-delete" onclick={() => removePickValue(idx, vi)} aria-label="Supprimer la valeur {vi + 1}">&#10005;</button>
                </div>
              {/each}
              <button type="button" class="btn btn-sm btn-secondary" onclick={() => addPickValue(idx)}>+ Valeur</button>
            </div>

          {:else if frag.type === 'FakeData'}
            <div class="form-field">
              <label for="fake-kind-{idx}">Type de donnée fictive</label>
              <select id="fake-kind-{idx}" value={frag.kind?.type ?? 'FirstName'} onchange={(e) => updateFakeKind(idx, e.target.value)}>
                {#each fakeKinds as fk}
                  <option value={fk.value}>{fk.label}</option>
                {/each}
              </select>
            </div>
            {#if frag.kind?.type === 'Integer'}
              <div class="int-range">
                <div class="form-field">
                  <label for="fake-min-{idx}">Min</label>
                  <input id="fake-min-{idx}" type="number" bind:value={frag.kind.min} />
                </div>
                <div class="form-field">
                  <label for="fake-max-{idx}">Max</label>
                  <input id="fake-max-{idx}" type="number" bind:value={frag.kind.max} />
                </div>
              </div>
            {/if}

          {:else if frag.type === 'PathSegment'}
            <div class="form-field">
              <label for="path-idx-{idx}">Position du segment dans l'URL (commence à 0)</label>
              <input id="path-idx-{idx}" type="number" bind:value={frag.index} min="0" aria-describedby="path-hint-{idx}" />
              <div class="path-help" id="path-hint-{idx}">
                <p class="field-hint">L'URL est découpée en segments séparés par <code>/</code>.</p>
                <table class="path-example" aria-label="Exemple de découpage d'URL">
                  <thead><tr><th>Position</th><th>0</th><th>1</th><th>2</th><th>3</th></tr></thead>
                  <tbody><tr><td>URL</td><td>v4</td><td>api</td><td>insee</td><td class="path-highlight">12345678</td></tr></tbody>
                </table>
                <p class="field-hint">Pour <code>/v4/api/insee/12345678</code>, la position <strong>3</strong> donne <strong>12345678</strong> (le siret).</p>
              </div>
            </div>
          {/if}
        </div>
      </div>
    {/each}

    <button type="button" class="btn btn-sm btn-primary" onclick={addFragment}>+ Ajouter un fragment</button>
  </fieldset>

  <fieldset class="editor-section chaos-section">
    <legend>Mode Chaos</legend>
    <div class="chaos-toggle">
      <button
        type="button"
        role="switch"
        aria-checked={chaosEnabled}
        aria-label="Activer le mode chaos"
        class="toggle-switch"
        class:active={chaosEnabled}
        onclick={toggleChaos}
      >
        <span class="toggle-knob"></span>
      </button>
      <span aria-live="polite">{chaosEnabled ? 'Activé' : 'Désactivé'}</span>
    </div>

    {#if chaosEnabled && chaos}
      <div class="chaos-fields">
        <div class="form-field">
          <label for="chaos-delay">Latence artificielle (ms)</label>
          <input id="chaos-delay" type="number" bind:value={chaos.delay_ms} min="0" max="30000" aria-describedby="chaos-delay-hint" />
          <span class="field-hint" id="chaos-delay-hint">Délai ajouté avant chaque réponse (0 = aucun)</span>
        </div>
        <div class="form-field">
          <label for="chaos-rate">Taux d'erreur (0 à 1)</label>
          <input id="chaos-rate" type="number" bind:value={chaos.error_rate} min="0" max="1" step="0.05" aria-describedby="chaos-rate-hint" />
          <span class="field-hint" id="chaos-rate-hint">Probabilité de retourner une erreur au lieu de la réponse (ex: 0.1 = 10%)</span>
        </div>
        <div class="form-field">
          <label for="chaos-status">Code d'erreur HTTP</label>
          <input id="chaos-status" type="number" bind:value={chaos.error_status} min="400" max="599" aria-describedby="chaos-status-hint" />
          <span class="field-hint" id="chaos-status-hint">Code HTTP retourné en cas d'erreur injectée</span>
        </div>
      </div>
    {/if}
  </fieldset>

  <div class="form-actions">
    <button type="submit" class="btn btn-primary">Enregistrer la réponse</button>
    <button type="button" class="btn btn-secondary" onclick={onCancel}>Annuler</button>
  </div>
</form>

<style>
  .response-editor {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 1.25rem;
  }

  .editor-section {
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 0.75rem;
    margin-bottom: 1rem;
  }

  .editor-section legend {
    font-weight: 600;
    font-size: 0.9375rem;
    padding: 0 0.375rem;
  }

  .section-hint {
    font-size: 0.8125rem;
    color: var(--color-text-muted);
    margin: 0 0 0.75rem;
  }

  .form-field { margin-bottom: 0.75rem; }
  .form-field label { display: block; font-weight: 600; font-size: 0.875rem; margin-bottom: 0.25rem; }
  .form-field input, .form-field select, .form-field textarea {
    width: 100%;
    padding: 0.375rem 0.5rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-family: inherit;
  }
  .form-field input[type="number"] { max-width: 10rem; }
  .form-field textarea { resize: vertical; font-family: 'Cascadia Code', 'Fira Code', monospace; }

  .header-row {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
    margin-bottom: 0.5rem;
  }
  .header-row .form-field { flex: 1; margin-bottom: 0; }

  .fragment-card {
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 0.75rem;
    margin-bottom: 0.5rem;
    background: var(--color-bg);
  }

  .fragment-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .fragment-header .form-field { flex: 1; margin-bottom: 0; }
  .fragment-header .form-field label { font-size: 0; width: 0; height: 0; overflow: hidden; position: absolute; }

  .frag-index {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 50%;
    background: var(--color-primary);
    color: #fff;
    font-size: 0.75rem;
    font-weight: 700;
    flex-shrink: 0;
  }

  .fragment-actions { display: flex; gap: 0.25rem; flex-shrink: 0; }

  .fragment-body { padding-left: 2rem; }

  .frag-info {
    font-size: 0.8125rem;
    color: var(--color-text-muted);
    font-style: italic;
    margin: 0;
  }

  .path-help { margin-top: 0.5rem; }
  .path-example { border-collapse: collapse; font-size: 0.8125rem; margin: 0.375rem 0; }
  .path-example th, .path-example td { border: 1px solid var(--color-border); padding: 0.25rem 0.5rem; text-align: center; }
  .path-example th { background: var(--color-bg); font-weight: 600; }
  .path-highlight { background: #d4edda; font-weight: 600; }

  .pick-values { display: flex; flex-direction: column; gap: 0.375rem; }
  .pick-label { font-weight: 600; font-size: 0.875rem; }
  .pick-row { display: flex; gap: 0.375rem; align-items: center; }
  .pick-row input { flex: 1; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .int-range { display: flex; gap: 0.75rem; }
  .int-range .form-field { flex: 1; }

  .chaos-section { border-color: var(--color-warning); }

  .chaos-toggle {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
    font-weight: 500;
  }

  .toggle-switch {
    position: relative;
    width: 44px;
    height: 24px;
    border-radius: 12px;
    border: 2px solid var(--color-border);
    background: var(--color-border);
    padding: 0;
    cursor: pointer;
    transition: background-color 0.2s, border-color 0.2s;
  }

  .toggle-switch.active {
    background: var(--color-warning);
    border-color: var(--color-warning);
  }

  .toggle-knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 2px rgba(0,0,0,0.2);
    transition: transform 0.2s;
  }

  .toggle-switch.active .toggle-knob {
    transform: translateX(20px);
  }

  .chaos-fields {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }
  .chaos-fields .form-field { flex: 1; min-width: 10rem; }

  .btn-icon { width: 1.75rem; height: 1.75rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.75rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }
</style>
