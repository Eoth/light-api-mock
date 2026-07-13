<script>
  // Orchestrateur du formulaire de regle : possede les champs identitaires
  // (nom/methode/sous-chemin), l'action (mock/proxy) et les conditions,
  // et pilote le flux de sauvegarde (validation locale, avertissement
  // proxy->mock obsolete, detecteur de conflit). Le detail de chaque
  // section est delegue a un sous-composant dedie (cf CLAUDE.md, "RuleForm
  // decoupe en sous-composants") :
  //   - RuleActionSelector.svelte  : selecteur Mock/Proxy
  //   - RuleConditionsEditor.svelte: fieldsets Conditions ET/OU
  //   - RuleResponseSection.svelte : fieldset "Reponse mockee" complet
  //     (mode/en-tetes/corps + les 3 blocs de script + Chaos), expose
  //     getPayload()/validate() via bind:this
  //   - RuleWarnings.svelte        : bandeaux d'avertissement non bloquants
  //     (regle proxy devenue obsolete / conflit de regles)
  // RuleTester reste cable ici (deja son propre composant avant ce decoupage).
  import RuleActionSelector from './RuleActionSelector.svelte';
  import RuleConditionsEditor from './RuleConditionsEditor.svelte';
  import RuleResponseSection from './RuleResponseSection.svelte';
  import RuleWarnings from './RuleWarnings.svelte';
  import RuleTester from './RuleTester.svelte';
  import { getLogs, checkRuleConflicts } from '../api.js';
  import { combinePathParamNames } from '../path-params.js';

  import { untrack } from 'svelte';

  let {
    rule = null,
    existingRules = [],
    // Position ou cette regle se retrouvera une fois sauvegardee (index
    // d'edition inchange, ou service.rules.length pour un ajout — toujours
    // en fin de liste, cf ServiceDetail.svelte::handleSaveRule). Sert
    // uniquement au detecteur de conflit pour determiner laquelle des deux
    // regles en cause s'appliquerait reellement. Par defaut
    // (composant utilise sans ce contexte, ex. tests unitaires isoles) on
    // suppose un ajout en fin de liste.
    draftPosition = null,
    serviceName = null,
    groupName = null,
    listenPath = '',
    // Service "purement mocke" (sujet 22, cf CLAUDE.md §3) : quand vrai,
    // l'action "Proxy" n'a plus de sens (aucune cible vers laquelle
    // relayer) — masquee ci-dessous plutot que simplement desactivee.
    isPurelyMocked = false,
    onSave = () => {},
    onCancel = () => {},
  } = $props();

  // Source unique : existingRuleNames (utilisee pour l'unicite du nom) est
  // toujours derivee de existingRules (utilisee aussi pour la detection de
  // conflit), jamais une prop separee — evite deux listes qui pourraient
  // diverger (meme principe que tpl-utils.js).
  let existingRuleNames = $derived(existingRules.map((r) => r.name));
  let effectiveDraftPosition = $derived(draftPosition ?? existingRules.length);

  const init = untrack(() => rule ? JSON.parse(JSON.stringify(rule)) : null);
  let name = $state(init?.name ?? '');
  let ruleMethod = $state(init?.method ?? 'GET');
  let subPath = $state(init?.sub_path ?? '');
  // Une regle heritee (action=proxy) editee alors que le service est deja
  // purement mocke n'a plus de choix valide autre que "mock" — l'option
  // Proxy est masquee par RuleActionSelector, donc ce cas ne doit pas
  // rester bloque sur une valeur qu'aucun radio visible ne represente.
  let ruleAction = $state(untrack(() => isPurelyMocked && init?.action === 'proxy' ? 'mock' : (init?.action ?? 'mock')));
  // Vrai uniquement pour LA regle qui vient d'etre ainsi repassee de proxy a
  // mock a l'ouverture (valeur stockee "proxy", affichage force "mock") —
  // stable pour toute la duree de vie du formulaire (ne depend jamais de
  // `ruleAction`, qui ne peut de toute facon plus reprendre "proxy" tant que
  // l'option est masquee). Sert uniquement a declencher l'avertissement de
  // sauvegarde ci-dessous ; ne doit jamais influencer `ruleAction` lui-meme.
  const isStaleProxyRule = untrack(() => isPurelyMocked && init?.action === 'proxy');

  const httpMethods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS', 'HEAD'];
  let allOf = $state(init?.conditions?.all_of ?? []);
  let anyOf = $state(init?.conditions?.any_of ?? []);

  // Assistance de saisie path/query param : liste fermee des
  // path params reellement presents (service + regle en cours d'edition) et
  // suggestions de query params vus dans le trafic reel du service — les deux
  // derivent d'un seul chargement des logs (pas de nouvel appel reseau par
  // champ).
  let availablePathParams = $derived(combinePathParamNames([listenPath, subPath]));

  let serviceLogs = $state([]);
  async function loadServiceLogs() {
    if (!serviceName) return;
    try {
      const logs = await getLogs(200);
      serviceLogs = logs.filter((l) => l.service_name === serviceName);
    } catch {
      serviceLogs = [];
    }
  }
  $effect(() => { loadServiceLogs(); });

  let queryParamSuggestions = $derived(
    [...new Set(serviceLogs.flatMap((l) => Object.keys(l.captured?.query_params ?? {})))].sort()
  );

  let formError = $state('');

  // Reference vers le fieldset "Reponse mockee" (mode/en-tetes/corps +
  // scripts + chaos) : reste montee en permanence quel que soit `ruleAction`
  // (cf RuleResponseSection.svelte pour la raison — preserver l'etat au
  // travers d'une bascule mock<->proxy avant sauvegarde). getPayload() est
  // donc toujours appelable ; validate() n'est appelee que si ruleAction
  // vaut 'mock' (seule partie de la logique deja conditionnee a l'origine).
  let responseSectionRef = $state(null);

  // Detecteur de conflit (a la sauvegarde uniquement) : quand
  // le brouillon chevauche une autre regle du service, un avertissement
  // NON BLOQUANT s'affiche avec deux issues possibles — "Enregistrer quand
  // meme" (sauvegarde immediatement) ou "Modifier la regle" (referme
  // l'avertissement, l'utilisateur reste sur le formulaire).
  let pendingConflicts = $state([]);
  let pendingRulePayload = $state(null);
  let checkingConflicts = $state(false);

  // Avertissement de changement d'action reelle (complement sujet 22, cf
  // CLAUDE.md §3) : ne concerne QUE `isStaleProxyRule` (valeur stockee
  // "proxy", forcee a "mock" a l'ouverture parce que le service est
  // purement mocke) — jamais affiche pour une regle mock ordinaire ni pour
  // une regle proxy sur un service qui a une cible. Meme pattern non
  // bloquant que le detecteur de conflit ci-dessus et que l'avertissement
  // de bascule de `ServiceForm.svelte` : informer, laisser "Enregistrer
  // quand meme" proceder, "Modifier la regle" referme sans rien sauvegarder.
  let pendingStaleProxyWarning = $state(false);
  let pendingStaleProxyPayload = $state(null);

  function buildRulePayload() {
    // Toujours calcule, quel que soit `ruleAction` (meme comportement que
    // l'origine, ou response/scripts vivaient dans un scope jamais demonte
    // — cf RuleResponseSection.svelte).
    const { response, pre_script, script, post_script } = responseSectionRef.getPayload();
    return {
      name: name.trim(),
      method: ruleMethod,
      sub_path: subPath.trim() || null,
      action: ruleAction,
      pre_script,
      script,
      post_script,
      conditions: { all_of: allOf, any_of: anyOf },
      response,
    };
  }

  async function handleSubmit(e) {
    e.preventDefault();
    formError = '';
    pendingConflicts = [];
    pendingRulePayload = null;
    pendingStaleProxyWarning = false;
    pendingStaleProxyPayload = null;

    const trimmedName = name.trim();
    if (!trimmedName) { formError = 'Le nom de la regle est requis.'; return; }
    if (existingRuleNames.some(n => n.toLowerCase() === trimmedName.toLowerCase())) {
      formError = `Une regle avec le nom "${trimmedName}" existe deja dans ce service.`;
      return;
    }

    if (ruleAction === 'mock') {
      const validationErr = responseSectionRef.validate();
      if (validationErr) { formError = validationErr; return; }
    }

    const builtRule = buildRulePayload();

    // Verification purement locale (pas d'appel reseau) : passe AVANT le
    // detecteur de conflit, pour ne pas interroger le backend tant que
    // l'utilisateur n'a pas confirme vouloir persister ce changement
    // d'action reel (proxy -> mock).
    if (isStaleProxyRule) {
      pendingStaleProxyWarning = true;
      pendingStaleProxyPayload = builtRule;
      return;
    }

    await checkConflictsAndSave(builtRule);
  }

  async function checkConflictsAndSave(builtRule) {
    checkingConflicts = true;
    try {
      const result = await checkRuleConflicts({
        draft: {
          method: builtRule.method,
          sub_path: builtRule.sub_path,
          conditions: builtRule.conditions,
        },
        other_rules: existingRules.map((r) => ({
          name: r.name,
          method: r.method,
          sub_path: r.sub_path,
          conditions: r.conditions,
        })),
        draft_position: effectiveDraftPosition,
      });
      const conflicts = result.conflicts ?? [];
      if (conflicts.length === 0) {
        onSave(builtRule);
      } else {
        pendingConflicts = conflicts;
        pendingRulePayload = builtRule;
      }
    } catch {
      // Fail-open : une detection de conflit indisponible ne doit jamais
      // empecher la sauvegarde reelle de la regle (fonctionnalite purement
      // informative).
      onSave(builtRule);
    } finally {
      checkingConflicts = false;
    }
  }

  function confirmSaveDespiteStaleProxyWarning() {
    const payload = pendingStaleProxyPayload;
    pendingStaleProxyWarning = false;
    pendingStaleProxyPayload = null;
    if (payload) checkConflictsAndSave(payload);
  }

  function cancelStaleProxyWarning() {
    pendingStaleProxyWarning = false;
    pendingStaleProxyPayload = null;
  }

  function confirmSaveDespiteConflicts() {
    if (pendingRulePayload) onSave(pendingRulePayload);
    pendingConflicts = [];
    pendingRulePayload = null;
  }

  function dismissConflictWarning() {
    pendingConflicts = [];
    pendingRulePayload = null;
  }
