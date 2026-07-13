<script>
  // Journal des messages Kafka traites (feature "messaging-kafka" cote
  // backend) — meme structure visuelle/RGAA que RequestLog.svelte (table,
  // filtres, modal de detail), adaptee au domaine messaging : direction
  // (entrant/sortant) au lieu de mode HTTP, badge "tronque" quand le corps
  // depasse MESSAGE_LOG_MAX_BODY_SIZE cote serveur (src/messaging/message_log.rs).
  // Inclut aussi un panneau "Simuler un message" (POST /api/messaging/simulate) :
  // utile pour tester une regle de messaging sans producteur Kafka reel, et
  // c'est le chemin utilise par les tests E2E dans un environnement sans
  // broker Kafka disponible (voir e2e/messaging.spec.js).
  import { getMessagingLogs, simulateMessage } from '../api.js';
  import { formatDateTimePrecise } from '../format-date.js';

  let { onNotify = () => {}, onBack = () => {} } = $props();

  let logs = $state([]);
  let loading = $state(true);
  let detailLog = $state(null);

  let filterDirection = $state('');
  let filterMatched = $state('');
  let filterText = $state('');

  let simTopic = $state('');
  let simPayload = $state('{\n  "type": "order.created"\n}');
  let simulating = $state(false);

  async function refresh() {
    loading = true;
    try {
      logs = await getMessagingLogs(200);
    } catch (e) {
      logs = [];
    } finally {
      loading = false;
    }
  }

  $effect(() => { refresh(); });

  let filteredLogs = $derived(() => {
    return logs.filter(l => {
      if (filterDirection && l.direction !== filterDirection) return false;
      if (filterMatched === 'matched' && !l.matched) return false;
      if (filterMatched === 'unmatched' && l.matched) return false;
      if (filterText && !l.topic.toLowerCase().includes(filterText.toLowerCase())) return false;
      return true;
    });
  });

  let activeFilterCount = $derived(
    [filterDirection, filterMatched, filterText].filter(Boolean).length
  );

  function clearFilters() {
    filterDirection = '';
    filterMatched = '';
    filterText = '';
  }

  function directionBadge(direction) {
    return direction === 'out' ? 'badge-proxy' : 'badge-mock';
  }

  function directionLabel(direction) {
    return direction === 'out' ? 'Sortant (reply)' : 'Entrant';
  }

  function openDetail(log) { detailLog = log; }
  function closeDetail() { detailLog = null; }
  function handleKeydown(e) { if (e.key === 'Escape') closeDetail(); }
  function handleBackdrop(e) { if (e.target === e.currentTarget) closeDetail(); }

  async function handleSimulate() {
    if (!simTopic.trim()) {
      onNotify('Le topic est requis pour simuler un message.', 'error');
      return;
    }
    simulating = true;
    try {
      await simulateMessage(simTopic.trim(), simPayload);
      onNotify(`Message simule sur "${simTopic.trim()}"`, 'success');
      await refresh();
    } catch (e) {
      onNotify(`Erreur simulation : ${e.message}`, 'error');
    } finally {
      simulating = false;
    }
  }
</script>

