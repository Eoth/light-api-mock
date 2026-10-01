<script>
  import ToggleSwitch from './ToggleSwitch.svelte';
  import StatusBadge from './StatusBadge.svelte';
  import UrlHealthBadge from './UrlHealthBadge.svelte';
  import { buildServiceTestUrl } from '../service-url.js';
  import { t } from '../i18n.svelte.js';

  let { service, groupCode = '', onToggle = () => {}, onSelect = () => {}, onClone = () => {} } = $props();

  let testUrl = $derived(buildServiceTestUrl({ name: service.name, listenPath: service.listen_path, groupCode }));
</script>

<article class="service-card" aria-label={t("Service {0}", service.name)} data-testid="service-card-{service.name}">
  <div class="card-header">
    <div class="card-info">
      <h3 class="card-title">{service.name}</h3>
      <StatusBadge active={service.is_mocked} />
    </div>
    <ToggleSwitch
      label={t("Mock {0}", service.name)}
      name="mock-{service.name}"
      checked={service.is_mocked}
      onchange={(val) => onToggle(service.name, val, service.group_name)}
    />
  </div>
  <div class="card-details" id="desc-{service.name}">
    <dl>
      <div class="detail-row">
        <dt>{t("Test URL")}</dt>
        <dd><code>{testUrl}</code></dd>
      </div>
      {#if service.real_target_url?.trim()}
        <div class="detail-row">
          <dt>{t("Target")}</dt>
          <dd><code>{service.real_target_url}</code></dd>
        </div>
        <div class="detail-row">
          <dt>{t("Availability")}</dt>
          <dd><UrlHealthBadge serviceName={service.name} groupName={service.group_name} /></dd>
        </div>
      {:else}
        <div class="detail-row">
          <dt>{t("Target")}</dt>
          <dd>{t("Purely mocked service (no target)")}</dd>
        </div>
      {/if}
      <div class="detail-row">
        <dt>{t("Rules")}</dt>
        <dd>{service.rules?.length ?? 0}</dd>
      </div>
    </dl>
  </div>
  <div class="card-actions">
    <button type="button" class="btn btn-sm btn-primary" onclick={() => onSelect(service.name, service.group_name)} aria-label={t("Configure the service {0}", service.name)} data-testid="service-card-configure-button-{service.name}">
      {t("Configure")}
    </button>
    <button type="button" class="btn btn-sm btn-outline" onclick={() => onClone(service)} aria-label={t("Duplicate the service {0}", service.name)} title={t("Duplicate")} data-testid="service-card-clone-button-{service.name}">
      &#10697;
    </button>
  </div>
</article>

<style>
  .service-card {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 1rem 1.25rem;
    box-shadow: var(--shadow);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .card-info { display: flex; align-items: center; gap: 0.75rem; }

  .card-title { margin: 0; font-size: 1.125rem; font-weight: 600; }

  .card-details {
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--color-border);
  }

  dl { margin: 0; }

  .detail-row { display: flex; gap: 0.5rem; margin-bottom: 0.125rem; font-size: 0.875rem; }

  dt { font-weight: 500; color: var(--color-text-muted); min-width: 4rem; }
  dd { margin: 0; }

  code { font-size: 0.8125rem; background: var(--color-bg); padding: 0.125rem 0.375rem; border-radius: 3px; }

  .card-actions {
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--color-border);
  }
</style>