</script>

<form class="rule-form" onsubmit={handleSubmit} aria-label={init ? `Modifier la regle ${init.name}` : 'Nouvelle regle'}>

  {#if formError}
    <div class="form-error" role="alert" aria-live="assertive">{formError}</div>
  {/if}

  <div class="form-field">
    <label for="rule-name">Nom de la regle</label>
    <input id="rule-name" type="text" bind:value={name} required placeholder="ex: get-siret" aria-describedby="rn-hint" data-testid="rule-form-name-input" />
    <span class="field-hint" id="rn-hint">Identifiant unique de cette regle dans le service</span>
  </div>

  <div class="form-row">
    <div class="form-field">
      <label for="rule-method">Methode HTTP</label>
      <select id="rule-method" bind:value={ruleMethod} aria-describedby="rule-method-hint" data-testid="rule-form-method-select">
        {#each httpMethods as m}
          <option value={m}>{m}</option>
        {/each}
      </select>
      <span class="field-hint" id="rule-method-hint">Methode HTTP que cette regle intercepte</span>
    </div>

    <div class="form-field">
      <label for="rule-subpath">Sous-chemin (optionnel)</label>
      <input id="rule-subpath" type="text" bind:value={subPath} placeholder="ex: /users/{'{id}'}" aria-describedby="rule-subpath-hint" data-testid="rule-form-subpath-input" />
      <span class="field-hint" id="rule-subpath-hint">Affine le matching au sein du service</span>
    </div>
  </div>

  <RuleActionSelector action={ruleAction} {isPurelyMocked} onChange={(v) => ruleAction = v} />

  {#if serviceName}
    <RuleTester
      {serviceName}
      {groupName}
      logs={serviceLogs}
      getDraftRule={() => ({ method: ruleMethod, subPath, allOf, anyOf })}
    />
  {/if}

  <RuleConditionsEditor
    {allOf}
    {anyOf}
    {availablePathParams}
    {queryParamSuggestions}
    onAllOfChange={(v) => allOf = v}
    onAnyOfChange={(v) => anyOf = v}
  />

  <RuleResponseSection bind:this={responseSectionRef} visible={ruleAction === 'mock'} initRule={init} />

  <RuleWarnings
    {pendingStaleProxyWarning}
    onConfirmStaleProxy={confirmSaveDespiteStaleProxyWarning}
    onCancelStaleProxy={cancelStaleProxyWarning}
    {pendingConflicts}
    onConfirmConflicts={confirmSaveDespiteConflicts}
    onDismissConflicts={dismissConflictWarning}
  />

  <!-- ACTIONS -->
  <div class="form-actions">
    <button type="submit" class="btn btn-primary" disabled={checkingConflicts} data-testid="rule-form-submit-button">
      {#if checkingConflicts}Vérification…{:else}{init ? 'Enregistrer la regle' : 'Ajouter la regle'}{/if}
    </button>
    <button type="button" class="btn btn-secondary" onclick={onCancel} data-testid="rule-form-cancel-button">Annuler</button>
  </div>
</form>

<style>
  .rule-form { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius); padding: 1.25rem; }
</style>
