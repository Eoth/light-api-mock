<script>
  import { untrack } from 'svelte';
  import FormField from './FormField.svelte';
  import ToggleSwitch from './ToggleSwitch.svelte';
  import { buildServiceTestUrl } from '../service-url.js';

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
  // Service "purement mocke" (sujet 22) : deduit de real_target_url plutot
  // qu'un nouveau champ persiste (voir CLAUDE.md §3) — une cible vide EST la
  // definition de "purement mocke". `??` ne remplace pas une chaine vide,
  // donc un service existant avec real_target_url: "" est correctement
  // detecte comme deja purement mocke a l'ouverture du formulaire.
  let purelyMocked = $state(untrack(() => service ? !service.real_target_url?.trim() : false));
  // Regles action=Proxy deja presentes sur ce service (avant edition) :
  // sert uniquement a l'avertissement de bascule a posteriori (point 7,
  // meme esprit non-bloquant que le detecteur de conflit de regles, sujet
  // 14) quand l'utilisateur coche "purement mocke" alors que ces regles
  // existent deja.
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
    if (!trimmed) return 'Le nom du service est requis.';
    if (RESERVED_NAMES.includes(trimmed.toLowerCase())) {
      return `Le nom "${trimmed}" est reserve par lightMock (noms interdits : ${RESERVED_NAMES.join(', ')}).`;
    }
    if (trimmed.includes('/') || trimmed.includes('\\')) {
      return 'Le nom du service ne peut pas contenir de separateur de chemin (/ ou \\).';
    }
    if (!/^[A-Za-z0-9_-]+$/.test(trimmed)) {
      return 'Le nom du service ne peut contenir que des lettres, chiffres, tirets (-) et underscores (_).';
    }
    return null;
  }

  function validatePath(_p) {
    return null;
  }

  // Bascule d'affichage : decocher reaffiche le champ cible sans perdre la
  // valeur precedemment saisie (realTargetUrl n'est jamais efface quand la
  // case est cochee, seul son rendu est conditionne — point 2 du sujet 22).
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

    if (!purelyMocked && !realTargetUrl.trim()) { error = "L'URL cible est requise."; return; }

    const payload = buildPayload();

    // Bascule a posteriori (point 7) : avertir plutot que bloquer, meme
    // esprit non-bloquant que le detecteur de conflit de regles (sujet 14,
    // RuleForm.svelte) — une regle Proxy existante ne casse rien tant que
    // l'utilisateur n'a pas explicitement confirme vouloir passer outre.
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

<form class="service-form" onsubmit={handleSubmit} aria-label={isEdit ? `Modifier le service ${name}` : 'Ajouter un service'}>
  {#if error}
    <div class="form-error" role="alert" aria-live="assertive" data-testid="service-form-error">{error}</div>
  {/if}

  <FormField id="svc-name" label="Nom du service" hint="Identifiant unique, sert aussi de prefixe URL : /{`{nom}`}/...">
    {#snippet children({ id, describedBy })}
      <input
        {id}
        type="text"
        bind:value={name}
        required
        disabled={isEdit}
        placeholder="ex: service-users"
        aria-describedby={describedBy}
        data-testid="service-form-name-input"
      />
    {/snippet}
  </FormField>

  <FormField id="svc-path" label="Chemin d'ecoute (optionnel)" hint="Laissez vide pour intercepter tout le trafic sous /{`{nom}`}/. Sinon, utilisez /* pour wildcard ou {`{param}`} pour capturer des segments.">
    {#snippet children({ id, describedBy })}
      <input
        {id}
        type="text"
        bind:value={listenPath}
        placeholder="Vide = intercepte tout sous le nom du service"
        aria-describedby={describedBy}
        data-testid="service-form-path-input"
      />
    {/snippet}
  </FormField>

  <div class="form-field">
    <ToggleSwitch
      label="Service purement mocké"
      checked={purelyMocked}
      onchange={handlePurelyMockedChange}
    />
    <span class="field-hint">Aucune cible réelle : pas de mode proxy, pas de test de disponibilité. Peut être activé à tout moment sans perdre les règles déjà configurées.</span>
  </div>

  {#if !purelyMocked}
    <FormField id="svc-target" label="URL cible réelle" hint="Adresse du vrai backend dans le cluster (utilisée en mode proxy)">
      {#snippet children({ id, describedBy })}
        <input
          {id}
          type="url"
          bind:value={realTargetUrl}
          required
          placeholder="ex: http://service-users.default.svc:8080"
          aria-describedby={describedBy}
          data-testid="service-form-target-input"
        />
      {/snippet}
    </FormField>
  {/if}

  {#if pendingPurelyMockedWarning}
    <div class="mode-warning" role="alert" data-testid="service-form-purely-mocked-warning">
      <p>
        &#9888; {proxyRulesAffected.length > 1 ? 'Ces règles' : 'Cette règle'} de ce service {proxyRulesAffected.length > 1 ? 'sont' : 'est'} en action "Proxy" et ne {proxyRulesAffected.length > 1 ? 'fonctionneront' : 'fonctionnera'} plus une fois le service marqué purement mocké (elle{proxyRulesAffected.length > 1 ? 's' : ''} renverra une erreur claire au lieu de relayer vers une cible) :
        {proxyRulesAffected.map(r => r.name).join(', ')}.
      </p>
      <div class="mode-warning-actions">
        <button type="button" class="btn btn-sm btn-primary" onclick={confirmSaveDespitePurelyMockedWarning} data-testid="service-form-purely-mocked-save-anyway-button">Enregistrer quand même</button>
        <button type="button" class="btn btn-sm btn-secondary" onclick={cancelPurelyMockedWarning} data-testid="service-form-purely-mocked-cancel-button">Revenir en arrière</button>
      </div>
    </div>
  {/if}

  <FormField id="svc-type" label="Type de service" hint={serviceType === 'soap' ? 'Les requetes ?wsdl seront automatiquement proxyfiees vers le backend reel.' : 'API REST standard (JSON).'}>
    {#snippet children({ id, describedBy })}
      <select {id} bind:value={serviceType} aria-describedby={describedBy} data-testid="service-form-type-select">
        <option value="rest">REST</option>
        <option value="soap">SOAP / XML</option>
      </select>
    {/snippet}
  </FormField>

  {#if availableGroups.length > 0}
    <FormField id="svc-group" label="Groupe" hint="Associe le service a un groupe pour gerer les droits d'acces">
      {#snippet children({ id, describedBy })}
        <select {id} bind:value={groupName} aria-describedby={describedBy} data-testid="service-form-group-select">
          <option value="">-- Aucun groupe --</option>
          {#each availableGroups as g}
            <option value={g.name}>{g.name} (/{g.code})</option>
          {/each}
        </select>
      {/snippet}
    </FormField>
  {/if}

  {#if name.trim()}
    <div class="url-preview">
      <strong>URL de test :</strong> <code data-testid="service-form-url-preview">{testUrl()}</code>
    </div>
  {/if}

  <div class="form-actions">
    <button type="submit" class="btn btn-primary" disabled={saving} data-testid="service-form-submit-button">
      {saving ? 'Enregistrement...' : isEdit ? 'Enregistrer' : 'Ajouter'}
    </button>
    <button type="button" class="btn btn-secondary" onclick={onCancel} disabled={saving} data-testid="service-form-cancel-button">
      Annuler
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
     (detecteur de conflit de regles, sujet 14) : classe locale, pas de
     redefinition d'une classe centralisee d'app.css (voir CLAUDE.md §5). */
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
