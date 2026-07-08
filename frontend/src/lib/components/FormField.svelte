<script>
  // Wrapper label+champ+hint+erreur, agnostique du type de controle : le
  // champ reel (input/select/textarea) est fourni par l'appelant via le
  // snippet `children`, qui recoit { id, describedBy, invalid } a poser sur
  // son element pour garder l'accessibilite correcte (for/id, aria-describedby,
  // aria-invalid) sans dupliquer cette logique a chaque formulaire.
  let {
    id,
    label,
    hint = '',
    error = '',
    required = false,
    checkbox = false,
    children,
  } = $props();

  let hintId = $derived(hint ? `${id}-hint` : undefined);
  let errorId = $derived(error ? `${id}-error` : undefined);
  let describedBy = $derived([hintId, errorId].filter(Boolean).join(' ') || undefined);
</script>

<div class="form-field" class:form-field-check={checkbox}>
  <label for={id}>{label}{#if required}<span aria-hidden="true"> *</span>{/if}</label>
  {@render children?.({ id, describedBy, invalid: !!error })}
  {#if hint}
    <span class="field-hint" id={hintId}>{hint}</span>
  {/if}
  {#if error}
    <span class="form-error" id={errorId} role="alert">{error}</span>
  {/if}
</div>
