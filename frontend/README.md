# frontend/ - Interface Web Svelte 5

SPA pure (sans SvelteKit ni router tiers) servie en statique par le backend Rust.

## Demarrage en dev

```bash
cd frontend
npm install
npm run dev
```

Le serveur Vite demarre sur `http://localhost:5173` et proxifie `/api/*` vers `http://localhost:3000` (le backend Rust doit tourner en parallele).

## Build production

```bash
npm run build
```

Les fichiers compiles sont dans `dist/` et seront servis par le binaire Rust via `STATIC_DIR`.

## Composants

| Composant           | Role                                                         |
|---------------------|--------------------------------------------------------------|
| `App.svelte`        | Layout principal, navigation liste/detail/ajout, notifications |
| `ServiceList`       | Liste des services avec etat vide                            |
| `ServiceCard`       | Carte d'un service (nom, chemin, toggle, badge MOCK/PROXY)   |
| `ServiceDetail`     | Vue detail avec edition, suppression, gestion des regles     |
| `ServiceForm`       | Formulaire ajout/edition de service                          |
| `RuleList`          | Liste ordonnee des regles (drag-and-drop + boutons clavier)  |
| `RuleForm`          | Formulaire de regle (conditions ET/OU + reponse)             |
| `ConditionForm`     | Formulaire inline de condition (6 sources x 4 operateurs)    |
| `ResponseEditor`    | Editeur visuel de reponse (fragments, headers, chaos mode)   |
| `ToggleSwitch`      | Interrupteur ON/OFF accessible (role="switch", aria-checked) |
| `StatusBadge`       | Badge MOCK/PROXY                                             |
| `Notification`      | Bandeau de feedback (role="alert", aria-live)                |

## Accessibilite (RGAA AA)

- Skip link vers le contenu principal
- Tous les controles ont des labels visibles et `aria-describedby`
- Interrupteurs avec `role="switch"` et `aria-checked`
- Feedbacks avec `role="alert"` et `aria-live="assertive"`
- Ordre de tabulation logique, `focus-visible` sur tous les elements interactifs
- Contrastes >= 4.5:1
- Drag-and-drop avec boutons alternatifs "Monter / Descendre" pour le clavier
