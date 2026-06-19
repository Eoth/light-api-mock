<script>
  import ServiceForm from './ServiceForm.svelte';
  import RuleList from './RuleList.svelte';
  import RuleForm from './RuleForm.svelte';
  import { putService, deleteService, reorderRules } from '../api.js';

  let {
    service,
    onBack = () => {},
    onUpdate = () => {},
    onDelete = () => {},
    onNotify = () => {},
  } = $props();

  let editing = $state(false);
  let editingRuleIdx = $state(null);
  let addingRule = $state(false);
  let confirmDelete = $state(false);

  async function handleSaveService(updated) {
    try {
      const result = await putService(service.name, updated);
      onUpdate(result);
      editing = false;
      onNotify(`Service "${result.name}" mis à jour`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function handleDeleteService() {
    try {
      await deleteService(service.name);
      onDelete(service.name);
      onNotify(`Service "${service.name}" supprimé`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function handleReorder(order) {
    try {
      const result = await reorderRules(service.name, order);
      onUpdate(result);
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
    try {
      const result = await putService(service.name, updated);
      onUpdate(result);
      editingRuleIdx = null;
      addingRule = false;
      onNotify(`Règle "${rule.name}" enregistrée`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function handleDeleteRule(idx) {
    const rules = service.rules.filter((_, i) => i !== idx);
    const updated = { ...service, rules };
    try {
      const result = await putService(service.name, updated);
      onUpdate(result);
      onNotify('Règle supprimée', 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }
</script>

<div class="service-detail">
  <nav class="detail-nav" aria-label="Navigation du service">
    <button type="button" class="btn btn-secondary btn-back" onclick={onBack}>
      &#8592; Retour
    </button>
    <h2>{service.name}</h2>
  </nav>

  {#if editing}
    <ServiceForm service={service} onSave={handleSaveService} onCancel={() => editing = false} />
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
          <dt>Réécriture annuaire</dt>
          <dd>{service.rewrite_directory_urls ? 'Oui' : 'Non'}</dd>
        </div>
      </dl>
      <div class="detail-actions">
        <button type="button" class="btn btn-primary" onclick={() => editing = true}>
          Modifier le service
        </button>
        {#if confirmDelete}
          <span class="confirm-msg" role="alert">
            Confirmer la suppression ?
            <button type="button" class="btn btn-danger btn-sm" onclick={handleDeleteService}>Oui, supprimer</button>
            <button type="button" class="btn btn-secondary btn-sm" onclick={() => confirmDelete = false}>Annuler</button>
          </span>
        {:else}
          <button type="button" class="btn btn-danger" onclick={() => confirmDelete = true}>
            Supprimer
          </button>
        {/if}
      </div>
    </div>
  {/if}

  {#if editingRuleIdx !== null}
    <RuleForm
      rule={service.rules[editingRuleIdx]}
      onSave={handleSaveRule}
      onCancel={() => editingRuleIdx = null}
    />
  {:else if addingRule}
    <RuleForm onSave={handleSaveRule} onCancel={() => addingRule = false} />
  {:else}
    <RuleList
      rules={service.rules ?? []}
      onReorder={handleReorder}
      onEditRule={(idx) => editingRuleIdx = idx}
      onDeleteRule={handleDeleteRule}
      onAddRule={() => addingRule = true}
    />
  {/if}
</div>

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

  .confirm-msg { display: flex; align-items: center; gap: 0.5rem; font-weight: 500; }

  .btn { padding: 0.5rem 1.25rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; font-size: 0.9375rem; }
  .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; }
  .btn-primary { background: var(--color-primary); color: #fff; }
  .btn-primary:hover { background: var(--color-primary-hover); }
  .btn-secondary { background: var(--color-surface); color: var(--color-text); border-color: var(--color-border); }
  .btn-secondary:hover { background: var(--color-bg); }
  .btn-danger { background: var(--color-danger); color: #fff; }
  .btn-danger:hover { background: #b02a37; }
  .btn-back { padding: 0.375rem 0.75rem; font-size: 0.875rem; }
</style>
