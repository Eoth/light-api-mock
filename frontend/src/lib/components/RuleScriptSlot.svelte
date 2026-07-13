<script>
  // Un bloc script Rhai independant (toggle + editeur + validation + aide
  // contextuelle). Reutilise 3 fois par RuleResponseSection.svelte pour
  // pre_script/script/post_script (cf CLAUDE.md, "Scripts rhai multi-blocs" :
  // meme structure, execution independante, pas de chainage). Le contenu
  // d'aide differe selon le slot (texte court partage pour pre_script/
  // post_script, doc etendue avec exemples/RHAI_FUNCTIONS pour le script
  // principal) — fourni par l'appelant via le snippet `help`, jamais
  // duplique ici.
  import ToggleSwitch from './ToggleSwitch.svelte';
  import RhaiScriptEditor from './RhaiScriptEditor.svelte';

  let {
    id,
    toggleLabel,
    code = '',
    enabled = false,
    onToggle = () => {},
    onCodeInput = () => {},
    validation = { status: '', message: '' },
    onValidate = () => {},
    rows = 5,
    placeholder = '',
    help,
  } = $props();
</script>

<div class="sub-section script-section">
  <ToggleSwitch label={toggleLabel} checked={enabled} onchange={onToggle} />
  {#if enabled}
    <div class="script-editor">
      <label for={id}>Code Rhai</label>
      <RhaiScriptEditor {id} value={code} onInput={onCodeInput} {rows} {placeholder} ariaDescribedby="{id}-hint" />
      <div class="script-actions">
        <button type="button" class="btn btn-outline btn-sm" onclick={onValidate} disabled={validation.status === 'pending'} data-testid="rule-form-validate-script-button-{id}">
          {validation.status === 'pending' ? 'Validation...' : 'Valider le script'}
        </button>
        {#if validation.status === 'ok'}
          <span class="script-valid" role="status" data-testid="rule-form-script-valid-{id}">&#10003; {validation.message}</span>
        {:else if validation.status === 'error'}
          <span class="script-invalid" role="alert" data-testid="rule-form-script-invalid-{id}">{validation.message}</span>
        {/if}
      </div>
      <div class="script-help" id="{id}-hint">
        {@render help()}
      </div>
    </div>
  {/if}
</div>

<style>
  .sub-section { margin-top: 0.75rem; padding-top: 0.75rem; border-top: 1px solid var(--color-border); }
  .script-section { border-top-color: var(--color-primary); }
  .script-editor { margin-top: 0.75rem; }
  .script-editor label { display: block; font-weight: 600; font-size: 0.875rem; margin-bottom: 0.25rem; }
  .script-actions { display: flex; align-items: center; gap: 0.75rem; margin-top: 0.375rem; }
  .script-valid { font-size: 0.8125rem; color: var(--color-success); font-weight: 600; }
  .script-invalid { font-size: 0.8125rem; color: var(--color-danger); }
  .script-help { margin-top: 0.375rem; }

  /* Le contenu du bloc "aide" est fourni par l'appelant via le snippet
     `help` (defini dans RuleResponseSection.svelte) : ces elements portent
     le hash de scope du PARENT, pas de ce composant. :global() est
     necessaire pour que ces regles les atteignent malgre la frontiere de
     composant (cf commentaire similaire cote RuleResponseSection.svelte). */
  .script-help :global(p) { margin: 0.25rem 0; }
  .script-help :global(code) { font-size: 0.8125rem; background: var(--color-bg); padding: 0.1rem 0.25rem; border-radius: 2px; }
  .script-help :global(.script-examples) { margin-top: 0.375rem; }
  .script-help :global(.script-examples summary) { cursor: pointer; color: var(--color-primary); font-size: 0.8125rem; }
  .script-help :global(.script-examples-content) { padding: 0.5rem; background: var(--color-bg); border-radius: var(--radius); margin-top: 0.25rem; font-size: 0.8125rem; }
  .script-help :global(.script-examples-content p) { margin: 0.25rem 0; }
  .script-help :global(.script-examples-content a) { color: var(--color-primary); }
  .script-help :global(.script-fn-list) { margin: 0.25rem 0 0.5rem; padding-left: 1.125rem; }
  .script-help :global(.script-fn-list li) { margin: 0.125rem 0; }
</style>
