<script>
  // Modal de confirmation standard pour les actions destructives (suppression
  // service/groupe, reset complet). Remplace les 3 patterns incoherents
  // precedemment dupliques (span inline dans ServiceDetail, window.confirm
  // dans GroupManager/App). Reutilise .modal-overlay/.modal-content/
  // .modal-header/.modal-footer/.btn-close d'app.css — ne redefinit rien.
  import { tick } from 'svelte';

  let {
    open = false,
    title = 'Confirmer',
    message = '',
    confirmLabel = 'Confirmer',
    cancelLabel = 'Annuler',
    danger = true,
    confirmKeyword = null,
    onConfirm = () => {},
    onCancel = () => {},
  } = $props();

  let keywordInput = $state('');
  let confirmBtn = $state();
  let keywordInputEl = $state();

  $effect(() => {
    if (open) {
      keywordInput = '';
      tick().then(() => {
        (confirmKeyword ? keywordInputEl : confirmBtn)?.focus();
      });
    }
  });

  let canConfirm = $derived(!confirmKeyword || keywordInput === confirmKeyword);

  function handleKeydown(e) {
    if (e.key === 'Escape') onCancel();
  }
  function handleBackdrop(e) {
    if (e.target === e.currentTarget) onCancel();
  }
  function handleConfirm() {
    if (canConfirm) onConfirm();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="modal-overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="confirm-dialog-title"
    tabindex="-1"
    onkeydown={handleKeydown}
    onclick={handleBackdrop}
  >
    <div class="modal-content" role="document">
      <div class="modal-header">
        <h3 id="confirm-dialog-title">{title}</h3>
        <button type="button" class="btn-close" onclick={onCancel} aria-label="Fermer">&#10005;</button>
      </div>
      <p>{message}</p>
      {#if confirmKeyword}
        <div class="form-field">
          <label for="confirm-keyword-input">Tapez « {confirmKeyword} » pour confirmer</label>
          <input
            id="confirm-keyword-input"
            type="text"
            bind:value={keywordInput}
            bind:this={keywordInputEl}
            autocomplete="off"
            spellcheck="false"
          />
        </div>
      {/if}
      <div class="modal-footer">
        <button type="button" class="btn btn-secondary" onclick={onCancel}>{cancelLabel}</button>
        <button
          type="button"
          class={danger ? 'btn btn-danger' : 'btn btn-primary'}
          bind:this={confirmBtn}
          onclick={handleConfirm}
          disabled={!canConfirm}
        >
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
