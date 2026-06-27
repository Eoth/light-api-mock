<script>
  import ServiceGroup from './ServiceGroup.svelte';

  let { services = [], groups = [], onToggle = () => {}, onSelect = () => {} } = $props();

  let search = $state('');
  let expandedGroups = $state(new Set());
  let initialized = $state(false);

  let filtered = $derived(
    search.trim() === ''
      ? services
      : services.filter(s =>
          s.name.toLowerCase().includes(search.toLowerCase()) ||
          (s.listen_path || '').toLowerCase().includes(search.toLowerCase()) ||
          s.real_target_url.toLowerCase().includes(search.toLowerCase()) ||
          (s.group_name || '').toLowerCase().includes(search.toLowerCase())
        )
  );

  let grouped = $derived(() => {
    const map = new Map();
    for (const s of filtered) {
      const key = s.group_name || '__ungrouped__';
      if (!map.has(key)) map.set(key, []);
      map.get(key).push(s);
    }
    const entries = [...map.entries()].sort((a, b) => {
      if (a[0] === '__ungrouped__') return 1;
      if (b[0] === '__ungrouped__') return -1;
      return a[0].localeCompare(b[0]);
    });
    return entries;
  });

  $effect(() => {
    if (!initialized && services.length > 0) {
      const groups = grouped();
      if (groups.length <= 3) {
        expandedGroups = new Set(groups.map(([key]) => key));
      }
      initialized = true;
    }
  });

  let effectiveExpanded = $derived(
    search.trim()
      ? new Set(grouped().map(([key]) => key))
      : expandedGroups
  );

  function toggleGroup(key) {
    const next = new Set(expandedGroups);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    expandedGroups = next;
  }

  function groupDisplayName(key) {
    return key === '__ungrouped__' ? 'Sans groupe' : key;
  }

  function groupId(key) {
    return key === '__ungrouped__' ? 'ungrouped' : key.replace(/[^a-zA-Z0-9-]/g, '_');
  }

  function groupCodeFor(key) {
    if (key === '__ungrouped__') return '';
    const g = groups.find(gr => gr.name === key);
    return g?.code ?? '';
  }
</script>

<section aria-label="Liste des services">
  {#if services.length === 0}
    <div class="empty-state" role="status">
      <p class="empty-title">Aucun service configure</p>
      <p>Ajoutez un service pour commencer a mocker ou proxifier des routes.</p>
    </div>
  {:else}
    <div class="search-bar">
      <label for="service-search" class="sr-only">Rechercher un service</label>
      <input
        id="service-search"
        type="search"
        bind:value={search}
        placeholder="Rechercher par nom, chemin, URL ou groupe..."
        aria-label="Rechercher un service"
      />
      {#if search.trim()}
        <span class="search-count" role="status" aria-live="polite">
          {filtered.length} / {services.length} service{filtered.length !== 1 ? 's' : ''}
        </span>
      {/if}
    </div>

    {#if filtered.length === 0}
      <div class="no-results" role="status">
        <p>Aucun service ne correspond a &laquo; {search} &raquo;</p>
      </div>
    {:else}
      <div class="groups-container">
        {#each grouped() as [key, groupServices] (key)}
          <ServiceGroup
            groupName={groupDisplayName(key)}
            groupId={groupId(key)}
            groupCode={groupCodeFor(key)}
            services={groupServices}
            expanded={effectiveExpanded.has(key)}
            onToggleGroup={() => toggleGroup(key)}
            {onToggle}
            {onSelect}
          />
        {/each}
      </div>
    {/if}
  {/if}
</section>

<style>
  .groups-container {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .search-bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1rem;
  }

  .search-bar input {
    flex: 1;
    padding: 0.625rem 1rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    font-size: 1rem;
    font-family: inherit;
    background: var(--color-surface);
    color: var(--color-text);
  }

  .search-bar input:focus-visible {
    outline: 3px solid var(--color-primary);
    outline-offset: 1px;
  }

  .search-count {
    font-size: 0.875rem;
    color: var(--color-text-muted);
    white-space: nowrap;
  }

  .empty-state {
    text-align: center;
    padding: 3rem 1rem;
    color: var(--color-text-muted);
    background: var(--color-surface);
    border: 2px dashed var(--color-border);
    border-radius: var(--radius);
  }

  .empty-title {
    font-weight: 600;
    font-size: 1.125rem;
    color: var(--color-text);
    margin-bottom: 0.25rem;
  }

  .empty-state p { margin: 0.25rem 0; }

  .no-results {
    text-align: center;
    padding: 2rem 1rem;
    color: var(--color-text-muted);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
  }

  .no-results p { margin: 0; }

  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }
</style>
