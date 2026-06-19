<script>
  import { getLogs } from '../api.js';

  let logs = $state([]);
  let loading = $state(true);
  let detailLog = $state(null);
  let filterService = $state('');

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

  let serviceNames = $derived([...new Set(logs.map(l => l.service_name))].sort());
  let filteredLogs = $derived(filterService ? logs.filter(l => l.service_name === filterService) : logs);

  function modeBadge(mode) {
    if (mode === 'mock') return 'badge-mock';
    if (mode === 'proxy') return 'badge-proxy';
    return 'badge-error';
  }

  function formatTime(ts) {
    return new Date(ts).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  function openDetail(log) {
    detailLog = log;
  }

  function closeDetail() {
    detailLog = null;
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') closeDetail();
  }

  function handleBackdrop(e) {
    if (e.target === e.currentTarget) closeDetail();
  }
</script>

<section class="log-section" aria-label="Journal des requetes">
  <div class="log-header">
    <h2>Journal des requetes</h2>
    <div class="log-controls">
      {#if serviceNames.length > 1}
        <select class="filter-select" bind:value={filterService} aria-label="Filtrer par service">
          <option value="">Tous les services</option>
          {#each serviceNames as sn}
            <option value={sn}>{sn}</option>
          {/each}
        </select>
      {/if}
      <button type="button" class="btn btn-sm btn-outline" onclick={refresh}>Rafraichir</button>
    </div>
  </div>

  {#if loading}
    <p class="loading">Chargement...</p>
  {:else if filteredLogs.length === 0}
    <p class="empty">{filterService ? `Aucune requete pour le service "${filterService}".` : 'Aucune requete interceptee pour le moment.'}</p>
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
            <th><span class="sr-only">Actions</span></th>
          </tr>
        </thead>
        <tbody>
          {#each filteredLogs as log}
            <tr>
              <td class="col-time">{formatTime(log.timestamp)}</td>
              <td><strong>{log.service_name}</strong></td>
              <td><span class="method-badge">{log.method}</span></td>
              <td class="col-path"><code>{log.path}</code></td>
              <td><span class="badge {modeBadge(log.mode)}">{log.mode}</span></td>
              <td class="col-detail" title={log.rule_matched || log.target_url || '-'}>{log.rule_matched || log.target_url || '-'}</td>
              <td><span class="status" class:status-ok={log.status < 400} class:status-err={log.status >= 400}>{log.status}</span></td>
              <td>
                <button type="button" class="btn-detail" onclick={() => openDetail(log)} aria-label="Voir le detail de la requete {log.path}" title="Detail">&#8942;</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

{#if detailLog}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="modal-overlay" role="dialog" aria-modal="true" aria-label="Detail de la requete" tabindex="-1" onkeydown={handleKeydown} onclick={handleBackdrop}>
    <div class="modal-content" role="document">
      <div class="modal-header">
        <h3>Detail de la requete</h3>
        <button type="button" class="btn-close" onclick={closeDetail} aria-label="Fermer">&#10005;</button>
      </div>
      <dl class="detail-list">
        <div class="detail-row">
          <dt>Heure</dt>
          <dd>{formatTime(detailLog.timestamp)}</dd>
        </div>
        <div class="detail-row">
          <dt>Service</dt>
          <dd>{detailLog.service_name}</dd>
        </div>
        <div class="detail-row">
          <dt>Methode</dt>
          <dd><span class="method-badge">{detailLog.method}</span></dd>
        </div>
        <div class="detail-row">
          <dt>Path</dt>
          <dd class="dd-mono">{detailLog.path}</dd>
        </div>
        <div class="detail-row">
          <dt>Mode</dt>
          <dd><span class="badge {modeBadge(detailLog.mode)}">{detailLog.mode}</span></dd>
        </div>
        {#if detailLog.rule_matched}
          <div class="detail-row">
            <dt>Regle matchee</dt>
            <dd class="dd-mono">{detailLog.rule_matched}</dd>
          </div>
        {/if}
        {#if detailLog.target_url}
          <div class="detail-row">
            <dt>URL cible</dt>
            <dd class="dd-mono dd-break">{detailLog.target_url}</dd>
          </div>
        {/if}
        <div class="detail-row">
          <dt>Status</dt>
          <dd><span class="status" class:status-ok={detailLog.status < 400} class:status-err={detailLog.status >= 400}>{detailLog.status}</span></dd>
        </div>
      </dl>
      <div class="modal-footer">
        <button type="button" class="btn btn-sm btn-secondary" onclick={closeDetail}>Fermer</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .log-section { margin-top: 1rem; }
  .log-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; }
  .log-header h2 { margin: 0; font-size: 1.25rem; }
  .log-controls { display: flex; gap: 0.5rem; align-items: center; }
  .filter-select { padding: 0.25rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; background: var(--color-surface); color: var(--color-text); }
  .loading, .empty { color: var(--color-text-muted); text-align: center; padding: 2rem; }

  .table-wrap { overflow-x: auto; }
  .log-table { width: 100%; border-collapse: collapse; font-size: 0.8125rem; }
  .log-table th { background: var(--color-bg); font-weight: 600; text-align: left; padding: 0.5rem; border-bottom: 2px solid var(--color-border); }
  .log-table td { padding: 0.375rem 0.5rem; border-bottom: 1px solid var(--color-border); vertical-align: middle; }
  .col-time { white-space: nowrap; color: var(--color-text-muted); font-family: monospace; }
  .col-path { max-width: 20rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-path code { background: none; padding: 0; font-size: 0.8125rem; }
  .col-detail { max-width: 12rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.75rem; color: var(--color-text-muted); cursor: default; }

  .method-badge { display: inline-block; padding: 0.1rem 0.375rem; border-radius: 3px; font-size: 0.7rem; font-weight: 700; background: #e8f0fe; color: var(--color-primary); }
  .badge { display: inline-block; padding: 0.1rem 0.375rem; border-radius: 3px; font-size: 0.7rem; font-weight: 700; }
  .badge-mock { background: #d4edda; color: #155724; }
  .badge-proxy { background: #cce5ff; color: #004085; }
  .badge-error { background: #f8d7da; color: #721c24; }
  .status { font-weight: 600; font-family: monospace; }
  .status-ok { color: var(--color-success); }
  .status-err { color: var(--color-danger); }

  .btn-detail {
    background: none; border: 1px solid var(--color-border); border-radius: var(--radius);
    padding: 0.15rem 0.4rem; font-size: 0.875rem; cursor: pointer; color: var(--color-text-muted);
    line-height: 1; letter-spacing: 0.05em;
  }
  .btn-detail:hover { background: var(--color-bg); color: var(--color-text); }
  .btn-detail:focus-visible { outline: 3px solid var(--color-primary); outline-offset: 1px; }

  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }

  .modal-overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.45); display: flex; align-items: center; justify-content: center; z-index: 1000; padding: 1rem;
  }
  .modal-content {
    background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius);
    padding: 1.25rem; max-width: 36rem; width: 100%; max-height: 80vh; overflow-y: auto;
    box-shadow: 0 4px 24px rgba(0,0,0,0.15);
  }
  .modal-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
  .modal-header h3 { margin: 0; font-size: 1.05rem; }
  .btn-close {
    background: none; border: 1px solid var(--color-border); border-radius: var(--radius);
    width: 2rem; height: 2rem; display: inline-flex; align-items: center; justify-content: center;
    font-size: 0.9rem; cursor: pointer; color: var(--color-text-muted);
  }
  .btn-close:hover { background: var(--color-bg); color: var(--color-danger); border-color: var(--color-danger); }
  .btn-close:focus-visible { outline: 3px solid var(--color-primary); outline-offset: 1px; }

  .detail-list { margin: 0; padding: 0; }
  .detail-row { display: flex; gap: 1rem; padding: 0.5rem 0; border-bottom: 1px solid var(--color-border); }
  .detail-row:last-child { border-bottom: none; }
  .detail-row dt { font-weight: 600; font-size: 0.8125rem; min-width: 7rem; flex-shrink: 0; color: var(--color-text-muted); }
  .detail-row dd { margin: 0; font-size: 0.875rem; word-break: break-word; }
  .dd-mono { font-family: 'Cascadia Code', 'Fira Code', monospace; font-size: 0.8125rem; }
  .dd-break { word-break: break-all; }

  .modal-footer { margin-top: 1rem; display: flex; justify-content: flex-end; }

  .btn { cursor: pointer; } .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; }
  .btn-outline { background: var(--color-surface); color: var(--color-text-muted); border-color: var(--color-border); }
  .btn-outline:hover { background: var(--color-bg); color: var(--color-text); }
  .btn-secondary { background: var(--color-surface); color: var(--color-text); border-color: var(--color-border); }
  .btn-secondary:hover { background: var(--color-bg); }
</style>
