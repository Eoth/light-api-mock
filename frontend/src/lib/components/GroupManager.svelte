<script>
  import { getGroups, createGroup, deleteGroup, updateGroupMembers, updateService } from '../api.js';

  let {
    services = [],
    authEnabled = false,
    onNotify = () => {},
    onBack = () => {},
    onServiceUpdate = () => {},
  } = $props();

  let groups = $state([]);
  let loading = $state(true);
  let showForm = $state(false);
  let newGroupName = $state('');
  let newGroupCode = $state('');
  let editingGroup = $state(null);
  let newMember = $state('');
  let newAdmin = $state('');
  let formError = $state('');

  let servicesOfGroup = $derived((groupName) =>
    services.filter(s => s.group_name === groupName)
  );
  let ungroupedServices = $derived(
    services.filter(s => !s.group_name)
  );

  async function loadGroups() {
    try {
      groups = await getGroups();
    } catch (e) {
      onNotify(`Erreur chargement groupes : ${e.message}`, 'error');
    } finally {
      loading = false;
    }
  }

  async function handleCreateGroup(e) {
    e.preventDefault();
    formError = '';
    const name = newGroupName.trim();
    if (!name) { formError = 'Le nom du groupe est requis.'; return; }

    try {
      const created = await createGroup({ name, code: newGroupCode.trim(), admins: [], members: [] });
      groups = [...groups, created];
      newGroupName = '';
      newGroupCode = '';
      showForm = false;
      onNotify(`Groupe "${name}" cree`, 'success');
    } catch (e) {
      formError = e.message;
    }
  }

  async function handleDeleteGroup(name) {
    if (!confirm(`Supprimer le groupe "${name}" ? Les services seront dissocies mais pas supprimes.`)) return;
    try {
      await deleteGroup(name);
      groups = groups.filter(g => g.name !== name);
      if (editingGroup === name) editingGroup = null;
      onNotify(`Groupe "${name}" supprime`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  function startEdit(name) {
    editingGroup = editingGroup === name ? null : name;
    newMember = '';
    newAdmin = '';
  }

  async function assignServiceToGroup(serviceName, groupName) {
    const svc = services.find(s => s.name === serviceName);
    if (!svc) return;
    try {
      const updated = await updateService(serviceName, { ...svc, group_name: groupName || null });
      onServiceUpdate(updated);
      onNotify(`Service "${serviceName}" associe au groupe "${groupName}"`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function removeServiceFromGroup(serviceName) {
    const svc = services.find(s => s.name === serviceName);
    if (!svc) return;
    try {
      const payload = { ...svc };
      delete payload.group_name;
      const updated = await updateService(serviceName, payload);
      onServiceUpdate(updated);
      onNotify(`Service "${serviceName}" retire du groupe`, 'success');
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function addMember(groupName) {
    const username = newMember.trim();
    if (!username) return;
    const group = groups.find(g => g.name === groupName);
    if (!group) return;
    if (group.members.includes(username) || group.admins.includes(username)) {
      onNotify(`"${username}" est deja dans le groupe`, 'error');
      return;
    }
    try {
      const updated = await updateGroupMembers(groupName, {
        admins: group.admins,
        members: [...group.members, username],
      });
      groups = groups.map(g => g.name === groupName ? updated : g);
      newMember = '';
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function addAdmin(groupName) {
    const username = newAdmin.trim();
    if (!username) return;
    const group = groups.find(g => g.name === groupName);
    if (!group) return;
    if (group.admins.includes(username)) {
      onNotify(`"${username}" est deja admin`, 'error');
      return;
    }
    try {
      const members = group.members.filter(m => m !== username);
      const updated = await updateGroupMembers(groupName, {
        admins: [...group.admins, username],
        members,
      });
      groups = groups.map(g => g.name === groupName ? updated : g);
      newAdmin = '';
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  async function removePerson(groupName, username, role) {
    const group = groups.find(g => g.name === groupName);
    if (!group) return;
    try {
      const admins = role === 'admin' ? group.admins.filter(a => a !== username) : group.admins;
      const members = role === 'member' ? group.members.filter(m => m !== username) : group.members;
      const updated = await updateGroupMembers(groupName, { admins, members });
      groups = groups.map(g => g.name === groupName ? updated : g);
    } catch (e) {
      onNotify(`Erreur : ${e.message}`, 'error');
    }
  }

  $effect(() => { loadGroups(); });
</script>

<div class="group-manager">
  <div class="list-header">
    <h2>Groupes de services</h2>
    <div class="header-actions">
      <button type="button" class="btn btn-primary btn-sm" onclick={() => { showForm = !showForm; formError = ''; }}>
        {showForm ? 'Annuler' : '+ Nouveau groupe'}
      </button>
      <button type="button" class="btn btn-outline btn-sm" onclick={onBack}>Retour</button>
    </div>
  </div>

  {#if showForm}
    <form class="group-create-form" onsubmit={handleCreateGroup}>
      {#if formError}
        <div class="form-error" role="alert">{formError}</div>
      {/if}
      <div class="inline-form">
        <input type="text" bind:value={newGroupName} placeholder="Nom du groupe (ex: API Internes)" required />
        <input type="text" bind:value={newGroupCode} placeholder="Code 5 chars (auto si vide)" maxlength="5" class="code-input" />
        <button type="submit" class="btn btn-primary btn-sm">Creer</button>
      </div>
      <span class="field-hint">Le code (5 caracteres) sert de prefixe URL : /{'{code}'}/{'{service}'}/...</span>
    </form>
  {/if}

  {#if loading}
    <p class="loading-text">Chargement des groupes...</p>
  {:else if groups.length === 0}
    <p class="empty-text">Aucun groupe. Creez-en un pour organiser vos services par domaine.</p>
  {:else}
    <div class="group-list">
      {#each groups as group}
        {@const groupServices = servicesOfGroup(group.name)}
        <div class="group-card">
          <div class="group-header-row">
            <h3>{group.name}</h3>
            <span class="group-code-badge">/{group.code}</span>
            <span class="group-count">{groupServices.length} service{groupServices.length !== 1 ? 's' : ''}</span>
            <div class="group-actions">
              <button type="button" class="btn btn-outline btn-sm" onclick={() => startEdit(group.name)}>
                {editingGroup === group.name ? 'Fermer' : 'Gerer'}
              </button>
              <button type="button" class="btn btn-danger-outline btn-sm" onclick={() => handleDeleteGroup(group.name)}>
                Supprimer
              </button>
            </div>
          </div>

          {#if groupServices.length > 0}
            <ul class="service-chips">
              {#each groupServices as svc}
                <li>
                  <span class="service-chip">
                    {svc.name}
                    <button type="button" class="chip-remove" onclick={() => removeServiceFromGroup(svc.name)} title="Retirer du groupe" aria-label="Retirer {svc.name} du groupe">x</button>
                  </span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="empty-hint">Aucun service dans ce groupe. Utilisez le menu ci-dessous pour en ajouter.</p>
          {/if}

          {#if editingGroup === group.name}
            <div class="group-edit">
              <div class="edit-section">
                <h4>Ajouter un service</h4>
                {#if ungroupedServices.length === 0}
                  <p class="empty-hint">Tous les services sont deja dans un groupe.</p>
                {:else}
                  <div class="service-assign-list">
                    {#each ungroupedServices as svc}
                      <button type="button" class="btn btn-outline btn-sm" onclick={() => assignServiceToGroup(svc.name, group.name)}>
                        + {svc.name}
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>

              {#if authEnabled}
                <div class="edit-section">
                  <h4>Administrateurs</h4>
                  {#if group.admins.length === 0}
                    <p class="empty-hint">Aucun administrateur</p>
                  {/if}
                  <ul class="people-list">
                    {#each group.admins as admin}
                      <li>
                        <span>{admin}</span>
                        <button type="button" class="chip-remove" onclick={() => removePerson(group.name, admin, 'admin')} title="Retirer">x</button>
                      </li>
                    {/each}
                  </ul>
                  <div class="inline-form">
                    <input type="text" bind:value={newAdmin} placeholder="Ajouter un admin" />
                    <button type="button" class="btn btn-outline btn-sm" onclick={() => addAdmin(group.name)}>+</button>
                  </div>
                </div>

                <div class="edit-section">
                  <h4>Membres</h4>
                  {#if group.members.length === 0}
                    <p class="empty-hint">Aucun membre</p>
                  {/if}
                  <ul class="people-list">
                    {#each group.members as member}
                      <li>
                        <span>{member}</span>
                        <button type="button" class="chip-remove" onclick={() => removePerson(group.name, member, 'member')} title="Retirer">x</button>
                      </li>
                    {/each}
                  </ul>
                  <div class="inline-form">
                    <input type="text" bind:value={newMember} placeholder="Ajouter un membre" />
                    <button type="button" class="btn btn-outline btn-sm" onclick={() => addMember(group.name)}>+</button>
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .group-manager { max-width: 60rem; }
  .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
  .list-header h2 { margin: 0; }
  .header-actions { display: flex; gap: 0.5rem; }

  .group-create-form { margin-bottom: 1rem; }
  .inline-form { display: flex; gap: 0.5rem; align-items: center; }
  .inline-form input { flex: 1; padding: 0.375rem 0.75rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; background: var(--color-bg); color: var(--color-text); }

  .group-list { display: flex; flex-direction: column; gap: 0.75rem; }
  .group-card { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius); padding: 1rem 1.25rem; }

  .group-header-row { display: flex; align-items: center; gap: 1rem; flex-wrap: wrap; }
  .group-header-row h3 { margin: 0; font-size: 1rem; }
  .group-code-badge { font-family: monospace; font-size: 0.8125rem; font-weight: 700; color: var(--color-primary); background: var(--color-focus); padding: 0.1rem 0.375rem; border-radius: 3px; }
  .group-count { color: var(--color-text-muted); font-size: 0.8125rem; }
  .code-input { max-width: 8rem; text-transform: lowercase; font-family: monospace; }
  .group-actions { margin-left: auto; display: flex; gap: 0.375rem; }

  .service-chips { list-style: none; padding: 0; margin: 0.75rem 0 0; display: flex; flex-wrap: wrap; gap: 0.375rem; }
  .service-chip {
    display: inline-flex; align-items: center; gap: 0.375rem;
    padding: 0.25rem 0.625rem; border-radius: 1rem;
    background: var(--color-focus); color: var(--color-primary);
    font-size: 0.8125rem; font-weight: 600;
  }
  .chip-remove {
    background: none; border: none; color: inherit; cursor: pointer;
    font-weight: bold; font-size: 0.75rem; padding: 0; opacity: 0.7;
  }
  .chip-remove:hover { opacity: 1; }

  .empty-hint { color: var(--color-text-muted); font-size: 0.8125rem; margin: 0.5rem 0; font-style: italic; }

  .group-edit { margin-top: 1rem; padding-top: 1rem; border-top: 1px solid var(--color-border); display: flex; gap: 2rem; flex-wrap: wrap; }
  .edit-section { flex: 1; min-width: 14rem; }
  .edit-section h4 { margin: 0 0 0.5rem; font-size: 0.875rem; color: var(--color-text-muted); }

  .service-assign-list { display: flex; flex-wrap: wrap; gap: 0.375rem; }

  .people-list { list-style: none; padding: 0; margin: 0 0 0.5rem; }
  .people-list li { display: flex; align-items: center; gap: 0.5rem; padding: 0.25rem 0; font-size: 0.875rem; }

  .loading-text, .empty-text { color: var(--color-text-muted); font-size: 0.875rem; text-align: center; padding: 1rem; }
</style>