<section class="log-section" aria-label="Journal des messages Kafka">
  <div class="log-header">
    <h2>Messages Kafka</h2>
    <div class="log-controls">
      <button type="button" class="btn btn-sm btn-outline" onclick={refresh} data-testid="messaging-log-refresh-button">Rafraichir</button>
      <button type="button" class="btn btn-outline btn-sm" onclick={onBack} data-testid="messaging-log-back-button">Retour</button>
    </div>
  </div>

  <div class="simulate-panel">
    <h3>Simuler un message entrant</h3>
    <p class="field-hint">Sans producteur Kafka reel : declenche le meme pipeline (matching, rendu, journal, publication eventuelle) qu'un vrai message recu sur le topic d'ecoute.</p>
    <div class="simulate-fields">
      <label class="form-field-inline">
        <span>Topic</span>
        <input type="text" bind:value={simTopic} placeholder="orders.in" aria-label="Topic du message simule" data-testid="messaging-log-sim-topic-input" />
      </label>
      <label class="form-field-inline form-field-inline-grow">
        <span>Payload</span>
        <textarea bind:value={simPayload} rows="3" aria-label="Corps du message simule" data-testid="messaging-log-sim-payload-textarea"></textarea>
      </label>
      <button type="button" class="btn btn-primary btn-sm" disabled={simulating} onclick={handleSimulate} data-testid="messaging-log-simulate-button">
        {simulating ? 'Envoi...' : 'Simuler'}
      </button>
    </div>
  </div>

  <div class="filters-bar">
    <select class="filter-select" bind:value={filterDirection} aria-label="Filtrer par direction" data-testid="messaging-log-filter-direction">
      <option value="">Toutes directions</option>
      <option value="in">Entrant</option>
      <option value="out">Sortant (reply)</option>
    </select>

    <select class="filter-select" bind:value={filterMatched} aria-label="Filtrer par statut de match" data-testid="messaging-log-filter-matched">
      <option value="">Tous les statuts</option>
      <option value="matched">Matches</option>
      <option value="unmatched">Non matches</option>
    </select>

    <input
      type="text"
      class="filter-search"
      bind:value={filterText}
      placeholder="Rechercher un topic..."
      aria-label="Recherche textuelle sur le topic"
      data-testid="messaging-log-filter-search"
    />

    {#if activeFilterCount > 0}
      <button type="button" class="btn btn-sm btn-outline btn-clear" onclick={clearFilters} title="Effacer tous les filtres" data-testid="messaging-log-clear-filters-button">
        Effacer ({activeFilterCount})
      </button>
    {/if}
  </div>

  {#if loading}
    <p class="loading">Chargement...</p>
  {:else if filteredLogs().length === 0}
    <p class="empty">
      {#if activeFilterCount > 0}
        Aucun message ne correspond aux filtres.
      {:else}
        Aucun message Kafka traite pour le moment.
      {/if}
    </p>
  {:else}
    <p class="result-count">{filteredLogs().length} message{filteredLogs().length !== 1 ? 's' : ''}{activeFilterCount > 0 ? ' (filtres)' : ''}</p>
    <div class="table-wrap">
      <table class="log-table" aria-label="Derniers messages Kafka">
        <thead>
          <tr>
            <th>Date/Heure</th>
            <th>Direction</th>
            <th>Topic</th>
            <th>Service / Regle</th>
            <th>Statut</th>
            <th>Taille</th>
            <th><span class="sr-only">Actions</span></th>
          </tr>
        </thead>
        <tbody>
          {#each filteredLogs() as log, idx}
            <tr data-testid="messaging-log-row-{idx}">
              <td class="col-time">{formatDateTimePrecise(log.timestamp)}</td>
              <td><span class="badge {directionBadge(log.direction)}">{directionLabel(log.direction)}</span></td>
              <td class="col-path"><code>{log.topic}</code></td>
              <td class="col-detail" title={log.rule_matched ? `${log.service_name} / ${log.rule_matched}` : '-'}>
                {log.rule_matched ? `${log.service_name} / ${log.rule_matched}` : '-'}
              </td>
              <td>
                <span class="badge {log.matched ? 'badge-mock' : 'badge-error'}">{log.matched ? 'Matche' : 'Non matche'}</span>
                {#if log.body_truncated}
                  <span class="badge badge-testing">Tronque</span>
                {/if}
              </td>
              <td class="col-size">{log.body_size_bytes} o</td>
              <td>
                <button type="button" class="btn-detail" onclick={() => openDetail(log)} aria-label="Voir le detail du message {log.topic}" title="Detail" data-testid="messaging-log-detail-button-{idx}">&#8942;</button>
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
  <div class="modal-overlay" role="dialog" aria-modal="true" aria-label="Detail du message" tabindex="-1" onkeydown={handleKeydown} onclick={handleBackdrop} data-testid="messaging-log-detail-modal">
    <div class="modal-content" role="document">
      <div class="modal-header">
        <h3>Detail du message</h3>
        <button type="button" class="btn-close" onclick={closeDetail} aria-label="Fermer" data-testid="messaging-log-detail-modal-close-button">&#10005;</button>
      </div>
      <dl class="detail-list">
        <div class="detail-row">
          <dt>Date/Heure</dt>
          <dd>{formatDateTimePrecise(detailLog.timestamp)}</dd>
        </div>
        <div class="detail-row">
          <dt>Direction</dt>
          <dd><span class="badge {directionBadge(detailLog.direction)}">{directionLabel(detailLog.direction)}</span></dd>
        </div>
        <div class="detail-row">
          <dt>Topic</dt>
          <dd class="dd-mono">{detailLog.topic}</dd>
        </div>
        {#if detailLog.service_name}
          <div class="detail-row">
            <dt>Service</dt>
            <dd>{detailLog.service_name}</dd>
          </div>
        {/if}
        {#if detailLog.rule_matched}
          <div class="detail-row">
            <dt>Regle matchee</dt>
            <dd class="dd-mono">{detailLog.rule_matched}</dd>
          </div>
        {/if}
        <div class="detail-row">
          <dt>Taille reelle</dt>
          <dd>{detailLog.body_size_bytes} octets{#if detailLog.body_truncated} <span class="badge badge-testing">Tronque dans l'apercu</span>{/if}</dd>
        </div>
        <div class="detail-row">
          <dt>Corps {detailLog.body_truncated ? '(apercu tronque)' : ''}</dt>
          <dd class="dd-mono dd-break dd-body">{detailLog.body_preview}</dd>
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

  .simulate-panel { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius); padding: 0.875rem 1.25rem; margin-bottom: 1rem; }
  .simulate-panel h3 { margin: 0 0 0.25rem; font-size: 1rem; }
  .simulate-fields { display: flex; gap: 0.75rem; align-items: flex-end; flex-wrap: wrap; margin-top: 0.5rem; }
  .form-field-inline { display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.8125rem; }
  .form-field-inline-grow { flex: 1; min-width: 14rem; }
  .form-field-inline input, .form-field-inline textarea {
    padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius);
    background: var(--color-bg); color: var(--color-text); font-family: inherit; font-size: 0.8125rem;
  }
  .form-field-inline textarea { font-family: 'Cascadia Code', 'Fira Code', monospace; resize: vertical; }

  .filters-bar { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-bottom: 0.75rem; padding: 0.625rem 0.75rem; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius); }
  .filter-select { padding: 0.25rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; background: var(--color-bg); color: var(--color-text); }
  .filter-search { padding: 0.25rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.8125rem; background: var(--color-bg); color: var(--color-text); min-width: 10rem; flex: 1; }
  .btn-clear { color: var(--color-danger); border-color: var(--color-danger); }
  .btn-clear:hover { background: var(--color-danger); color: #fff; }

  .result-count { font-size: 0.8125rem; color: var(--color-text-muted); margin: 0 0 0.5rem; }
  .loading, .empty { color: var(--color-text-muted); text-align: center; padding: 2rem; }

  .table-wrap { overflow-x: auto; }
  .log-table { width: 100%; border-collapse: collapse; font-size: 0.8125rem; }
  .log-table th { background: var(--color-bg); font-weight: 600; text-align: left; padding: 0.5rem; border-bottom: 2px solid var(--color-border); }
  .log-table td { padding: 0.375rem 0.5rem; border-bottom: 1px solid var(--color-border); vertical-align: middle; }
  .col-time { white-space: nowrap; color: var(--color-text-muted); font-family: monospace; }
  .col-path { max-width: 16rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-path code { background: none; padding: 0; font-size: 0.8125rem; }
  .col-detail { max-width: 14rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.75rem; color: var(--color-text-muted); }
  .col-size { white-space: nowrap; font-family: monospace; color: var(--color-text-muted); }

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
  .dd-body { white-space: pre-wrap; max-height: 12rem; overflow-y: auto; }
</style>
