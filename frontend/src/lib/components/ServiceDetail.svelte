<script>
  import ServiceForm from './ServiceForm.svelte';
  import RuleList from './RuleList.svelte';
  import RuleForm from './RuleForm.svelte';
  import UrlHealthBadge from './UrlHealthBadge.svelte';
  import ObservationSuggestions from './ObservationSuggestions.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { updateService, deleteService, reorderRules } from '../api.js';
  import { buildServiceTestUrl } from '../service-url.js';
  import { t } from '../i18n.svelte.js';

  let {
    service,
    availableGroups = [],
    onBack = () => {},
    onUpdate = () => {},
    onDelete = () => {},
    onNotify = () => {},
  } = $props();

  // The URL to call, as the service form shows it, where the rules of the service are written.
  let testUrl = $derived(service ? buildServiceTestUrl({
    name: service.name,
    listenPath: service.listen_path,
    groupCode: availableGroups.find(g => g.name === service.group_name)?.code ?? '',
    baseUrl: typeof window !== 'undefined' ? window.location.origin : '',
  }) : '');
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
  // masquerait le symptome sans corriger la cause.
  async function handleSaveService(updated) {
    const name = service.name;
    const groupName = service.group_name;
    try {
      const result = await updateService(name, groupName, updated);
      onUpdate(result, groupName);
      editing = false;
      onNotify(t("Service \"{0}\" updated", result.name), 'success');
    } catch (e) {
      onNotify(t("Error: {0}", e.message), 'error');
    }
  }

  async function handleDeleteService() {
    confirmDelete = false;
    const name = service.name;
    const groupName = service.group_name;
    try {
      await deleteService(name, groupName);
      onNotify(t("Service \"{0}\" deleted", name), 'success');
      onDelete(name, groupName);
    } catch (e) {
      onNotify(t("Error: {0}", e.message), 'error');
    }
  }

  async function handleReorder(order) {
    const name = service.name;
    const groupName = service.group_name;
    try {
      const result = await reorderRules(name, groupName, order);
      onUpdate(result, groupName);
    } catch (e) {
      onNotify(t("Reordering error: {0}", e.message), 'error');
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
      onNotify(t("Rule \"{0}\" saved", rule.name), 'success');
    } catch (e) {
      onNotify(t("Error: {0}", e.message), 'error');
    }
  }

  function handleCloneRule(idx) {
    const source = JSON.parse(JSON.stringify(service.rules[idx]));
    source.name = '';
    editingRuleIdx = null;
    addingRule = true;
    clonedRule = source;
  }

  // Meme flux que handleCloneRule : pre-remplit le formulaire de creation
  // avec le brouillon de regle suggere, l'utilisateur reste maitre de la
  // relecture/edition/sauvegarde (RuleForm inchange, meme validation, meme
  // detecteur de conflit).
  function handleUseSuggestion(ruleDraft) {
    editingRuleIdx = null;
    addingRule = true;
    clonedRule = ruleDraft;
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
      onNotify(t("Rule deleted"), 'success');
    } catch (e) {
      onNotify(t("Error: {0}", e.message), 'error');
    }
  }
</script>

{#if !service}
  <p>{t("Loading...")}</p>
{:else}
<div class="service-detail">
  <nav class="detail-nav" aria-label={t("Service navigation")}>
    <button type="button" class="btn btn-secondary btn-back" onclick={onBack} data-testid="service-detail-back-button">
      &#8592; {t("Back")}
    </button>
    <h2>{service.name}</h2>
  </nav>

  {#if editing}
    <ServiceForm service={service} {availableGroups} isEdit={true} onSave={handleSaveService} onCancel={() => editing = false} />
  {:else}
    <div class="detail-card">
      <dl class="detail-dl">
        <div class="dl-row">
          <dt>{t("Listen path")}</dt>
          <dd><code>{service.listen_path}</code></dd>
        </div>
        <div class="dl-row">
          <dt>{t("Test URL")}</dt>
          <dd><code data-testid="service-detail-test-url">{testUrl}</code></dd>
        </div>
        {#if service.real_target_url?.trim()}
          <div class="dl-row">
            <dt>{t("Real target URL")}</dt>
            <dd><code>{service.real_target_url}</code></dd>
          </div>
          <div class="dl-row">
            <dt>{t("Availability")}</dt>
            <dd><UrlHealthBadge serviceName={service.name} groupName={service.group_name} /></dd>
          </div>
        {:else}
          <div class="dl-row">
            <dt>{t("Real target URL")}</dt>
            <dd>{t("Purely mocked service (no target)")}</dd>
          </div>
        {/if}
        <div class="dl-row">
          <dt>{t("Directory URL rewriting")}</dt>
          <dd>{service.rewrite_directory_urls ? t("Yes") : t("No")}</dd>
        </div>
      </dl>
      <div class="detail-actions">
        <button type="button" class="btn btn-primary" onclick={() => editing = true} data-testid="service-detail-edit-button">
          {t("Edit the service")}
        </button>
        <button type="button" class="btn btn-danger" onclick={() => confirmDelete = true} data-testid="service-detail-delete-button">
          {t("Delete")}
        </button>
      </div>
    </div>
  {/if}

  <ConfirmDialog
    open={confirmDelete}
    title={t("Delete the service")}
    message={t("Delete the service \"{0}\"? This cannot be undone.", service.name)}
    confirmLabel={t("Yes, delete")}
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
      isPurelyMocked={!service.real_target_url?.trim()}
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
      isPurelyMocked={!service.real_target_url?.trim()}
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
    <ObservationSuggestions
      serviceName={service.name}
      groupName={service.group_name}
      isMocked={service.is_mocked}
      onUseSuggestion={handleUseSuggestion}
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
