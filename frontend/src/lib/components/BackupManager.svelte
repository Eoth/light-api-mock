<script>
  // Liste les sauvegardes YAML disponibles (backups/ + backups/protected/) et
  // permet de restaurer la configuration depuis l'une d'elles. Reutilise
  // ConfirmDialog.svelte (confirmKeyword, meme pattern que le reset complet
  // dans App.svelte) — aucun pattern de confirmation maison. Reserve aux
  // super-admins cote backend (require_super_admin sur les deux routes) ;
  // ce composant n'est monte par App.svelte que si canShowReset est vrai
  // (meme garde que le bouton Reset, cf CLAUDE.md).
  import { getBackups, restoreBackup } from '../api.js';
  import ConfirmDialog from './ConfirmDialog.svelte';

  let { onNotify = () => {}, onBack = () => {} } = $props();

  let backups = $state([]);
  let loading = $state(true);
  let restorePending = $state(null);
  let restoring = $state(false);

  async function loadBackups() {
    loading = true;
    try {
      backups = await getBackups();
    } catch (e) {
      onNotify(`Erreur chargement des sauvegardes : ${e.message}`, 'error');
    } finally {
      loading = false;
    }
  }

  async function handleRestore() {
    const filename = restorePending;
    restorePending = null;
    restoring = true;
    try {
      await restoreBackup(filename);
      onNotify(`Configuration restauree depuis "${filename}"`, 'success');
      await loadBackups();
    } catch (e) {
      onNotify(`Erreur restauration : ${e.message}`, 'error');
    } finally {
      restoring = false;
    }
  }

  function formatDate(ms) {
    return new Date(ms).toLocaleString('fr-FR');
  }

  function formatSize(bytes) {
    if (bytes < 1024) return `${bytes} o`;
    return `${(bytes / 1024).toFixed(1)} Ko`;
  }

  $effect(() => { loadBackups(); });
</script>

<div class="backup-manager">
  <div class="list-header">
    <h2>Sauvegardes de configuration</h2>
    <button type="button" class="btn btn-outline btn-sm" onclick={onBack} data-testid="backup-manager-back-button">Retour</button>
  </div>

  {#if loading}
    <p class="loading-text">Chargement des sauvegardes...</p>
  {:else if backups.length === 0}
    <p class="empty-text" data-testid="backup-manager-empty-message">Aucune sauvegarde disponible pour le moment.</p>
  {:else}
    <ul class="backup-list">
      {#each backups as backup (backup.filename)}
        <li class="backup-card" data-testid="backup-manager-item-{backup.filename}">
          <div class="backup-info">
            <span class="backup-name">{backup.filename}</span>
            <span class="backup-meta">
              {formatDate(backup.created_at_ms)} · {formatSize(backup.size_bytes)}
              {#if backup.protected}
                <span class="badge badge-testing">Protegee (pre-reset)</span>
              {/if}
            </span>
          </div>
          <button
            type="button"
            class="btn btn-outline btn-sm"
            disabled={restoring}
            onclick={() => restorePending = backup.filename}
            data-testid="backup-manager-restore-button-{backup.filename}"
          >
            Restaurer
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <ConfirmDialog
    open={restorePending !== null}
    title="Restaurer une sauvegarde"
    message={restorePending ? `Restaurer la configuration depuis "${restorePending}" ? La configuration actuelle sera d'abord sauvegardee automatiquement (rollback possible), puis remplacee par le contenu de ce fichier.` : ''}
    confirmLabel="Restaurer"
    confirmKeyword="RESTAURER"
    onConfirm={handleRestore}
    onCancel={() => restorePending = null}
  />
</div>

<style>
  .backup-manager { max-width: 60rem; }
  .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
  .list-header h2 { margin: 0; }

  .backup-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.75rem; }
  .backup-card {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 0.875rem 1.25rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .backup-info { display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }
  .backup-name { font-family: monospace; font-size: 0.875rem; font-weight: 600; word-break: break-all; }
  .backup-meta { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; color: var(--color-text-muted); font-size: 0.8125rem; }

  .loading-text, .empty-text { color: var(--color-text-muted); font-size: 0.875rem; text-align: center; padding: 1rem; }
</style>
