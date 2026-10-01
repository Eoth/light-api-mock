<script>
  // Gestion des services de mock TCP brut (feature backend "tcp-mock") :
  // liste + CRUD (GET/POST/PUT/DELETE /api/tcp/services) + statut d'ecoute
  // (GET /api/tcp/status, sans auth). Mock UNIQUEMENT — pas de mode proxy :
  // un relais qui ne fait que retransmettre sans matching n'ajoute aucune
  // valeur de mock, et route inutilement le trafic vers un intermediaire.
  // Utile pour un protocole binaire simple ou chaque connexion est UN
  // message envoye par le client suivi d'UNE reponse fixe (ping/heartbeat
  // proprietaire, handshake fixe) — PAS pour LDAP/SMTP ou tout protocole
  // qui enchaine plusieurs messages sur la meme connexion.
  //
  // Les champs stockes en hexadecimal cote backend (prefixe de matching,
  // reponse) sont saisis en TEXTE par defaut (conversion via hex-utils.js) :
  // un utilisateur qui veut mocker un protocole texte simple n'a jamais
  // besoin de taper de l'hexadecimal a la main. Le mode "Hexadecimal" reste
  // disponible pour les protocoles binaires reels (ex: prefixe BER).
  import { getTcpServices, getTcpStatus, createTcpService, updateTcpService, deleteTcpService } from '../api.js';
  import { textToHex, hexToTextOrNull, isValidHex } from '../hex-utils.js';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import FormField from './FormField.svelte';
  import { t, tCount } from '../i18n.svelte.js';

  let { onNotify = () => {}, onBack = () => {} } = $props();

  let services = $state([]);
  let statuses = $state([]);
  let loading = $state(true);
  let mode = $state('list'); // 'list' | 'form'
  let editingName = $state(null); // null = creation
  let saving = $state(false);
  let deletePending = $state(null);

  let formName = $state('');
  let formPort = $state('');
  let formRules = $state([]);
  let formError = $state('');

  function emptyRule() {
    return {
      name: '',
      matcherType: 'Any',
      matcherMode: 'text',
      matcherValue: '',
      responseMode: 'text',
      responseValue: '',
    };
  }

  function ruleFromApi(rule) {
    const matcherType = rule.matcher.type;
    let matcherMode = 'text';
    let matcherValue = '';
    if (matcherType === 'Prefix') {
      const asText = hexToTextOrNull(rule.matcher.value);
      matcherMode = asText !== null ? 'text' : 'hex';
      matcherValue = asText !== null ? asText : rule.matcher.value;
    } else if (matcherType === 'Regex') {
      matcherValue = rule.matcher.value;
    }
    const asText = hexToTextOrNull(rule.response_hex);
    return {
      name: rule.name,
      matcherType,
      matcherMode,
      matcherValue,
      responseMode: asText !== null ? 'text' : 'hex',
      responseValue: asText !== null ? asText : rule.response_hex,
    };
  }

  function ruleToApi(rule) {
    let matcher;
    if (rule.matcherType === 'Prefix') {
      const value = rule.matcherMode === 'hex' ? rule.matcherValue : textToHex(rule.matcherValue);
      matcher = { type: 'Prefix', value };
    } else if (rule.matcherType === 'Regex') {
      matcher = { type: 'Regex', value: rule.matcherValue };
    } else {
      matcher = { type: 'Any' };
    }
    const response_hex = rule.responseMode === 'hex' ? rule.responseValue : textToHex(rule.responseValue);
    return { name: rule.name, matcher, response_hex };
  }

  async function loadAll() {
    loading = true;
    try {
      const [svcList, statusList] = await Promise.all([getTcpServices(), getTcpStatus()]);
      services = svcList;
      statuses = statusList;
    } catch (e) {
      onNotify(t("Error while loading the TCP services: {0}", e.message), 'error');
    } finally {
      loading = false;
    }
  }

  $effect(() => { loadAll(); });

  function listeningFor(name) {
    return statuses.find((s) => s.name === name)?.listening ?? null;
  }

  function statusFor(name) {
    return statuses.find((s) => s.name === name) ?? null;
  }

  function startCreate() {
    editingName = null;
    formName = '';
    formPort = '';
    formRules = [emptyRule()];
    formError = '';
    mode = 'form';
  }

  function startEdit(svc) {
    editingName = svc.name;
    formName = svc.name;
    formPort = String(svc.listen_port);
    formRules = svc.rules.map(ruleFromApi);
    if (formRules.length === 0) formRules = [emptyRule()];
    formError = '';
    mode = 'form';
  }

  function cancelForm() {
    mode = 'list';
    formError = '';
  }

  function addRule() {
    formRules = [...formRules, emptyRule()];
  }

  function removeRule(index) {
    formRules = formRules.filter((_, i) => i !== index);
  }

  async function submitForm(e) {
    e.preventDefault();
    formError = '';

    const name = formName.trim();
    if (!name) { formError = t("The service name is required."); return; }

    const port = Number.parseInt(formPort, 10);
    if (!Number.isInteger(port) || port < 0 || port > 65535) {
      formError = t("The port must be a number between 0 and 65535.");
      return;
    }

    if (formRules.length === 0) {
      formError = t("At least one rule is required (otherwise no connection gets an answer).");
      return;
    }
    for (const r of formRules) {
      if (!r.name.trim()) { formError = t("Every rule needs a name."); return; }
      if (r.matcherType === 'Prefix' && r.matcherMode === 'hex' && !isValidHex(r.matcherValue)) {
        formError = t("Rule \"{0}\": the hexadecimal prefix is invalid.", r.name);
        return;
      }
      if (r.responseMode === 'hex' && !isValidHex(r.responseValue)) {
        formError = t("Rule \"{0}\": the hexadecimal response is invalid.", r.name);
        return;
      }
    }

    const payload = { name, listen_port: port, rules: formRules.map(ruleToApi) };

    saving = true;
    try {
      if (editingName) {
        const updated = await updateTcpService(editingName, payload);
        services = services.map((s) => (s.name === editingName ? updated : s));
        onNotify(t("TCP service \"{0}\" updated", updated.name), 'success');
      } else {
        const created = await createTcpService(payload);
        services = [...services, created];
        onNotify(t("TCP service \"{0}\" created", created.name), 'success');
      }
      await loadAll();
      mode = 'list';
    } catch (err) {
      formError = err.message;
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    const name = deletePending;
    deletePending = null;
    try {
      await deleteTcpService(name);
      services = services.filter((s) => s.name !== name);
      statuses = statuses.filter((s) => s.name !== name);
      onNotify(t("TCP service \"{0}\" deleted", name), 'success');
    } catch (e) {
      onNotify(t("Error: {0}", e.message), 'error');
    }
  }
</script>

<div class="tcp-manager">
  <div class="list-header">
    <h2>{t("Raw TCP mock")}</h2>
    <div class="header-actions-inline">
      {#if mode === 'list'}
        <button type="button" class="btn btn-primary btn-sm" onclick={startCreate} data-testid="tcp-manager-add-button">{t("+ Add a service")}</button>
      {/if}
      <button type="button" class="btn btn-outline btn-sm" onclick={onBack} data-testid="tcp-manager-back-button">{t("Back")}</button>
    </div>
  </div>

  <p class="field-hint tcp-scope-hint">
    {t("Mocks a simple binary protocol where each connection is ONE message followed by ONE fixed response (ping/heartbeat, handshake). Not suited to LDAP, SMTP or any protocol that sends several messages on the same connection (each one expects its own response, which this mode does not handle).")}
  </p>

  {#if mode === 'form'}
    <form class="tcp-form" onsubmit={submitForm} data-testid="tcp-manager-form">
      <div class="form-row">
        <FormField id="tcp-form-name" label={t("Service name")} required>
          {#snippet children({ id })}
            <input {id} type="text" bind:value={formName} disabled={!!editingName} data-testid="tcp-manager-form-name-input" />
          {/snippet}
        </FormField>
        <FormField id="tcp-form-port" label={t("Listen port")} required hint={t("0-65535. A port already taken by another process makes listening fail (shown in the status).")}>
          {#snippet children({ id })}
            <input {id} type="number" min="0" max="65535" bind:value={formPort} data-testid="tcp-manager-form-port-input" />
          {/snippet}
        </FormField>
      </div>

      <h3>{t("Rules (the first match wins)")}</h3>
      {#each formRules as rule, i (i)}
        <div class="rule-editor" data-testid="tcp-manager-rule-{i}">
          <div class="rule-editor-header">
            <span class="rule-index">{t("Rule {0}", i + 1)}</span>
            <button type="button" class="btn-close" onclick={() => removeRule(i)} aria-label={t("Remove the rule {0}", i + 1)} data-testid="tcp-manager-rule-{i}-remove-button">&times;</button>
          </div>

          <div class="form-row">
            <FormField id="tcp-rule-{i}-name" label={t("Rule name")} required>
              {#snippet children({ id })}
                <input {id} type="text" bind:value={rule.name} data-testid="tcp-manager-rule-{i}-name-input" />
              {/snippet}
            </FormField>
            <FormField id="tcp-rule-{i}-matcher-type" label={t("Condition")}>
              {#snippet children({ id })}
                <select {id} bind:value={rule.matcherType} data-testid="tcp-manager-rule-{i}-matcher-type-select">
                  <option value="Any">{t("Anything (fallback rule)")}</option>
                  <option value="Prefix">{t("Starts with")}</option>
                  <option value="Regex">{t("Regex pattern (on the bytes)")}</option>
                </select>
              {/snippet}
            </FormField>
          </div>

          {#if rule.matcherType === 'Prefix'}
            <div class="form-row">
              <FormField id="tcp-rule-{i}-matcher-mode" label={t("Prefix format")}>
                {#snippet children({ id })}
                  <select {id} bind:value={rule.matcherMode} data-testid="tcp-manager-rule-{i}-matcher-mode-select">
                    <option value="text">{t("Text (UTF-8)")}</option>
                    <option value="hex">{t("Hexadecimal")}</option>
                  </select>
                {/snippet}
              </FormField>
              <FormField id="tcp-rule-{i}-matcher-value" label={t("Expected prefix")} hint={rule.matcherMode === 'hex' ? t("Bytes in hexadecimal, e.g. 300c02010060") : t("The message must start with exactly this text")}>
                {#snippet children({ id })}
                  <input {id} type="text" bind:value={rule.matcherValue} class:mono-input={rule.matcherMode === 'hex'} data-testid="tcp-manager-rule-{i}-matcher-value-input" />
                {/snippet}
              </FormField>
            </div>
          {:else if rule.matcherType === 'Regex'}
            <FormField id="tcp-rule-{i}-matcher-regex" label={t("Regex pattern")} hint={t("Applied to the raw bytes of the message (not necessarily valid UTF-8), regex::bytes syntax.")}>
              {#snippet children({ id })}
                <input {id} type="text" bind:value={rule.matcherValue} class="mono-input" data-testid="tcp-manager-rule-{i}-matcher-regex-input" />
              {/snippet}
            </FormField>
          {/if}

          <div class="form-row">
            <FormField id="tcp-rule-{i}-response-mode" label={t("Response format")}>
              {#snippet children({ id })}
                <select {id} bind:value={rule.responseMode} data-testid="tcp-manager-rule-{i}-response-mode-select">
                  <option value="text">{t("Text (UTF-8)")}</option>
                  <option value="hex">{t("Hexadecimal")}</option>
                </select>
              {/snippet}
            </FormField>
            <FormField id="tcp-rule-{i}-response-value" label={t("Response sent to the client")} hint={rule.responseMode === 'hex' ? t("Bytes in hexadecimal") : t("Empty = closes the connection without sending anything")}>
              {#snippet children({ id })}
                <input {id} type="text" bind:value={rule.responseValue} class:mono-input={rule.responseMode === 'hex'} data-testid="tcp-manager-rule-{i}-response-value-input" />
              {/snippet}
            </FormField>
          </div>
        </div>
      {/each}
      <button type="button" class="btn btn-outline btn-sm" onclick={addRule} data-testid="tcp-manager-add-rule-button">{t("+ Add a rule")}</button>

      {#if formError}
        <p class="form-error" role="alert" data-testid="tcp-manager-form-error">{formError}</p>
      {/if}

      <div class="form-actions">
        <button type="button" class="btn btn-secondary" onclick={cancelForm} data-testid="tcp-manager-form-cancel-button">{t("Cancel")}</button>
        <button type="submit" class="btn btn-primary" disabled={saving} data-testid="tcp-manager-form-save-button">
          {saving ? t("Saving...") : editingName ? t("Save") : t("Create")}
        </button>
      </div>
    </form>
  {:else if loading}
    <p class="loading-text">{t("Loading the TCP services...")}</p>
  {:else if services.length === 0}
    <p class="empty-text" data-testid="tcp-manager-empty-message">{t("No TCP service configured yet.")}</p>
  {:else}
    <ul class="tcp-list">
      {#each services as svc (svc.name)}
        {@const st = statusFor(svc.name)}
        <li class="tcp-card" data-testid="tcp-manager-item-{svc.name}">
          <div class="tcp-info">
            <div class="tcp-info-line">
              <span class="tcp-name">{svc.name}</span>
              <span class="tcp-port">:{svc.listen_port}</span>
              {#if st === null}
                <span class="badge-pill badge-unknown">{t("Unknown status")}</span>
              {:else if st.listening}
                <span class="badge-pill badge-reachable">{t("Listening")}</span>
              {:else}
                <span class="badge-pill badge-unreachable" title={st.error ?? ''}>{t("Bind failed")}</span>
              {/if}
            </div>
            <span class="tcp-meta">{tCount(svc.rules.length, "{0} rule", "{0} rules")}</span>
          </div>
          <div class="tcp-actions">
            <button type="button" class="btn btn-outline btn-sm" onclick={() => startEdit(svc)} data-testid="tcp-manager-edit-button-{svc.name}">{t("Edit")}</button>
            <button type="button" class="btn btn-danger-outline btn-sm" onclick={() => deletePending = svc.name} data-testid="tcp-manager-delete-button-{svc.name}">{t("Delete")}</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <ConfirmDialog
    open={deletePending !== null}
    title={t("Delete the TCP service")}
    message={deletePending ? t("Delete the TCP service \"{0}\"? Its port is released at once.", deletePending) : ''}
    confirmLabel={t("Delete")}
    onConfirm={handleDelete}
    onCancel={() => deletePending = null}
  />
</div>

<style>
  .tcp-manager { max-width: 60rem; }
  .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; gap: 0.75rem; flex-wrap: wrap; }
  .list-header h2 { margin: 0; }
  .header-actions-inline { display: flex; gap: 0.5rem; }

  .tcp-scope-hint { margin: 0 0 1rem; }

  .tcp-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.75rem; }
  .tcp-card {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 0.875rem 1.25rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .tcp-info { display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }
  .tcp-info-line { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  .tcp-name { font-weight: 600; font-size: 0.9375rem; }
  .tcp-port { font-family: monospace; color: var(--color-text-muted); }
  .tcp-meta { color: var(--color-text-muted); font-size: 0.8125rem; }
  .tcp-actions { display: flex; gap: 0.5rem; }

  .loading-text, .empty-text { color: var(--color-text-muted); font-size: 0.875rem; text-align: center; padding: 1rem; }

  .tcp-form { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius); padding: 1rem 1.25rem; }
  .tcp-form h3 { margin: 1rem 0 0.5rem; font-size: 0.9375rem; }

  .rule-editor { border: 1px dashed var(--color-border); border-radius: var(--radius); padding: 0.75rem 1rem; margin-bottom: 0.75rem; }
  .rule-editor-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }
  .rule-index { font-size: 0.8125rem; font-weight: 600; color: var(--color-text-muted); }

  .mono-input { font-family: 'Cascadia Code', 'Fira Code', monospace; }
</style>
