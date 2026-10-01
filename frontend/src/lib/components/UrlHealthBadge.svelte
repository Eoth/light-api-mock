<script>
  import { pingService } from '../api.js';
  import { t } from '../i18n.svelte.js';

  // Doit rester alignee avec PING_TTL_MS cote backend (src/server/ping.rs).
  // Duplique volontairement plutot que d'ajouter un aller-retour reseau
  // dedie juste pour lire la TTL : c'est une simple constante d'affichage.
  const PING_TTL_MS = 120_000;

  let { serviceName, groupName = null } = $props();

  let status = $state(null);
  let loading = $state(false);
  let error = $state('');
  let nowTick = $state(Date.now());

  // Rafraichit uniquement l'affichage (etat "expire") toutes les 15s — pas
  // d'appel reseau, juste un recalcul local pour que le badge ne reste pas
  // indefiniment sur "Accessible" alors que le cache serveur a expire.
  $effect(() => {
    const id = setInterval(() => { nowTick = Date.now(); }, 15_000);
    return () => clearInterval(id);
  });

  async function handleTest() {
    loading = true;
    error = '';
    try {
      status = await pingService(serviceName, groupName);
      nowTick = Date.now();
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  let isExpired = $derived(!!status && (nowTick - status.checked_at) >= PING_TTL_MS);

  let state = $derived(() => {
    if (loading) return 'testing';
    if (!status) return 'unknown';
    if (isExpired) return 'expired';
    return status.reachable ? 'reachable' : 'unreachable';
  });

  function label(current) {
    switch (current) {
      case 'testing': return t("Testing...");
      case 'expired': return t("Expired");
      case 'reachable': return t("Reachable");
      case 'unreachable': return t("Unreachable");
      default: return t("Not tested");
    }
  }
</script>

<div class="url-health">
  <span
    class="badge badge-pill badge-{state()}"
    role="status"
    aria-live="polite"
    data-testid="url-health-badge-status-{serviceName}"
  >
    {label(state())}
  </span>
  <button type="button" class="btn btn-sm btn-outline" onclick={handleTest} disabled={loading} data-testid="url-health-badge-test-button-{serviceName}">
    {t("Test the target (network only)")}
  </button>
  {#if error}
    <span class="ping-error" role="alert" data-testid="url-health-badge-error-{serviceName}">{error}</span>
  {/if}
  {#if status && !isExpired && !status.reachable}
    <p class="ping-warning" role="alert" data-testid="url-health-badge-warning-{serviceName}">
      {t("Only the mock mode can be used for this service while its target is unreachable (network test only: a TCP connection, no application call).")}
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
