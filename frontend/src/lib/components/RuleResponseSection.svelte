<script>
  // Le fieldset complet "Reponse mockee" d'une regle : selecteur de mode
  // (JSON par exemple/guide, XML guide, texte, template avance, vide),
  // en-tetes, corps de reponse, PLUS les 3 blocs de script (pre_script/
  // script/post_script) et le mode Chaos — regroupes ici car ils partagent
  // le meme fieldset repliable dans le rendu original (voir CLAUDE.md,
  // "Scripts rhai multi-blocs" : ces champs sont geographiquement dans la
  // section Reponse bien que ce soient des champs de Rule independants de
  // la reponse elle-meme).
  //
  // Reste MONTE en permanence par RuleForm.svelte (jamais derriere un
  // {#if}) : c'est `visible` (derive de `ruleAction === 'mock'` cote
  // parent) qui gate uniquement l'AFFICHAGE ci-dessous. Ce choix est
  // deliberement different d'un simple {#if} au niveau du parent : il
  // preserve le comportement d'origine ou tout cet etat (fragments,
  // en-tetes, scripts, chaos) vivait dans le scope jamais demonte de
  // RuleForm.svelte et survivait donc a un aller-retour mock -> proxy ->
  // mock sur le selecteur d'action avant sauvegarde. Si ce composant etait
  // demonte/remonte via un {#if} cote parent, cet etat serait perdu a
  // chaque bascule — regression silencieuse a ne jamais introduire.
  //
  // Expose getPayload()/validate() via bind:this (meme pattern que
  // JsonResponseBuilder.svelte/XmlResponseBuilder.svelte/toTemplate()).
  // getPayload() est appelee INCONDITIONNELLEMENT par RuleForm (meme
  // raison que ci-dessus : l'origine calculait deja response/scripts sans
  // egard a ruleAction) ; validate() n'est appelee par RuleForm que si
  // ruleAction === 'mock' (seule partie de la logique originale qui etait
  // deja conditionnee sur l'action).
  import { untrack } from 'svelte';
  import JsonResponseBuilder from './JsonResponseBuilder.svelte';
  import JsonPasteBuilder from './JsonPasteBuilder.svelte';
  import XmlResponseBuilder from './XmlResponseBuilder.svelte';
  import XmlPasteBuilder from './XmlPasteBuilder.svelte';
  import RuleScriptSlot from './RuleScriptSlot.svelte';
  import ToggleSwitch from './ToggleSwitch.svelte';
  import { templateToFields, templateToXmlFields, validateTemplateAsJson, validateTemplateAsXml } from '../tpl-utils.js';
  import { validateScript as apiValidateScript } from '../api.js';
  import { RHAI_FUNCTIONS } from '../rhai-functions.js';

  let { visible = true, initRule = null } = $props();

  const init = untrack(() => initRule);

  let status = $state(init?.response?.status ?? 200);
  let respHeaders = $state(init?.response?.headers ?? []);
  let fragments = $state(init?.response?.body ?? [{ type: 'Literal', value: '' }]);
  let chaosEnabled = $state(!!init?.response?.chaos);
  let chaos = $state(init?.response?.chaos ?? { delay_ms: 0, delay_min_ms: null, delay_max_ms: null, error_rate: 0, error_status: 500 });

  let responseOpen = $state(true);

  // Options avancees (pre_script/post_script) repliees par defaut (retour
  // beta-testeur : ces deux blocs, peu utilises, ajoutaient de la charge
  // visuelle permanente pour une fonctionnalite que la plupart des
  // utilisateurs n'exploitent pas). Le script principal (`script`) reste
  // toujours visible, jamais concerne par ce repli. Ouverture automatique
  // si une regle EXISTANTE a deja du contenu dans l'un des deux (ne jamais
  // cacher une configuration deja faite par l'utilisateur sans qu'il la
  // voie) — calcule une seule fois a l'ouverture, comme `responseMode`
  // ci-dessus. Le contenu de pre_script/post_script (preScriptCode/
  // postScriptCode plus bas) vit dans CE composant, jamais dans
  // RuleScriptSlot lui-meme : replier/deplier ne fait que masquer
  // l'affichage (attribut `hidden`, jamais un `{#if}` qui demonterait
  // RuleScriptSlot) et ne perd donc jamais de donnees deja saisies.
  let advancedOpen = $state(!!init?.pre_script?.trim() || !!init?.post_script?.trim());

  // Restauration de la vue d'origine a l'edition (retour 1, cf CLAUDE.md
  // "Restauration de la vue d'origine..."). Avant cette passe, une regle
  // deja construite via n'importe lequel des 4 modes structures (json-paste/
  // json-guided/xml-paste/xml-guided) retombait TOUJOURS sur 'advanced' des
  // que response.body etait un unique fragment Template — exactement la
  // forme que ces 4 modes produisent tous, indiscernables entre eux ET du
  // "vrai" mode avance a la seule lecture du corps. `Rule.response_mode`
  // (backend, src/models/mod.rs, EXCEPTION documentee au point 16 -- champ
  // purement UI) leve cette ambiguite en memorisant explicitement quelle vue
  // a produit ce template. `computeInitialEditorState()` degrade
  // gracieusement vers l'ancienne heuristique par forme si `response_mode`
  // est absent (regle sauvegardee avant cette passe) et retombe sur
  // 'advanced' des qu'une restauration echoue (JSON invalide, XML invalide,
  // ou racine tableau JSON -- limite assumee, cf tpl-utils.js::
  // templateToFields, seul le mode "par exemple" JSON peut produire cette
  // forme) plutot que de planter l'ouverture du formulaire.
  function computeInitialEditorState() {
    const body = init?.response?.body ?? [];
    const singleTemplate = body.length === 1 && body[0].type === 'Template' ? body[0].template : null;
    const singleLiteral = body.length === 1 && body[0].type === 'Literal' ? body[0].value : null;

    function fallbackMode() {
      if (!body.length) return 'json-paste';
      if (singleTemplate !== null) return 'advanced';
      if (singleLiteral !== null) return 'text';
      if (init?.response?.status === 204) return 'empty';
      return 'advanced';
    }

    let mode = init?.response_mode ?? fallbackMode();

    const structuredModes = ['json-paste', 'json-guided', 'xml-paste', 'xml-guided'];
    const structured = {};

    if (structuredModes.includes(mode)) {
      if (body.length === 0) {
        // Corps vide (regle neuve, ou regle existante sans reponse encore
        // configuree) : rien a restaurer, la vue demarre a vide (paste zone
        // ou detail vide selon le mode) -- ce n'est PAS une incoherence a
        // degrader, sans quoi TOUTE regle neuve retomberait a tort sur
        // 'advanced' (bug rencontre et corrige pendant ce sujet : une regle
        // neuve doit demarrer sur le format JSON assiste par defaut, comme
        // avant cette passe).
      } else if (singleTemplate === null) {
        // Un corps existe mais ne correspond pas a la forme structuree
        // attendue (config editee hors UI) -- degrade plutot que de planter.
        mode = 'advanced';
      } else if (mode === 'json-paste' || mode === 'json-guided') {
        if (singleTemplate.trim().startsWith('[')) {
          mode = 'advanced';
        } else {
          try { structured.fields = templateToFields(singleTemplate); }
          catch { mode = 'advanced'; }
        }
      } else if (mode === 'xml-paste' || mode === 'xml-guided') {
        try {
          const r = templateToXmlFields(singleTemplate);
          structured.fields = r.fields;
          structured.rootTag = r.rootTag;
          structured.rootAttributes = r.rootAttributes;
        } catch { mode = 'advanced'; }
      }
    } else if (mode === 'text') {
      structured.textContent = singleLiteral ?? '';
    }

    return { mode, structured };
  }

  const { mode: initialMode, structured: initialStructured } = computeInitialEditorState();

  let responseMode = $state(initialMode);

  // jsonPasteFields/xmlPasteFields determinent directement `startParsed` du
  // builder assiste correspondant, passe comme `startParsed={xxxPasteFields
  // .length > 0}` au point d'usage (plus bas) -- jamais deduit du seul mode
  // courant (une regle NEUVE demarre aussi en 'json-paste'/'xml-paste' mais
  // n'a RIEN a restaurer : la zone de collage doit s'afficher, pas une
  // liste de champs vide, cf CLAUDE.md point 78). Cette longueur reste
  // correcte y compris apres un aller-retour "Modifier en detail" <->
  // "Revenir a la vue par exemple" (revealDetailMode/backToPasteMode plus
  // bas), puisque Svelte demonte/remonte le builder assiste a chaque fois
  // qu'on rentre dans sa branche {:else if} et relit `startParsed` a cet
  // instant precis.
  let jsonFields = $state(initialMode === 'json-guided' ? (initialStructured.fields ?? []) : []);
  let jsonBuilderRef = $state(null);
  let jsonPasteFields = $state(initialMode === 'json-paste' ? (initialStructured.fields ?? []) : []);
  let jsonPasteRef = $state(null);
  let xmlFields = $state(initialMode === 'xml-guided' ? (initialStructured.fields ?? []) : []);
  let xmlBuilderRef = $state(null);
  let xmlPasteFields = $state(initialMode === 'xml-paste' ? (initialStructured.fields ?? []) : []);
  let xmlPasteRef = $state(null);
  let xmlRootTag = $state(initialStructured.rootTag ?? 'response');
  let xmlRootAttributes = $state(initialStructured.rootAttributes ?? []);
  let textContent = $state(initialStructured.textContent ?? '');

  // pre_script/script/post_script : blocs additionnels independants (meme
  // ScriptContext, pas de chainage entre eux — voir Rule dans
  // src/models/mod.rs). Meme endpoint de validation (content-agnostic),
  // reutilise pour les 3 slots.
  let scriptEnabled = $state(!!init?.script);
  let scriptCode = $state(init?.script ?? '');
  let scriptValidation = $state({ status: '', message: '' });

  let preScriptEnabled = $state(!!init?.pre_script);
  let preScriptCode = $state(init?.pre_script ?? '');
  let preScriptValidation = $state({ status: '', message: '' });

  let postScriptEnabled = $state(!!init?.post_script);
  let postScriptCode = $state(init?.post_script ?? '');
  let postScriptValidation = $state({ status: '', message: '' });

  async function validateScriptCode(code) {
    if (!code.trim()) {
      return { status: 'error', message: 'Le script est vide.' };
    }
    try {
      const result = await apiValidateScript(code);
      return result.valid
        ? { status: 'ok', message: 'Script valide.' }
        : { status: 'error', message: result.error };
    } catch (e) {
      return { status: 'error', message: e.message };
    }
  }

  async function handleValidateScript() {
    scriptValidation = { status: 'pending', message: 'Validation...' };
    scriptValidation = await validateScriptCode(scriptCode);
  }

  async function handleValidatePreScript() {
    preScriptValidation = { status: 'pending', message: 'Validation...' };
    preScriptValidation = await validateScriptCode(preScriptCode);
  }

  async function handleValidatePostScript() {
    postScriptValidation = { status: 'pending', message: 'Validation...' };
    postScriptValidation = await validateScriptCode(postScriptCode);
  }

  function buildFragmentsFromMode() {
    if (responseMode === 'empty') return [];
    if (responseMode === 'json-guided' && jsonBuilderRef) {
      return [{ type: 'Template', template: jsonBuilderRef.toTemplate() }];
    }
    if (responseMode === 'json-paste' && jsonPasteRef) {
      return [{ type: 'Template', template: jsonPasteRef.toTemplate() }];
    }
    if (responseMode === 'xml-guided' && xmlBuilderRef) {
      return [{ type: 'Template', template: xmlBuilderRef.toTemplate() }];
    }
    if (responseMode === 'xml-paste' && xmlPasteRef) {
      return [{ type: 'Template', template: xmlPasteRef.toTemplate() }];
    }
    if (responseMode === 'text') {
      return [{ type: 'Literal', value: textContent }];
    }
    return fragments;
  }

  const fragmentTypes = [
    { value: 'Template', label: 'Template (expressions)' },
    { value: 'Literal', label: 'Texte fixe' },
    { value: 'Uuid', label: 'UUID v4' },
    { value: 'PickFrom', label: 'Choix aleatoire' },
    { value: 'FakeData', label: 'Donnee fictive' },
    { value: 'PathSegment', label: 'Segment URL (index)' },
  ];

  const fakeKinds = [
    { value: 'FirstName', label: 'Prenom' },
    { value: 'LastName', label: 'Nom de famille' },
    { value: 'Email', label: 'Adresse email' },
    { value: 'PhoneNumberFR', label: 'Telephone FR' },
    { value: 'Integer', label: 'Nombre entier' },
    { value: 'CompanyName', label: 'Nom d\'entreprise' },
    { value: 'StreetName', label: 'Nom de rue' },
    { value: 'CityFR', label: 'Ville (FR)' },
    { value: 'PostcodeFR', label: 'Code postal (FR)' },
    { value: 'Siren', label: 'SIREN (9 chiffres)' },
    { value: 'Siret', label: 'SIRET (14 chiffres)' },
    { value: 'FullAddressFR', label: 'Adresse complete (FR)' },
    { value: 'DatePast', label: 'Date passee' },
    { value: 'DateFuture', label: 'Date future' },
    { value: 'TimestampMs', label: 'Timestamp (ms)' },
    { value: 'BoolRandom', label: 'Booleen aleatoire' },
    { value: 'LoremSentence', label: 'Phrase Lorem Ipsum' },
    { value: 'CountryFR', label: 'Pays francophone' },
    { value: 'IbanFR', label: 'IBAN francais' },
  ];

  let pendingMode = $state(null);
  let modeKey = $state(0);
  let pendingConvMessage = $state('');

  function requestModeSwitch(newMode) {
    if (newMode === responseMode) return;
    const hasContent = currentModeHasContent();
    if (!hasContent) {
      applyModeSwitch(newMode);
      return;
    }
    const convResult = tryConvert(responseMode, newMode);
    if (convResult.ok) {
      applyModeSwitch(newMode, convResult);
      return;
    }
    pendingMode = newMode;
    pendingConvMessage = convResult.reason || '';
    modeKey++;
  }

  function confirmModeSwitch() {
    if (pendingMode) {
      applyModeSwitch(pendingMode);
      pendingMode = null;
      pendingConvMessage = '';
    }
  }

  function cancelModeSwitch() {
    pendingMode = null;
    pendingConvMessage = '';
    modeKey++;
  }

  function applyModeSwitch(newMode, convResult) {
    if (convResult?.jsonFields) jsonFields = convResult.jsonFields;
    if (convResult?.jsonPasteFields) jsonPasteFields = convResult.jsonPasteFields;
    if (convResult?.xmlFields) xmlFields = convResult.xmlFields;
    if (convResult?.xmlPasteFields) xmlPasteFields = convResult.xmlPasteFields;
    if (convResult?.xmlRootTag !== undefined) xmlRootTag = convResult.xmlRootTag;
    if (convResult?.xmlRootAttributes !== undefined) xmlRootAttributes = convResult.xmlRootAttributes;
    if (convResult?.textContent !== undefined) textContent = convResult.textContent;
    if (convResult?.fragments) fragments = convResult.fragments;
    responseMode = newMode;
    modeKey++;
  }

  // "Modifier en detail" (retour 3, fusion Format x Assiste/Detail, cf
  // CLAUDE.md) : bascule depuis le sous-mode assiste (json-paste/xml-paste)
  // vers le sous-mode detail (json-guided/xml-guided) EN CONSERVANT le meme
  // tableau `fields` (structure identique entre les deux, verifie en etape 0
  // de ce sujet) -- DELIBEREMENT hors du systeme d'avertissement de
  // requestModeSwitch/tryConvert : c'est une REVELATION de capacites
  // supplementaires sur les MEMES donnees, jamais une conversion avec risque
  // de perte, donc zero avertissement.
  function revealDetailMode() {
    if (responseMode === 'json-paste') {
      jsonFields = jsonPasteFields;
      responseMode = 'json-guided';
    } else if (responseMode === 'xml-paste') {
      xmlFields = xmlPasteFields;
      responseMode = 'xml-guided';
    }
  }

  // Chemin retour, symetrique de revealDetailMode() ci-dessus (correctif
  // "diagnostic reponse JSON/XML" : la fusion Format x Assiste/Detail
  // n'offrait jusqu'ici qu'un aller simple vers le detail, aucun moyen de
  // revenir a la vue "par exemple" sans perdre le travail en cours -- cf
  // CLAUDE.md). Meme principe : structure de Fields identique entre les
  // deux sous-modes, donc copie directe SANS avertissement de perte (memes
  // raisons que revealDetailMode : ceci est une REDUCTION de capacites
  // visibles sur les MEMES donnees, pas une conversion avec perte reelle).
  // `jsonPasteFields`/`xmlPasteFields` sont lus reactivement (pas geles a
  // l'ouverture) par les vues assistees ci-dessous via
  // `startParsed={jsonPasteFields.length > 0}` : au remontage du composant
  // assiste (Svelte demonte/remonte en changeant de branche {:else if}),
  // la liste de champs deja peuplee s'affiche directement plutot que la
  // zone de collage vide.
  function backToPasteMode() {
    if (responseMode === 'json-guided') {
      jsonPasteFields = jsonFields;
      responseMode = 'json-paste';
    } else if (responseMode === 'xml-guided') {
      xmlPasteFields = xmlFields;
      responseMode = 'xml-paste';
    }
  }

  // Regroupement des 4 modes structures en 2 "Format" (JSON/XML), cf CLAUDE.md
  // "Fusion Format x Assiste/Detail" -- Texte/Avance/Vide restent 1 bouton
  // = 1 mode (pas de distinction assiste/detail pour eux, rien a fusionner).
  function formatOfMode(mode) {
    if (mode === 'json-paste' || mode === 'json-guided') return 'json';
    if (mode === 'xml-paste' || mode === 'xml-guided') return 'xml';
    return mode;
  }

  function selectFormat(fmt) {
    // Deja ce format (assiste OU detail) : pas de reinitialisation, evite de
    // faire perdre une structure detail deja construite par un simple
    // re-clic accidentel sur le meme bouton de format.
    if (formatOfMode(responseMode) === fmt) return;
    const target = fmt === 'json' ? 'json-paste' : fmt === 'xml' ? 'xml-paste' : fmt;
    requestModeSwitch(target);
  }

  function currentModeHasContent() {
    if (responseMode === 'json-paste') return jsonPasteFields.length > 0;
    if (responseMode === 'json-guided') return jsonFields.length > 0;
    if (responseMode === 'xml-guided') return xmlFields.length > 0;
    if (responseMode === 'xml-paste') return xmlPasteFields.length > 0;
    if (responseMode === 'text') return textContent.trim().length > 0;
    if (responseMode === 'advanced') return fragments.some(f => {
      if (f.type === 'Template') return f.template?.trim();
      if (f.type === 'Literal') return f.value?.trim();
      return true;
    });
    return false;
  }

  function getAdvancedTemplate() {
    return fragments.map(f => {
      if (f.type === 'Literal') return f.value ?? '';
      if (f.type === 'Template') return f.template ?? '';
      return '';
    }).join('');
  }

  function tryConvert(from, to) {
    if (from === 'advanced' && to === 'text') {
      return { ok: true, textContent: getAdvancedTemplate() };
    }
    if (from === 'text' && to === 'advanced') {
      return { ok: true, fragments: [{ type: 'Literal', value: textContent }] };
    }
    if (from === 'advanced' && to === 'json-guided') {
      return tryAdvancedToJsonGuided();
    }
    if (from === 'advanced' && to === 'json-paste') {
      // Format=JSON cible desormais 'json-paste' en entree par defaut (cf
      // selectFormat) -- reutilise la MEME conversion sans perte que vers
      // 'json-guided' pour ne pas regresser ce cas deja fluide avant la
      // fusion (retour 3), juste range dans jsonPasteFields au lieu de
      // jsonFields.
      const r = tryAdvancedToJsonGuided();
      return r.ok ? { ok: true, jsonPasteFields: r.jsonFields ?? [] } : r;
    }
    if (from === 'advanced' && to === 'xml-guided') {
      return tryAdvancedToXmlGuided();
    }
    if (from === 'advanced' && to === 'xml-paste') {
      const r = tryAdvancedToXmlGuided();
      return r.ok
        ? { ok: true, xmlPasteFields: r.xmlFields ?? [], xmlRootTag: r.xmlRootTag, xmlRootAttributes: r.xmlRootAttributes }
        : r;
    }
    if (from === 'json-guided' && to === 'advanced') {
      if (jsonBuilderRef) {
        return { ok: true, fragments: [{ type: 'Template', template: jsonBuilderRef.toTemplate() }] };
      }
      return { ok: true };
    }
    if (from === 'xml-guided' && to === 'advanced') {
      if (xmlBuilderRef) {
        return { ok: true, fragments: [{ type: 'Template', template: xmlBuilderRef.toTemplate() }] };
      }
      return { ok: true };
    }
    if (from === 'json-guided' && to === 'xml-guided') {
      return tryJsonFieldsToXmlFields(jsonFields);
    }
    if (from === 'json-guided' && to === 'xml-paste') {
      const r = tryJsonFieldsToXmlFields(jsonFields);
      return r.ok ? { ok: true, xmlPasteFields: r.xmlFields ?? [] } : r;
    }
    if (from === 'json-paste' && to === 'xml-paste') {
      // Meme conversion, sourcee sur jsonPasteFields (structure identique a
      // jsonFields, cf etape 0 de ce sujet) -- capacite nouvelle, aucune
      // regression a preserver ici (json-paste n'existait pas comme cible/
      // source de conversion avant cette passe).
      const r = tryJsonFieldsToXmlFields(jsonPasteFields);
      return r.ok ? { ok: true, xmlPasteFields: r.xmlFields ?? [] } : r;
    }
    if (from === 'xml-guided' && to === 'json-guided') {
      return { ok: false, reason: 'La conversion XML vers JSON guide n\'est pas supportee. Passez par le mode template avance comme intermediaire.' };
    }
    if ((from === 'xml-guided' || from === 'xml-paste') && to === 'json-paste') {
      return { ok: false, reason: 'La conversion XML vers JSON n\'est pas supportee. Passez par le mode template avance comme intermediaire.' };
    }
    return { ok: false };
  }

  function tryAdvancedToJsonGuided() {
    const tpl = getAdvancedTemplate();
    if (!tpl.trim()) return { ok: true, jsonFields: [] };
    const jsonErr = validateTemplateAsJson(tpl);
    if (jsonErr) {
      return { ok: false, reason: `Conversion impossible : ${jsonErr}. Verifiez les accolades ({{ pour JSON literal, { pour variable).` };
    }
    try {
      const fields = templateToFields(tpl);
      return { ok: true, jsonFields: fields };
    } catch (e) {
      return { ok: false, reason: `Conversion impossible : ${e.message}` };
    }
  }

  // Corrige (sujet "diagnostic reponse JSON/XML") : cette fonction etait un
  // stub qui echouait TOUJOURS, meme pour un template XML parfaitement
  // valide -- `xmlErr` n'etait teste que pour produire un message d'erreur
  // different, jamais pour autoriser la conversion. Consequence concrete :
  // une regle dont le contenu XML avait ete tape/colle en mode "Template
  // avance" ne pouvait JAMAIS rejoindre la vue XML (ni assistee ni detail)
  // sans perdre son contenu, alors que coller le MEME texte directement
  // dans la zone de collage du mode "par exemple" fonctionnait sans
  // probleme (ce dernier ne passe jamais par cette fonction). Desormais
  // alignee sur `tryAdvancedToJsonGuided` ci-dessus : reutilise
  // `templateToXmlFields` (deja utilisee pour restaurer la vue d'origine a
  // la reouverture d'une regle, cf CLAUDE.md "Restauration de la vue
  // d'origine...") pour reanalyser le template `{{expr | pipe}}` en Fields
  // structures, y compris le tag racine et les attributs de racine.
  function tryAdvancedToXmlGuided() {
    const tpl = getAdvancedTemplate();
    if (!tpl.trim()) return { ok: true, xmlFields: [] };
    const xmlErr = validateTemplateAsXml(tpl);
    if (xmlErr) {
      return { ok: false, reason: `Conversion impossible : ${xmlErr}` };
    }
    try {
      const parsed = templateToXmlFields(tpl);
      return { ok: true, xmlFields: parsed.fields, xmlRootTag: parsed.rootTag, xmlRootAttributes: parsed.rootAttributes };
    } catch (e) {
      return { ok: false, reason: `Conversion impossible : ${e.message}` };
    }
  }

  // Generalisee (retour 3) pour accepter n'importe quel tableau de Fields
  // JSON en source -- jsonFields (mode detail) OU jsonPasteFields (mode
  // assiste), structurellement identiques (cf etape 0 de ce sujet) -- au
  // lieu de ne lire que jsonFields comme avant cette passe.
  function tryJsonFieldsToXmlFields(sourceFields) {
    if (!sourceFields.length) return { ok: true, xmlFields: [] };
    try {
      const xmlF = sourceFields.filter(f => f.key?.trim()).map(f => jsonFieldToXmlNode(f));
      return { ok: true, xmlFields: xmlF };
    } catch {
      return { ok: false, reason: 'La structure JSON contient des elements incompatibles avec XML (tableaux de valeurs scalaires).' };
    }
  }

  function jsonFieldToXmlNode(f) {
    const ft = f.fieldType || 'value';
    if (ft === 'object') {
      return { tag: f.key, nodeType: 'parent', children: (f.children || []).filter(c => c.key?.trim()).map(c => jsonFieldToXmlNode(c)) };
    }
    if (ft === 'array-objects') {
      return { tag: f.key, nodeType: 'parent', children: (f.template || []).filter(c => c.key?.trim()).map(c => jsonFieldToXmlNode(c)) };
    }
    if (ft === 'array-values') {
      throw new Error('incompatible');
    }
    return { tag: f.key, nodeType: 'value', source: f.source || 'fixed', value: f.value || '' };
  }

  function addFragment() { fragments = [...fragments, { type: 'Literal', value: '' }]; }
  function removeFragment(idx) { fragments = fragments.filter((_, i) => i !== idx); }
  function moveFragment(idx, dir) {
    const t = idx + dir;
    if (t < 0 || t >= fragments.length) return;
    const c = [...fragments]; [c[idx], c[t]] = [c[t], c[idx]]; fragments = c;
  }
  function updateFragmentType(idx, newType) {
    const c = [...fragments];
    if (newType === 'Literal') c[idx] = { type: 'Literal', value: '' };
    else if (newType === 'Uuid') c[idx] = { type: 'Uuid' };
    else if (newType === 'PickFrom') c[idx] = { type: 'PickFrom', values: [''] };
    else if (newType === 'FakeData') c[idx] = { type: 'FakeData', kind: { type: 'FirstName' } };
    else if (newType === 'PathSegment') c[idx] = { type: 'PathSegment', index: 0 };
    else if (newType === 'Template') c[idx] = { type: 'Template', template: '' };
    fragments = c;
  }
  function updateFakeKind(idx, kindType) {
    const c = [...fragments];
    c[idx] = kindType === 'Integer'
      ? { type: 'FakeData', kind: { type: 'Integer', min: 0, max: 100 } }
      : { type: 'FakeData', kind: { type: kindType } };
    fragments = c;
  }
  function addPickValue(idx) {
    const c = [...fragments]; c[idx] = { ...c[idx], values: [...c[idx].values, ''] }; fragments = c;
  }
  function removePickValue(fi, vi) {
    const c = [...fragments]; c[fi] = { ...c[fi], values: c[fi].values.filter((_, i) => i !== vi) }; fragments = c;
  }

  const commonHeaders = [
    'Content-Type', 'Accept', 'Authorization', 'Cache-Control',
    'X-Request-Id', 'X-Correlation-Id', 'X-Forwarded-For',
    'Access-Control-Allow-Origin', 'Access-Control-Allow-Methods',
  ];

  const commonContentTypes = [
    'application/json', 'application/xml', 'text/plain', 'text/html',
    'application/x-www-form-urlencoded', 'multipart/form-data',
    'application/octet-stream', 'application/pdf',
  ];

  function addHeader() { respHeaders = [...respHeaders, { name: '', value: '' }]; }
  function removeHeader(idx) { respHeaders = respHeaders.filter((_, i) => i !== idx); }

  export function validate() {
    if (responseMode === 'json-paste' && jsonPasteRef) {
      const err = validateTemplateAsJson(jsonPasteRef.toTemplate());
      if (err) return `JSON par exemple invalide : ${err}`;
    }
    if (responseMode === 'json-guided' && jsonBuilderRef) {
      const err = validateTemplateAsJson(jsonBuilderRef.toTemplate());
      if (err) return `JSON guide invalide : ${err}`;
    }
    if (responseMode === 'xml-guided' && xmlBuilderRef) {
      const err = validateTemplateAsXml(xmlBuilderRef.toTemplate());
      if (err) return err;
    }
    if (responseMode === 'xml-paste' && xmlPasteRef) {
      const err = validateTemplateAsXml(xmlPasteRef.toTemplate());
      if (err) return err;
    }
    if (responseMode === 'advanced') {
      const tpl = getAdvancedTemplate();
      const ct = respHeaders.find(h => h.name?.toLowerCase() === 'content-type')?.value?.toLowerCase() || '';
      if (ct.includes('json') && tpl.trim()) {
        const err = validateTemplateAsJson(tpl);
        if (err) return `Content-Type JSON mais template invalide : ${err}`;
      }
      if (ct.includes('xml') && tpl.trim()) {
        const xmlErr = validateTemplateAsXml(tpl);
        if (xmlErr) return `Content-Type XML mais template invalide : ${xmlErr}`;
      }
    }
    return null;
  }

  export function getPayload() {
    const finalStatus = responseMode === 'empty' ? 204 : status;
    const finalHeaders = responseMode === 'empty' ? [] : respHeaders.filter(h => h.name.trim());
    if ((responseMode === 'json-guided' || responseMode === 'json-paste') && !finalHeaders.some(h => h.name.toLowerCase() === 'content-type')) {
      finalHeaders.push({ name: 'Content-Type', value: 'application/json' });
    }
    if ((responseMode === 'xml-guided' || responseMode === 'xml-paste') && !finalHeaders.some(h => h.name.toLowerCase() === 'content-type')) {
      finalHeaders.push({ name: 'Content-Type', value: 'application/xml' });
    }
    const finalBody = buildFragmentsFromMode();
    return {
      response: {
        status: finalStatus,
        headers: finalHeaders,
        body: finalBody,
        chaos: chaosEnabled ? chaos : null,
      },
      pre_script: preScriptEnabled && preScriptCode.trim() ? preScriptCode.trim() : null,
      script: scriptEnabled && scriptCode.trim() ? scriptCode.trim() : null,
      post_script: postScriptEnabled && postScriptCode.trim() ? postScriptCode.trim() : null,
      // Memorise la vue d'origine pour la restaurer a la prochaine edition
      // (retour 1, cf CLAUDE.md) -- meme valeur que le discriminant interne
      // `responseMode` (les 7 chaines correspondent deja aux variants
      // kebab-case de ResponseEditorMode cote backend, cf src/models/mod.rs,
      // aucune table de correspondance necessaire).
      response_mode: responseMode,
    };
  }
