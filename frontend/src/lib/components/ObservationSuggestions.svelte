<script>
  // Observation de trafic proxy (niveau service, is_mocked=false uniquement,
  // cf server::observation cote backend) + suggestions de regles calculees a
  // partir du trafic reellement capture. Rien n'est automatique : l'utilisateur
  // active/desactive explicitement l'observation, et rafraichit lui-meme les
  // suggestions (pas de polling en arriere-plan) — meme philosophie "cout
  // proportionnel a l'usage reel" que UrlHealthBadge.svelte.
  //
  // "Utiliser cette suggestion" ne cree PAS la regle directement : ca
  // pre-remplit le formulaire de regle existant (RuleForm.svelte, via
  // onUseSuggestion -> ServiceDetail.svelte) pour que l'utilisateur relise/
  // edite avant de sauvegarder par le chemin deja valide (validate_service,
  // detecteur de conflit) — decide explicitement avec l'utilisateur plutot
  // qu'un endpoint "materialiser en un clic" qui bypasserait ces gardes.
  import { observeService, unobserveService, getObservationStatus, getServiceSuggestions } from '../api.js';

  let { serviceName, groupName = null, isMocked = true, onUseSuggestion = () => {} } = $props();

  let observing = $state(false);
  let statusLoaded = $state(false);
  let toggling = $state(false);
  let suggestions = $state([]);
  let loadingSuggestions = $state(false);
  let error = $state('');

  async function refreshStatus() {
    if (isMocked) return;
    try {
      const active = await getObservationStatus();
      observing = active.some(
        (e) => e.service_name === serviceName && (e.group_name ?? null) === (groupName ?? null)
      );
    } catch {
      // Statut non critique : reste sur la derniere valeur connue plutot que
      // de bloquer l'affichage du panneau pour une erreur reseau ponctuelle.
    } finally {
      statusLoaded = true;
    }
  }
  $effect(() => { refreshStatus(); });

  async function handleToggleObserve() {
    toggling = true;
    error = '';
    try {
      if (observing) {
        await unobserveService(serviceName, groupName);
        observing = false;
        suggestions = [];
      } else {
        await observeService(serviceName, groupName);
        observing = true;
      }
    } catch (e) {
      error = e.message;
    } finally {
      toggling = false;
    }
  }

  async function refreshSuggestions() {
    loadingSuggestions = true;
    error = '';
    try {
      suggestions = await getServiceSuggestions(serviceName, groupName);
    } catch (e) {
      error = e.message;
    } finally {
      loadingSuggestions = false;
    }
  }

  const SOURCE_LABELS = {
    QueryParam: 'Paramètre de requête',
    Header: 'En-tête HTTP',
    JsonPointer: 'Champ JSON du corps',
  };

  function conditionSummary(condition) {
    if (!condition) return null;
    const sourceLabel = SOURCE_LABELS[condition.source?.type] ?? condition.source?.type;
    return `${sourceLabel} "${condition.source?.key}" = "${condition.operator?.value}"`;
  }

  function toRuleDraft(rule) {
    return {
      name: '',
      method: rule.method,
      sub_path: rule.sub_path,
      action: 'mock',
      pre_script: null,
      script: null,
      post_script: null,
      response_mode: null,
      conditions: {
        all_of: rule.condition ? [rule.condition] : [],
        any_of: [],
      },
      response: rule.response,
    };
  }

  function bodyPreview(rule) {
    const literal = rule.response?.body?.find((f) => f.type === 'Literal');
    const text = literal?.value ?? '';
    return text.length > 120 ? `${text.slice(0, 120)}…` : text;
  }
</script>

