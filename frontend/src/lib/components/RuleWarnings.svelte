<script>
  // Orchestration purement presentationnelle des deux avertissements NON
  // BLOQUANTS affiches a la sauvegarde d'une regle :
  // - stale-proxy : une regle heritee action=proxy rouverte alors que le
  //   service est devenu purement mocke (l'enregistrer la fera basculer
  //   reellement en mock) ;
  // - conflict : la regle chevauche une autre regle existante du service
  //   (detecteur de conflit, POST /api/rule-conflicts).
  // Aucun etat interne : tout est controle par le parent (RuleForm.svelte),
  // qui reste seul responsable de decider QUAND ces avertissements
  // apparaissent (verifications locale/reseau imbriquees dans handleSubmit).
  let {
    pendingStaleProxyWarning = false,
    onConfirmStaleProxy = () => {},
    onCancelStaleProxy = () => {},
    pendingConflicts = [],
    onConfirmConflicts = () => {},
    onDismissConflicts = () => {},
  } = $props();
</script>

{#if pendingStaleProxyWarning}
  <div class="conflict-warning" role="alert" data-testid="rule-form-stale-proxy-warning">
    <p class="conflict-warning-title">
      &#9888; Cette règle était enregistrée en action « Proxy », mais ce service est désormais purement mocké : l'enregistrer maintenant la fera basculer réellement en « Mock » (elle ne relaiera plus jamais vers une cible).
    </p>
    <div class="mode-warning-actions">
      <button type="button" class="btn btn-sm btn-primary" onclick={onConfirmStaleProxy} data-testid="rule-form-stale-proxy-save-anyway-button">Enregistrer quand même</button>
      <button type="button" class="btn btn-sm btn-secondary" onclick={onCancelStaleProxy} data-testid="rule-form-stale-proxy-cancel-button">Modifier la règle</button>
    </div>
  </div>
{/if}

{#if pendingConflicts.length > 0}
  <div class="conflict-warning" role="alert" data-testid="rule-form-conflict-warning">
    <p class="conflict-warning-title">
      &#9888; Cette règle pourrait entrer en conflit avec {pendingConflicts.length > 1 ? 'ces règles existantes' : 'une règle existante'} de ce service :
    </p>
    <ul class="conflict-warning-list">
      {#each pendingConflicts as c (c.other_rule_name)}
        <li data-testid="rule-form-conflict-warning-item-{c.other_rule_name}">
          {#if c.winner === 'other'}
            La règle « {c.other_rule_name} » a des conditions identiques ou incluses — avec l'ordre actuel, c'est « {c.other_rule_name} » qui s'appliquera, cette règle-ci ne se déclenchera jamais pour les requêtes concernées.
          {:else}
            La règle « {c.other_rule_name} » a des conditions identiques ou incluses — avec l'ordre actuel, c'est cette règle-ci qui s'appliquera en premier, la règle « {c.other_rule_name} » sera ignorée pour les requêtes concernées.
          {/if}
        </li>
      {/each}
    </ul>
    <p class="field-hint">Ce conflit peut être volontaire (ex. règle générale + règle plus spécifique en repli). Vous restez libre d'enregistrer quand même.</p>
    <div class="mode-warning-actions">
      <button type="button" class="btn btn-sm btn-primary" onclick={onConfirmConflicts} data-testid="rule-form-conflict-save-anyway-button">Enregistrer quand même</button>
      <button type="button" class="btn btn-sm btn-secondary" onclick={onDismissConflicts} data-testid="rule-form-conflict-cancel-button">Modifier la règle</button>
    </div>
  </div>
{/if}

<style>
  .conflict-warning { background: #fff3cd; border: 1px solid #ffc107; color: #664d03; padding: 0.75rem; border-radius: var(--radius); margin-bottom: 0.75rem; }
  :global([data-theme="dark"]) .conflict-warning { background: #332701; border-color: #e5a50a; color: #ffe082; }
  .conflict-warning-title { margin: 0 0 0.5rem; font-weight: 600; font-size: 0.875rem; }
  .conflict-warning-list { margin: 0 0 0.5rem; padding-left: 1.25rem; display: flex; flex-direction: column; gap: 0.375rem; }
  .conflict-warning-list li { font-size: 0.875rem; word-break: break-word; }

  .mode-warning-actions { display: flex; gap: 0.5rem; flex-wrap: wrap; }
</style>
