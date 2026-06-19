<script>
  import { getServices, toggleService, putService } from './lib/api.js';
  import ServiceList from './lib/components/ServiceList.svelte';
  import ServiceDetail from './lib/components/ServiceDetail.svelte';
  import ServiceForm from './lib/components/ServiceForm.svelte';
  import Notification from './lib/components/Notification.svelte';

  let services = $state([]);
  let notification = $state({ message: '', type: 'info', visible: false });
  let selectedService = $state(null);
  let view = $state('list');
  let loading = $state(true);

  async function loadServices() {
    try {
      services = await getServices();
    } catch (e) {
      showNotification(`Erreur de chargement : ${e.message}`, 'error');
    } finally {
      loading = false;
    }
  }

  async function handleToggle(name, isMocked) {
    try {
      const updated = await toggleService(name, isMocked);
      services = services.map(s => s.name === name ? updated : s);
      showNotification(
        `${name} : mode ${isMocked ? 'mock' : 'proxy'} activé`,
        'success'
      );
    } catch (e) {
      showNotification(`Erreur : ${e.message}`, 'error');
    }
  }

  function handleSelect(name) {
    selectedService = name;
    view = 'detail';
  }

  function handleBack() {
    selectedService = null;
    view = 'list';
  }

  function handleServiceUpdate(updated) {
    services = services.map(s => s.name === updated.name ? updated : s);
    selectedService = updated.name;
  }

  function handleServiceDelete(name) {
    services = services.filter(s => s.name !== name);
    selectedService = null;
    view = 'list';
  }

  async function handleAddService(svc) {
    try {
      const result = await putService(svc.name, svc);
      services = [...services, result];
      view = 'detail';
      selectedService = result.name;
      showNotification(`Service "${result.name}" créé`, 'success');
    } catch (e) {
      showNotification(`Erreur : ${e.message}`, 'error');
      throw e;
    }
  }

  function showNotification(message, type) {
    notification = { message, type, visible: true };
    setTimeout(() => {
      notification = { ...notification, visible: false };
    }, 4000);
  }

  $effect(() => {
    loadServices();
  });

  let currentService = $derived(services.find(s => s.name === selectedService) ?? null);
</script>

<a href="#main-content" class="sr-only skip-link">
  Aller au contenu principal
</a>

<header class="app-header">
  <div class="header-content">
    <button type="button" class="app-title-btn" onclick={handleBack}>
      <h1 class="app-title">lightMock</h1>
    </button>
    <p class="app-subtitle">Mock &amp; Proxy Intelligent</p>
  </div>
</header>

<main id="main-content" class="app-main">
  <Notification
    message={notification.message}
    type={notification.type}
    visible={notification.visible}
  />

  {#if loading}
    <div class="loading" role="status" aria-label="Chargement en cours">
      <p>Chargement des services...</p>
    </div>
  {:else if view === 'add'}
    <ServiceForm
      onSave={handleAddService}
      onCancel={handleBack}
    />
  {:else if view === 'detail' && currentService}
    <ServiceDetail
      service={currentService}
      onBack={handleBack}
      onUpdate={handleServiceUpdate}
      onDelete={handleServiceDelete}
      onNotify={showNotification}
    />
  {:else}
    <div class="list-header">
      <h2>Services</h2>
      <button type="button" class="btn btn-primary" onclick={() => view = 'add'}>
        + Ajouter un service
      </button>
    </div>
    <ServiceList
      {services}
      onToggle={handleToggle}
      onSelect={handleSelect}
    />
  {/if}
</main>

<style>
  :global(.skip-link:focus) {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 1000;
    width: auto;
    height: auto;
    clip: auto;
    padding: 0.75rem 1.5rem;
    background: var(--color-primary);
    color: #fff;
    font-weight: 600;
    text-decoration: none;
  }

  .app-header {
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    padding: 1rem 1.5rem;
    box-shadow: var(--shadow);
  }

  .header-content {
    max-width: 60rem;
    margin: 0 auto;
    display: flex;
    align-items: baseline;
    gap: 1rem;
  }

  .app-title-btn {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .app-title {
    font-size: 1.5rem;
    margin: 0;
    color: var(--color-primary);
  }

  .app-subtitle {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 0.875rem;
  }

  .app-main {
    max-width: 60rem;
    margin: 1.5rem auto;
    padding: 0 1.5rem;
  }

  .loading {
    text-align: center;
    padding: 3rem;
    color: var(--color-text-muted);
  }

  .list-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .list-header h2 {
    margin: 0;
  }

  .btn {
    padding: 0.5rem 1.25rem;
    border-radius: var(--radius);
    border: 1px solid transparent;
    font-weight: 600;
    font-size: 0.9375rem;
  }

  .btn-primary {
    background: var(--color-primary);
    color: #fff;
  }

  .btn-primary:hover {
    background: var(--color-primary-hover);
  }
</style>
