<script>
  import RhaiScriptEditor from '../../lib/components/RhaiScriptEditor.svelte';

  let { id = 'test-script' } = $props();
  let value = $state('');
</script>

<RhaiScriptEditor {id} {value} onInput={(v) => (value = v)} rows={5} ariaDescribedby="{id}-hint" />
