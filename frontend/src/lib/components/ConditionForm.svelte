<script>
  let { condition = null, onSave = () => {}, onCancel = () => {} } = $props();

  const initial = condition;
  const sourceTypes = [
    { value: 'QueryParam', label: 'Parametre de requete' },
    { value: 'Header', label: 'En-tete HTTP' },
    { value: 'PathParam', label: 'Parametre de chemin ({param})' },
    { value: 'JsonPointer', label: 'JSON Pointer' },
    { value: 'XPath', label: 'XPath (XML/SOAP)' },
    { value: 'FormField', label: 'Champ formulaire' },
    { value: 'BodyRaw', label: 'Corps brut (texte entier)' },
  ];

  const operatorTypes = [
    { value: 'Eq', label: 'Égal à' },
    { value: 'Contains', label: 'Contient' },
    { value: 'Regex', label: 'Expression régulière' },
    { value: 'Exists', label: 'Existe (peu importe la valeur)' },
  ];

  let sourceType = $state(initial?.source?.type ?? 'QueryParam');
  let sourceKey = $state(initial?.source?.key ?? '');
  let operatorType = $state(initial?.operator?.type ?? 'Eq');
  let operatorValue = $state(initial?.operator?.value ?? '');

  let needsKey = $derived(sourceType !== 'BodyRaw');
  let needsValue = $derived(operatorType !== 'Exists');

  function handleSubmit(e) {
    e.preventDefault();
    const source = sourceType === 'BodyRaw'
      ? { type: 'BodyRaw' }
      : { type: sourceType, key: sourceKey };
    const operator = operatorType === 'Exists'
      ? { type: 'Exists' }
      : { type: operatorType, value: operatorValue };
    onSave({ source, operator });
  }
</script>

<form class="condition-form" onsubmit={handleSubmit} aria-label="Condition de matching">
  <div class="form-row">
    <div class="form-field">
      <label for="cond-source">Source</label>
      <select id="cond-source" bind:value={sourceType}>
        {#each sourceTypes as st}
          <option value={st.value}>{st.label}</option>
        {/each}
      </select>
    </div>

    {#if needsKey}
      <div class="form-field">
        <label for="cond-key">Clé / Chemin</label>
        <input
          id="cond-key"
          type="text"
          bind:value={sourceKey}
          required
          placeholder={sourceType === 'JsonPointer' ? '/user/role' : sourceType === 'XPath' ? 'Envelope/Body/id' : 'nom'}
          aria-describedby="cond-key-hint"
        />
        <span class="field-hint" id="cond-key-hint">
          {#if sourceType === 'JsonPointer'}Chemin JSON Pointer (ex: /user/role)
          {:else if sourceType === 'XPath'}Chemin XPath simplifié (ex: Envelope/Body/id)
          {:else}Nom du paramètre, en-tête ou champ
          {/if}
        </span>
      </div>
    {/if}
  </div>

  <div class="form-row">
    <div class="form-field">
      <label for="cond-op">Opérateur</label>
      <select id="cond-op" bind:value={operatorType}>
        {#each operatorTypes as op}
          <option value={op.value}>{op.label}</option>
        {/each}
      </select>
    </div>

    {#if needsValue}
      <div class="form-field">
        <label for="cond-val">Valeur attendue</label>
        <input
          id="cond-val"
          type="text"
          bind:value={operatorValue}
          required
          placeholder={operatorType === 'Regex' ? '^\\d{3}$' : 'valeur'}
        />
      </div>
    {/if}
  </div>

  <div class="form-actions">
    <button type="submit" class="btn btn-sm btn-primary">Valider</button>
    <button type="button" class="btn btn-sm btn-secondary" onclick={onCancel}>Annuler</button>
  </div>
</form>

<style>
  .condition-form {
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 1rem;
    margin: 0.5rem 0;
  }

  .form-row {
    display: flex;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
    flex-wrap: wrap;
  }

  .form-field {
    flex: 1;
    min-width: 12rem;
  }

  .form-field label {
    display: block;
    font-weight: 600;
    font-size: 0.875rem;
    margin-bottom: 0.25rem;
  }

  .form-field input,
  .form-field select {
    width: 100%;
    padding: 0.375rem 0.5rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    font-size: 0.875rem;
    font-family: inherit;
  }

  .field-hint {
    display: block;
    font-size: 0.75rem;
    color: var(--color-text-muted);
    margin-top: 0.125rem;
  }

  .form-actions {
    display: flex;
    gap: 0.5rem;
  }

  .btn { padding: 0.25rem 0.75rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; font-size: 0.8125rem; }
  .btn-primary { background: var(--color-primary); color: #fff; }
  .btn-primary:hover { background: var(--color-primary-hover); }
  .btn-secondary { background: var(--color-surface); color: var(--color-text); border-color: var(--color-border); }
  .btn-secondary:hover { background: var(--color-bg); }
</style>
