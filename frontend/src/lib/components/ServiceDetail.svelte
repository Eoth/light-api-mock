<script>
  import ServiceForm from './ServiceForm.svelte';
  import RuleList from './RuleList.svelte';
  import RuleForm from './RuleForm.svelte';
  import UrlHealthBadge from './UrlHealthBadge.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { updateService, deleteService, reorderRules } from '../api.js';

  let {
    service,
    availableGroups = [],
    onBack = () => {},
    onUpdate = () => {},
    onDelete = () => {},
    onNotify = () => {},
  } = $props();

  let editing = $state(false);
  let editingRuleIdx = $state(null);
  let addingRule = $state(false);
  let confirmDelete = $state(false);

  // Capture le nom/groupe AVANT tout point d'attente (await) : `service` est
  // une prop reactive, et onDelete()/onUpdate() peuvent faire disparaitre le
  // service courant du parent (App.svelte) pendant qu'une requete est en
  // vol, ce qui rend `service` null en cours de route. Lire une valeur
  // capturee au debut de la fonction, plutot que relire la prop apres un
  // await, evite cette course — pas un simple garde `if (!service)` qui
  // masquerait le symptome sans corriger la cause (cf CLAUDE.md).
  async function handleSaveService(updated) {
    const name = service.name;
    const groupName = service.group_name;
    try {
      const result = await updateService(name, groupName, updated);
      onUpdate(result, groupName);
      editing = false;
      onNotify(`Service "${result.name}" mis à jour`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function handleDeleteService() {
    confirmDelete = false;
    const name = service.name;
    const groupName = service.group_name;
    try {
      await deleteService(name, groupName);
      onNotify(`Service "${name}" supprimé`, 'success');
      onDelete(name, groupName);
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function handleReorder(order) {
    const name = service.name;
    const groupName = service.group_name;
    try {
      const result = await reorderRules(name, groupName, order);
      onUpdate(result, groupName);
    } catch (e) {
      onNotify(`Erreur de réordonnancement : ${e.message}`, 'error');
    }
  }

  async function handleSaveRule(rule) {
    const rules = [...(service.rules || [])];
    if (editingRuleIdx !== null) {
      rules[editingRuleIdx] = rule;
    } else {
      rules.push(rule);
    }
    const updated = { ...service, rules };
    const name = service.name;
    const groupName = service.group_name;
    try {
      const result = await updateService(name, groupName, updated);
      onUpdate(result, groupName);
      editingRuleIdx = null;
      addingRule = false;
      onNotify(`Règle "${rule.name}" enregistrée`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  function handleCloneRule(idx) {
    const source = JSON.parse(JSON.stringify(service.rules[idx]));
    source.name = '';
    editingRuleIdx = null;
    addingRule = true;
    clonedRule = source;
  }

  let clonedRule = $state(null);

  async function handleDeleteRule(idx) {
    const rules = service.rules.filter((_, i) => i !== idx);
    const updated = { ...service, rules };
    const name = service.name;
    const groupName = service.group_name;
    try {
      const result = await updateService(name, groupName, updated);
      onUpdate(result, groupName);
      onNotify('Règle supprimée', 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }
</script>

{#if !service}
  <p>Chargement...</p>
{:else}
<div class="service-detail">
  <nav class="detail-nav" aria-label="Navigation du service">
    <button type="button" class="btn btn-secondary btn-back" onclick={onBack} data-testid="service-detail-back-button">
      &#8592; Retour
    </button>
    <h2>{service.name}</h2>
  </nav>

  {#if editing}
    <ServiceForm service={service} {availableGroups} isEdit={true} onSave={handleSaveService} onCancel={() => editing = false} />
  {:else}
    <div class="detail-card">
      <dl class="detail-dl">
        <div class="dl-row">
          <dt>Chemin d'écoute</dt>
          <dd><code>{service.listen_path}</code></dd>
        </div>
        <div class="dl-row">
          <dt>URL cible réelle</dt>
          <dd><code>{service.real_target_url}</code></dd>
        </div>
        <div class="dl-row">
          <dt>Disponibilité</dt>
          <dd><UrlHealthBadge serviceName={service.name} groupName={service.group_name} /></dd>
        </div>
        <div class="dl-row">
          <dt>Réécriture annuaire</dt>
          <dd>{service.rewrite_directory_urls ? 'Oui' : 'Non'}</dd>
        </div>
      </dl>
      <div class="detail-actions">
        <button type="button" class="btn btn-primary" onclick={() => editing = true} data-testid="service-detail-edit-button">
          Modifier le service
        </button>
        <button type="button" class="btn btn-danger" onclick={() => confirmDelete = true} data-testid="service-detail-delete-button">
          Supprimer
        </button>
      </div>
    </div>
  {/if}

  <ConfirmDialog
    open={confirmDelete}
    title="Supprimer le service"
    message={`Confirmer la suppression du service "${service.name}" ? Cette action est irreversible.`}
    confirmLabel="Oui, supprimer"
    onConfirm={handleDeleteService}
    onCancel={() => confirmDelete = false}
  />

  {#if editingRuleIdx !== null}
    <RuleForm
      rule={service.rules[editingRuleIdx]}
      existingRules={(service.rules ?? []).filter((_, i) => i !== editingRuleIdx)}
      draftPosition={editingRuleIdx}
      serviceName={service.name}
      groupName={service.group_name}
      listenPath={service.listen_path}
      onSave={handleSaveRule}
      onCancel={() => editingRuleIdx = null}
    />
  {:else if addingRule}
    <RuleForm
      rule={clonedRule}
      existingRules={service.rules ?? []}
      draftPosition={(service.rules ?? []).length}
      serviceName={service.name}
      groupName={service.group_name}
      listenPath={service.listen_path}
      onSave={handleSaveRule}
      onCancel={() => { addingRule = false; clonedRule = null; }}
    />
  {:else}
    <RuleList
      rules={service.rules ?? []}
      onReorder={handleReorder}
      onEditRule={(idx) => editingRuleIdx = idx}
      onDeleteRule={handleDeleteRule}
      onCloneRule={handleCloneRule}
      onAddRule={() => { addingRule = true; clonedRule = null; }}
    />
  {/if}
</div>
{/if}

<style>
  .service-detail { display: flex; flex-direction: column; gap: 1rem; }

  .detail-nav {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .detail-nav h2 { margin: 0; }

  .detail-card {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 1.25rem;
  }

  .detail-dl { margin: 0; }
  .dl-row { display: flex; gap: 0.5rem; margin-bottom: 0.375rem; }
  dt { font-weight: 500; color: var(--color-text-muted); min-width: 10rem; }
  dd { margin: 0; }
  code { font-size: 0.875rem; background: var(--color-bg); padding: 0.125rem 0.375rem; border-radius: 3px; }

  .detail-actions {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--color-border);
  }

  .btn-back { padding: 0.375rem 0.75rem; font-size: 0.875rem; }
</style>
