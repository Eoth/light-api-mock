<script>
  // Liste generique "chip/ligne + bouton supprimer". `getKey`/`getLabel`
  // restent personnalisables pour reutiliser ce composant avec des items
  // simples (chaines) ou des objets (ex: membres de groupe).
  let {
    items = [],
    getKey = (item) => item,
    getLabel = (item) => String(item),
    onRemove = () => {},
    emptyText = 'Aucun élément.',
  } = $props();
</script>

{#if items.length === 0}
  <p class="removable-list-empty">{emptyText}</p>
{:else}
  <ul class="removable-list">
    {#each items as item (getKey(item))}
      <li class="removable-list-item">
        <span>{getLabel(item)}</span>
        <button
          type="button"
          class="chip-remove"
          onclick={() => onRemove(item)}
          aria-label={`Retirer ${getLabel(item)}`}
          title="Retirer"
        >
          &times;
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .removable-list {
    list-style: none;
    padding: 0;
    margin: 0 0 0.5rem;
  }
  .removable-list-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.25rem 0;
    font-size: 0.875rem;
  }
  .removable-list-empty {
    color: var(--color-text-muted);
    font-size: 0.8125rem;
    margin: 0.5rem 0;
    font-style: italic;
  }
  .chip-remove {
    background: none;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    font-weight: bold;
    font-size: 0.875rem;
    padding: 0 0.25rem;
    line-height: 1;
  }
  .chip-remove:hover {
    color: var(--color-danger);
  }
</style>
