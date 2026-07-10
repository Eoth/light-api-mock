<script>
  // Editeur de script Rhai avec autocompletion legere des fonctions natives
  // lightMock. Liste des fonctions : source unique dans ../rhai-functions.js
  // (partagee avec la doc contextuelle affichee sous chaque editeur dans
  // RuleForm.svelte — ne pas dupliquer cette liste ici).
  //
  // Choix technique : simple <textarea> + liste deroulante positionnee sous
  // le champ (pas de suivi pixel-precis du curseur, pas de coordonnees de
  // caret calculees). Aucun editeur de code (CodeMirror/Monaco) n'est
  // present ailleurs dans le projet ; en ajouter un uniquement pour ce
  // besoin serait disproportionne (cf CLAUDE.md).
  //
  // Accessibilite : le focus DOM reste toujours sur le <textarea> (role
  // implicite "textbox", qui supporte aria-autocomplete/aria-activedescendant
  // — pas besoin d'un role="combobox" explicite). La suggestion mise en
  // surbrillance est signalee via aria-activedescendant vers l'option
  // correspondante dans la listbox, jamais par un deplacement de focus reel.
  import { tick, untrack } from 'svelte';
  import { filterRhaiFunctions, tokenAtCursor, computeInsertSelection } from '../rhai-functions.js';

  let {
    id,
    value = '',
    onInput = () => {},
    rows = 5,
    placeholder = '',
    ariaDescribedby = undefined,
  } = $props();

  let textareaEl = $state(null);
  let showSuggestions = $state(false);
  let suggestions = $state([]);
  let activeIndex = $state(0);

  // untrack() : `id` est fige par instance (nouvel id -> nouveau composant
  // via {#snippet}/{@render}, jamais mute en place), lecture unique
  // volontaire — meme pattern que le reste du projet (cf CLAUDE.md).
  const listboxId = untrack(() => `${id}-rhai-suggestions`);

  function openSuggestionsFor(text, cursorPos, { allowEmpty = false } = {}) {
    const { token } = tokenAtCursor(text, cursorPos);
    if (!token && !allowEmpty) {
      showSuggestions = false;
      return;
    }
    const matches = filterRhaiFunctions(token);
    if (matches.length === 0) {
      showSuggestions = false;
      return;
    }
    suggestions = matches;
    activeIndex = 0;
    showSuggestions = true;
  }

  function handleInput(e) {
    const newValue = e.target.value;
    onInput(newValue);
    openSuggestionsFor(newValue, e.target.selectionStart);
  }

  function handleKeydown(e) {
    // Raccourci explicite : ouvre l'autocompletion meme sans prefixe deja
    // tape (liste complete), ou filtree si le curseur est deja dans un mot.
    if (e.ctrlKey && e.code === 'Space') {
      e.preventDefault();
      openSuggestionsFor(value, e.target.selectionStart, { allowEmpty: true });
      return;
    }
    if (!showSuggestions) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      activeIndex = (activeIndex + 1) % suggestions.length;
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      activeIndex = (activeIndex - 1 + suggestions.length) % suggestions.length;
    } else if (e.key === 'Enter') {
      e.preventDefault();
      selectSuggestion(suggestions[activeIndex]);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      showSuggestions = false;
    }
  }

  function handleBlur() {
    showSuggestions = false;
  }

  async function selectSuggestion(fn) {
    const el = textareaEl;
    if (!el) return;
    const cursorPos = el.selectionStart;
    const { start } = tokenAtCursor(value, cursorPos);
    const newValue = value.slice(0, start) + fn.insertText + value.slice(cursorPos);
    const sel = computeInsertSelection(fn.insertText);
    showSuggestions = false;
    onInput(newValue);
    // La valeur du textarea est mise a jour de facon reactive (prop `value`,
    // pas bind:) : attendre le prochain tick avant de repositionner la
    // selection, sinon setSelectionRange s'applique a l'ancien contenu DOM.
    await tick();
    el.focus();
    el.setSelectionRange(start + sel.start, start + sel.end);
  }
</script>

<div class="rhai-editor">
  <textarea
    bind:this={textareaEl}
    {id}
    {value}
    oninput={handleInput}
    onkeydown={handleKeydown}
    onblur={handleBlur}
    {rows}
    class="script-textarea"
    {placeholder}
    aria-describedby={ariaDescribedby}
    aria-autocomplete="list"
    aria-controls={showSuggestions ? listboxId : undefined}
    aria-activedescendant={showSuggestions ? `${listboxId}-opt-${activeIndex}` : undefined}
    data-testid="rhai-script-editor-textarea-{id}"
  ></textarea>
  {#if showSuggestions}
    <ul class="rhai-suggestions" id={listboxId} role="listbox" aria-label="Fonctions Rhai disponibles" data-testid="rhai-script-editor-suggestions-{id}">
      {#each suggestions as fn, i (fn.name)}
        <li
          id="{listboxId}-opt-{i}"
          role="option"
          aria-selected={i === activeIndex}
          class="rhai-suggestion"
          class:active={i === activeIndex}
          onmousedown={(e) => { e.preventDefault(); selectSuggestion(fn); }}
          onmouseenter={() => activeIndex = i}
          data-testid="rhai-script-editor-suggestion-{id}-{fn.name}"
        >
          <code class="rhai-suggestion-sig">{fn.signature}</code>
          <span class="rhai-suggestion-desc">{fn.description}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .rhai-editor { position: relative; }

  .script-textarea { width: 100%; font-family: 'Cascadia Code', 'Fira Code', monospace; font-size: 0.8125rem; padding: 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-bg); color: var(--color-text); resize: vertical; font-variant-ligatures: none; }

  .rhai-suggestions {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    z-index: 30;
    margin: 0.25rem 0 0;
    padding: 0.25rem;
    list-style: none;
    max-height: 14rem;
    overflow-y: auto;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    max-width: 100%;
    box-sizing: border-box;
  }

  .rhai-suggestion {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    padding: 0.375rem 0.5rem;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .rhai-suggestion.active,
  .rhai-suggestion:hover {
    background: var(--color-focus);
  }

  .rhai-suggestion-sig {
    font-family: 'Cascadia Code', 'Fira Code', monospace;
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--color-primary);
  }

  .rhai-suggestion-desc {
    font-size: 0.75rem;
    color: var(--color-text-muted);
  }

  @media (max-width: 30rem) {
    .rhai-suggestions { max-height: 10rem; }
  }
</style>
