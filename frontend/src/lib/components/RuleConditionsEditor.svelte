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
  // Condition en cours d'edition en place : { group, idx } | null. Mutuellement
  // exclusif avec addingConditionTo (ouvrir l'un referme l'autre) — meme
  // discipline que le reste du formulaire (un seul mini-formulaire ouvert a la
  // fois). Aucun contenu n'est jamais perdu par ce mecanisme : ConditionForm
  // est demontee/remontee via {#if}, mais elle ne fait que RE-INITIALISER ses
  // champs a partir de la condition deja sauvegardee a chaque ouverture — rien
  // n'est jamais tape "dans le vide" avant que ce {#if} ne bascule, contrairement
  // au piege des sujets 24/25 (perte d'une saisie en cours par demontage).
  let editingCondition = $state(null);

  function addCondition(group, condition) {
    if (group === 'all_of') onAllOfChange([...allOf, condition]);
    else onAnyOfChange([...anyOf, condition]);
    addingConditionTo = null;
  }

  function removeCondition(group, idx) {
    const list = group === 'all_of' ? allOf : anyOf;
    const updated = list.filter((_, i) => i !== idx);
    if (group === 'all_of') onAllOfChange(updated);
    else onAnyOfChange(updated);
    if (editingCondition?.group === group) {
      if (editingCondition.idx === idx) editingCondition = null;
      else if (editingCondition.idx > idx) editingCondition = { group, idx: editingCondition.idx - 1 };
    }
  }

  function startAdd(group) {
    editingCondition = null;
    addingConditionTo = group;
  }

  function startEdit(group, idx) {
    addingConditionTo = null;
    editingCondition = { group, idx };
  }

  function cancelEdit() {
    editingCondition = null;
  }

  // Remplace UNIQUEMENT l'entree a `idx` (map, jamais filter/push) : les
  // autres conditions du meme groupe gardent leur position et leur contenu
  // intacts, aucun reordonnancement induit par une edition en place.
  function saveEdit(condition) {
    const { group, idx } = editingCondition;
    const list = group === 'all_of' ? allOf : anyOf;
    const updated = list.map((c, i) => (i === idx ? condition : c));
    if (group === 'all_of') onAllOfChange(updated);
    else onAnyOfChange(updated);
    editingCondition = null;
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
        {#if editingCondition?.group === 'all_of' && editingCondition?.idx === idx}
          <li class="cond-item-editing">
            <ConditionForm
              condition={cond}
              {availablePathParams}
              {queryParamSuggestions}
              onSave={saveEdit}
              onCancel={cancelEdit}
            />
          </li>
        {:else}
          <li class="cond-item">
            <button
              type="button"
              class="cond-label-button"
              onclick={() => startEdit('all_of', idx)}
              aria-label="Modifier la condition : {conditionLabel(cond)}"
              data-testid="rule-form-edit-condition-allof-button-{idx}"
            >
              {conditionLabel(cond)}
            </button>
            <button type="button" class="btn-icon btn-delete" onclick={() => removeCondition('all_of', idx)} aria-label="Supprimer" data-testid="rule-form-remove-condition-allof-button-{idx}">&#10005;</button>
          </li>
        {/if}
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
    <button type="button" class="btn btn-sm btn-outline" onclick={() => startAdd('all_of')} data-testid="rule-form-add-condition-allof-button">+ Condition ET</button>
  {/if}
</fieldset>

<fieldset class="section">
  <legend>Conditions OU (au moins une doit correspondre)</legend>
  {#if anyOf.length > 0}
    <ul class="cond-list" role="list">
      {#each anyOf as cond, idx}
        {#if editingCondition?.group === 'any_of' && editingCondition?.idx === idx}
          <li class="cond-item-editing">
            <ConditionForm
              condition={cond}
              {availablePathParams}
              {queryParamSuggestions}
              onSave={saveEdit}
              onCancel={cancelEdit}
            />
          </li>
        {:else}
          <li class="cond-item">
            <button
              type="button"
              class="cond-label-button"
              onclick={() => startEdit('any_of', idx)}
              aria-label="Modifier la condition : {conditionLabel(cond)}"
              data-testid="rule-form-edit-condition-anyof-button-{idx}"
            >
              {conditionLabel(cond)}
            </button>
            <button type="button" class="btn-icon btn-delete" onclick={() => removeCondition('any_of', idx)} aria-label="Supprimer" data-testid="rule-form-remove-condition-anyof-button-{idx}">&#10005;</button>
          </li>
        {/if}
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
    <button type="button" class="btn btn-sm btn-outline" onclick={() => startAdd('any_of')} data-testid="rule-form-add-condition-anyof-button">+ Condition OU</button>
  {/if}
</fieldset>

<style>
  .section { border: 1px solid var(--color-border); border-radius: var(--radius); padding: 0.75rem; margin-bottom: 1rem; }
  .section legend { font-weight: 600; font-size: 0.875rem; padding: 0 0.375rem; }
  .section-help { font-size: 0.8125rem; color: var(--color-text-muted); margin: 0 0 0.5rem; }

  .cond-list { list-style: none; padding: 0; margin: 0 0 0.5rem; }
  .cond-item { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); margin-bottom: 0.25rem; background: var(--color-bg); font-size: 0.875rem; }
  .cond-item-editing { margin-bottom: 0.25rem; }

  .cond-label-button {
    flex: 1;
    min-width: 0;
    text-align: left;
    background: none;
    border: none;
    padding: 0.25rem 0.375rem;
    margin: -0.25rem -0.375rem;
    border-radius: var(--radius);
    font: inherit;
    color: inherit;
    cursor: pointer;
    overflow-wrap: anywhere;
  }
  .cond-label-button:hover { background: var(--color-surface); text-decoration: underline; }
  .cond-label-button:focus-visible { outline: 2px solid var(--color-primary, currentColor); outline-offset: 2px; }

  .btn-icon { flex-shrink: 0; width: 1.75rem; height: 1.75rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.75rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }
</style>
