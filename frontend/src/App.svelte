<script>
  import { getServices, getConfig, putConfig, toggleService, createService, updateService, resetConfig, getAuthStatus, validateToken, getGroups } from './lib/api.js';
  import { auth, isLoggedIn, setAuth, logout, restoreAuth } from './lib/auth.svelte.js';
  import ServiceList from './lib/components/ServiceList.svelte';
  import ServiceDetail from './lib/components/ServiceDetail.svelte';
  import ServiceForm from './lib/components/ServiceForm.svelte';
  import Notification from './lib/components/Notification.svelte';
  import RequestLog from './lib/components/RequestLog.svelte';
  import LoginForm from './lib/components/LoginForm.svelte';
  import GroupManager from './lib/components/GroupManager.svelte';

  let services = $state([]);
  let groups = $state([]);
  let notification = $state({ message: '', type: 'info', visible: false });
  let selectedService = $state(null);
  let view = $state('list');
  let loading = $state(true);
  let darkMode = $state(
    typeof localStorage !== 'undefined' && localStorage.getItem('lightmock-theme') !== null
      ? localStorage.getItem('lightmock-theme') === 'dark'
      : typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches
  );

  $effect(() => {
    document.documentElement.setAttribute('data-theme', darkMode ? 'dark' : 'light');
    localStorage.setItem('lightmock-theme', darkMode ? 'dark' : 'light');
  });

  const demoService = {
    name: 'users-api',
    listen_path: '/users/{id}',
    real_target_url: 'https://jsonplaceholder.typicode.com',
    is_mocked: true,
    rewrite_directory_urls: false,
    group_name: null,
    wsdl_mode: 'auto',
    rules: [
      {
        name: 'get-user-mock',
        method: 'GET',
        sub_path: null,
        action: 'mock',
        script: 'let role = if random_int(1,5) <= 4 { "user" } else { "admin" };\n#{ role: role, since: date_past() }',
        conditions: { all_of: [], any_of: [] },
        response: {
          status: 200,
          headers: [{ name: 'Content-Type', value: 'application/json' }],
          body: [{ type: 'Template', template: '{"id":{{path.id}},"name":"{{fake.FirstName}} {{fake.LastName}}","email":"{{fake.Email}}","phone":"{{fake.PhoneNumberFR}}","company":"{{fake.CompanyName}}","role":"{{script.role}}","member_since":"{{script.since}}","address":{"street":"{{fake.StreetName}}","city":"{{fake.CityFR}}","zipcode":"{{fake.PostcodeFR}}"},"meta":{"request_id":"{{uuid}}","timestamp":{{now_ms}},"seq":{{seq}}}}' }],
          chaos: null,
        },
      },
      {
        name: 'get-user-proxy',
        method: 'GET',
        sub_path: null,
        action: 'proxy',
        script: null,
        conditions: { all_of: [{ source: { type: 'Header', key: 'x-real-backend' }, operator: { type: 'Eq', value: 'true' } }], any_of: [] },
        response: { status: 200, headers: [], body: [{ type: 'Literal', value: '' }], chaos: null },
      },
    ],
  };

  async function init() {
    restoreAuth();

    try {
      const status = await getAuthStatus();
      auth.enabled = status.enabled;
    } catch {
      auth.enabled = false;
    }

    const params = new URLSearchParams(window.location.search);
    const silentToken = params.get('token');
    if (silentToken && auth.enabled) {
      window.history.replaceState({}, '', window.location.pathname);
      try {
        const result = await validateToken(silentToken);
        setAuth(result);
      } catch {
        showNotification('Token invalide', 'error');
      }
    }

    if (isLoggedIn()) {
      await loadData();
    }
    loading = false;
  }

  async function loadData() {
    try {
      services = await getServices();
      try { groups = await getGroups(); } catch { groups = []; }
    } catch (e) {
      showNotification(`Erreur de chargement : ${e.message}`, 'error');
    }
  }

  async function handleLogin() {
    await loadData();
  }

  function handleLogout() {
    logout();
    services = [];
    groups = [];
    view = 'list';
    selectedService = null;
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
      const result = await createService(svc);
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
      const result = await createService(demoService);
      services = [...services, result];
      showNotification('Service de demo charge (users-api avec mock, proxy, script rhai)', 'success');
    } catch (e) {
      showNotification(`Erreur : ${e.message}`, 'error');
    }
  }

  async function exportConfig() {
    try {
      const config = { services, groups };
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
  let importPending = $state(null);
  async function importConfig() { fileInput?.click(); }

  async function handleFileImport(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const config = JSON.parse(text);
      if (!config.services || !Array.isArray(config.services)) {
        throw new Error('Format invalide : "services" attendu');
      }
      if (!config.groups) config.groups = [];
      const canReplace = !auth.enabled || auth.isSuperAdmin;
      if (canReplace) {
        importPending = config;
      } else {
        await doImportMerge(config);
      }
    } catch (e) {
      showNotification(`Erreur import : ${e.message}`, 'error');
    }
    e.target.value = '';
  }

  async function doImportReplace(config) {
    importPending = null;
    try {
      await putConfig(config);
      services = config.services;
      groups = config.groups || [];
      showNotification(`Configuration remplacee (${config.services.length} services)`, 'success');
      view = 'list'; selectedService = null;
    } catch (e) {
      showNotification(`Erreur import : ${e.message}`, 'error');
    }
  }

  async function doImportMerge(config) {
    importPending = null;
    try {
      let added = 0;
      for (const svc of config.services) {
        if (!services.some(s => s.name === svc.name)) {
          const result = await createService(svc);
          services = [...services, result];
          added++;
        }
      }
      showNotification(`${added} service(s) ajoute(s), ${config.services.length - added} doublon(s) ignore(s)`, 'success');
      view = 'list'; selectedService = null;
    } catch (e) {
      showNotification(`Erreur import : ${e.message}`, 'error');
    }
  }

  function showNotification(message, type) {
    notification = { message, type, visible: true };
    setTimeout(() => { notification = { ...notification, visible: false }; }, 4000);
  }

  async function handleReset() {
    if (!confirm('Supprimer tous les services et repartir de zero ?')) return;
    try {
      await resetConfig();
      services = [];
      selectedService = null;
      view = 'list';
      showNotification('Configuration reinitialised — tous les services supprimes', 'success');
    } catch (e) {
      showNotification(`Erreur reset : ${e.message}`, 'error');
    }
  }

  $effect(() => { init(); });

  let currentService = $derived(services.find(s => s.name === selectedService) ?? null);
  let availableGroupsList = $derived(groups.map(g => ({ name: g.name, code: g.code })));
</script>

<a href="#main-content" class="sr-only skip-link">Aller au contenu principal</a>

{#if loading}
  <div class="loading" role="status"><p>Chargement...</p></div>
{:else if auth.enabled && !isLoggedIn()}
  <LoginForm onLogin={handleLogin} />
{:else}
  <header class="app-header">
    <div class="header-content">
      <button type="button" class="app-title-btn" onclick={handleBack}>
        <h1 class="app-title">lightMock</h1>
      </button>
      <p class="app-subtitle">Mock &amp; Proxy Intelligent</p>
      <div class="header-actions">
        <button type="button" class="btn btn-sm btn-outline" onclick={() => view = 'logs'} title="Journal des requetes">Logs</button>
        <button type="button" class="btn btn-sm btn-outline" onclick={() => view = 'groups'} title="Gestion des groupes">Groupes</button>
        <button type="button" class="btn btn-sm btn-outline" onclick={exportConfig} title="Telecharger la configuration">Export</button>
        <button type="button" class="btn btn-sm btn-outline" onclick={importConfig} title="Charger une configuration">Import</button>
        <button type="button" class="btn btn-sm btn-outline btn-danger-outline" onclick={handleReset} title="Supprimer tous les services">Reset</button>
        <button type="button" class="btn btn-sm btn-outline" onclick={() => darkMode = !darkMode} title={darkMode ? 'Mode clair' : 'Mode sombre'} aria-label={darkMode ? 'Activer le mode clair' : 'Activer le mode sombre'}>
          {darkMode ? 'Clair' : 'Sombre'}
        </button>
        {#if auth.enabled}
          <span class="user-badge" title={auth.isSuperAdmin ? 'Super-admin' : 'Utilisateur'}>{auth.username}</span>
          <button type="button" class="btn btn-sm btn-outline" onclick={handleLogout}>Deconnexion</button>
        {/if}
        <input type="file" accept=".json" style="display:none" bind:this={fileInput} onchange={handleFileImport} />
      </div>
    </div>
  </header>

  {#if view !== 'list'}
    <nav class="breadcrumb" aria-label="Fil d'Ariane">
      <ol>
        <li><button type="button" class="breadcrumb-link" onclick={handleBack}>Services</button></li>
        <li aria-current="page">
          {#if view === 'logs'}Journal des requetes
          {:else if view === 'groups'}Groupes de services
          {:else if view === 'add'}Ajouter un service
          {:else if view === 'detail' && currentService}Detail : {currentService.name}
          {/if}
        </li>
      </ol>
    </nav>
  {/if}

  {#if importPending}
    <div class="modal-overlay" role="dialog" aria-modal="true" aria-label="Mode d'import">
      <div class="modal-content">
        <div class="modal-header">
          <h3>Importer la configuration</h3>
          <button type="button" class="btn-close" onclick={() => importPending = null} aria-label="Fermer">&#10005;</button>
        </div>
        <p>{importPending.services.length} service(s) et {importPending.groups?.length ?? 0} groupe(s) trouves dans le fichier.</p>
        <div class="import-actions">
          <button type="button" class="btn btn-primary" onclick={() => doImportReplace(importPending)}>
            Remplacer tout
          </button>
          <button type="button" class="btn btn-outline" onclick={() => doImportMerge(importPending)}>
            Fusionner (ajouter les manquants)
          </button>
          <button type="button" class="btn btn-secondary" onclick={() => importPending = null}>
            Annuler
          </button>
        </div>
      </div>
    </div>
  {/if}

  <main id="main-content" class="app-main">
    <Notification message={notification.message} type={notification.type} visible={notification.visible} />

    {#if view === 'logs'}
      <RequestLog />
    {:else if view === 'groups'}
      <GroupManager {services} authEnabled={auth.enabled} onNotify={showNotification} onBack={handleBack} onServiceUpdate={handleServiceUpdate} onGroupsChange={(g) => groups = g} />
    {:else if view === 'add'}
      <ServiceForm
        existingNames={services.map(s => s.name)}
        availableGroups={availableGroupsList}
        authEnabled={auth.enabled}
        onSave={handleAddService}
        onCancel={handleBack}
      />
    {:else if view === 'detail' && currentService}
      <ServiceDetail service={currentService} onBack={handleBack} onUpdate={handleServiceUpdate} onDelete={handleServiceDelete} onNotify={showNotification} />
    {:else}
      <div class="list-header">
        <h2>Services</h2>
        <button type="button" class="btn btn-primary" onclick={() => view = 'add'}>+ Ajouter un service</button>
      </div>
      <ServiceList {services} {groups} onToggle={handleToggle} onSelect={handleSelect} />
      {#if services.length === 0}
        <div class="demo-section">
          <button type="button" class="btn btn-outline btn-demo" onclick={loadDemo}>
            Charger un exemple
          </button>
          <span class="field-hint">Service users-api avec mock (fake data, script rhai ratio 4/5) et proxy conditionnel.</span>
        </div>
      {/if}
    {/if}
  </main>
{/if}

<style>
  :global(.skip-link:focus) { position: fixed; top: 0; left: 0; z-index: 1000; width: auto; height: auto; clip: auto; padding: 0.75rem 1.5rem; background: var(--color-primary); color: #fff; font-weight: 600; text-decoration: none; }

  .app-header { background: var(--color-surface); border-bottom: 1px solid var(--color-border); padding: 1rem 1.5rem; box-shadow: var(--shadow); }
  .header-content { max-width: 60rem; margin: 0 auto; display: flex; align-items: baseline; gap: 1rem; flex-wrap: wrap; }
  .header-actions { margin-left: auto; display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .app-title-btn { background: none; border: none; padding: 0; cursor: pointer; }
  .app-title { font-size: 1.5rem; margin: 0; color: var(--color-primary); }
  .app-subtitle { margin: 0; color: var(--color-text-muted); font-size: 0.875rem; }
  .app-main { max-width: 60rem; margin: 1.5rem auto; padding: 0 1.5rem; }

  .breadcrumb { max-width: 60rem; margin: 0 auto; padding: 0.5rem 1.5rem; }
  .breadcrumb ol { list-style: none; display: flex; align-items: center; gap: 0.375rem; margin: 0; padding: 0; font-size: 0.875rem; }
  .breadcrumb li { display: flex; align-items: center; gap: 0.375rem; color: var(--color-text-muted); }
  .breadcrumb li:not(:last-child)::after { content: "/"; color: var(--color-border); }
  .breadcrumb li[aria-current="page"] { color: var(--color-text); font-weight: 600; }
  .breadcrumb-link { background: none; border: none; padding: 0; color: var(--color-primary); cursor: pointer; font: inherit; text-decoration: underline; text-underline-offset: 2px; }
  .breadcrumb-link:hover { color: var(--color-primary-hover); }

  .import-actions { display: flex; gap: 0.75rem; flex-wrap: wrap; margin-top: 1rem; }
  .loading { text-align: center; padding: 3rem; color: var(--color-text-muted); }
  .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
  .list-header h2 { margin: 0; }

  .demo-section { text-align: center; margin-top: 1rem; }
  .btn-demo { font-size: 1rem; padding: 0.75rem 1.5rem; }

  .user-badge {
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--color-primary);
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 0.2rem 0.625rem;
  }

</style>
