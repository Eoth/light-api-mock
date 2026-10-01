<script>
  import { t } from '../i18n.svelte.js';
  let { active = false } = $props();
</script>

<span
  class="badge"
  class:active
  role="status"
  aria-label={active ? t("Mock mode on") : t("Proxy mode on")}
  data-testid="status-badge"
>
  {active ? t("MOCK") : t("PROXY")}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    padding: 0.25rem 0.625rem;
    border-radius: var(--radius-m);
    font-size: 0.75rem;
    font-weight: var(--weight-heavy);
    letter-spacing: 0.05em;
    text-transform: uppercase;
    background: var(--color-proxy-bg);
    color: var(--color-proxy-text);
    border: var(--line-thin) solid var(--color-proxy);
  }

  /* Dashed: the mode that imitates (app.css, badges). */
  .badge.active {
    background: var(--color-mock-bg);
    color: var(--color-mock-text);
    border-style: dashed;
    border-color: var(--color-mock);
  }
</style>
