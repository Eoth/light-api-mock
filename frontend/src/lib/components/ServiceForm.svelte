<script>
  import { untrack } from 'svelte';
  import FormField from './FormField.svelte';
  import ToggleSwitch from './ToggleSwitch.svelte';
  import { buildServiceTestUrl } from '../service-url.js';
  import { t, tCount } from '../i18n.svelte.js';

  let {
    service = null,
    existingNames = [],
    availableGroups = [],
    isEdit = false,
    onSave = () => {},
    onCancel = () => {},
  } = $props();
  let name = $state(untrack(() => service?.name ?? ''));
  let listenPath = $state(untrack(() => service?.listen_path ?? ''));
  let realTargetUrl = $state(untrack(() => service?.real_target_url ?? 'http://'));
  // Service "purement mocke" : deduit de real_target_url plutot qu'un
  // nouveau champ persiste — une cible vide EST la definition de "purement
  // mocke". `??` ne remplace pas une chaine vide, donc un service existant
  // avec real_target_url: "" est correctement detecte comme deja purement
  // mocke a l'ouverture du formulaire.
  let purelyMocked = $state(untrack(() => service ? !service.real_target_url?.trim() : false));
  // Regles action=Proxy deja presentes sur ce service (avant edition) :
  // sert uniquement a l'avertissement de bascule a posteriori (meme esprit
  // non-bloquant que le detecteur de conflit de regles) quand l'utilisateur
  // coche "purement mocke" alors que ces regles existent deja.
  const proxyRulesAffected = untrack(() => (service?.rules ?? []).filter((r) => r.action === 'proxy'));
  let pendingPurelyMockedWarning = $state(false);
  let pendingPayload = $state(null);
  let serviceType = $state(untrack(() => {
    if (service?.wsdl_mode === 'mock' || service?.wsdl_mode === 'proxy') return 'soap';
    return service?.rewrite_directory_urls ? 'soap' : 'rest';
  }));
  let groupName = $state(untrack(() => service?.group_name ?? ''));

  const RESERVED_NAMES = ['api', 'auth', 'index.html', 'assets', 'favicon.ico'];

  const baseUrl = typeof window !== 'undefined' ? window.location.origin : '';

  let testUrl = $derived(() => {
    const g = availableGroups.find(gr => gr.name === groupName);
    return buildServiceTestUrl({ name, listenPath, groupCode: g?.code ?? '', baseUrl });
  });
  let saving = $state(false);
  let error = $state('');


  function validateName(n) {
    const trimmed = n.trim();
    if (!trimmed) return t("The service name is required.");
    if (RESERVED_NAMES.includes(trimmed.toLowerCase())) {
      return t("The name \"{0}\" is reserved by Mimicway (forbidden names: {1}).", trimmed, RESERVED_NAMES.join(', '));
    }
    if (trimmed.includes('/') || trimmed.includes('\\')) {
      return t("A service name cannot contain a path separator (/ or \\).");
    }
    if (!/^[A-Za-z0-9_-]+$/.test(trimmed)) {
      return t("A service name can only contain letters, digits, dashes (-) and underscores (_).");
    }
    return null;
  }

  function validatePath(_p) {
    return null;
  }

  // Bascule d'affichage : decocher reaffiche le champ cible sans perdre la
  // valeur precedemment saisie (realTargetUrl n'est jamais efface quand la
  // case est cochee, seul son rendu est conditionne).
  // Si le champ n'a jamais eu de valeur exploitable, un point de depart
  // pratique ('http://') est propose, comme pour un service tout neuf.
  function handlePurelyMockedChange(val) {
    purelyMocked = val;
    if (!val && !realTargetUrl.trim()) {
      realTargetUrl = 'http://';
    }
  }

  function buildPayload() {
    const isSoap = serviceType === 'soap';
    // Service purement mocke = is_mocked force a true (une cible vide en
    // proxy direct n'a aucun sens, cf validate_service cote backend) et
    // real_target_url toujours envoye vide, quoi que contienne encore le
    // champ cache (il n'est jamais lu dans ce cas).
    const payload = {
      name: name.trim(),
      listen_path: listenPath.trim(),
      real_target_url: purelyMocked ? '' : realTargetUrl.trim(),
      is_mocked: purelyMocked ? true : (service?.is_mocked ?? false),
      rewrite_directory_urls: isSoap,
      wsdl_mode: isSoap ? 'auto' : 'auto',
      rules: service?.rules ?? [],
    };
    if (groupName) payload.group_name = groupName;
    return payload;
  }

  async function submitPayload(payload) {
    saving = true;
    try {
      await onSave(payload);
    } catch (e) {
      error = e.message;
    } finally {
      saving = false;
    }
  }

  async function handleSubmit(e) {
    e.preventDefault();
    error = '';
    pendingPurelyMockedWarning = false;
    pendingPayload = null;

    const nameErr = validateName(name);
    if (nameErr) { error = nameErr; return; }

    const pathErr = validatePath(listenPath);
    if (pathErr) { error = pathErr; return; }

    if (!purelyMocked && !realTargetUrl.trim()) { error = t("The target URL is required."); return; }

    const payload = buildPayload();

    // Bascule a posteriori : avertir plutot que bloquer, meme esprit
    // non-bloquant que le detecteur de conflit de regles (RuleForm.svelte)
    // — une regle Proxy existante ne casse rien tant que l'utilisateur n'a
    // pas explicitement confirme vouloir passer outre.
    if (purelyMocked && proxyRulesAffected.length > 0) {
      pendingPurelyMockedWarning = true;
      pendingPayload = payload;
      return;
    }

    await submitPayload(payload);
  }

  function confirmSaveDespitePurelyMockedWarning() {
    const payload = pendingPayload;
    pendingPurelyMockedWarning = false;
    pendingPayload = null;
    if (payload) submitPayload(payload);
  }

  function cancelPurelyMockedWarning() {
    pendingPurelyMockedWarning = false;
    pendingPayload = null;
  }
