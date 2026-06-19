<script>
  let { message = '', type = 'info', visible = false } = $props();
</script>

{#if visible}
  <div
    class="notification {type}"
    role="alert"
    aria-live="assertive"
    aria-atomic="true"
  >
    <span class="notification-icon">
      {#if type === 'success'}&#10003;{:else if type === 'error'}&#10007;{:else}&#9432;{/if}
    </span>
    <span class="notification-message">{message}</span>
  </div>
{/if}

<style>
  .notification {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    border-radius: var(--radius);
    font-weight: 500;
    margin-bottom: 1rem;
    border: 1px solid;
  }

  .notification.success {
    background: #d1e7dd;
    border-color: #a3cfbb;
    color: #0a3622;
  }

  .notification.error {
    background: #f8d7da;
    border-color: #f1aeb5;
    color: #58151c;
  }

  .notification.info {
    background: #cff4fc;
    border-color: #9eeaf9;
    color: #055160;
  }

  :global([data-theme="dark"]) .notification.success {
    background: #0a3622;
    border-color: #198754;
    color: #a3cfbb;
  }
  :global([data-theme="dark"]) .notification.error {
    background: #3b1219;
    border-color: #dc3545;
    color: #f1aeb5;
  }
  :global([data-theme="dark"]) .notification.info {
    background: #032830;
    border-color: #0dcaf0;
    color: #9eeaf9;
  }

  .notification-icon {
    font-size: 1.125rem;
    flex-shrink: 0;
  }
</style>
