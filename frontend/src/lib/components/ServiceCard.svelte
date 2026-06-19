<script>
  import ToggleSwitch from './ToggleSwitch.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let { service, onToggle = () => {}, onSelect = () => {} } = $props();
</script>

<article class="service-card" aria-label="Service {service.name}">
  <div class="card-header">
    <div class="card-info">
      <span class="method-badge">{service.method || 'GET'}</span>
      <h3 class="card-title">{service.name}</h3>
      <StatusBadge active={service.is_mocked} />
    </div>
    <ToggleSwitch
      label="Mock {service.name}"
      checked={service.is_mocked}
      onchange={(val) => onToggle(service.name, val)}
    />
  </div>
  <div class="card-details" id="desc-{service.name}">
    <dl>
      <div class="detail-row">
        <dt>URL test</dt>
        <dd><code>/{service.name}{service.listen_path}</code></dd>
      </div>
      <div class="detail-row">
        <dt>Cible</dt>
        <dd><code>{service.real_target_url}</code></dd>
      </div>
      <div class="detail-row">
        <dt>Regles</dt>
        <dd>{service.rules?.length ?? 0}</dd>
      </div>
    </dl>
  </div>
  <div class="card-actions">
    <button type="button" class="btn btn-sm btn-primary" onclick={() => onSelect(service.name)} aria-label="Configurer le service {service.name}">
      Configurer
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
  .method-badge { display: inline-block; padding: 0.125rem 0.5rem; border-radius: var(--radius); font-size: 0.75rem; font-weight: 700; background: #e8f0fe; color: var(--color-primary); letter-spacing: 0.03em; }

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

  .btn { padding: 0.5rem 1.25rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; cursor: pointer; }
  .btn-sm { padding: 0.375rem 1rem; font-size: 0.875rem; }
  .btn-primary { background: var(--color-primary); color: #fff; }
  .btn-primary:hover { background: var(--color-primary-hover); }
</style>
