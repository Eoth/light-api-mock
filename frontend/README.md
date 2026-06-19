# frontend/ — Interface Web Svelte 5

SPA pure (sans SvelteKit) servie en statique par le backend Rust.

## Dev

```bash
npm install
npm run dev       # http://localhost:5173, proxy /api/* vers :3000
```

## Build / Tests

```bash
npm run build     # -> dist/
npm test          # Vitest (35 tests unitaires)
npm run test:e2e  # Playwright (19 tests, serveur doit tourner)
```

## Composants

| Composant | Role |
|---|---|
| `App.svelte` | Layout, navigation, import/export, demo, vue logs |
| `ServiceList` | Liste filtrable des services (recherche) |
| `ServiceCard` | Carte service (badge methode, URL namespace, toggle, configurer) |
| `ServiceDetail` | Vue detail : edition, suppression, gestion des regles |
| `ServiceForm` | Formulaire service (name, method, listen_path, URL preview) |
| `RuleList` | Liste ordonnee des regles (drag-and-drop + clavier) |
| `RuleForm` | Formulaire regle : conditions + reponse (5 modes) |
| `ConditionForm` | Condition inline (7 sources x 4 operateurs) |
| `JsonResponseBuilder` | Editeur visuel JSON cle/valeur avec source dynamique |
| `XmlResponseBuilder` | Editeur visuel XML tag/valeur |
| `RequestLog` | Tableau des dernieres requetes (service, method, mode, status) |
| `ToggleSwitch` | Interrupteur ON/OFF (role="switch", aria-checked) |
| `StatusBadge` | Badge MOCK/PROXY |
| `Notification` | Bandeau feedback (role="alert") |

## Modes de reponse (RuleForm)

| Mode | Description |
|---|---|
| JSON guide | Editeur cle/valeur, genere un Template |
| XML guide | Editeur tag/valeur, genere un Template |
| Texte | Textarea libre, genere un Literal |
| Template avance | Syntaxe `{path.siret \| first(9)}` brute |
| Vide (204) | Pas de body |

## Accessibilite RGAA AA

Skip link, labels visibles, aria-describedby, role="switch", role="alert", focus-visible, contrastes >= 4.5:1, drag-and-drop avec boutons alternatifs.
