<script>
  import { login as apiLogin } from '../api.js';
  import { setAuth } from '../auth.svelte.js';

  let { onLogin = () => {} } = $props();

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleSubmit(e) {
    e.preventDefault();
    error = '';

    if (!username.trim()) { error = "Le nom d'utilisateur est requis."; return; }
    if (!password) { error = 'Le mot de passe est requis.'; return; }

    loading = true;
    try {
      const result = await apiLogin(username.trim(), password);
      setAuth(result);
      onLogin(result);
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }
</script>

<div class="login-container">
  <div class="login-card">
    <h1 class="login-title">lightMock</h1>
    <p class="login-subtitle">Connexion requise</p>

    <form class="login-form" onsubmit={handleSubmit}>
      {#if error}
        <div class="form-error" role="alert" aria-live="assertive">{error}</div>
      {/if}

      <div class="form-field">
        <label for="login-user">Nom d'utilisateur</label>
        <input
          id="login-user"
          type="text"
          bind:value={username}
          required
          autocomplete="username"
          disabled={loading}
        />
      </div>

      <div class="form-field">
        <label for="login-pass">Mot de passe</label>
        <input
          id="login-pass"
          type="password"
          bind:value={password}
          required
          autocomplete="current-password"
          disabled={loading}
        />
      </div>

      <button type="submit" class="btn btn-primary btn-login" disabled={loading}>
        {loading ? 'Connexion...' : 'Se connecter'}
      </button>
    </form>
  </div>
</div>

<style>
  .login-container {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: 1rem;
  }

  .login-card {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 2.5rem;
    width: 100%;
    max-width: 24rem;
    box-shadow: var(--shadow);
  }

  .login-title {
    font-size: 1.75rem;
    color: var(--color-primary);
    margin: 0 0 0.25rem;
    text-align: center;
  }

  .login-subtitle {
    color: var(--color-text-muted);
    font-size: 0.875rem;
    margin: 0 0 1.5rem;
    text-align: center;
  }

  .login-form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .form-field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .form-field label {
    font-weight: 600;
    font-size: 0.875rem;
    color: var(--color-text);
  }

  .form-field input {
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    font-size: 0.9375rem;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .form-field input:focus {
    outline: 2px solid var(--color-primary);
    outline-offset: 1px;
  }

  .form-error {
    background: #fef2f2;
    color: var(--color-danger);
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius);
    font-size: 0.875rem;
    border: 1px solid #fecaca;
  }

  :global([data-theme="dark"]) .form-error {
    background: #451a1a;
    border-color: #7f1d1d;
  }

  .btn-login {
    width: 100%;
    padding: 0.625rem;
    font-size: 1rem;
    margin-top: 0.5rem;
  }

  .btn { padding: 0.5rem 1.25rem; border-radius: var(--radius); border: 1px solid transparent; font-weight: 600; font-size: 0.9375rem; cursor: pointer; }
  .btn-primary { background: var(--color-primary); color: #fff; }
  .btn-primary:hover { background: var(--color-primary-hover); }
  .btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
