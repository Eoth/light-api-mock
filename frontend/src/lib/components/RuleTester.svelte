<script>
  // Testeur de regle : rejeu en lecture seule du brouillon de regle en cours
  // d'edition (RuleForm) contre une requete deja capturee dans les logs du
  // service. Aucune mutation, aucun appel proxy — un simple POST vers
  // /api/rule-test (stateless, cf src/server/api.rs) qui recalcule
  // method/sub_path/conditions et renvoie le detail condition par condition
  // (MatchEngine::evaluate_rule_test, src/engine/matcher.rs) ET, si la regle
  // matche, execute reellement les 3 blocs de script (pre_script/script/
  // post_script) contre la VRAIE requete capturee choisie.
  //
  // Pourquoi l'execution de script est ici et pas ailleurs : en production,
  // une erreur d'execution de script (fonction Rhai inexistante, erreur de type...)
  // est deliberement avalee en soft-fail (intercept.rs::run_rule_script) —
  // la requete n'est jamais bloquee par un script casse, seul un
  // tracing::warn! cote serveur en garde trace. Ce testeur est le SEUL
  // endroit ou cette meme erreur redevient visible pour l'utilisateur,
  // AVANT sauvegarde, contre une requete reelle (jamais un contexte
  // synthetique/vide qui produirait de faux positifs sur des scripts qui
  // dependent legitimement du corps/des params de la requete, ex. le
  // pattern parse_json(request.body) documente dans scripts-rhai.md).
  //
  // `logs` est deja filtre par le parent (RuleForm) aux entrees du service
  // courant — on ne fait ici que filtrer celles qui ont un detail capture
  // (`captured` non-null). Les requetes en proxy direct (service
  // is_mocked=false) n'en ont jamais : le chemin de streaming zero-buffering
  // ne bufferise pas le corps, donc rien n'est capturable.
  import { testRule } from '../api.js';
  import { formatDateTime } from '../format-date.js';
  import FormField from './FormField.svelte';

  let { serviceName, groupName = null, logs = [], getDraftRule } = $props();

  let testableLogs = $derived(logs.filter((l) => l.captured));

  let selectedIndex = $state('');
  let testing = $state(false);
  let result = $state(null);
  let errorMessage = $state('');

  const SOURCE_LABELS = {
    QueryParam: 'paramètre de requête',
    Header: 'en-tête',
    PathParam: 'paramètre de chemin',
    JsonPointer: 'JSON Pointer',
    XPath: 'XPath',
    FormField: 'champ de formulaire',
    BodyRaw: 'corps brut',
  };

  function sourceLabel(source) {
    const base = SOURCE_LABELS[source.type] ?? source.type;
    return source.type === 'BodyRaw' ? base : `${base} '${source.key}'`;
  }

  function operatorLabel(operator) {
    if (operator.type === 'Exists') return 'existe';
    if (operator.type === 'Eq') return `= '${operator.value}'`;
    if (operator.type === 'Contains') return `contient '${operator.value}'`;
    if (operator.type === 'Regex') return `correspond à /${operator.value}/`;
    return operator.type;
  }

  function logLabel(log) {
    const date = formatDateTime(log.timestamp);
    return `${log.method} ${log.path} — ${log.mode} — ${date}`;
  }

  async function handleTest() {
    if (selectedIndex === '') return;
    const log = testableLogs[Number(selectedIndex)];
    const draft = getDraftRule();
    testing = true;
    errorMessage = '';
    result = null;
    try {
      result = await testRule({
        method: draft.method,
        sub_path: draft.subPath || null,
        conditions: { all_of: draft.allOf, any_of: draft.anyOf },
        action: draft.action ?? 'mock',
        pre_script: draft.preScript ?? null,
        script: draft.script ?? null,
        post_script: draft.postScript ?? null,
        request: {
          method: log.method,
          remaining_path: log.captured.remaining_path,
          path_params: log.captured.path_params,
          query_params: log.captured.query_params,
          headers: log.captured.headers,
          body: log.captured.body,
          body_truncated: log.captured.body_truncated,
          content_type: log.captured.content_type,
        },
      });
    } catch (e) {
      errorMessage = e.message;
    } finally {
      testing = false;
    }
  }

  function bodyBasedSource(type) {
    return type === 'JsonPointer' || type === 'XPath' || type === 'FormField' || type === 'BodyRaw';
  }

  let showBodyTruncationWarning = $derived(
    !!result?.body_truncated &&
    [...(result.all_of ?? []), ...(result.any_of ?? [])].some((e) => bodyBasedSource(e.condition.source.type))
  );

  const SLOT_LABELS = {
    pre_script: 'Pré-script (préparation)',
    script: 'Script personnalisé',
    post_script: 'Post-script (finalisation)',
  };

  function slotLabel(slot) {
    return SLOT_LABELS[slot] ?? slot;
  }
