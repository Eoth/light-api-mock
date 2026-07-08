<script>
  import FormField from '../../lib/components/FormField.svelte';

  let { id, label, hint = '', error = '', required = false } = $props();
  let value = $state('');
</script>

<FormField {id} {label} {hint} {error} {required}>
  {#snippet children({ id, describedBy, invalid })}
    <input {id} type="text" bind:value aria-describedby={describedBy} aria-invalid={invalid} />
  {/snippet}
</FormField>
