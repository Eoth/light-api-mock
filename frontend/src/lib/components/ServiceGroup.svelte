<script>
  import ServiceCard from './ServiceCard.svelte';

  let {
    groupName = 'Sans groupe',
    groupId = 'ungrouped',
    services = [],
    expanded = false,
    onToggleGroup = () => {},
    onToggle = () => {},
    onSelect = () => {},
  } = $props();
</script>

<div class="service-group">
  <button
    type="button"
    class="group-header"
    id="group-header-{groupId}"
    aria-expanded={expanded}
    aria-controls="group-panel-{groupId}"
    onclick={onToggleGroup}
  >
    <span class="group-chevron" class:expanded aria-hidden="true">&#9654;</span>
    <h3 class="group-name">{groupName}</h3>
    <span class="group-count">{services.length} service{services.length !== 1 ? 's' : ''}</span>
  </button>

  {#if expanded}
    <div
      id="group-panel-{groupId}"
      role="region"
      aria-labelledby="group-header-{groupId}"
      class="group-panel"
    >
      <ul class="service-list" role="list">
        {#each services as service (service.name)}
          <li>
            <ServiceCard {service} {onToggle} {onSelect} />
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .service-group {
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    background: var(--color-surface);
    overflow: hidden;
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    width: 100%;
    padding: 0.75rem 1rem;
    background: var(--color-bg);
    border: none;
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: var(--color-text);
    transition: background 0.15s;
  }

  .group-header:hover {
    background: var(--color-border);
  }

  .group-header:focus-visible {
    outline: 3px solid var(--color-primary);
    outline-offset: -3px;
  }

  .group-chevron {
    font-size: 0.625rem;
    transition: transform 0.2s ease;
    flex-shrink: 0;
    color: var(--color-text-muted);
  }

  .group-chevron.expanded {
    transform: rotate(90deg);
  }

  .group-name {
    margin: 0;
    font-size: 0.9375rem;
    font-weight: 700;
  }

  .group-count {
    margin-left: auto;
    font-size: 0.8125rem;
    color: var(--color-text-muted);
    font-weight: 400;
  }

  .group-panel {
    padding: 0.5rem;
  }

  .service-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
</style>
