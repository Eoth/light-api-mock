<script>
  import ServiceCard from './ServiceCard.svelte';

  let { services = [], onToggle = () => {}, onSelect = () => {} } = $props();

  let search = $state('');

  let filtered = $derived(
    search.trim() === ''
      ? services
      : services.filter(s =>
          s.name.toLowerCase().includes(search.toLowerCase()) ||
          s.listen_path.toLowerCase().includes(search.toLowerCase()) ||
          s.real_target_url.toLowerCase().includes(search.toLowerCase())
        )
  );
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
        placeholder="Rechercher par nom, chemin ou URL..."
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
        <p>Aucun service ne correspond a « {search} »</p>
      </div>
    {:else}
      <ul class="service-list" role="list">
        {#each filtered as service (service.name)}
          <li>
            <ServiceCard {service} {onToggle} {onSelect} />
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  .service-list {
    list-style: none;
    padding: 0;
    margin: 0;
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
</style>
