<script>
  let { checked = false, label = '', disabled = false, onchange = () => {} } = $props();

  function handleClick() {
    if (disabled) return;
    onchange(!checked);
  }

  function handleKeydown(e) {
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      handleClick();
    }
  }
</script>

<div class="toggle-wrapper">
  <span class="toggle-label" id="toggle-label-{label.replace(/\s+/g, '-')}">{label}</span>
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-labelledby="toggle-label-{label.replace(/\s+/g, '-')}"
    class="toggle-switch"
    class:active={checked}
    {disabled}
    onclick={handleClick}
    onkeydown={handleKeydown}
  >
    <span class="toggle-knob"></span>
    <span class="sr-only">{checked ? 'Activé' : 'Désactivé'}</span>
  </button>
  <span class="toggle-status" aria-live="polite">
    {checked ? 'ON' : 'OFF'}
  </span>
</div>

<style>
  .toggle-wrapper {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .toggle-label {
    font-weight: 500;
    color: var(--color-text);
  }

  .toggle-switch {
    position: relative;
    width: 52px;
    height: 28px;
    border-radius: 14px;
    border: 2px solid var(--color-border);
    background: var(--color-border);
    padding: 0;
    transition: background-color 0.2s, border-color 0.2s;
  }

  .toggle-switch:focus-visible {
    outline: 3px solid var(--color-primary);
    outline-offset: 2px;
  }

  .toggle-switch.active {
    background: var(--color-success);
    border-color: var(--color-success);
  }

  .toggle-switch:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .toggle-knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--color-surface);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
    transition: transform 0.2s;
  }

  .toggle-switch.active .toggle-knob {
    transform: translateX(24px);
  }

  .toggle-status {
    font-size: 0.875rem;
    font-weight: 600;
    min-width: 2rem;
    color: var(--color-text-muted);
  }

  .toggle-switch.active + .toggle-status {
    color: var(--color-success);
  }
</style>
