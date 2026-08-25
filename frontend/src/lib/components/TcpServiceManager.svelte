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
      onNotify(`Erreur chargement des services TCP : ${e.message}`, 'error');
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
    if (!name) { formError = 'Le nom du service est requis.'; return; }

    const port = Number.parseInt(formPort, 10);
    if (!Number.isInteger(port) || port < 0 || port > 65535) {
      formError = 'Le port doit etre un nombre entre 0 et 65535.';
      return;
    }

    if (formRules.length === 0) {
      formError = 'Au moins une regle est requise (sinon aucune connexion ne recevra de reponse).';
      return;
    }
    for (const r of formRules) {
      if (!r.name.trim()) { formError = 'Chaque regle doit avoir un nom.'; return; }
      if (r.matcherType === 'Prefix' && r.matcherMode === 'hex' && !isValidHex(r.matcherValue)) {
        formError = `Regle "${r.name}" : le prefixe hexadecimal saisi est invalide.`;
        return;
      }
      if (r.responseMode === 'hex' && !isValidHex(r.responseValue)) {
        formError = `Regle "${r.name}" : la reponse hexadecimale saisie est invalide.`;
        return;
      }
    }

    const payload = { name, listen_port: port, rules: formRules.map(ruleToApi) };

    saving = true;
    try {
      if (editingName) {
        const updated = await updateTcpService(editingName, payload);
        services = services.map((s) => (s.name === editingName ? updated : s));
        onNotify(`Service TCP "${updated.name}" mis a jour`, 'success');
      } else {
        const created = await createTcpService(payload);
        services = [...services, created];
        onNotify(`Service TCP "${created.name}" cree`, 'success');
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
      onNotify(`Service TCP "${name}" supprime`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }
</script>

<div class="tcp-manager">
  <div class="list-header">
    <h2>Mock TCP brut</h2>
    <div class="header-actions-inline">
      {#if mode === 'list'}
        <button type="button" class="btn btn-primary btn-sm" onclick={startCreate} data-testid="tcp-manager-add-button">+ Ajouter un service</button>
      {/if}
      <button type="button" class="btn btn-outline btn-sm" onclick={onBack} data-testid="tcp-manager-back-button">Retour</button>
    </div>
  </div>

  <p class="field-hint tcp-scope-hint">
    Mocke un protocole binaire simple ou chaque connexion est UN message suivi d'UNE reponse fixe
    (ping/heartbeat, handshake). Ne convient pas a LDAP/SMTP ou tout protocole qui enchaine plusieurs
    messages sur la meme connexion (chaque message y attend sa propre reponse, ce que ce mode ne gere
    pas).
  </p>

  {#if mode === 'form'}
    <form class="tcp-form" onsubmit={submitForm} data-testid="tcp-manager-form">
      <div class="form-row">
        <FormField id="tcp-form-name" label="Nom du service" required>
          {#snippet children({ id })}
            <input {id} type="text" bind:value={formName} disabled={!!editingName} data-testid="tcp-manager-form-name-input" />
          {/snippet}
        </FormField>
        <FormField id="tcp-form-port" label="Port d'ecoute" required hint="0-65535. Un port deja pris par un autre processus fera echouer l'ecoute (visible dans le statut).">
          {#snippet children({ id })}
            <input {id} type="number" min="0" max="65535" bind:value={formPort} data-testid="tcp-manager-form-port-input" />
          {/snippet}
        </FormField>
      </div>

      <h3>Regles (premiere qui matche gagne)</h3>
      {#each formRules as rule, i (i)}
        <div class="rule-editor" data-testid="tcp-manager-rule-{i}">
          <div class="rule-editor-header">
            <span class="rule-index">Regle {i + 1}</span>
            <button type="button" class="btn-close" onclick={() => removeRule(i)} aria-label="Retirer la regle {i + 1}" data-testid="tcp-manager-rule-{i}-remove-button">&times;</button>
          </div>

          <div class="form-row">
            <FormField id="tcp-rule-{i}-name" label="Nom de la regle" required>
              {#snippet children({ id })}
                <input {id} type="text" bind:value={rule.name} data-testid="tcp-manager-rule-{i}-name-input" />
              {/snippet}
            </FormField>
            <FormField id="tcp-rule-{i}-matcher-type" label="Condition">
              {#snippet children({ id })}
                <select {id} bind:value={rule.matcherType} data-testid="tcp-manager-rule-{i}-matcher-type-select">
                  <option value="Any">N'importe quoi (regle de repli)</option>
                  <option value="Prefix">Commence par</option>
                  <option value="Regex">Motif regex (sur les octets)</option>
                </select>
              {/snippet}
            </FormField>
          </div>

          {#if rule.matcherType === 'Prefix'}
            <div class="form-row">
              <FormField id="tcp-rule-{i}-matcher-mode" label="Format du prefixe">
                {#snippet children({ id })}
                  <select {id} bind:value={rule.matcherMode} data-testid="tcp-manager-rule-{i}-matcher-mode-select">
                    <option value="text">Texte (UTF-8)</option>
                    <option value="hex">Hexadecimal</option>
                  </select>
                {/snippet}
              </FormField>
              <FormField id="tcp-rule-{i}-matcher-value" label="Prefixe attendu" hint={rule.matcherMode === 'hex' ? 'Octets en hexadecimal, ex: 300c02010060' : 'Le debut du message doit correspondre exactement a ce texte'}>
                {#snippet children({ id })}
                  <input {id} type="text" bind:value={rule.matcherValue} class:mono-input={rule.matcherMode === 'hex'} data-testid="tcp-manager-rule-{i}-matcher-value-input" />
                {/snippet}
              </FormField>
            </div>
          {:else if rule.matcherType === 'Regex'}
            <FormField id="tcp-rule-{i}-matcher-regex" label="Motif regex" hint="Applique aux octets bruts du message (pas necessairement de l'UTF-8 valide) — syntaxe regex::bytes.">
              {#snippet children({ id })}
                <input {id} type="text" bind:value={rule.matcherValue} class="mono-input" data-testid="tcp-manager-rule-{i}-matcher-regex-input" />
              {/snippet}
            </FormField>
          {/if}

          <div class="form-row">
            <FormField id="tcp-rule-{i}-response-mode" label="Format de la reponse">
              {#snippet children({ id })}
                <select {id} bind:value={rule.responseMode} data-testid="tcp-manager-rule-{i}-response-mode-select">
                  <option value="text">Texte (UTF-8)</option>
                  <option value="hex">Hexadecimal</option>
                </select>
              {/snippet}
            </FormField>
            <FormField id="tcp-rule-{i}-response-value" label="Reponse renvoyee au client" hint={rule.responseMode === 'hex' ? 'Octets en hexadecimal' : 'Vide = ferme la connexion sans rien renvoyer'}>
              {#snippet children({ id })}
                <input {id} type="text" bind:value={rule.responseValue} class:mono-input={rule.responseMode === 'hex'} data-testid="tcp-manager-rule-{i}-response-value-input" />
              {/snippet}
            </FormField>
          </div>
        </div>
      {/each}
      <button type="button" class="btn btn-outline btn-sm" onclick={addRule} data-testid="tcp-manager-add-rule-button">+ Ajouter une regle</button>

      {#if formError}
        <p class="form-error" role="alert" data-testid="tcp-manager-form-error">{formError}</p>
      {/if}

      <div class="form-actions">
        <button type="button" class="btn btn-secondary" onclick={cancelForm} data-testid="tcp-manager-form-cancel-button">Annuler</button>
        <button type="submit" class="btn btn-primary" disabled={saving} data-testid="tcp-manager-form-save-button">
          {saving ? 'Enregistrement...' : editingName ? 'Enregistrer' : 'Creer'}
        </button>
      </div>
    </form>
  {:else if loading}
    <p class="loading-text">Chargement des services TCP...</p>
  {:else if services.length === 0}
    <p class="empty-text" data-testid="tcp-manager-empty-message">Aucun service TCP configure pour le moment.</p>
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
                <span class="badge-pill badge-unknown">Statut inconnu</span>
              {:else if st.listening}
                <span class="badge-pill badge-reachable">Ecoute active</span>
              {:else}
                <span class="badge-pill badge-unreachable" title={st.error ?? ''}>Echec du bind</span>
              {/if}
            </div>
            <span class="tcp-meta">{svc.rules.length} regle{svc.rules.length !== 1 ? 's' : ''}</span>
          </div>
          <div class="tcp-actions">
            <button type="button" class="btn btn-outline btn-sm" onclick={() => startEdit(svc)} data-testid="tcp-manager-edit-button-{svc.name}">Modifier</button>
            <button type="button" class="btn btn-danger-outline btn-sm" onclick={() => deletePending = svc.name} data-testid="tcp-manager-delete-button-{svc.name}">Supprimer</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <ConfirmDialog
    open={deletePending !== null}
    title="Supprimer le service TCP"
    message={deletePending ? `Supprimer le service TCP "${deletePending}" ? Le port sera immediatement libere.` : ''}
    confirmLabel="Supprimer"
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
