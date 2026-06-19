<script>
  let { active = false } = $props();
</script>

<span
  class="badge"
  class:active
  role="status"
  aria-label={active ? 'Mode mock activé' : 'Mode proxy activé'}
>
  {active ? 'MOCK' : 'PROXY'}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    padding: 0.25rem 0.625rem;
    border-radius: var(--radius);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    background: var(--color-border);
    color: var(--color-text-muted);
  }

  .badge.active {
    background: #198754;
    color: #ffffff;
  }
</style>
