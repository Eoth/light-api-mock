<script>
  let {
    rules = [],
    onReorder = () => {},
    onEditRule = () => {},
    onDeleteRule = () => {},
    onAddRule = () => {},
  } = $props();

  let dragIdx = $state(null);
  let dragOverIdx = $state(null);

  function handleDragStart(e, idx) {
    dragIdx = idx;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', String(idx));
  }

  function handleDragOver(e, idx) {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    dragOverIdx = idx;
  }

  function handleDragLeave() {
    dragOverIdx = null;
  }

  function handleDrop(e, targetIdx) {
    e.preventDefault();
    dragOverIdx = null;
    if (dragIdx === null || dragIdx === targetIdx) return;
    reorder(dragIdx, targetIdx);
    dragIdx = null;
  }

  function handleDragEnd() {
    dragIdx = null;
    dragOverIdx = null;
  }

  function moveUp(idx) {
    if (idx <= 0) return;
    reorder(idx, idx - 1);
  }

  function moveDown(idx) {
    if (idx >= rules.length - 1) return;
    reorder(idx, idx + 1);
  }

  function reorder(fromIdx, toIdx) {
    const reordered = [...rules];
    const [item] = reordered.splice(fromIdx, 1);
    reordered.splice(toIdx, 0, item);
    onReorder(reordered.map(r => r.name));
  }

  function clickEdit(e, idx) {
    e.stopPropagation();
    e.preventDefault();
    onEditRule(idx);
  }

  function clickDelete(e, idx) {
    e.stopPropagation();
    e.preventDefault();
    onDeleteRule(idx);
  }

  function clickAdd(e) {
    e.stopPropagation();
    e.preventDefault();
    onAddRule();
  }
</script>

<section class="rule-list-section" aria-label="Liste des regles">
  <div class="rule-list-header">
    <h3>Regles de matching</h3>
    <button type="button" class="btn btn-sm btn-primary" onclick={clickAdd}>
      + Ajouter une regle
    </button>
  </div>

  {#if rules.length === 0}
    <p class="empty-rules" role="status">Aucune regle definie. Les requetes retourneront 404.</p>
  {:else}
    <p class="rule-hint" id="rule-order-hint">
      Premiere regle correspondante gagne. Reordonnez par glisser-deposer ou avec les boutons.
    </p>
    <ol class="rule-list" aria-describedby="rule-order-hint" role="list">
      {#each rules as rule, idx (rule.name)}
        <li
          class="rule-item"
          class:dragging={dragIdx === idx}
          class:drag-over={dragOverIdx === idx}
        >
          <div
            class="rule-grip"
            aria-hidden="true"
            draggable="true"
            ondragstart={(e) => handleDragStart(e, idx)}
            ondragover={(e) => handleDragOver(e, idx)}
            ondragleave={handleDragLeave}
            ondrop={(e) => handleDrop(e, idx)}
            ondragend={handleDragEnd}
            role="button"
            tabindex="-1"
          >&#9776;</div>

          <div class="rule-content">
            <span class="rule-index" aria-hidden="true">{idx + 1}</span>
            <span class="rule-method-badge">{rule.method}</span>
            <span class="rule-action-badge" class:proxy={rule.action === 'proxy'}>{(rule.action ?? 'mock').toUpperCase()}</span>
            <span class="rule-name">{rule.name}</span>
            <span class="rule-meta">
              {#if rule.conditions?.all_of?.length || rule.conditions?.any_of?.length}
                {(rule.conditions?.all_of?.length ?? 0) + (rule.conditions?.any_of?.length ?? 0)} condition{((rule.conditions?.all_of?.length ?? 0) + (rule.conditions?.any_of?.length ?? 0)) !== 1 ? 's' : ''}
              {:else}
                Catch-all
              {/if}
            </span>
          </div>

          <div class="rule-actions">
            <button type="button" class="btn-icon" onclick={() => moveUp(idx)} disabled={idx === 0} aria-label="Monter la regle {rule.name}" title="Monter">&#9650;</button>
            <button type="button" class="btn-icon" onclick={() => moveDown(idx)} disabled={idx === rules.length - 1} aria-label="Descendre la regle {rule.name}" title="Descendre">&#9660;</button>
            <button type="button" class="btn-icon btn-edit" onclick={(e) => clickEdit(e, idx)} aria-label="Modifier la regle {rule.name}" title="Modifier">&#9998;</button>
            <button type="button" class="btn-icon btn-delete" onclick={(e) => clickDelete(e, idx)} aria-label="Supprimer la regle {rule.name}" title="Supprimer">&#10005;</button>
          </div>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .rule-list-section { margin-top: 1.25rem; }
  .rule-list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }
  .rule-list-header h3 { margin: 0; font-size: 1rem; }
  .rule-hint { font-size: 0.8125rem; color: var(--color-text-muted); margin: 0 0 0.5rem; }
  .empty-rules { color: var(--color-text-muted); font-style: italic; padding: 1rem; text-align: center; background: var(--color-bg); border-radius: var(--radius); }

  .rule-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.375rem; }

  .rule-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    transition: box-shadow 0.15s, border-color 0.15s;
  }

  .rule-item.dragging { opacity: 0.4; }
  .rule-item.drag-over { border-color: var(--color-primary); box-shadow: 0 0 0 2px var(--color-focus); }

  .rule-grip {
    color: var(--color-text-muted);
    font-size: 0.875rem;
    flex-shrink: 0;
    user-select: none;
    cursor: grab;
    padding: 0.25rem;
  }

  .rule-grip:active { cursor: grabbing; }

  .rule-content { flex: 1; display: flex; align-items: center; gap: 0.5rem; min-width: 0; }
  .rule-index { display: inline-flex; align-items: center; justify-content: center; width: 1.5rem; height: 1.5rem; border-radius: 50%; background: var(--color-bg); font-size: 0.75rem; font-weight: 700; flex-shrink: 0; }
  .rule-method-badge { display: inline-block; padding: 0.0625rem 0.375rem; border-radius: var(--radius); font-size: 0.6875rem; font-weight: 700; letter-spacing: 0.04em; background: var(--color-focus); color: var(--color-primary); flex-shrink: 0; }
  .rule-action-badge { display: inline-block; padding: 0.0625rem 0.375rem; border-radius: var(--radius); font-size: 0.6875rem; font-weight: 700; letter-spacing: 0.04em; background: var(--color-success); color: #fff; flex-shrink: 0; }
  .rule-action-badge.proxy { background: var(--color-primary); }
  .rule-name { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rule-meta { font-size: 0.8125rem; color: var(--color-text-muted); white-space: nowrap; }

  .rule-actions { display: flex; gap: 0.25rem; flex-shrink: 0; }
  .btn-icon { width: 2rem; height: 2rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.875rem; cursor: pointer; transition: background-color 0.15s, color 0.15s; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }
  .btn-icon.btn-edit:hover:not(:disabled) { color: var(--color-primary); border-color: var(--color-primary); }
</style>
