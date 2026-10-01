<script>
  import { t } from '../i18n.svelte.js';
  // A list of items, each with a remove button. `getKey` and `getLabel` adapt it to plain strings or to objects (the
  // members of a group, for instance).
  let {
    items = [],
    getKey = (item) => item,
    getLabel = (item) => String(item),
    onRemove = () => {},
    emptyText = null,
  } = $props();
</script>

{#if items.length === 0}
  <p class="removable-list-empty">{emptyText ?? t("No item.")}</p>
{:else}
  <ul class="removable-list">
    {#each items as item (getKey(item))}
      <li class="removable-list-item" data-testid="removable-list-item-{getKey(item)}">
        <span>{getLabel(item)}</span>
        <button
          type="button"
          class="chip-remove"
          onclick={() => onRemove(item)}
          aria-label={t("Remove {0}", getLabel(item))}
          title={t("Remove")}
          data-testid="removable-list-remove-button-{getKey(item)}"
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
