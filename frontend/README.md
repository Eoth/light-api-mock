# frontend/ — Interface Web Svelte 5

SPA pure (sans SvelteKit) servie en statique par le backend Rust.

## Dev

```bash
npm install
npm run dev       # http://localhost:5173, proxy /api/* vers :7342
```

## Build / Tests

```bash
npm run build     # -> dist/
npm test          # Vitest (91+ tests unitaires)
npm run test:e2e  # Playwright (17 tests, serveur doit tourner)
```

## Composants

| Composant | Role |
|---|---|
| `App.svelte` | Layout, navigation, import/export, reset, dark mode, vue logs |
| `ServiceList` | Liste filtrable des services (recherche) |
| `ServiceCard` | Carte service (badge methode, URL namespace, toggle, configurer) |
| `ServiceDetail` | Vue detail : edition, suppression, gestion des regles |
| `ServiceForm` | Formulaire service (name, method, listen_path, validation securite) |
| `RuleList` | Liste ordonnee des regles avec badge MOCK/PROXY (drag-and-drop + clavier) |
| `RuleForm` | Formulaire regle : action mock/proxy, conditions, reponse (5 modes) |
| `ConditionForm` | Condition inline (7 sources x 4 operateurs) |
| `JsonResponseBuilder` | Editeur visuel JSON avec sources dynamiques, pipes, preview |
| `XmlResponseBuilder` | Editeur visuel XML avec sources dynamiques, pipes, preview |
| `RequestLog` | Journal des requetes avec filtre par service |
| `ToggleSwitch` | Interrupteur ON/OFF (role="switch", aria-checked) |
| `StatusBadge` | Badge MOCK/PROXY |
| `Notification` | Bandeau feedback (role="alert") |

## Module partage : tpl-utils.js

Source unique de verite pour le format template lightMock. Centralise :

| Fonction | Role |
|---|---|
| `fieldsToTemplate(fields)` | Fields JS → template string (serialisation JSON) |
| `templateToFields(tpl)` | Template string → Fields JS (deserialisation JSON) |
| `xmlFieldsToTemplate(fields, rootTag)` | Fields JS → template string (serialisation XML) |
| `templateToTestJson(tpl)` | Template → JSON de test (pour validation par JSON.parse) |
| `validateTemplateAsJson(tpl)` | Validation structurelle JSON |
| `validateTemplateAsXml(tpl)` | Validation structurelle XML |
| `templateToPreview(tpl)` | Rendu lisible avec «variables» |
| `buildExpr(field)` | Construction d'expression variable + pipe |
| `templateToXmlFields(tpl)` | Template string → Fields JS (deserialisation XML, restauration a l'edition) |

## Modes de reponse (RuleForm / RuleResponseSection)

Le selecteur expose 5 boutons de **Format** (JSON, XML, Texte, Template avance, Vide) ; JSON et
XML se declinent chacun en 2 sous-modes internes (`responseMode`), jamais visibles comme 2 boutons
distincts : le format demarre toujours en `*-paste` (assiste), et un bouton "Modifier en detail"
revele `*-guided` (detail) SANS conversion (memes Fields, juste plus de capacites d'edition).

| responseMode (interne) | Description |
|---|---|
| json-paste | Coller un exemple JSON, editer uniquement source/pipe par champ deja detecte |
| json-guided | Editeur cle/valeur complet (renommage/ajout/suppression), sources dynamiques et pipes, genere un Template |
| xml-paste | Coller un exemple XML, editer source/pipe/attributs par noeud deja detecte |
| xml-guided | Editeur tag/valeur complet, sources dynamiques et pipes, genere un Template |
| Texte | Textarea libre, genere un Literal |
| Template avance | Syntaxe `{path.siret \| first(9)}` brute |
| Vide (204) | Pas de body |

`Rule.response_mode` (backend, `#[serde(default)]`) persiste cette valeur pour restaurer la bonne
vue a la reouverture d'une regle (cf `RuleResponseSection.svelte::computeInitialEditorState()`).

### Conversions entre modes

| De → Vers | Supporte | Notes |
|---|---|---|
| json-paste ↔ json-guided | Oui, sans perte, hors systeme d'avertissement | `revealDetailMode()`, memes Fields |
| json-guided → Avance | Oui, sans perte | Via `fieldsToTemplate()` |
| Avance → json-paste / json-guided | Oui, si JSON objet racine | Via `templateToFields()` |
| json-guided/json-paste → xml-paste/xml-guided | Partiel | Pas de tableaux scalaires |
| Avance ↔ Texte | Oui | Concatenation / fragment Literal |
| xml-guided/xml-paste → json-guided/json-paste | Non | Structures incompatibles |

## Securite frontend

- Noms de service reserves refuses (api, index.html, assets, favicon.ico)
- Chemins dangereux refuses (vide, `/`, `/*`)
- Noms de service dupliques refuses (409 backend, validation frontend)
- Noms de regle dupliques refuses dans un meme service
- Routes internes protegees contre interception

## Accessibilite RGAA AA

Skip link, labels visibles, aria-describedby, role="switch", role="radio", role="alert", focus-visible, contrastes >= 4.5:1, drag-and-drop avec boutons alternatifs, mode sombre respectant les contrastes.
