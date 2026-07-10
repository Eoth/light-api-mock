<script>
  import { untrack } from 'svelte';

  let {
    condition = null,
    availablePathParams = [],
    queryParamSuggestions = [],
    onSave = () => {},
    onCancel = () => {},
  } = $props();

  const allSourceTypes = [
    { value: 'QueryParam', label: 'Parametre de requete (?cle=valeur)' },
    { value: 'Header', label: 'En-tete HTTP' },
    { value: 'PathParam', label: 'Parametre de chemin ({param} dans l\'URL)' },
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

  let sourceType = $state(untrack(() => condition?.source?.type ?? 'QueryParam'));
  let sourceKey = $state(untrack(() => condition?.source?.key ?? ''));
  let operatorType = $state(untrack(() => condition?.operator?.type ?? 'Eq'));
  let operatorValue = $state(untrack(() => condition?.operator?.value ?? ''));

  // "Path param" est retire du selecteur quand aucun path param n'est
  // disponible (URL statique) — sauf si une condition existante utilise deja
  // ce type (garde defensive : ne jamais faire disparaitre une condition
  // deja saisie, meme si la liste devient vide entre-temps, ex. sub_path
  // efface pendant l'edition).
  let sourceTypes = $derived(
    allSourceTypes.filter((st) => st.value !== 'PathParam' || availablePathParams.length > 0 || sourceType === 'PathParam')
  );

  // Idem pour la liste d'options du <select> PathParam : si la valeur
  // existante n'est plus dans availablePathParams, on la garde quand meme
  // comme option pour ne pas perdre silencieusement la donnee.
  let pathParamOptions = $derived(
    sourceKey && !availablePathParams.includes(sourceKey)
      ? [...availablePathParams, sourceKey]
      : availablePathParams
  );

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
      {#if availablePathParams.length > 0}
        <span class="field-hint path-param-badge">
          {availablePathParams.length} paramètre{availablePathParams.length > 1 ? 's' : ''} de chemin disponible{availablePathParams.length > 1 ? 's' : ''} : {availablePathParams.join(', ')}
        </span>
      {/if}
      <select id="cond-source" bind:value={sourceType} data-testid="condition-form-source-select">
        {#each sourceTypes as st}
          <option value={st.value}>{st.label}</option>
        {/each}
      </select>
    </div>

    {#if needsKey}
      <div class="form-field">
        {#if sourceType === 'PathParam'}
          <label for="cond-key">Paramètre de chemin</label>
          <select id="cond-key" bind:value={sourceKey} required aria-describedby="cond-key-hint" data-testid="condition-form-key-select">
            <option value="" disabled>Choisir un paramètre</option>
            {#each pathParamOptions as name}
              <option value={name}>{name}</option>
            {/each}
          </select>
          <span class="field-hint" id="cond-key-hint">Sélection stricte parmi les paramètres réels de l'URL</span>
        {:else if sourceType === 'QueryParam'}
          <label for="cond-key">Clé / Chemin</label>
          <input
            id="cond-key"
            type="text"
            list="cond-query-param-suggestions"
            bind:value={sourceKey}
            required
            placeholder="nom"
            aria-describedby="cond-key-hint"
            data-testid="condition-form-key-input"
          />
          <datalist id="cond-query-param-suggestions">
            {#each queryParamSuggestions as name}
              <option value={name}></option>
            {/each}
          </datalist>
          <span class="field-hint" id="cond-key-hint">
            Nom du paramètre de requête — suggestions basées sur le trafic réel du service, saisie libre possible
          </span>
        {:else}
          <label for="cond-key">Clé / Chemin</label>
          <input
            id="cond-key"
            type="text"
            bind:value={sourceKey}
            required
            placeholder={sourceType === 'JsonPointer' ? '/user/role' : sourceType === 'XPath' ? 'Envelope/Body/id' : 'nom'}
            aria-describedby="cond-key-hint"
            data-testid="condition-form-key-input"
          />
          <span class="field-hint" id="cond-key-hint">
            {#if sourceType === 'JsonPointer'}Chemin JSON Pointer (ex: /user/role)
            {:else if sourceType === 'XPath'}Chemin XPath simplifié (ex: Envelope/Body/id)
            {:else}Nom du paramètre, en-tête ou champ
            {/if}
          </span>
        {/if}
      </div>
    {/if}
  </div>

  <div class="form-row">
    <div class="form-field">
      <label for="cond-op">Opérateur</label>
      <select id="cond-op" bind:value={operatorType} data-testid="condition-form-operator-select">
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
          data-testid="condition-form-value-input"
        />
      </div>
    {/if}
  </div>

  <div class="form-actions">
    <button type="submit" class="btn btn-sm btn-primary" data-testid="condition-form-submit-button">Valider</button>
    <button type="button" class="btn btn-sm btn-secondary" onclick={onCancel} data-testid="condition-form-cancel-button">Annuler</button>
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

  .form-actions {
    display: flex;
    gap: 0.5rem;
  }

  .path-param-badge {
    display: block;
    margin-bottom: 0.35rem;
    font-weight: 600;
    color: var(--color-primary, inherit);
  }
</style>
