<script>
  import { getLogs } from '../api.js';

  let logs = $state([]);
  let loading = $state(true);

  async function refresh() {
    loading = true;
    try {
      logs = await getLogs(100);
    } catch (e) {
      logs = [];
    } finally {
      loading = false;
    }
  }

  $effect(() => { refresh(); });

  function modeBadge(mode) {
    if (mode === 'mock') return 'badge-mock';
    if (mode === 'proxy') return 'badge-proxy';
    return 'badge-error';
  }

  function formatTime(ts) {
    return new Date(ts).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }
</script>

<section class="log-section" aria-label="Journal des requetes">
  <div class="log-header">
    <h2>Journal des requetes</h2>
    <button type="button" class="btn btn-sm btn-outline" onclick={refresh}>Rafraichir</button>
  </div>

  {#if loading}
    <p class="loading">Chargement...</p>
  {:else if logs.length === 0}
    <p class="empty">Aucune requete interceptee pour le moment.</p>
  {:else}
    <div class="table-wrap">
      <table class="log-table" aria-label="Dernieres requetes">
        <thead>
          <tr>
            <th>Heure</th>
            <th>Service</th>
            <th>Methode</th>
            <th>Path</th>
            <th>Mode</th>
            <th>Regle / Cible</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {#each logs as log}
            <tr>
              <td class="col-time">{formatTime(log.timestamp)}</td>
              <td><strong>{log.service_name}</strong></td>
              <td><span class="method-badge">{log.method}</span></td>
              <td class="col-path"><code>{log.path}</code></td>
              <td><span class="badge {modeBadge(log.mode)}">{log.mode}</span></td>
              <td class="col-detail">{log.rule_matched || log.target_url || '-'}</td>
              <td><span class="status" class:status-ok={log.status < 400} class:status-err={log.status >= 400}>{log.status}</span></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .log-section { margin-top: 1rem; }
  .log-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; }
  .log-header h2 { margin: 0; font-size: 1.25rem; }
  .loading, .empty { color: var(--color-text-muted); text-align: center; padding: 2rem; }

  .table-wrap { overflow-x: auto; }
  .log-table { width: 100%; border-collapse: collapse; font-size: 0.8125rem; }
  .log-table th { background: var(--color-bg); font-weight: 600; text-align: left; padding: 0.5rem; border-bottom: 2px solid var(--color-border); }
  .log-table td { padding: 0.375rem 0.5rem; border-bottom: 1px solid var(--color-border); vertical-align: middle; }
  .col-time { white-space: nowrap; color: var(--color-text-muted); font-family: monospace; }
  .col-path { max-width: 20rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-path code { background: none; padding: 0; font-size: 0.8125rem; }
  .col-detail { max-width: 12rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.75rem; color: var(--color-text-muted); }

  .method-badge { display: inline-block; padding: 0.1rem 0.375rem; border-radius: 3px; font-size: 0.7rem; font-weight: 700; background: #e8f0fe; color: var(--color-primary); }
  .badge { display: inline-block; padding: 0.1rem 0.375rem; border-radius: 3px; font-size: 0.7rem; font-weight: 700; }
  .badge-mock { background: #d4edda; color: #155724; }
  .badge-proxy { background: #cce5ff; color: #004085; }
  .badge-error { background: #f8d7da; color: #721c24; }
  .status { font-weight: 600; font-family: monospace; }
  .status-ok { color: var(--color-success); }
  .status-err { color: var(--color-danger); }

  .btn { cursor: pointer; } .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; }
  .btn-outline { background: var(--color-surface); color: var(--color-text-muted); border-color: var(--color-border); }
  .btn-outline:hover { background: var(--color-bg); color: var(--color-text); }
</style>
