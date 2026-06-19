<script>
  import { getServices, getConfig, putConfig, toggleService, putService } from './lib/api.js';
  import ServiceList from './lib/components/ServiceList.svelte';
  import ServiceDetail from './lib/components/ServiceDetail.svelte';
  import ServiceForm from './lib/components/ServiceForm.svelte';
  import Notification from './lib/components/Notification.svelte';
  import RequestLog from './lib/components/RequestLog.svelte';

  let services = $state([]);
  let notification = $state({ message: '', type: 'info', visible: false });
  let selectedService = $state(null);
  let view = $state('list');
  let loading = $state(true);

  const demoService = {
    name: 'insee-demo',
    method: 'GET',
    listen_path: '/v4/insee/sirene/etablissements/{siret}',
    real_target_url: 'https://staging.entreprise.api.gouv.fr',
    is_mocked: true,
    rewrite_directory_urls: false,
    rules: [{
      name: 'etablissement-mock',
      conditions: { all_of: [], any_of: [] },
      response: {
        status: 200,
        headers: [{ name: 'Content-Type', value: 'application/json' }],
        body: [{ type: 'Template', template: '{{"siret":"{path.siret}","siren":"{path.siret | first(9)}","unite_legale":{{"denomination":"{fake.CompanyName}","date_creation":"{fake.DatePast}","adresse":{{"voie":"{fake.StreetName}","code_postal":"{fake.PostcodeFR}","ville":"{fake.CityFR}"}}}},"meta":{{"timestamp":{now_ms},"seq":{seq}}}}}' }],
        chaos: null,
      },
    }],
  };

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
      showNotification(`${name} : mode ${isMocked ? 'mock' : 'proxy'} active`, 'success');
    } catch (e) {
      showNotification(`Erreur : ${e.message}`, 'error');
    }
  }

  function handleSelect(name) { selectedService = name; view = 'detail'; }
  function handleBack() { selectedService = null; view = 'list'; }
  function handleServiceUpdate(updated) { services = services.map(s => s.name === updated.name ? updated : s); selectedService = updated.name; }
  function handleServiceDelete(name) { services = services.filter(s => s.name !== name); selectedService = null; view = 'list'; }

  async function handleAddService(svc) {
    try {
      const result = await putService(svc.name, svc);
      services = [...services, result];
      view = 'detail';
      selectedService = result.name;
      showNotification(`Service "${result.name}" cree`, 'success');
    } catch (e) {
      showNotification(`Erreur : ${e.message}`, 'error');
      throw e;
    }
  }

  async function loadDemo() {
    try {
      const result = await putService(demoService.name, demoService);
      services = [...services, result];
      showNotification('Service de demo INSEE charge', 'success');
    } catch (e) {
      showNotification(`Erreur : ${e.message}`, 'error');
    }
  }

  async function exportConfig() {
    try {
      const config = await getConfig();
      const yaml = JSON.stringify(config, null, 2);
      const blob = new Blob([yaml], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `lightmock-config-${new Date().toISOString().slice(0,10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
      showNotification('Configuration exportee', 'success');
    } catch (e) {
      showNotification(`Erreur export : ${e.message}`, 'error');
    }
  }

  let fileInput = $state(null);
  async function importConfig() {
    fileInput?.click();
  }
  async function handleFileImport(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const config = JSON.parse(text);
      if (!config.services || !Array.isArray(config.services)) {
        throw new Error('Format invalide : "services" attendu');
      }
      await putConfig(config);
      services = config.services;
      showNotification(`Configuration importee (${config.services.length} services)`, 'success');
      view = 'list';
      selectedService = null;
    } catch (e) {
      showNotification(`Erreur import : ${e.message}`, 'error');
    }
    e.target.value = '';
  }

  function showNotification(message, type) {
    notification = { message, type, visible: true };
    setTimeout(() => { notification = { ...notification, visible: false }; }, 4000);
  }

  $effect(() => { loadServices(); });

  let currentService = $derived(services.find(s => s.name === selectedService) ?? null);
</script>

<a href="#main-content" class="sr-only skip-link">Aller au contenu principal</a>

<header class="app-header">
  <div class="header-content">
    <button type="button" class="app-title-btn" onclick={handleBack}>
      <h1 class="app-title">lightMock</h1>
    </button>
    <p class="app-subtitle">Mock &amp; Proxy Intelligent</p>
    <div class="header-actions">
      <button type="button" class="btn btn-sm btn-outline" onclick={() => view = 'logs'} title="Journal des requetes">Logs</button>
      <button type="button" class="btn btn-sm btn-outline" onclick={exportConfig} title="Telecharger la configuration">Export</button>
      <button type="button" class="btn btn-sm btn-outline" onclick={importConfig} title="Charger une configuration">Import</button>
      <input type="file" accept=".json" style="display:none" bind:this={fileInput} onchange={handleFileImport} />
    </div>
  </div>
</header>

<main id="main-content" class="app-main">
  <Notification message={notification.message} type={notification.type} visible={notification.visible} />

  {#if loading}
    <div class="loading" role="status"><p>Chargement des services...</p></div>
  {:else if view === 'logs'}
    <RequestLog />
  {:else if view === 'add'}
    <ServiceForm onSave={handleAddService} onCancel={handleBack} />
  {:else if view === 'detail' && currentService}
    <ServiceDetail service={currentService} onBack={handleBack} onUpdate={handleServiceUpdate} onDelete={handleServiceDelete} onNotify={showNotification} />
  {:else}
    <div class="list-header">
      <h2>Services</h2>
      <button type="button" class="btn btn-primary" onclick={() => view = 'add'}>+ Ajouter un service</button>
    </div>
    <ServiceList {services} onToggle={handleToggle} onSelect={handleSelect} />
    {#if services.length === 0}
      <div class="demo-section">
        <button type="button" class="btn btn-outline btn-demo" onclick={loadDemo}>
          Charger un exemple (service INSEE)
        </button>
        <span class="field-hint">Cree un service de demo avec path params, template et fake data.</span>
      </div>
    {/if}
  {/if}
</main>

<style>
  :global(.skip-link:focus) { position: fixed; top: 0; left: 0; z-index: 1000; width: auto; height: auto; clip: auto; padding: 0.75rem 1.5rem; background: var(--color-primary); color: #fff; font-weight: 600; text-decoration: none; }

  .app-header { background: var(--color-surface); border-bottom: 1px solid var(--color-border); padding: 1rem 1.5rem; box-shadow: var(--shadow); }
  .header-content { max-width: 60rem; margin: 0 auto; display: flex; align-items: baseline; gap: 1rem; }
  .header-actions { margin-left: auto; display: flex; gap: 0.5rem; }
  .app-title-btn { background: none; border: none; padding: 0; cursor: pointer; }
  .app-title { font-size: 1.5rem; margin: 0; color: var(--color-primary); }
  .app-subtitle { margin: 0; color: var(--color-text-muted); font-size: 0.875rem; }
  .app-main { max-width: 60rem; margin: 1.5rem auto; padding: 0 1.5rem; }
  .loading { text-align: center; padding: 3rem; color: var(--color-text-muted); }
  .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
  .list-header h2 { margin: 0; }

  .demo-section { text-align: center; margin-top: 1rem; }
  .btn-demo { font-size: 1rem; padding: 0.75rem 1.5rem; }
  .field-hint { display: block; font-size: 0.8125rem; color: var(--color-text-muted); margin-top: 0.375rem; }

  .btn { padding: 0.5rem 1.25rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; font-size: 0.9375rem; cursor: pointer; }
  .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; }
  .btn-primary { background: var(--color-primary); color: #fff; }
  .btn-primary:hover { background: var(--color-primary-hover); }
  .btn-outline { background: var(--color-surface); color: var(--color-text-muted); border-color: var(--color-border); }
  .btn-outline:hover { background: var(--color-bg); color: var(--color-text); }
</style>
