<script>
  // Editeur des deux groupes de conditions d'une regle (ET/OU). Regroupe la
  // liste des conditions deja ajoutees + le formulaire d'ajout (ConditionForm)
  // pour les deux groupes, qui partageaient un template quasi identique dans
  // RuleForm.svelte. `allOf`/`anyOf` sont controles par le parent (memes
  // valeurs utilisees par buildRulePayload/le detecteur de conflit) : ce
  // composant ne fait que proposer add/remove via callbacks, jamais de copie
  // locale des tableaux.
  import ConditionForm from './ConditionForm.svelte';

  let {
    allOf = [],
    anyOf = [],
    availablePathParams = [],
    queryParamSuggestions = [],
    onAllOfChange = () => {},
    onAnyOfChange = () => {},
  } = $props();

  let addingConditionTo = $state(null);

  function addCondition(group, condition) {
    if (group === 'all_of') onAllOfChange([...allOf, condition]);
    else onAnyOfChange([...anyOf, condition]);
    addingConditionTo = null;
  }

  function removeCondition(group, idx) {
    if (group === 'all_of') onAllOfChange(allOf.filter((_, i) => i !== idx));
    else onAnyOfChange(anyOf.filter((_, i) => i !== idx));
  }

  function conditionLabel(c) {
    const src = c.source.type === 'BodyRaw' ? 'Corps brut' : `${c.source.type}(${c.source.key})`;
    const op = c.operator.type === 'Exists' ? 'existe' : `${c.operator.type}(${c.operator.value})`;
    return `${src} ${op}`;
  }
</script>

<fieldset class="section">
  <legend>Conditions ET (toutes doivent correspondre)</legend>
  <p class="section-help">Sans condition, la regle matche toutes les requetes.</p>
  {#if allOf.length > 0}
    <ul class="cond-list" role="list">
      {#each allOf as cond, idx}
        <li class="cond-item">
          <span>{conditionLabel(cond)}</span>
          <button type="button" class="btn-icon btn-delete" onclick={() => removeCondition('all_of', idx)} aria-label="Supprimer" data-testid="rule-form-remove-condition-allof-button-{idx}">&#10005;</button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if addingConditionTo === 'all_of'}
    <ConditionForm
      {availablePathParams}
      {queryParamSuggestions}
      onSave={(c) => addCondition('all_of', c)}
      onCancel={() => addingConditionTo = null}
    />
  {:else}
    <button type="button" class="btn btn-sm btn-outline" onclick={() => addingConditionTo = 'all_of'} data-testid="rule-form-add-condition-allof-button">+ Condition ET</button>
  {/if}
</fieldset>

<fieldset class="section">
  <legend>Conditions OU (au moins une doit correspondre)</legend>
  {#if anyOf.length > 0}
    <ul class="cond-list" role="list">
      {#each anyOf as cond, idx}
        <li class="cond-item">
          <span>{conditionLabel(cond)}</span>
          <button type="button" class="btn-icon btn-delete" onclick={() => removeCondition('any_of', idx)} aria-label="Supprimer" data-testid="rule-form-remove-condition-anyof-button-{idx}">&#10005;</button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if addingConditionTo === 'any_of'}
    <ConditionForm
      {availablePathParams}
      {queryParamSuggestions}
      onSave={(c) => addCondition('any_of', c)}
      onCancel={() => addingConditionTo = null}
    />
  {:else}
    <button type="button" class="btn btn-sm btn-outline" onclick={() => addingConditionTo = 'any_of'} data-testid="rule-form-add-condition-anyof-button">+ Condition OU</button>
  {/if}
</fieldset>

<style>
  .section { border: 1px solid var(--color-border); border-radius: var(--radius); padding: 0.75rem; margin-bottom: 1rem; }
  .section legend { font-weight: 600; font-size: 0.875rem; padding: 0 0.375rem; }
  .section-help { font-size: 0.8125rem; color: var(--color-text-muted); margin: 0 0 0.5rem; }

  .cond-list { list-style: none; padding: 0; margin: 0 0 0.5rem; }
  .cond-item { display: flex; align-items: center; justify-content: space-between; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); margin-bottom: 0.25rem; background: var(--color-bg); font-size: 0.875rem; }

  .btn-icon { width: 1.75rem; height: 1.75rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.75rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }
</style>
