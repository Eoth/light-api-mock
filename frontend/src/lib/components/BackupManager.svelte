<script>
  // Lists the configuration backups (backups/ and backups/protected/) and restores one of them, after a confirmation
  // by keyword in ConfirmDialog.svelte, as the full reset of App.svelte asks. Both routes are for super-admins only
  // (require_super_admin on the server).
  import { getBackups, restoreBackup } from '../api.js';
  import { formatDateTime } from '../format-date.js';
  import { t, intlLocale } from '../i18n.svelte.js';
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
      onNotify(t("Error while loading the backups: {0}", e.message), 'error');
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
      onNotify(t("Configuration restored from \"{0}\"", filename), 'success');
      await loadBackups();
    } catch (e) {
      onNotify(t("Restore error: {0}", e.message), 'error');
    } finally {
      restoring = false;
    }
  }

  function formatDate(ms) {
    return formatDateTime(ms, undefined, intlLocale());
  }

  function formatSize(bytes) {
    if (bytes < 1024) return t("{0} B", bytes);
    return t("{0} KB", (bytes / 1024).toFixed(1));
  }

  $effect(() => { loadBackups(); });
</script>

<div class="backup-manager">
  <div class="list-header">
    <h2>{t("Configuration backups")}</h2>
    <button type="button" class="btn btn-outline btn-sm" onclick={onBack} data-testid="backup-manager-back-button">{t("Back")}</button>
  </div>

  {#if loading}
    <p class="loading-text">{t("Loading the backups...")}</p>
  {:else if backups.length === 0}
    <p class="empty-text" data-testid="backup-manager-empty-message">{t("No backup available yet.")}</p>
  {:else}
    <ul class="backup-list">
      {#each backups as backup (backup.filename)}
        <li class="backup-card" data-testid="backup-manager-item-{backup.filename}">
          <div class="backup-info">
            <span class="backup-name" translate="no">{backup.filename}</span>
            <span class="backup-meta">
              {formatDate(backup.created_at_ms)} · {formatSize(backup.size_bytes)}
              {#if backup.protected}
                <span class="badge badge-testing">{t("Protected (before a reset)")}</span>
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
            {t("Restore")}
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <ConfirmDialog
    open={restorePending !== null}
    title={t("Restore a backup")}
    message={restorePending ? t("Restore the configuration from \"{0}\"? The current configuration is first backed up automatically (so this can be rolled back), then replaced by the content of this file.", restorePending) : ''}
    confirmLabel={t("Restore")}
    confirmKeyword={t("RESTORE")}
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
