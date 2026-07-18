// Source unique de verite pour les fonctions natives Rhai exposees par
// lightMock (miroir de src/engine/script.rs::ScriptEngine::new). Reutilisee a
// la fois par la doc contextuelle de l'editeur de script (RuleForm.svelte) ET
// par l'autocompletion (RhaiScriptEditor.svelte) — meme principe que
// tpl-utils.js pour le format template : une seule liste a mettre a jour
// quand une fonction native est ajoutee/modifiee cote moteur.
export const RHAI_FUNCTIONS = [
  // --- Acces au contexte de la requete (variable `request`, pas des
  // fonctions a proprement parler, mais listees ici pour beneficier de la
  // meme autocompletion/doc que le reste — meme principe de source unique).
  // Miroir de ScriptEngine::execute() (src/engine/script.rs), qui pousse un
  // seul objet `request` dans le scope Rhai avec 4 champs : body/headers/
  // query/path. IMPORTANT : acceder a une cle absente ne leve JAMAIS
  // d'erreur en Rhai (ni via `.cle`, ni via `["cle"]`) — ca renvoie
  // simplement une valeur vide/absente. Une regle qui ne matche jamais a
  // cause d'un nom de cle errone (ex. `request.path.id` alors que le path
  // param s'appelle `orderId`) echoue donc SILENCIEUSEMENT (aucune erreur a
  // l'execution), contrairement a l'appel d'une fonction native inexistante
  // (voir plus bas) qui, lui, produit une vraie erreur. Utiliser le testeur
  // de regle (section "Tester contre une requete reelle" ci-dessus) pour
  // verifier qu'une cle est bien trouvee avant de se fier au script.
  {
    name: 'request.path',
    signature: 'request.path.nom_du_parametre',
    description: 'Parametres de chemin extraits de l\'URL (ex. {id} dans /orders/{id} -> request.path.id). Cle absente = valeur vide, jamais d\'erreur.',
    insertText: 'request.path',
  },
  {
    name: 'request.query',
    signature: 'request.query.nom_du_parametre',
    description: 'Parametres de la query string (ex. ?page=2 -> request.query.page). Cle absente = valeur vide, jamais d\'erreur.',
    insertText: 'request.query',
  },
  {
    name: 'request.headers',
    signature: 'request.headers.nom_entete',
    description: 'En-tetes HTTP de la requete. ATTENTION : les noms sont toujours normalises en minuscules cote serveur (ex. "SOAPAction" devient request.headers.soapaction) — utiliser un nom en minuscules, sinon la cle est silencieusement introuvable.',
    insertText: 'request.headers',
  },
  {
    name: 'request.body',
    signature: 'request.body',
    description: 'Corps brut de la requete, en texte. A parser avec parse_json()/parse_xml_items() si le corps est structure.',
    insertText: 'request.body',
  },
  {
    name: 'random_int',
    signature: 'random_int(min, max)',
    description: 'Entier aleatoire entre min et max (inclus).',
    insertText: 'random_int(min, max)',
  },
  {
    name: 'now_ms',
    signature: 'now_ms()',
    description: 'Timestamp Unix courant en millisecondes.',
    insertText: 'now_ms()',
  },
  {
    name: 'now_iso',
    signature: 'now_iso()',
    description: "Date et heure courantes au format ISO 8601 complet (avec l'heure).",
    insertText: 'now_iso()',
  },
  {
    name: 'year',
    signature: 'year()',
    description: 'Annee courante sur 4 chiffres.',
    insertText: 'year()',
  },
  {
    name: 'date_now',
    signature: 'date_now(format)',
    description: 'Date du jour. format optionnel : "iso" (defaut), "fr", "en".',
    insertText: 'date_now("iso")',
  },
  {
    name: 'date_past',
    signature: 'date_past(jours, format)',
    description: "Date dans le passe, \"jours\" jours avant aujourd'hui (0 ou negatif = aujourd'hui).",
    insertText: 'date_past(jours, "iso")',
  },
  {
    name: 'date_future',
    signature: 'date_future(jours, format)',
    description: "Date dans le futur, \"jours\" jours apres aujourd'hui (0 ou negatif = aujourd'hui).",
    insertText: 'date_future(jours, "iso")',
  },
  {
    name: 'parse_date',
    signature: 'parse_date(texte, "pattern")',
    description: 'Sens inverse de date_now/date_past/date_future : parse une date SAISIE selon un pattern explicite (yyyy/MM/dd/HH/mm/ss, tout autre caractere est litteral) et retourne le nombre de millisecondes depuis epoch. Ex. parse_date("15/03/2026", "dd/MM/yyyy"). Heure optionnelle (defaut 00:00:00). Erreur d\'execution si le texte ne correspond pas au pattern ou si la date est invalide (ex. 31 fevrier).',
    insertText: 'parse_date(texte, "dd/MM/yyyy")',
  },
  {
    name: 'uuid',
    signature: 'uuid()',
    description: 'Identifiant UUID v4 aleatoire.',
    insertText: 'uuid()',
  },
  {
    name: 'fake',
    signature: 'fake("Kind")',
    description: 'Donnee fictive (ex. "FirstName", "Email", "CompanyName"...).',
    insertText: 'fake("FirstName")',
  },
  {
    name: 'seeded_int',
    signature: 'seeded_int(seed, min, max)',
    description: 'Entier deterministe dans [min, max] : meme resultat pour un meme seed.',
    insertText: 'seeded_int(seed, min, max)',
  },
  {
    name: 'seeded_pick',
    signature: 'seeded_pick(seed, [liste])',
    description: 'Choisit un element de la liste de facon deterministe pour un meme seed.',
    insertText: 'seeded_pick(seed, ["a", "b"])',
  },
  {
    name: 'parse_json',
    signature: 'parse_json(texte)',
    description: 'Parse un texte JSON (ex. request.body) en liste/objet Rhai navigable.',
    insertText: 'parse_json(request.body)',
  },
  {
    name: 'to_json',
    signature: 'to_json(valeur)',
    description: 'Serialise une liste/objet Rhai en texte JSON, a inserer via {{script.champ}}.',
    insertText: 'to_json(valeur)',
  },
  {
    name: 'parse_xml_items',
    signature: 'parse_xml_items(texte, "chemin/vers/item")',
    description: 'Extrait tous les elements XML repetes a un chemin en liste d\'objets Rhai (un niveau de champs enfants).',
    insertText: 'parse_xml_items(request.body, "chemin/vers/item")',
  },
  {
    name: 'xml_element',
    signature: 'xml_element(tag, valeur)',
    description: 'Construit un element XML <tag>...</tag> a partir d\'une liste/objet Rhai (recursif).',
    insertText: 'xml_element("tag", valeur)',
  },
];