</script>

<form class="service-form" onsubmit={handleSubmit} aria-label={isEdit ? t("Edit the service {0}", name) : t("Add a service")}>
  {#if error}
    <div class="form-error" role="alert" aria-live="assertive" data-testid="service-form-error">{error}</div>
  {/if}

  <FormField id="svc-name" label={t("Service name")} hint={t("Unique identifier, also the URL prefix: /{name}/...")}>
    {#snippet children({ id, describedBy })}
      <input
        {id}
        type="text"
        bind:value={name}
        required
        disabled={isEdit}
        placeholder={t("e.g. users-service")}
        aria-describedby={describedBy}
        data-testid="service-form-name-input"
      />
    {/snippet}
  </FormField>

  <FormField id="svc-path" label={t("Listen path (optional)")} hint={t("Leave empty to intercept all the traffic under /{name}/. Otherwise use /* as a wildcard or {param} to capture segments.")}>
    {#snippet children({ id, describedBy })}
      <input
        {id}
        type="text"
        bind:value={listenPath}
        placeholder={t("Empty = intercepts everything under the service name")}
        aria-describedby={describedBy}
        data-testid="service-form-path-input"
      />
    {/snippet}
  </FormField>

  <div class="form-field">
    <ToggleSwitch
      label={t("Purely mocked service")}
      name="purely-mocked"
      checked={purelyMocked}
      onchange={handlePurelyMockedChange}
    />
    <span class="field-hint">{t("No real target: no proxy mode, no availability test. Can be switched on at any time without losing the rules already configured.")}</span>
  </div>

  {#if !purelyMocked}
    <FormField id="svc-target" label={t("Real target URL")} hint={t("Address of the real backend (used in proxy mode)")}>
      {#snippet children({ id, describedBy })}
        <input
          {id}
          type="url"
          bind:value={realTargetUrl}
          required
          placeholder={t("e.g. http://users-service.default.svc:8080")}
          aria-describedby={describedBy}
          data-testid="service-form-target-input"
        />
      {/snippet}
    </FormField>
  {/if}

  {#if pendingPurelyMockedWarning}
    <div class="mode-warning" role="alert" data-testid="service-form-purely-mocked-warning">
      <p>
        {tCount(proxyRulesAffected.length, "⚠ This rule of the service uses the \"Proxy\" action and will stop working once the service is purely mocked (it will return a clear error instead of forwarding to a target): {1}.", "⚠ These rules of the service use the \"Proxy\" action and will stop working once the service is purely mocked (they will return a clear error instead of forwarding to a target): {1}.", proxyRulesAffected.map(r => r.name).join(', '))}
      </p>
      <div class="mode-warning-actions">
        <button type="button" class="btn btn-sm btn-primary" onclick={confirmSaveDespitePurelyMockedWarning} data-testid="service-form-purely-mocked-save-anyway-button">{t("Save anyway")}</button>
        <button type="button" class="btn btn-sm btn-secondary" onclick={cancelPurelyMockedWarning} data-testid="service-form-purely-mocked-cancel-button">{t("Go back")}</button>
      </div>
    </div>
  {/if}

  <FormField id="svc-type" label={t("Service type")} hint={serviceType === 'soap' ? t("?wsdl requests will be forwarded to the real backend automatically.") : t("Standard REST API (JSON).")}>
    {#snippet children({ id, describedBy })}
      <select {id} bind:value={serviceType} aria-describedby={describedBy} data-testid="service-form-type-select">
        <option value="rest">{t("REST")}</option>
        <option value="soap">{t("SOAP / XML")}</option>
      </select>
    {/snippet}
  </FormField>

  {#if availableGroups.length > 0}
    <FormField id="svc-group" label={t("Group")} hint={t("Puts the service in a group, which manages access rights")}>
      {#snippet children({ id, describedBy })}
        <select {id} bind:value={groupName} aria-describedby={describedBy} data-testid="service-form-group-select">
          <option value="">{t("-- No group --")}</option>
          {#each availableGroups as g}
            <option value={g.name}>{g.name} (/{g.code})</option>
          {/each}
        </select>
      {/snippet}
    </FormField>
  {/if}

  {#if name.trim()}
    <div class="url-preview">
      <strong>{t("Test URL:")}</strong> <code data-testid="service-form-url-preview">{testUrl()}</code>
    </div>
  {/if}

  <div class="form-actions">
    <button type="submit" class="btn btn-primary" disabled={saving} data-testid="service-form-submit-button">
      {saving ? t("Saving...") : isEdit ? t("Save") : t("Add")}
    </button>
    <button type="button" class="btn btn-secondary" onclick={onCancel} disabled={saving} data-testid="service-form-cancel-button">
      {t("Cancel")}
    </button>
  </div>
</form>

<style>
  .service-form {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 1.5rem;
  }

  /* Meme pattern d'avertissement non-bloquant que RuleForm.svelte
     (detecteur de conflit de regles) : classe locale, pas de redefinition
     d'une classe centralisee d'app.css. */
  .mode-warning { background: #fff3cd; border: 1px solid #ffc107; color: #664d03; padding: 0.75rem; border-radius: var(--radius); margin-bottom: 0.75rem; }
  :global([data-theme="dark"]) .mode-warning { background: #332701; border-color: #e5a50a; color: #ffe082; }
  .mode-warning p { margin: 0 0 0.5rem; font-size: 0.875rem; }
  .mode-warning-actions { display: flex; gap: 0.5rem; flex-wrap: wrap; }

  .url-preview {
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 0.625rem 0.75rem;
    margin-bottom: 1rem;
    font-size: 0.875rem;
  }
  .url-preview code { background: none; padding: 0; font-weight: 600; color: var(--color-primary); }

</style>
