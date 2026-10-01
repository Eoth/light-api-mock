<script>
  import { untrack } from 'svelte';
  import { t, tCount } from '../i18n.svelte.js';

  let {
    condition = null,
    availablePathParams = [],
    queryParamSuggestions = [],
    onSave = () => {},
    onCancel = () => {},
  } = $props();

  // Labels are getters, read when rendered, so that they follow a change of language.
  const allSourceTypes = [
    { value: 'QueryParam', get label() { return t("Query parameter (?key=value)"); } },
    { value: 'Header', get label() { return t("HTTP header"); } },
    { value: 'PathParam', get label() { return t("Path parameter ({param} in the URL)"); } },
    { value: 'JsonPointer', get label() { return t("JSON Pointer"); } },
    { value: 'XPath', get label() { return t("XPath (XML/SOAP)"); } },
    { value: 'FormField', get label() { return t("Form field"); } },
    { value: 'BodyRaw', get label() { return t("Raw body (whole text)"); } },
  ];

  const operatorTypes = [
    { value: 'Eq', get label() { return t("Equals"); } },
    { value: 'Contains', get label() { return t("Contains"); } },
    { value: 'Regex', get label() { return t("Regular expression"); } },
    { value: 'Exists', get label() { return t("Exists (any value)"); } },
  ];

  // `condition` non-null = edition en place d'une condition existante
  // (RuleConditionsEditor.svelte), non-fourni/null = ajout d'une nouvelle
  // condition — seule difference d'usage entre les deux appelants de ce
  // composant, jamais un mode distinct a gerer explicitement ailleurs.
  const isEditing = untrack(() => condition != null);

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

<form class="condition-form" onsubmit={handleSubmit} aria-label={isEditing ? t("Edit the condition") : t("Add a condition")}>
  <div class="form-row">
    <div class="form-field">
      <label for="cond-source">{t("Source")}</label>
      {#if availablePathParams.length > 0}
        <span class="field-hint path-param-badge">
          {tCount(availablePathParams.length, "{0} path parameter available: {1}", "{0} path parameters available: {1}", availablePathParams.join(', '))}
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
          <label for="cond-key">{t("Path parameter")}</label>
          <select id="cond-key" bind:value={sourceKey} required aria-describedby="cond-key-hint" data-testid="condition-form-key-select">
            <option value="" disabled>{t("Choose a parameter")}</option>
            {#each pathParamOptions as name}
              <option value={name}>{name}</option>
            {/each}
          </select>
          <span class="field-hint" id="cond-key-hint">{t("Strict choice among the actual parameters of the URL")}</span>
        {:else if sourceType === 'QueryParam'}
          <label for="cond-key">{t("Key / path")}</label>
          <input
            id="cond-key"
            type="text"
            list="cond-query-param-suggestions"
            bind:value={sourceKey}
            required
            placeholder={t("name")}
            aria-describedby="cond-key-hint"
            data-testid="condition-form-key-input"
          />
          <datalist id="cond-query-param-suggestions">
            {#each queryParamSuggestions as name}
              <option value={name}></option>
            {/each}
          </datalist>
          <span class="field-hint" id="cond-key-hint">
            {t("Name of the query parameter: suggestions come from the real traffic of the service, any name can be typed")}
          </span>
        {:else}
          <label for="cond-key">{t("Key / path")}</label>
          <input
            id="cond-key"
            type="text"
            bind:value={sourceKey}
            required
            placeholder={sourceType === 'JsonPointer' ? '/user/role' : sourceType === 'XPath' ? 'Envelope/Body/id' : t("name")}
            aria-describedby="cond-key-hint"
            data-testid="condition-form-key-input"
          />
          <span class="field-hint" id="cond-key-hint">
            {#if sourceType === 'JsonPointer'}{t("JSON Pointer path (e.g. /user/role)")}
            {:else if sourceType === 'XPath'}{t("Simplified XPath path (e.g. Envelope/Body/id)")}
            {:else}{t("Name of the parameter, header or field")}
            {/if}
          </span>
        {/if}
      </div>
    {/if}
  </div>

  <div class="form-row">
    <div class="form-field">
      <label for="cond-op">{t("Operator")}</label>
      <select id="cond-op" bind:value={operatorType} data-testid="condition-form-operator-select">
        {#each operatorTypes as op}
          <option value={op.value}>{op.label}</option>
        {/each}
      </select>
    </div>

    {#if needsValue}
      <div class="form-field">
        <label for="cond-val">{t("Expected value")}</label>
        <input
          id="cond-val"
          type="text"
          bind:value={operatorValue}
          required
          placeholder={operatorType === 'Regex' ? '^\\d{3}$' : t("value")}
          data-testid="condition-form-value-input"
        />
      </div>
    {/if}
  </div>

  <div class="form-actions">
    <button type="submit" class="btn btn-sm btn-primary" data-testid="condition-form-submit-button">{isEditing ? t("Save") : t("OK")}</button>
    <button type="button" class="btn btn-sm btn-secondary" onclick={onCancel} data-testid="condition-form-cancel-button">{t("Cancel")}</button>
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
