<script>
  import { untrack } from 'svelte';

  let {
    service = null,
    existingNames = [],
    availableGroups = [],
    onSave = () => {},
    onCancel = () => {},
  } = $props();

  const isEdit = untrack(() => !!service);
  let name = $state(untrack(() => service?.name ?? ''));
  let listenPath = $state(untrack(() => service?.listen_path ?? ''));
  let realTargetUrl = $state(untrack(() => service?.real_target_url ?? 'http://'));
  let serviceType = $state(untrack(() => {
    if (service?.wsdl_mode === 'mock' || service?.wsdl_mode === 'proxy') return 'soap';
    return service?.rewrite_directory_urls ? 'soap' : 'rest';
  }));
  let groupName = $state(untrack(() => service?.group_name ?? ''));

  const RESERVED_NAMES = ['api', 'auth', 'index.html', 'assets', 'favicon.ico'];

  const baseUrl = typeof window !== 'undefined' ? window.location.origin : '';

  let testUrl = $derived(() => {
    const n = name.trim() || '...';
    const p = listenPath.trim();
    const g = availableGroups.find(gr => gr.name === groupName);
    const prefix = g ? `/${g.code}/${n}` : `/${n}`;
    const path = p ? (p.startsWith('/') ? p : '/' + p) : '/*';
    return `${baseUrl}${prefix}${path}`;
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
    return null;
  }

  function validatePath(_p) {
    return null;
  }

  async function handleSubmit(e) {
    e.preventDefault();
    error = '';

    const nameErr = validateName(name);
    if (nameErr) { error = nameErr; return; }

    const pathErr = validatePath(listenPath);
    if (pathErr) { error = pathErr; return; }

    if (!realTargetUrl.trim()) { error = "L'URL cible est requise."; return; }

    saving = true;
    try {
      const isSoap = serviceType === 'soap';
      const payload = {
        name: name.trim(),
        listen_path: listenPath.trim(),
        real_target_url: realTargetUrl.trim(),
        is_mocked: service?.is_mocked ?? false,
        rewrite_directory_urls: isSoap,
        wsdl_mode: isSoap ? 'auto' : 'auto',
        rules: service?.rules ?? [],
      };
      if (groupName) payload.group_name = groupName;
      await onSave(payload);
    } catch (e) {
      error = e.message;
    } finally {
      saving = false;
    }
  }
</script>

<form class="service-form" onsubmit={handleSubmit} aria-label={isEdit ? `Modifier le service ${name}` : 'Ajouter un service'}>
  {#if error}
    <div class="form-error" role="alert" aria-live="assertive">{error}</div>
  {/if}

  <div class="form-field">
    <label for="svc-name">Nom du service</label>
    <input
      id="svc-name"
      type="text"
      bind:value={name}
      required
      disabled={isEdit}
      placeholder="ex: service-users"
      aria-describedby="svc-name-hint"
    />
    <span class="field-hint" id="svc-name-hint">Identifiant unique, sert aussi de prefixe URL : /{`{nom}`}/...</span>
  </div>

  <div class="form-field">
    <label for="svc-path">Chemin d'ecoute (optionnel)</label>
    <input
      id="svc-path"
      type="text"
      bind:value={listenPath}
      placeholder="Vide = intercepte tout sous le nom du service"
      aria-describedby="svc-path-hint"
    />
    <span class="field-hint" id="svc-path-hint">Laissez vide pour intercepter tout le trafic sous /{`{nom}`}/. Sinon, utilisez /* pour wildcard ou {`{param}`} pour capturer des segments.</span>
  </div>

  <div class="form-field">
    <label for="svc-target">URL cible réelle</label>
    <input
      id="svc-target"
      type="url"
      bind:value={realTargetUrl}
      required
      placeholder="ex: http://service-users.default.svc:8080"
      aria-describedby="svc-target-hint"
    />
    <span class="field-hint" id="svc-target-hint">Adresse du vrai backend dans le cluster (utilisée en mode proxy)</span>
  </div>

  <div class="form-field">
    <label for="svc-type">Type de service</label>
    <select id="svc-type" bind:value={serviceType} aria-describedby="svc-type-hint">
      <option value="rest">REST</option>
      <option value="soap">SOAP / XML</option>
    </select>
    <span class="field-hint" id="svc-type-hint">
      {#if serviceType === 'soap'}
        Les requetes ?wsdl seront automatiquement proxyfiees vers le backend reel.
      {:else}
        API REST standard (JSON).
      {/if}
    </span>
  </div>

  {#if availableGroups.length > 0}
    <div class="form-field">
      <label for="svc-group">Groupe</label>
      <select id="svc-group" bind:value={groupName} aria-describedby="svc-group-hint">
        <option value="">-- Aucun groupe --</option>
        {#each availableGroups as g}
          <option value={g.name}>{g.name} (/{g.code})</option>
        {/each}
      </select>
      <span class="field-hint" id="svc-group-hint">Associe le service a un groupe pour gerer les droits d'acces</span>
    </div>
  {/if}

  {#if name.trim()}
    <div class="url-preview">
      <strong>URL de test :</strong> <code>{testUrl()}</code>
    </div>
  {/if}

  <div class="form-actions">
    <button type="submit" class="btn btn-primary" disabled={saving}>
      {saving ? 'Enregistrement...' : isEdit ? 'Enregistrer' : 'Ajouter'}
    </button>
    <button type="button" class="btn btn-secondary" onclick={onCancel} disabled={saving}>
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
