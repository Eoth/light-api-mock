<script>
  // Rule tester: replays the draft rule of RuleForm, read-only, against a request already captured in the service's
  // log. Nothing changes and nothing is proxied: one POST to /api/rule-test (stateless, src/server/api.rs) evaluates
  // the method, the sub-path and each condition (MatchEngine::evaluate_rule_test, src/engine/matcher.rs) and, when the
  // rule matches and is not a proxy rule, runs its three script slots against that real request.
  //
  // Why scripts run here: in production a failing script (unknown Rhai function, type error...) is swallowed
  // (run_rule_script in src/server/intercept.rs), so that a broken script never blocks a request; only a server log
  // line records it. The tester is where the user sees that error, before saving, on a real request: an empty, made-up
  // context would report false errors for scripts that rightly read the body or the parameters (the
  // `parse_json(request.body)` pattern of docs/en/rhai-scripts.md).
  //
  // RuleForm passes the logs of the current service; only the entries with captured details are kept here. A pure
  // proxy service streams its requests without buffering them, so they have none.
  import { testRule } from '../api.js';
  import { formatDateTime } from '../format-date.js';
  import FormField from './FormField.svelte';
  import { t, tCount, intlLocale } from '../i18n.svelte.js';

  let { serviceName, groupName = null, logs = [], getDraftRule } = $props();

  let testableLogs = $derived(logs.filter((l) => l.captured));

  let selectedIndex = $state('');
  let testing = $state(false);
  let result = $state(null);
  let errorMessage = $state('');

  function sourceName(type) {
    switch (type) {
      case 'QueryParam': return t("query parameter");
      case 'Header': return t("header");
      case 'PathParam': return t("path parameter");
      case 'JsonPointer': return t("JSON Pointer");
      case 'XPath': return t("XPath");
      case 'FormField': return t("form field");
      case 'BodyRaw': return t("raw body");
      default: return type;
    }
  }

  function sourceLabel(source) {
    const base = sourceName(source.type);
    return source.type === 'BodyRaw' ? base : `${base} '${source.key}'`;
  }

  function operatorLabel(operator) {
    if (operator.type === 'Exists') return t("exists");
    if (operator.type === 'Eq') return `= '${operator.value}'`;
    if (operator.type === 'Contains') return t("contains '{0}'", operator.value);
    if (operator.type === 'Regex') return t("matches /{0}/", operator.value);
    return operator.type;
  }

  function logLabel(log) {
    const date = formatDateTime(log.timestamp, undefined, intlLocale());
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

  function slotLabel(slot) {
    switch (slot) {
      case 'pre_script': return t("Pre-script (preparation)");
      case 'script': return t("Custom script");
      case 'post_script': return t("Post-script (finalization)");
      default: return slot;
    }
  }
</script>

<section class="rule-tester" aria-label={t("Rule tester against a real request")}>
  <h3>{t("Test against a real request")}</h3>

  {#if logs.length === 0}
    <p class="section-help">{t("No request has been captured for this service yet.")}</p>
  {:else if testableLogs.length === 0}
    <p class="section-help">
      {t("No request with captured details for this service: requests proxied directly (service not mocked) are not buffered, so no details are available for a test.")}
    </p>
  {:else}
    <FormField id="rule-tester-log" label={t("Captured request")} hint={t("Read-only replay: no request is sent again")}>
      {#snippet children({ id, describedBy })}
        <select {id} bind:value={selectedIndex} aria-describedby={describedBy} data-testid="rule-tester-log-select">
          <option value="" disabled>{t("Choose a request")}</option>
          {#each testableLogs as log, idx}
            <option value={String(idx)}>{logLabel(log)}</option>
          {/each}
        </select>
      {/snippet}
    </FormField>

    <button type="button" class="btn btn-sm btn-secondary" disabled={selectedIndex === '' || testing} onclick={handleTest} data-testid="rule-tester-test-button">
      {testing ? t("Testing…") : t("Test against this request")}
    </button>

    {#if errorMessage}
      <p class="form-error" role="alert" data-testid="rule-tester-error">{errorMessage}</p>
    {/if}

    {#if result}
      <div class="tester-result" role="status" data-testid="rule-tester-result">
        <p class="result-banner" class:result-ok={result.overall_matched} class:result-fail={!result.overall_matched}>
          {#if result.overall_matched}
            {t("✓ This rule would match this request")}
          {:else}
            {t("✗ This rule would not match this request")}
          {/if}
        </p>

        <ul class="result-summary">
          <li>{result.method_matches ? t("✓ HTTP method matches") : t("✗ HTTP method does not match")}</li>
          <li>{result.sub_path_matches ? t("✓ Sub-path matches") : t("✗ Sub-path does not match")}</li>
        </ul>

        {#if result.script_errors?.length > 0}
          <div class="script-error-banner" role="alert" data-testid="rule-tester-script-errors">
            <p class="script-error-title">
              {tCount(result.script_errors.length, "⚠ A script failed to run: the response would be rendered with an empty result for this script (no error is returned to the client, as in production).", "⚠ Scripts failed to run: the response would be rendered with an empty result for these scripts (no error is returned to the client, as in production).")}
            </p>
            <ul class="script-error-list">
              {#each result.script_errors as err}
                <li data-testid="rule-tester-script-error-{err.slot}">
                  <strong>{t("{0}:", slotLabel(err.slot))}</strong> <code>{err.message}</code>
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if result.script_results?.length > 0}
          <div class="script-result-panel" data-testid="rule-tester-script-results">
            <p class="script-result-title">
              {tCount(result.script_results.length, "Result of this script (no error, but check that these are the expected values):", "Result of these scripts (no error, but check that these are the expected values):")}
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
            {t("⚠ The body of this request was truncated in the log: comparisons on the body may be wrong.")}
          </p>
        {/if}

        {#each [['all_of', t("AND conditions"), result.all_of], ['any_of', t("OR conditions"), result.any_of]] as [key, title, evaluations]}
          {#if evaluations.length > 0}
            <div class="condition-group-result">
              <h4>{title}</h4>
              <ul class="condition-eval-list">
                {#each evaluations as ev}
                  <li class="condition-eval" class:eval-ok={ev.matched} class:eval-fail={!ev.matched}>
                    <div class="eval-line">
                      <span class="eval-icon" aria-hidden="true">{ev.matched ? '✓' : '✗'}</span>
                      <span class="eval-text">
                        {ev.matched
                          ? t("{0} {1}: matches (value found: {2})", sourceLabel(ev.condition.source), operatorLabel(ev.condition.operator), ev.found_value != null ? `'${ev.found_value}'` : t("none"))
                          : t("{0} {1}: does not match (value found: {2})", sourceLabel(ev.condition.source), operatorLabel(ev.condition.operator), ev.found_value != null ? `'${ev.found_value}'` : t("none"))}
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
