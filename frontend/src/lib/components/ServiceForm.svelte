<script>
  let {
    service = null,
    onSave = () => {},
    onCancel = () => {},
  } = $props();

  const initial = service;
  let name = $state(initial?.name ?? '');
  let method = $state(initial?.method ?? 'GET');
  let listenPath = $state(initial?.listen_path ?? '/*');
  let realTargetUrl = $state(initial?.real_target_url ?? 'http://');
  let rewriteDirectoryUrls = $state(initial?.rewrite_directory_urls ?? false);

  const httpMethods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS', 'HEAD'];

  let testUrl = $derived(
    `http://localhost:3000/${name.trim() || '...'}${listenPath.trim().startsWith('/') ? listenPath.trim() : '/' + listenPath.trim()}`
  );
  let saving = $state(false);
  let error = $state('');

  const isEdit = !!initial;

  async function handleSubmit(e) {
    e.preventDefault();
    error = '';

    if (!name.trim()) { error = 'Le nom du service est requis.'; return; }
    if (!listenPath.trim()) { error = "Le chemin d'écoute est requis."; return; }
    if (!realTargetUrl.trim()) { error = "L'URL cible est requise."; return; }

    saving = true;
    try {
      await onSave({
        name: name.trim(),
        method,
        listen_path: listenPath.trim(),
        real_target_url: realTargetUrl.trim(),
        is_mocked: initial?.is_mocked ?? false,
        rewrite_directory_urls: rewriteDirectoryUrls,
        rules: initial?.rules ?? [],
      });
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
    <label for="svc-method">Methode HTTP</label>
    <select id="svc-method" bind:value={method} aria-describedby="svc-method-hint">
      {#each httpMethods as m}
        <option value={m}>{m}</option>
      {/each}
    </select>
    <span class="field-hint" id="svc-method-hint">Un service = une methode. Pour GET + POST sur le meme path, creez 2 services.</span>
  </div>

  <div class="form-field">
    <label for="svc-path">Chemin d'ecoute</label>
    <input
      id="svc-path"
      type="text"
      bind:value={listenPath}
      required
      placeholder={`ex: /v4/api/insee/{siret}`}
      aria-describedby="svc-path-hint"
    />
    <span class="field-hint" id="svc-path-hint">Route interceptee. Utilisez /* pour wildcard ou {`{param}`} pour capturer des segments nommes (ex: /v4/insee/{`{siret}`})</span>
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

  <div class="form-field form-field-check">
    <label>
      <input type="checkbox" bind:checked={rewriteDirectoryUrls} />
      Réécrire les URL d'annuaire
    </label>
    <span class="field-hint">Remplace les URL des backends dans les réponses d'annuaire pour les rediriger via lightMock</span>
  </div>

  {#if name.trim()}
    <div class="url-preview">
      <strong>URL de test :</strong> <code>{method} {testUrl}</code>
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

  .form-error {
    background: #f8d7da;
    border: 1px solid #f1aeb5;
    color: #58151c;
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius);
    margin-bottom: 1rem;
    font-weight: 500;
  }

  .form-field {
    margin-bottom: 1rem;
  }

  .form-field label {
    display: block;
    font-weight: 600;
    margin-bottom: 0.25rem;
  }

  .form-field select {
    width: 100%;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    font-size: 1rem;
    font-family: inherit;
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

  .form-field input[type="text"],
  .form-field input[type="url"] {
    width: 100%;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    font-size: 1rem;
    font-family: inherit;
  }

  .form-field input:focus-visible {
    outline: 3px solid var(--color-primary);
    outline-offset: 1px;
  }

  .form-field input:disabled {
    background: var(--color-bg);
    color: var(--color-text-muted);
  }

  .form-field-check label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 500;
    cursor: pointer;
  }

  .form-field-check input[type="checkbox"] {
    width: 1.125rem;
    height: 1.125rem;
  }

  .field-hint {
    display: block;
    font-size: 0.8125rem;
    color: var(--color-text-muted);
    margin-top: 0.25rem;
  }

  .form-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 1.25rem;
  }

  .btn {
    padding: 0.5rem 1.25rem;
    border-radius: var(--radius);
    border: 1px solid transparent;
    font-weight: 600;
    font-size: 0.9375rem;
    transition: background-color 0.15s;
  }

  .btn-primary {
    background: var(--color-primary);
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--color-primary-hover);
  }

  .btn-primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: var(--color-surface);
    color: var(--color-text);
    border-color: var(--color-border);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg);
  }
</style>
