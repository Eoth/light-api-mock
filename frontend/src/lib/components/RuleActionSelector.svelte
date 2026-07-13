<script>
  // Selecteur d'action (Mock/Proxy) d'une regle. Purement controle : aucun
  // etat interne, la valeur courante et les changements transitent par
  // props/callback (meme convention que les autres composants du dossier,
  // cf ConditionForm.svelte). L'option Proxy est retiree du DOM (pas juste
  // desactivee) quand `isPurelyMocked` est vrai (cf CLAUDE.md §5 point 61).
  let {
    action = 'mock',
    isPurelyMocked = false,
    onChange = () => {},
  } = $props();
</script>

<fieldset class="section action-section">
  <legend>Action quand cette regle matche</legend>
  {#if isPurelyMocked}
    <p class="section-help" data-testid="rule-form-purely-mocked-hint">Ce service est purement mocké (aucune cible configurée) : seule l'action Mock est disponible.</p>
  {/if}
  <div class="action-selector">
    <label class="action-option" class:selected={action === 'mock'} data-testid="rule-form-action-mock-option">
      <input type="radio" checked={action === 'mock'} onchange={() => onChange('mock')} data-testid="rule-form-action-mock-radio" />
      <span class="action-label">Mock</span>
      <span class="action-desc">Retourner la reponse simulee ci-dessous</span>
    </label>
    {#if !isPurelyMocked}
      <label class="action-option" class:selected={action === 'proxy'} data-testid="rule-form-action-proxy-option">
        <input type="radio" checked={action === 'proxy'} onchange={() => onChange('proxy')} data-testid="rule-form-action-proxy-radio" />
        <span class="action-label">Proxy</span>
        <span class="action-desc">Forwarder vers la cible reelle du service</span>
      </label>
    {/if}
  </div>
</fieldset>

<style>
  .section { border: 1px solid var(--color-border); border-radius: var(--radius); padding: 0.75rem; margin-bottom: 1rem; }
  .section legend { font-weight: 600; font-size: 0.875rem; padding: 0 0.375rem; }
  .section-help { font-size: 0.8125rem; color: var(--color-text-muted); margin: 0 0 0.5rem; }

  .action-section { border-color: var(--color-success); }
  .action-selector { display: flex; gap: 0.75rem; flex-wrap: wrap; }
  .action-option { display: flex; flex-direction: column; gap: 0.125rem; padding: 0.625rem 1rem; border: 2px solid var(--color-border); border-radius: var(--radius); cursor: pointer; min-width: 10rem; background: var(--color-bg); }
  .action-option.selected { border-color: var(--color-primary); background: var(--color-surface); }
  .action-option input { display: none; }
  .action-label { font-weight: 700; font-size: 0.9375rem; }
  .action-desc { font-size: 0.8125rem; color: var(--color-text-muted); }
</style>