</script>

{#snippet basicScriptHelp(varName)}
  <p class="field-hint">
    Execute independamment des autres blocs de script (meme contexte requete, pas de chainage).
    Resultat accessible via <code>{`{{${varName}}}`}</code> ou <code>{`{{${varName}.champ}}`}</code>.
    Meme syntaxe Rhai que le "Script personnalise" ci-dessous (voir ses exemples).
  </p>
{/snippet}

{#snippet mainScriptHelp()}
  <p class="field-hint"><strong>Contexte disponible :</strong> <code>request.body</code> (texte), <code>request.headers</code>, <code>request.query</code>, <code>request.path</code> (maps cle/valeur)</p>
  <p class="field-hint"><strong>Resultat :</strong> La derniere expression est le retour. String → <code>{"{{script}}"}</code>. Objet <code>#{"{cle: val}"}</code> → <code>{"{{script.cle}}"}</code></p>
  <details class="script-examples">
    <summary class="field-hint">Exemples et syntaxe Rhai</summary>
    <div class="script-examples-content">
      <p><strong>Variables :</strong> <code>let x = 42;</code> <code>let s = "hello";</code></p>
      <p><strong>Conditions :</strong> <code>if x &gt; 10 {"{"} "grand" {"}"} else {"{"} "petit" {"}"}</code></p>
      <p><strong>Strings :</strong> <code>s.to_upper()</code> <code>s.len()</code> <code>s.contains("el")</code> <code>s.replace("a", "b")</code></p>
      <p><strong>Fonctions et donnees de contexte disponibles</strong> (autocompletion dans l'editeur : tapez le debut d'un nom, ou <kbd>Ctrl</kbd>+<kbd>Espace</kbd>) :</p>
      <ul class="script-fn-list">
        {#each RHAI_FUNCTIONS as fn}
          <li><code>{fn.signature}</code> — {fn.description}</li>
        {/each}
      </ul>
      <p><strong>Objet retour :</strong> <code>#{"{"} cle: "val", n: random_int(1,100) {"}"}</code> → accessible via <code>{"{{script.cle}}"}</code></p>
      <p><strong>Ratio 4/5 :</strong> <code>if random_int(1,5) &lt;= 4 {"{"} #{"{"} status: "ok" {"}"} {"}"} else {"{"} #{"{"} status: "ko" {"}"} {"}"}</code></p>
      <p><strong>Nom fixe par SIRET :</strong> <code>seeded_pick(request.path.siret, ["Dupont SARL", "Martin SAS"])</code></p>
      <p class="field-hint">Sandbox : pas d'acces fichier/reseau, 10K ops max. <a href="https://rhai.rs/book/" target="_blank" rel="noopener">Doc Rhai</a></p>
    </div>
  </details>
{/snippet}

{#if visible}
<fieldset class="section section-response">
  <legend>
    <button type="button" class="legend-toggle" onclick={() => responseOpen = !responseOpen} aria-expanded={responseOpen} data-testid="rule-form-response-toggle-button">
      {responseOpen ? '▼' : '▶'} Reponse mockee
    </button>
  </legend>

  {#if responseOpen}
    {#key modeKey}
    <!--
      Fusion Format x Assiste/Detail (retour 3, cf CLAUDE.md) : remplace les
      7 boutons de mode a plat par 5 boutons de FORMAT (JSON/XML se
      declinent chacun en 2 sous-modes -- assiste "par exemple"/detail
      "guide" -- geres via le bouton "Modifier en detail" plus bas, jamais
      un second niveau de bouton visible d'emblee, cf revealDetailMode()).
      `data-testid` inchange (`rule-form-mode-button-{val}`, cf selectors.json)
      -- seules les VALEURS acceptees changent (json/xml/text/advanced/empty
      au lieu des 7 anciennes).
    -->
    <div class="mode-selector" role="radiogroup" aria-label="Format de la reponse">
      {#each [['json','JSON'],['xml','XML'],['text','Texte'],['advanced','Template avance'],['empty','Vide (204)']] as [val, label]}
        <button type="button" class="mode-btn" class:mode-active={formatOfMode(responseMode) === val} onclick={() => selectFormat(val)} role="radio" aria-checked={formatOfMode(responseMode) === val} data-testid="rule-form-mode-button-{val}">{label}</button>
      {/each}
    </div>
    {/key}

    {#if pendingMode}
      <div class="mode-warning" role="alert">
        <p>{pendingConvMessage || `Changer vers le mode "${pendingMode}" pourrait entrainer une perte de donnees.`}</p>
        <div class="mode-warning-actions">
          <button type="button" class="btn btn-sm btn-primary" onclick={confirmModeSwitch} data-testid="rule-form-mode-switch-confirm-button">Changer quand meme</button>
          <button type="button" class="btn btn-sm btn-secondary" onclick={cancelModeSwitch} data-testid="rule-form-mode-switch-cancel-button">Annuler</button>
        </div>
      </div>
    {/if}

    {#if responseMode !== 'empty'}
      <div class="form-row">
        <div class="form-field" style="max-width:8rem">
          <label for="resp-status">Code HTTP</label>
          <input id="resp-status" type="number" bind:value={status} min="100" max="599" data-testid="rule-form-status-input" />
        </div>
      </div>

      <div class="sub-section">
        <strong>En-tetes</strong>
        {#each respHeaders as hdr, idx}
          <div class="header-row">
            <input type="text" bind:value={hdr.name} placeholder="Content-Type" aria-label="Nom de l'en-tete {idx + 1}" list="dl-header-names" autocomplete="off" data-testid="rule-form-header-name-input-{idx}" />
            <input type="text" bind:value={hdr.value} placeholder="application/json" aria-label="Valeur de l'en-tete {idx + 1}" list={hdr.name?.toLowerCase() === 'content-type' ? 'dl-content-types' : undefined} autocomplete="off" data-testid="rule-form-header-value-input-{idx}" />
            <button type="button" class="btn-icon btn-delete" onclick={() => removeHeader(idx)} aria-label="Supprimer l'en-tete" data-testid="rule-form-remove-header-button-{idx}">&#10005;</button>
          </div>
        {/each}
        <datalist id="dl-header-names">
          {#each commonHeaders as h}<option value={h}></option>{/each}
        </datalist>
        <datalist id="dl-content-types">
          {#each commonContentTypes as ct}<option value={ct}></option>{/each}
        </datalist>
        <button type="button" class="btn btn-sm btn-outline" onclick={addHeader} data-testid="rule-form-add-header-button">+ En-tete</button>
        {#if responseMode === 'json-guided' || responseMode === 'json-paste'}
          <span class="field-hint">Content-Type: application/json sera ajoute automatiquement.</span>
        {/if}
      </div>
    {/if}

    {#if responseMode === 'json-paste'}
      <div class="sub-section">
        <JsonPasteBuilder bind:this={jsonPasteRef} fields={jsonPasteFields} startParsed={jsonPasteFields.length > 0} onUpdate={(f) => jsonPasteFields = f} />
        <button type="button" class="btn btn-sm btn-outline open-detail-button" onclick={revealDetailMode} data-testid="rule-form-open-detail-button">
          Modifier en détail (structure complète) →
        </button>
      </div>

    {:else if responseMode === 'json-guided'}
      <div class="sub-section">
        <JsonResponseBuilder bind:this={jsonBuilderRef} fields={jsonFields} onUpdate={(f) => jsonFields = f} />
        <button type="button" class="btn btn-sm btn-outline back-to-paste-button" onclick={backToPasteMode} data-testid="rule-form-back-to-paste-button">
          ← Revenir à la vue « par exemple »
        </button>
      </div>

    {:else if responseMode === 'xml-paste'}
      <div class="sub-section">
        <XmlPasteBuilder bind:this={xmlPasteRef} fields={xmlPasteFields} rootTag={xmlRootTag} rootAttributes={xmlRootAttributes} startParsed={xmlPasteFields.length > 0} onUpdate={(f) => xmlPasteFields = f} />
        <button type="button" class="btn btn-sm btn-outline open-detail-button" onclick={revealDetailMode} data-testid="rule-form-open-detail-button">
          Modifier en détail (structure complète) →
        </button>
      </div>

    {:else if responseMode === 'xml-guided'}
      <div class="sub-section">
        <XmlResponseBuilder bind:this={xmlBuilderRef} fields={xmlFields} rootTag={xmlRootTag} onUpdate={(f) => xmlFields = f} />
        <button type="button" class="btn btn-sm btn-outline back-to-paste-button" onclick={backToPasteMode} data-testid="rule-form-back-to-paste-button">
          ← Revenir à la vue « par exemple »
        </button>
      </div>

    {:else if responseMode === 'text'}
      <div class="sub-section">
        <strong>Contenu texte</strong>
        <textarea bind:value={textContent} rows="5" placeholder="Contenu de la reponse en texte brut" aria-label="Contenu texte de la reponse" class="text-area" data-testid="rule-form-text-content-textarea"></textarea>
      </div>

    {:else if responseMode === 'advanced'}
      <div class="sub-section">
        <strong>Corps de la reponse (fragments)</strong>
        <p class="section-help">Composez la reponse en ajoutant des blocs concatenes dans l'ordre.</p>

        {#each fragments as frag, idx}
          <div class="fragment-card" data-testid="rule-form-fragment-card-{idx}">
            <div class="fragment-header">
              <span class="frag-index">{idx + 1}</span>
              <select value={frag.type} onchange={(e) => updateFragmentType(idx, e.target.value)} aria-label="Type du fragment {idx + 1}" data-testid="rule-form-fragment-type-select-{idx}">
                {#each fragmentTypes as ft}
                  <option value={ft.value}>{ft.label}</option>
                {/each}
              </select>
              <div class="fragment-actions">
                <button type="button" class="btn-icon" onclick={() => moveFragment(idx, -1)} disabled={idx === 0} aria-label="Monter" title="Monter" data-testid="rule-form-fragment-moveup-button-{idx}">&#9650;</button>
                <button type="button" class="btn-icon" onclick={() => moveFragment(idx, 1)} disabled={idx === fragments.length - 1} aria-label="Descendre" title="Descendre" data-testid="rule-form-fragment-movedown-button-{idx}">&#9660;</button>
                <button type="button" class="btn-icon btn-delete" onclick={() => removeFragment(idx)} aria-label="Supprimer" title="Supprimer" data-testid="rule-form-fragment-delete-button-{idx}">&#10005;</button>
              </div>
            </div>
            <div class="fragment-body">
              {#if frag.type === 'Literal'}
                <textarea bind:value={frag.value} rows="2" placeholder='ex: {`{"siret":"`}' aria-label="Contenu texte" data-testid="rule-form-fragment-literal-textarea-{idx}"></textarea>
              {:else if frag.type === 'Uuid'}
                <p class="frag-info">UUID v4 genere a chaque requete.</p>
              {:else if frag.type === 'PickFrom'}
                {#each frag.values as val, vi}
                  <div class="pick-row">
                    <input type="text" bind:value={frag.values[vi]} placeholder="Valeur {vi + 1}" aria-label="Valeur {vi + 1}" data-testid="rule-form-fragment-pick-input-{idx}-{vi}" />
                    <button type="button" class="btn-icon btn-delete" onclick={() => removePickValue(idx, vi)} aria-label="Supprimer" data-testid="rule-form-fragment-pick-remove-button-{idx}-{vi}">&#10005;</button>
                  </div>
                {/each}
                <button type="button" class="btn btn-sm btn-outline" onclick={() => addPickValue(idx)} data-testid="rule-form-fragment-pick-add-button-{idx}">+ Valeur</button>
              {:else if frag.type === 'FakeData'}
                <select value={frag.kind?.type ?? 'FirstName'} onchange={(e) => updateFakeKind(idx, e.target.value)} aria-label="Type fictif" data-testid="rule-form-fragment-fake-select-{idx}">
                  {#each fakeKinds as fk}<option value={fk.value}>{fk.label}</option>{/each}
                </select>
              {:else if frag.type === 'PathSegment'}
                <label class="inline-label">Position <input type="number" bind:value={frag.index} min="0" style="width:5rem" data-testid="rule-form-fragment-pathsegment-input-{idx}" /></label>
              {:else if frag.type === 'Template'}
                <textarea bind:value={frag.template} rows="5" class="template-textarea"
                  placeholder={`Ex: {{"siret":"{path.siret}","siren":"{path.siret | first(9)}"}}`}
                  aria-label="Template" data-testid="rule-form-fragment-template-textarea-{idx}"></textarea>
                <div class="template-help">
                  <span class="field-hint"><strong>Variables :</strong> <code>{`{path.nom}`}</code>, <code>{`{query.id}`}</code>, <code>{`{uuid}`}</code>, <code>{`{now_ms}`}</code>, <code>{`{fake.CompanyName}`}</code>, <code>{`{seq}`}</code></span>
                  <span class="field-hint"><strong>Pipes :</strong> <code>| lower</code>, <code>| upper</code>, <code>| capitalize</code>, <code>| first(N)</code>, <code>| last(N)</code>, <code>| substr(start,len)</code>, <code>| replace("a","b")</code>, <code>| prepend("x")</code>, <code>| append("x")</code>, <code>| default("val")</code>, <code>| length</code>, <code>| trim</code>. JSON : <code>{`{{`}</code> / <code>{`}}`}</code></span>
                </div>
              {/if}
            </div>
          </div>
        {/each}
        <button type="button" class="btn btn-sm btn-outline" onclick={addFragment} data-testid="rule-form-fragment-add-button">+ Ajouter un fragment</button>
      </div>

    {:else if responseMode === 'empty'}
      <p class="section-help" style="margin-top:0.5rem">La reponse sera 204 No Content, sans body.</p>
    {/if}

    <RuleScriptSlot
      id="rule-script" toggleLabel="Script personnalise"
      enabled={scriptEnabled} code={scriptCode}
      onToggle={(v) => scriptEnabled = v} onCodeInput={(v) => scriptCode = v}
      validation={scriptValidation} onValidate={handleValidateScript}
      rows={8}
      placeholder={'// Exemples Rhai :\n// Retourner une valeur simple :\nlet id = request.path.id;\n`user_${id}`\n\n// Retourner un objet (accessible via {{script.champ}}) :\n#{ nom: "Alice", age: "30" }'}
    >
      {#snippet help()}{@render mainScriptHelp()}{/snippet}
    </RuleScriptSlot>

    <div class="sub-section advanced-options-section">
      <button
        type="button"
        class="legend-toggle"
        onclick={() => advancedOpen = !advancedOpen}
        aria-expanded={advancedOpen}
        aria-controls="rule-form-advanced-options-panel"
        data-testid="rule-form-advanced-options-toggle-button"
      >
        {advancedOpen ? '▼' : '▶'} Options avancées (pré-script / post-script)
      </button>
      <div id="rule-form-advanced-options-panel" class="advanced-options-panel" hidden={!advancedOpen} data-testid="rule-form-advanced-options-panel">
        <RuleScriptSlot
          id="rule-pre-script" toggleLabel="Pré-script (préparation)"
          enabled={preScriptEnabled} code={preScriptCode}
          onToggle={(v) => preScriptEnabled = v} onCodeInput={(v) => preScriptCode = v}
          validation={preScriptValidation} onValidate={handleValidatePreScript}
          rows={5}
        >
          {#snippet help()}{@render basicScriptHelp('pre_script')}{/snippet}
        </RuleScriptSlot>

        <RuleScriptSlot
          id="rule-post-script" toggleLabel="Post-script (finalisation)"
          enabled={postScriptEnabled} code={postScriptCode}
          onToggle={(v) => postScriptEnabled = v} onCodeInput={(v) => postScriptCode = v}
          validation={postScriptValidation} onValidate={handleValidatePostScript}
          rows={5}
        >
          {#snippet help()}{@render basicScriptHelp('post_script')}{/snippet}
        </RuleScriptSlot>
      </div>
    </div>

    <div class="sub-section chaos-section">
      <ToggleSwitch label="Mode Chaos" checked={chaosEnabled} onchange={(v) => chaosEnabled = v} />
      {#if chaosEnabled}
        <div class="chaos-fields">
          <label>Latence fixe (ms) <input type="number" bind:value={chaos.delay_ms} min="0" max="30000" data-testid="rule-form-chaos-delay-input" /></label>
          <label>Latence min (ms) <input type="number" bind:value={chaos.delay_min_ms} min="0" max="30000" data-testid="rule-form-chaos-delay-min-input" /></label>
          <label>Latence max (ms) <input type="number" bind:value={chaos.delay_max_ms} min="0" max="30000" data-testid="rule-form-chaos-delay-max-input" /></label>
          <label>Taux d'erreur (0-1) <input type="number" bind:value={chaos.error_rate} min="0" max="1" step="0.05" data-testid="rule-form-chaos-error-rate-input" /></label>
          <label>Code erreur <input type="number" bind:value={chaos.error_status} min="400" max="599" data-testid="rule-form-chaos-error-status-input" /></label>
        </div>
        <span class="field-hint">Si min/max sont remplis, la latence est aleatoire dans la plage (ignore la latence fixe).</span>
      {/if}
    </div>
  {/if}
</fieldset>
{/if}

<style>
  .section-response { border-color: var(--color-primary); }
  .legend-toggle { background: none; border: none; font: inherit; font-weight: 600; font-size: 0.875rem; cursor: pointer; padding: 0; color: var(--color-text); }

  .section-help { font-size: 0.8125rem; color: var(--color-text-muted); margin: 0 0 0.5rem; }

  .mode-selector { display: flex; gap: 0.5rem; margin-bottom: 0.75rem; flex-wrap: wrap; }
  .mode-btn { font-size: 0.875rem; font-weight: 500; cursor: pointer; padding: 0.375rem 0.75rem; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-bg); color: var(--color-text); font-family: inherit; }
  .mode-btn:hover { border-color: var(--color-primary); }
  .mode-btn.mode-active { border-color: var(--color-primary); background: var(--color-focus); font-weight: 600; }

  .open-detail-button, .back-to-paste-button { margin-top: 0.5rem; }

  .mode-warning { background: #fff3cd; border: 1px solid #ffc107; color: #664d03; padding: 0.75rem; border-radius: var(--radius); margin-bottom: 0.75rem; }
  :global([data-theme="dark"]) .mode-warning { background: #332701; border-color: #e5a50a; color: #ffe082; }
  .mode-warning p { margin: 0 0 0.5rem; font-size: 0.875rem; }
  .mode-warning-actions { display: flex; gap: 0.5rem; flex-wrap: wrap; }

  .sub-section { margin-top: 0.75rem; padding-top: 0.75rem; border-top: 1px solid var(--color-border); }
  .sub-section strong { display: block; margin-bottom: 0.375rem; font-size: 0.875rem; }

  .advanced-options-section { border-top-color: var(--color-border); }
  .advanced-options-panel { margin-top: 0.5rem; }

  .header-row { display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.375rem; }
  .header-row input { flex: 1; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .fragment-card { border: 1px solid var(--color-border); border-radius: var(--radius); padding: 0.625rem; margin-bottom: 0.5rem; background: var(--color-bg); }
  .fragment-header { display: flex; align-items: center; gap: 0.5rem; }
  .fragment-header select { flex: 1; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }
  .fragment-actions { display: flex; gap: 0.25rem; flex-shrink: 0; }
  .fragment-body { margin-top: 0.5rem; }
  .fragment-body textarea { width: 100%; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; font-family: 'Cascadia Code', 'Fira Code', monospace; resize: vertical; }
  .fragment-body select { width: 100%; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; margin-bottom: 0.375rem; }
  .frag-index { display: inline-flex; align-items: center; justify-content: center; width: 1.5rem; height: 1.5rem; border-radius: 50%; background: var(--color-primary); color: #fff; font-size: 0.75rem; font-weight: 700; flex-shrink: 0; }
  .frag-info { font-size: 0.8125rem; color: var(--color-text-muted); font-style: italic; margin: 0; }

  .text-area { width: 100%; padding: 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; font-family: inherit; resize: vertical; }
  .template-textarea { min-height: 5rem; }
  .template-help { margin-top: 0.375rem; display: flex; flex-direction: column; gap: 0.125rem; }
  .template-help code { background: var(--color-bg); padding: 0.1rem 0.25rem; border-radius: 2px; font-size: 0.8rem; }

  .pick-row { display: flex; gap: 0.375rem; align-items: center; margin-bottom: 0.25rem; }
  .pick-row input { flex: 1; padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .inline-label { display: flex; align-items: center; gap: 0.5rem; font-size: 0.875rem; font-weight: 500; margin-bottom: 0.375rem; }
  .inline-label input { padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .chaos-section { border-top-color: var(--color-warning); }
  .chaos-fields { display: flex; flex-wrap: wrap; gap: 0.75rem; margin-top: 0.5rem; }
  .chaos-fields label { display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.875rem; min-width: 8rem; }
  .chaos-fields input { padding: 0.375rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius); font-size: 0.875rem; }

  .btn-icon { width: 1.75rem; height: 1.75rem; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--color-border); border-radius: var(--radius); background: var(--color-surface); color: var(--color-text-muted); font-size: 0.75rem; cursor: pointer; }
  .btn-icon:hover:not(:disabled) { background: var(--color-bg); color: var(--color-text); }
  .btn-icon:disabled { opacity: 0.35; cursor: not-allowed; }
  .btn-icon.btn-delete:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }

  /* .section : le fieldset racine lui-meme (section-response ci-dessus
     l'affine), duplique volontairement depuis RuleActionSelector.svelte/
     RuleConditionsEditor.svelte plutot que centralise (cf CLAUDE.md,
     "CSS Design System" — pas encore fait a l'echelle du projet). */
  .section { border: 1px solid var(--color-border); border-radius: var(--radius); padding: 0.75rem; margin-bottom: 1rem; }
  .section legend { font-weight: 600; font-size: 0.875rem; padding: 0 0.375rem; }
</style>