// Fonctions dont le nom commence par `query` (insensible a la casse).
// Liste complete si query est vide/absent (utilise pour Ctrl+Espace sans
// prefixe deja tape).
export function filterRhaiFunctions(query) {
  if (!query) return RHAI_FUNCTIONS;
  const lower = query.toLowerCase();
  return RHAI_FUNCTIONS.filter((f) => f.name.toLowerCase().startsWith(lower));
}

// Identifiant Rhai (lettres/chiffres/underscore) immediatement avant
// `cursorPos` dans `text`. Sert a la fois a determiner le prefixe de
// recherche de l'autocompletion et la portion de texte a remplacer lors de
// l'insertion d'une suggestion.
export function tokenAtCursor(text, cursorPos) {
  const before = text.slice(0, cursorPos);
  const match = before.match(/[A-Za-z_][A-Za-z0-9_]*$/);
  return match
    ? { token: match[0], start: cursorPos - match[0].length }
    : { token: '', start: cursorPos };
}

// Selection (offsets relatifs a insertText) a appliquer juste apres
// insertion : les parametres entre parentheses sont selectionnes pour etre
// remplaces immediatement par la frappe (ex. "random_int(min, max)" ->
// "min, max" selectionne) ; sans parametres, le curseur est simplement place
// apres l'appel (ex. "now_ms()" -> curseur apres la parenthese fermante).
export function computeInsertSelection(insertText) {
  const openIdx = insertText.indexOf('(');
  const closeIdx = insertText.lastIndexOf(')');
  if (openIdx === -1 || closeIdx === -1 || closeIdx <= openIdx + 1) {
    return { start: insertText.length, end: insertText.length };
  }
  return { start: openIdx + 1, end: closeIdx };
}
