<script>
  import { login as apiLogin } from '../api.js';
  import { setAuth } from '../auth.svelte.js';
  import { t } from '../i18n.svelte.js';

  let { onLogin = () => {} } = $props();

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleSubmit(e) {
    e.preventDefault();
    error = '';

    if (!username.trim()) { error = t("The user name is required."); return; }
    if (!password) { error = t("The password is required."); return; }

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
    <p class="login-subtitle">{t("Sign-in required")}</p>

    <form class="login-form" onsubmit={handleSubmit}>
      {#if error}
        <div class="form-error" role="alert" aria-live="assertive" data-testid="login-form-error">{error}</div>
      {/if}

      <div class="form-field">
        <label for="login-user">{t("User name")}</label>
        <input
          id="login-user"
          type="text"
          bind:value={username}
          required
          autocomplete="username"
          disabled={loading}
          data-testid="login-form-username-input"
        />
      </div>

      <div class="form-field">
        <label for="login-pass">{t("Password")}</label>
        <input
          id="login-pass"
          type="password"
          bind:value={password}
          required
          autocomplete="current-password"
          disabled={loading}
          data-testid="login-form-password-input"
        />
      </div>

      <button type="submit" class="btn btn-primary btn-login" disabled={loading} data-testid="login-form-submit-button">
        {loading ? t("Signing in...") : t("Sign in")}
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

  .btn-login {
    width: 100%;
    padding: 0.625rem;
    font-size: 1rem;
    margin-top: 0.5rem;
  }
</style>