</script>

<section class="rule-tester" aria-label="Testeur de règle contre une requête réelle">
  <h3>Tester contre une requête réelle</h3>

  {#if logs.length === 0}
    <p class="section-help">Aucune requête n'a encore été capturée pour ce service.</p>
  {:else if testableLogs.length === 0}
    <p class="section-help">
      Aucune requête avec détail capturé pour ce service — les requêtes en proxy direct
      (service non mocké) ne sont pas bufferisées, donc aucun détail n'est disponible pour le test.
    </p>
  {:else}
    <FormField id="rule-tester-log" label="Requête capturée" hint="Rejeu en lecture seule, aucune requête n'est renvoyée">
      {#snippet children({ id, describedBy })}
        <select {id} bind:value={selectedIndex} aria-describedby={describedBy} data-testid="rule-tester-log-select">
          <option value="" disabled>Choisir une requête</option>
          {#each testableLogs as log, idx}
            <option value={String(idx)}>{logLabel(log)}</option>
          {/each}
        </select>
      {/snippet}
    </FormField>

    <button type="button" class="btn btn-sm btn-secondary" disabled={selectedIndex === '' || testing} onclick={handleTest} data-testid="rule-tester-test-button">
      {testing ? 'Test en cours…' : 'Tester contre cette requête'}
    </button>

    {#if errorMessage}
      <p class="form-error" role="alert" data-testid="rule-tester-error">{errorMessage}</p>
    {/if}

    {#if result}
      <div class="tester-result" role="status" data-testid="rule-tester-result">
        <p class="result-banner" class:result-ok={result.overall_matched} class:result-fail={!result.overall_matched}>
          {#if result.overall_matched}
            ✓ Cette règle matcherait cette requête
          {:else}
            ✗ Cette règle ne matcherait pas cette requête
          {/if}
        </p>

        <ul class="result-summary">
          <li>{result.method_matches ? '✓' : '✗'} Méthode HTTP {result.method_matches ? 'correspond' : 'ne correspond pas'}</li>
          <li>{result.sub_path_matches ? '✓' : '✗'} Sous-chemin {result.sub_path_matches ? 'correspond' : 'ne correspond pas'}</li>
        </ul>

        {#if result.script_errors?.length > 0}
          <div class="script-error-banner" role="alert" data-testid="rule-tester-script-errors">
            <p class="script-error-title">
              ⚠ {result.script_errors.length === 1 ? 'Un script a échoué à l\'exécution' : 'Des scripts ont échoué à l\'exécution'}
              — la réponse serait rendue avec un résultat vide pour {result.script_errors.length === 1 ? 'ce script' : 'ces scripts'}
              (aucune erreur n'est renvoyée au client, cf comportement de production).
            </p>
            <ul class="script-error-list">
              {#each result.script_errors as err}
                <li data-testid="rule-tester-script-error-{err.slot}">
                  <strong>{slotLabel(err.slot)}</strong> : <code>{err.message}</code>
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if result.script_results?.length > 0}
          <div class="script-result-panel" data-testid="rule-tester-script-results">
            <p class="script-result-title">
              Résultat produit par {result.script_results.length === 1 ? 'ce script' : 'ces scripts'} (aucune erreur, mais vérifiez que ce sont bien les valeurs attendues) :
            </p>
            {#each result.script_results as sr}
              <div class="script-result-slot" data-testid="rule-tester-script-result-{sr.slot}">
                <strong>{slotLabel(sr.slot)}</strong>
                {#if Object.keys(sr.fields).length > 0}
                  <ul class="script-result-fields">
                    {#each Object.entries(sr.fields) as [key, value]}
                      <li>
                        <code>{`{{${sr.slot}.${key}}}`}</code> = <code class="script-result-value">{value}</code>
                      </li>
                    {/each}
                  </ul>
                {:else}
                  <p class="script-result-value-line">
                    <code>{`{{${sr.slot}}}`}</code> = <code class="script-result-value">{sr.value}</code>
                  </p>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if showBodyTruncationWarning}
          <p class="body-truncation-warning">
            ⚠ Le corps de cette requête a été tronqué dans le journal — la comparaison sur le corps peut être invalide.
          </p>
        {/if}

        {#each [['all_of', 'Conditions ET', result.all_of], ['any_of', 'Conditions OU', result.any_of]] as [key, title, evaluations]}
          {#if evaluations.length > 0}
            <div class="condition-group-result">
              <h4>{title}</h4>
              <ul class="condition-eval-list">
                {#each evaluations as ev}
                  <li class="condition-eval" class:eval-ok={ev.matched} class:eval-fail={!ev.matched}>
                    <div class="eval-line">
                      <span class="eval-icon" aria-hidden="true">{ev.matched ? '✓' : '✗'}</span>
                      <span class="eval-text">
                        {sourceLabel(ev.condition.source)} {operatorLabel(ev.condition.operator)}
                        — {ev.matched ? 'correspond' : 'ne correspond pas'}
                        (valeur trouvée : {ev.found_value != null ? `'${ev.found_value}'` : 'absente'})
                      </span>
                    </div>
                    {#if ev.hint}
                      <p class="eval-hint">{ev.hint}</p>
                    {/if}
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  {/if}
</section>

<style>
  .rule-tester {
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 1rem;
    margin: 0.5rem 0 1rem;
  }

  .rule-tester h3 {
    margin: 0 0 0.5rem;
    font-size: 1rem;
  }

  .tester-result {
    margin-top: 0.75rem;
  }

  .result-banner {
    font-weight: 600;
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius);
  }

  .result-ok {
    background: var(--color-success-bg, #e6f4ea);
    color: var(--color-success-text, #1e7e34);
  }

  .result-fail {
    background: var(--color-error-bg, #fdecea);
    color: var(--color-error-text, #c0392b);
  }

  .result-summary {
    list-style: none;
    padding: 0;
    margin: 0.5rem 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.875rem;
  }

  .body-truncation-warning {
    font-size: 0.875rem;
    color: var(--color-warning-text, #8a6d3b);
    background: var(--color-warning-bg, #fcf8e3);
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius);
  }

  .script-error-banner {
    margin: 0.5rem 0;
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius);
    background: var(--color-error-bg, #fdecea);
    border: 1px solid var(--color-error-text, #c0392b);
  }

  .script-error-title {
    margin: 0 0 0.375rem;
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-error-text, #c0392b);
  }

  .script-error-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.8125rem;
    word-break: break-word;
  }

  .script-result-panel {
    margin: 0.5rem 0;
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius);
    background: var(--color-bg-secondary, #f5f5f5);
    border: 1px solid var(--color-border);
  }

  .script-result-title {
    margin: 0 0 0.375rem;
    font-size: 0.8125rem;
    color: var(--color-text-muted, inherit);
  }

  .script-result-slot {
    font-size: 0.8125rem;
    margin: 0.375rem 0;
  }

  .script-result-slot:first-of-type {
    margin-top: 0;
  }

  .script-result-fields {
    list-style: none;
    padding: 0;
    margin: 0.25rem 0 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    word-break: break-word;
  }

  .script-result-value-line {
    margin: 0.25rem 0 0;
    word-break: break-word;
  }

  .script-result-value {
    background: var(--color-bg);
    padding: 0.05rem 0.3rem;
    border-radius: 0.2rem;
  }

  .condition-group-result h4 {
    font-size: 0.875rem;
    margin: 0.75rem 0 0.25rem;
  }

  .condition-eval-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .condition-eval {
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius);
    border: 1px solid var(--color-border);
  }

  .eval-ok {
    border-left: 3px solid var(--color-success-text, #1e7e34);
  }

  .eval-fail {
    border-left: 3px solid var(--color-error-text, #c0392b);
  }

  .eval-line {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .eval-text {
    font-size: 0.875rem;
    word-break: break-word;
  }

  .eval-hint {
    margin: 0.35rem 0 0 1.4rem;
    font-size: 0.8125rem;
    font-style: italic;
    color: var(--color-text-muted, inherit);
  }
</style>
