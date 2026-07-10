<script>
  import { pingService } from '../api.js';

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

  const LABELS = {
    testing: 'Test en cours...',
    unknown: 'Non testé',
    expired: 'Expiré',
    reachable: 'Accessible',
    unreachable: 'Inaccessible',
  };
</script>

<div class="url-health">
  <span
    class="badge badge-pill badge-{state()}"
    role="status"
    aria-live="polite"
  >
    {LABELS[state()]}
  </span>
  <button type="button" class="btn btn-sm btn-outline" onclick={handleTest} disabled={loading}>
    Tester la cible (reseau uniquement)
  </button>
  {#if error}
    <span class="ping-error" role="alert">{error}</span>
  {/if}
  {#if status && !isExpired && !status.reachable}
    <p class="ping-warning" role="alert">
      Seul le mode mock est utilisable pour ce service tant que la cible n'est pas accessible
      (test reseau uniquement — connexion TCP, pas d'appel applicatif).
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
