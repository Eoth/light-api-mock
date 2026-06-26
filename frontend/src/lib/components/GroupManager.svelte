<script>
  import { getGroups, createGroup, deleteGroup, updateGroupMembers } from '../api.js';

  let { onNotify = () => {}, onBack = () => {} } = $props();

  let groups = $state([]);
  let loading = $state(true);
  let showForm = $state(false);
  let newGroupName = $state('');
  let editingGroup = $state(null);
  let newMember = $state('');
  let newAdmin = $state('');
  let formError = $state('');

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
      const created = await createGroup({ name, admins: [], members: [] });
      groups = [...groups, created];
      newGroupName = '';
      showForm = false;
      onNotify(`Groupe "${name}" cree`, 'success');
    } catch (e) {
      formError = e.message;
    }
  }

  async function handleDeleteGroup(name) {
    if (!confirm(`Supprimer le groupe "${name}" ?`)) return;
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
    <h2>Groupes</h2>
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
        <input type="text" bind:value={newGroupName} placeholder="Nom du groupe" required />
        <button type="submit" class="btn btn-primary btn-sm">Creer</button>
      </div>
    </form>
  {/if}

  {#if loading}
    <p class="loading-text">Chargement des groupes...</p>
  {:else if groups.length === 0}
    <p class="empty-text">Aucun groupe. Creez-en un pour organiser les services.</p>
  {:else}
    <div class="group-list">
      {#each groups as group}
        <div class="group-card">
          <div class="group-header">
            <h3>{group.name}</h3>
            <span class="group-count">{group.admins.length + group.members.length} utilisateur(s)</span>
            <div class="group-actions">
              <button type="button" class="btn btn-outline btn-sm" onclick={() => startEdit(group.name)}>
                {editingGroup === group.name ? 'Fermer' : 'Gerer'}
              </button>
              <button type="button" class="btn btn-danger-outline btn-sm" onclick={() => handleDeleteGroup(group.name)}>
                Supprimer
              </button>
            </div>
          </div>

          {#if editingGroup === group.name}
            <div class="group-edit">
              <div class="people-section">
                <h4>Administrateurs</h4>
                {#if group.admins.length === 0}
                  <p class="empty-text">Aucun administrateur</p>
                {/if}
                <ul class="people-list">
                  {#each group.admins as admin}
                    <li>
                      <span>{admin}</span>
                      <button type="button" class="btn-remove" onclick={() => removePerson(group.name, admin, 'admin')} title="Retirer">x</button>
                    </li>
                  {/each}
                </ul>
                <div class="inline-form">
                  <input type="text" bind:value={newAdmin} placeholder="Ajouter un admin" />
                  <button type="button" class="btn btn-outline btn-sm" onclick={() => addAdmin(group.name)}>+</button>
                </div>
              </div>

              <div class="people-section">
                <h4>Membres</h4>
                {#if group.members.length === 0}
                  <p class="empty-text">Aucun membre</p>
                {/if}
                <ul class="people-list">
                  {#each group.members as member}
                    <li>
                      <span>{member}</span>
                      <button type="button" class="btn-remove" onclick={() => removePerson(group.name, member, 'member')} title="Retirer">x</button>
                    </li>
                  {/each}
                </ul>
                <div class="inline-form">
                  <input type="text" bind:value={newMember} placeholder="Ajouter un membre" />
                  <button type="button" class="btn btn-outline btn-sm" onclick={() => addMember(group.name)}>+</button>
                </div>
              </div>
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
  .group-header { display: flex; align-items: center; gap: 1rem; flex-wrap: wrap; }
  .group-header h3 { margin: 0; font-size: 1rem; }
  .group-count { color: var(--color-text-muted); font-size: 0.8125rem; }
  .group-actions { margin-left: auto; display: flex; gap: 0.375rem; }

  .group-edit { margin-top: 1rem; padding-top: 1rem; border-top: 1px solid var(--color-border); display: flex; gap: 2rem; flex-wrap: wrap; }
  .people-section { flex: 1; min-width: 14rem; }
  .people-section h4 { margin: 0 0 0.5rem; font-size: 0.875rem; color: var(--color-text-muted); }

  .people-list { list-style: none; padding: 0; margin: 0 0 0.5rem; }
  .people-list li { display: flex; align-items: center; gap: 0.5rem; padding: 0.25rem 0; font-size: 0.875rem; }
  .btn-remove { background: none; border: none; color: var(--color-danger); cursor: pointer; font-weight: bold; font-size: 0.875rem; padding: 0 0.25rem; }
  .btn-remove:hover { text-decoration: underline; }

  .form-error { background: #fef2f2; color: var(--color-danger); padding: 0.5rem 0.75rem; border-radius: var(--radius); font-size: 0.875rem; border: 1px solid #fecaca; margin-bottom: 0.5rem; }
  :global([data-theme="dark"]) .form-error { background: #451a1a; border-color: #7f1d1d; }

  .loading-text, .empty-text { color: var(--color-text-muted); font-size: 0.875rem; text-align: center; padding: 1rem; }

  .btn { padding: 0.5rem 1.25rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; font-size: 0.9375rem; cursor: pointer; }
  .btn-sm { padding: 0.25rem 0.75rem; font-size: 0.8125rem; }
  .btn-primary { background: var(--color-primary); color: #fff; }
  .btn-primary:hover { background: var(--color-primary-hover); }
  .btn-outline { background: var(--color-surface); color: var(--color-text-muted); border-color: var(--color-border); }
  .btn-outline:hover { background: var(--color-bg); color: var(--color-text); }
  .btn-danger-outline { color: var(--color-danger); border-color: var(--color-danger); }
  .btn-danger-outline:hover { background: var(--color-danger); color: #fff; }
</style>
