// Source unique de verite pour les fonctions natives Rhai exposees par
// lightMock (miroir de src/engine/script.rs::ScriptEngine::new). Reutilisee a
// la fois par la doc contextuelle de l'editeur de script (RuleForm.svelte) ET
// par l'autocompletion (RhaiScriptEditor.svelte) — meme principe que
// tpl-utils.js pour le format template : une seule liste a mettre a jour
// quand une fonction native est ajoutee/modifiee cote moteur.
import { t } from './i18n.svelte.js';

// Texts are getters, read when shown, so that they follow a change of language.
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
    get signature() { return t("request.path.parameter_name"); },
    get description() { return t("Path parameters taken from the URL (e.g. {id} in /orders/{id} -> request.path.id). A missing key gives an empty value, never an error."); },
    insertText: 'request.path',
  },
  {
    name: 'request.query',
    get signature() { return t("request.query.parameter_name"); },
    get description() { return t("Parameters of the query string (e.g. ?page=2 -> request.query.page). A missing key gives an empty value, never an error."); },
    insertText: 'request.query',
  },
  {
    name: 'request.headers',
    get signature() { return t("request.headers.header_name"); },
    get description() { return t("HTTP headers of the request. WARNING: the server always lowercases their names (e.g. \"SOAPAction\" becomes request.headers.soapaction): use a lowercase name, or the key is silently not found."); },
    insertText: 'request.headers',
  },
  {
    name: 'request.body',
    signature: 'request.body',
    get description() { return t("Raw body of the request, as text. Parse it with parse_json() or parse_xml_items() when it is structured."); },
    insertText: 'request.body',
  },
  {
    name: 'random_int',
    signature: 'random_int(min, max)',
    get description() { return t("Random integer between min and max (inclusive)."); },
    insertText: 'random_int(min, max)',
  },
  {
    name: 'now_ms',
    signature: 'now_ms()',
    get description() { return t("Current Unix timestamp in milliseconds."); },
    insertText: 'now_ms()',
  },
  {
    name: 'now_iso',
    signature: 'now_iso()',
    get description() { return t("Current date and time in full ISO 8601 format (with the time)."); },
    insertText: 'now_iso()',
  },
  {
    name: 'year',
    signature: 'year()',
    get description() { return t("Current year, 4 digits."); },
    insertText: 'year()',
  },
  {
    name: 'date_now',
    signature: 'date_now(format)',
    get description() { return t("Today's date. Optional format: \"iso\" (default), \"fr\", \"en\"."); },
    insertText: 'date_now("iso")',
  },
  {
    name: 'date_past',
    get signature() { return t("date_past(days, format)"); },
    get description() { return t("A date in the past, \"days\" days before today (0 or negative = today)."); },
    get insertText() { return t("date_past(days, \"iso\")"); },
  },
  {
    name: 'date_future',
    get signature() { return t("date_future(days, format)"); },
    get description() { return t("A date in the future, \"days\" days after today (0 or negative = today)."); },
    get insertText() { return t("date_future(days, \"iso\")"); },
  },
  {
    name: 'parse_date',
    get signature() { return t("parse_date(text, \"pattern\")"); },
    get description() { return t("The reverse of date_now/date_past/date_future: parses a date TYPED in an explicit pattern (yyyy/MM/dd/HH/mm/ss, any other character is literal) and returns milliseconds since the epoch. E.g. parse_date(\"15/03/2026\", \"dd/MM/yyyy\"). The time is optional (00:00:00 by default). A run error when the text does not follow the pattern or the date does not exist (e.g. February 31)."); },
    get insertText() { return t("parse_date(text, \"dd/MM/yyyy\")"); },
  },
  {
    name: 'uuid',
    signature: 'uuid()',
    get description() { return t("Random UUID v4 identifier."); },
    insertText: 'uuid()',
  },
  {
    name: 'fake',
    signature: 'fake("Kind")',
    get description() { return t("Fake data (e.g. \"FirstName\", \"Email\", \"CompanyName\"...)."); },
    insertText: 'fake("FirstName")',
  },
  {
    name: 'seeded_int',
    signature: 'seeded_int(seed, min, max)',
    get description() { return t("Deterministic integer in [min, max]: the same seed always gives the same result."); },
    insertText: 'seeded_int(seed, min, max)',
  },
  {
    name: 'seeded_pick',
    get signature() { return t("seeded_pick(seed, [list])"); },
    get description() { return t("Picks an element of the list, always the same one for the same seed."); },
    insertText: 'seeded_pick(seed, ["a", "b"])',
  },
  {
    name: 'parse_json',
    get signature() { return t("parse_json(text)"); },
    get description() { return t("Parses JSON text (e.g. request.body) into a Rhai list or object you can navigate."); },
    insertText: 'parse_json(request.body)',
  },
  {
    name: 'to_json',
    get signature() { return t("to_json(value)"); },
    get description() { return t("Serializes a Rhai list or object to JSON text, to insert through {{script.field}}."); },
    get insertText() { return t("to_json(value)"); },
  },
  {
    name: 'parse_xml_items',
    get signature() { return t("parse_xml_items(text, \"path/to/item\")"); },
    get description() { return t("Extracts every XML element repeated at a path into a list of Rhai objects (one level of child fields)."); },
    get insertText() { return t("parse_xml_items(request.body, \"path/to/item\")"); },
  },
  {
    name: 'xml_element',
    get signature() { return t("xml_element(tag, value)"); },
    get description() { return t("Builds an XML element <tag>...</tag> from a Rhai list or object (recursive)."); },
    get insertText() { return t("xml_element(\"tag\", value)"); },
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
