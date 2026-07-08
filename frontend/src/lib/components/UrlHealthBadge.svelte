<script>
  import { pingService } from '../api.js';

  let { serviceName } = $props();

  let status = $state(null);
  let loading = $state(false);
  let error = $state('');

  async function handleTest() {
    loading = true;
    error = '';
    try {
      status = await pingService(serviceName);
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  let label = $derived(() => {
    if (loading) return 'Test en cours...';
    if (!status) return 'Non testé';
    return status.reachable ? 'Accessible' : 'Inaccessible';
  });
</script>

<div class="url-health">
  <span
    class="badge"
    class:reachable={status?.reachable === true}
    class:unreachable={status?.reachable === false}
    role="status"
    aria-live="polite"
  >
    {label()}
  </span>
  <button type="button" class="btn btn-sm btn-outline" onclick={handleTest} disabled={loading}>
    Tester la cible
  </button>
  {#if error}
    <span class="ping-error" role="alert">{error}</span>
  {/if}
  {#if status?.reachable === false}
    <p class="ping-warning" role="alert">
      Seul le mode mock est utilisable pour ce service tant que la cible n'est pas accessible.
    </p>
  {/if}
</div>

<style>
  .url-health {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 0.25rem 0.625rem;
    border-radius: var(--radius);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    background: var(--color-border);
    color: var(--color-text-muted);
  }

  .badge.reachable {
    background: #198754;
    color: #ffffff;
  }

  .badge.unreachable {
    background: var(--color-danger);
    color: #ffffff;
  }

  .ping-error {
    font-size: 0.8125rem;
    color: var(--color-danger);
  }

  .ping-warning {
    flex-basis: 100%;
    margin: 0;
    font-size: 0.8125rem;
    color: var(--color-danger);
  }
</style>
