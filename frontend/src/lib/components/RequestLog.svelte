<script>
  import { getLogs } from '../api.js';

  let logs = $state([]);
  let loading = $state(true);
  let detailLog = $state(null);

  let filterService = $state('');
  let filterMode = $state('');
  let filterStatus = $state('');
  let filterText = $state('');
  let filterTime = $state('');

  async function refresh() {
    loading = true;
    try {
      logs = await getLogs(200);
    } catch (e) {
      logs = [];
    } finally {
      loading = false;
    }
  }

  $effect(() => { refresh(); });

  let serviceNames = $derived([...new Set(logs.map(l => l.service_name))].sort());

  let filteredLogs = $derived(() => {
    const now = Date.now();
    return logs.filter(l => {
      if (filterService && l.service_name !== filterService) return false;
      if (filterMode && l.mode !== filterMode) return false;

      if (filterStatus) {
        const s = l.status;
        if (filterStatus === '2xx' && (s < 200 || s >= 300)) return false;
        if (filterStatus === '3xx' && (s < 300 || s >= 400)) return false;
        if (filterStatus === '4xx' && (s < 400 || s >= 500)) return false;
        if (filterStatus === '5xx' && s < 500) return false;
      }

      if (filterText && !l.path.toLowerCase().includes(filterText.toLowerCase())) return false;

      if (filterTime) {
        const age = now - l.timestamp;
        if (filterTime === '1m' && age > 60_000) return false;
        if (filterTime === '5m' && age > 300_000) return false;
        if (filterTime === '1h' && age > 3_600_000) return false;
      }

      return true;
    });
  });

  let activeFilterCount = $derived(
    [filterService, filterMode, filterStatus, filterText, filterTime].filter(Boolean).length
  );

  function clearFilters() {
    filterService = '';
    filterMode = '';
    filterStatus = '';
    filterText = '';
    filterTime = '';
  }

  function modeBadge(mode) {
    if (mode === 'mock') return 'badge-mock';
    if (mode === 'proxy') return 'badge-proxy';
    return 'badge-error';
  }

  function formatTime(ts) {
    return new Date(ts).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  function openDetail(log) { detailLog = log; }
  function closeDetail() { detailLog = null; }
  function handleKeydown(e) { if (e.key === 'Escape') closeDetail(); }
  function handleBackdrop(e) { if (e.target === e.currentTarget) closeDetail(); }
</script>

<section class="log-section" aria-label="Journal des requetes">
  <div class="log-header">
    <h2>Journal des requetes</h2>
    <div class="log-controls">
      <button type="button" class="btn btn-sm btn-outline" onclick={refresh}>Rafraichir</button>
    </div>
  </div>

  <div class="filters-bar">
    {#if serviceNames.length > 1}
      <select class="filter-select" bind:value={filterService} aria-label="Filtrer par service">
        <option value="">Tous les services</option>
        {#each serviceNames as sn}
          <option value={sn}>{sn}</option>
        {/each}
      </select>
    {/if}

    <select class="filter-select" bind:value={filterMode} aria-label="Filtrer par mode">
      <option value="">Tous les modes</option>
      <option value="mock">Mock</option>
      <option value="proxy">Proxy</option>
      <option value="no-rule">No-rule</option>
    </select>

    <select class="filter-select" bind:value={filterStatus} aria-label="Filtrer par statut HTTP">
      <option value="">Tous les status</option>
      <option value="2xx">2xx (succes)</option>
      <option value="3xx">3xx (redirection)</option>
      <option value="4xx">4xx (erreur client)</option>
      <option value="5xx">5xx (erreur serveur)</option>
    </select>

    <select class="filter-select" bind:value={filterTime} aria-label="Filtrer par periode">
      <option value="">Toute la periode</option>
      <option value="1m">Derniere minute</option>
      <option value="5m">5 dernieres minutes</option>
      <option value="1h">Derniere heure</option>
    </select>

    <input
      type="text"
      class="filter-search"
      bind:value={filterText}
      placeholder="Rechercher un path..."
      aria-label="Recherche textuelle sur le path"
    />

    {#if activeFilterCount > 0}
      <button type="button" class="btn btn-sm btn-outline btn-clear" onclick={clearFilters} title="Effacer tous les filtres">
        Effacer ({activeFilterCount})
      </button>
    {/if}
  </div>

  {#if loading}
    <p class="loading">Chargement...</p>
  {:else if filteredLogs().length === 0}
    <p class="empty">
      {#if activeFilterCount > 0}
        Aucune requete ne correspond aux filtres.
      {:else}
        Aucune requete interceptee pour le moment.
      {/if}
    </p>
  {:else}
    <p class="result-count">{filteredLogs().length} requete{filteredLogs().length !== 1 ? 's' : ''}{activeFilterCount > 0 ? ' (filtrees)' : ''}</p>
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
          {#each filteredLogs() as log}
            <tr>
              <td class="col-time">{formatTime(log.timestamp)}</td>
              <td><strong>{log.service_name}</strong></td>
              <td><span class="method-badge" data-method={log.method}>{log.method}</span></td>
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
          <dd><span class="method-badge" data-method={detailLog.method}>{detailLog.method}</span></dd>
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

  .filters-bar { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-bottom: 0.75rem; padding: 0.625rem 0.75rem; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius); }
  .filter-select { padding: 0.25rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; background: var(--color-bg); color: var(--color-text); }
  .filter-search { padding: 0.25rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; background: var(--color-bg); color: var(--color-text); min-width: 10rem; flex: 1; }
  .filter-search::placeholder { color: var(--color-text-muted); }
  .btn-clear { color: var(--color-danger); border-color: var(--color-danger); }
  .btn-clear:hover { background: var(--color-danger); color: #fff; }

  .result-count { font-size: 0.8125rem; color: var(--color-text-muted); margin: 0 0 0.5rem; }

  .loading, .empty { color: var(--color-text-muted); text-align: center; padding: 2rem; }

  .table-wrap { overflow-x: auto; }
  .log-table { width: 100%; border-collapse: collapse; font-size: 0.8125rem; }
  .log-table th { background: var(--color-bg); font-weight: 600; text-align: left; padding: 0.5rem; border-bottom: 2px solid var(--color-border); }
  .log-table td { padding: 0.375rem 0.5rem; border-bottom: 1px solid var(--color-border); vertical-align: middle; }
  .col-time { white-space: nowrap; color: var(--color-text-muted); font-family: monospace; }
  .col-path { max-width: 20rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-path code { background: none; padding: 0; font-size: 0.8125rem; }
  .col-detail { max-width: 12rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.75rem; color: var(--color-text-muted); cursor: default; }

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

  .detail-list { margin: 0; padding: 0; }
  .detail-row { display: flex; gap: 1rem; padding: 0.5rem 0; border-bottom: 1px solid var(--color-border); }
  .detail-row:last-child { border-bottom: none; }
  .detail-row dt { font-weight: 600; font-size: 0.8125rem; min-width: 7rem; flex-shrink: 0; color: var(--color-text-muted); }
  .detail-row dd { margin: 0; font-size: 0.875rem; word-break: break-word; }
  .dd-mono { font-family: 'Cascadia Code', 'Fira Code', monospace; font-size: 0.8125rem; }
  .dd-break { word-break: break-all; }

</style>