{#if !isMocked}
  <section class="observation-panel" aria-label="Observation du trafic proxy" data-testid="observation-panel-{serviceName}">
    <div class="panel-header">
      <h4>Suggestions de règles à partir du trafic réel</h4>
      <button
        type="button"
        class="btn btn-sm {observing ? 'btn-outline' : 'btn-primary'}"
        onclick={handleToggleObserve}
        disabled={toggling || !statusLoaded}
        data-testid="observation-toggle-button-{serviceName}"
      >
        {observing ? 'Arrêter d\'observer' : 'Observer ce service'}
      </button>
    </div>
    <p class="panel-hint">
      Capture bornée requête+réponse pendant que ce service est en mode proxy pur, pour proposer
      des règles de mock à partir d'appels réellement observés — jamais activé automatiquement.
    </p>

    {#if observing}
      <div class="panel-actions">
        <button
          type="button"
          class="btn btn-sm btn-outline"
          onclick={refreshSuggestions}
          disabled={loadingSuggestions}
          data-testid="observation-refresh-suggestions-button-{serviceName}"
        >
          {loadingSuggestions ? 'Chargement...' : 'Actualiser les suggestions'}
        </button>
      </div>

      {#if suggestions.length === 0 && !loadingSuggestions}
        <p class="panel-empty" data-testid="observation-suggestions-empty-{serviceName}">
          Aucune suggestion pour l'instant — appelez ce service via le proxy plusieurs fois, puis
          actualisez.
        </p>
      {/if}

      {#each suggestions as suggestion, i (i)}
        {#if suggestion.outcome === 'VarianceUnexplained'}
          <p class="panel-unexplained" role="status" data-testid="observation-suggestion-unexplained-{serviceName}-{i}">
            Réponses variables observées ({suggestion.sample_count} appels, {suggestion.response_class_count}
            réponses distinctes) mais aucun champ de la requête ne permet de les distinguer de façon
            fiable — aucune règle proposée.
          </p>
        {:else}
          {#each (suggestion.outcome === 'Unconditional' ? [suggestion.rule] : suggestion.rules) as rule, j (j)}
            <div class="suggestion-card" data-testid="observation-suggestion-{serviceName}-{i}-{j}">
              <div class="suggestion-summary">
                <code>{rule.method} {rule.sub_path}</code>
                {#if conditionSummary(rule.condition)}
                  <span class="suggestion-condition">si {conditionSummary(rule.condition)}</span>
                {:else}
                  <span class="suggestion-condition">sans condition ({rule.sample_count} appels identiques)</span>
                {/if}
              </div>
              <div class="suggestion-response">
                <span class="suggestion-status">{rule.response.status}</span>
                <code class="suggestion-body">{bodyPreview(rule)}</code>
              </div>
              <button
                type="button"
                class="btn btn-sm btn-primary"
                onclick={() => onUseSuggestion(toRuleDraft(rule))}
                data-testid="observation-use-suggestion-{serviceName}-{i}-{j}"
              >
                Utiliser cette suggestion
              </button>
            </div>
          {/each}
        {/if}
      {/each}
    {/if}

    {#if error}
      <p class="panel-error" role="alert" data-testid="observation-error-{serviceName}">{error}</p>
    {/if}
  </section>
{/if}

<style>
  .observation-panel {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 1rem 1.25rem;
    margin-top: 1rem;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .panel-header h4 { margin: 0; font-size: 1rem; }

  .panel-hint {
    margin: 0.5rem 0 0;
    font-size: 0.8125rem;
    color: var(--color-text-muted);
  }

  .panel-actions { margin-top: 0.75rem; }

  .panel-empty, .panel-unexplained {
    margin: 0.75rem 0 0;
    font-size: 0.875rem;
    color: var(--color-text-muted);
  }

  .suggestion-card {
    margin-top: 0.75rem;
    padding: 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.375rem;
  }

  .suggestion-summary { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }

  .suggestion-condition { font-size: 0.8125rem; color: var(--color-text-muted); }

  .suggestion-response { display: flex; align-items: center; gap: 0.5rem; }

  .suggestion-status {
    font-weight: 700;
    font-size: 0.8125rem;
    padding: 0.125rem 0.5rem;
    border-radius: var(--radius);
    background: var(--color-bg);
  }

  .suggestion-body {
    font-size: 0.8125rem;
    background: var(--color-bg);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    overflow-wrap: anywhere;
  }

  code { font-size: 0.8125rem; }

  .panel-error {
    margin: 0.75rem 0 0;
    font-size: 0.8125rem;
    color: var(--color-danger);
  }
</style>
