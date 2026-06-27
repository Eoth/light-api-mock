# lightMock — Handoff de reprise

## 1. Objectif

Mock & Proxy HTTP intelligent pour Kubernetes. Un seul binaire Rust (Axum) sert une UI Svelte 5 et intercepte les requetes HTTP pour les mocker ou les proxyfier vers le vrai backend, configurable en temps reel. Chaque service est namespace sous `/{name}/{path}`.

## 2. Architecture

```
Backend: Rust (Axum 0.7) — port 7342
  src/main.rs              — bootstrap, shutdown graceful, auth init, script engine init
  src/models/mod.rs        — Service, Rule, RuleAction(Mock|Proxy), Condition, MockResponse, FakeKind, ChaosConfig, Group, WsdlMode
  src/auth/mod.rs          — AuthConfig, role helpers (can_access_service, can_manage_group, visible_services)
  src/auth/keycloak.rs     — KeycloakClient: ROPC login, JWT validation (JWKS cache 5min), token refresh, userinfo fallback
  src/auth/middleware.rs   — AuthUser, auth_middleware (inject into extensions), extract_user
  src/engine/matcher.rs    — first-match rules engine (method + sub_path + all_of/any_of conditions), match_path()
  src/engine/proxy.rs      — HTTP forwarding streaming (reqwest, BodyStream, zero buffering)
  src/engine/renderer.rs   — body fragment rendering + chaos injection + 19 FakeKind
  src/engine/template.rs   — template engine: {{var}}, {{var | pipe}}, 12 pipes, script vars
  src/engine/script.rs     — ScriptEngine (rhai sandboxe), ScriptResult, ScriptContext
  src/store/mod.rs         — YAML persistence (Arc<RwLock<Arc<MockConfig>>>, atomic write, snapshot O(1))
  src/server/mod.rs        — Router: /api/* (API REST) + fallback (ServeDir + intercept + auth middleware)
  src/server/api.rs        — CRUD services + groups, auth endpoints (login/validate/me), health, authorization
  src/server/intercept.rs  — middleware: is_internal_route → skip, WSDL bypass configurable, match service → per-rule action (mock or proxy)
  src/server/validation.rs — validate_service: reserved names (incl. auth), duplicate rule names, method validation
  src/server/request_log.rs — in-memory FIFO (200 entries)

Frontend: Svelte 5 + Vite 6 (SPA servie statiquement)
  frontend/src/lib/auth.svelte.js      — auth state reactif ($state), login/logout/persist/restore
  frontend/src/lib/tpl-utils.js        — SOURCE UNIQUE de verite pour le format template (serialisation, deserialisation, validation, preview)
  frontend/src/lib/api.js              — client REST avec auth token injection, endpoints auth/groups
  frontend/src/lib/components/         — 18 composants Svelte 5 ($state, $derived, $effect, $props)
  frontend/src/tests/                  — 9 fichiers de tests Vitest
  frontend/e2e/security.spec.js        — 17 tests Playwright

K8s: namespace entreprise-tools, Gloo Edge, PVC 64Mi, readOnlyRootFilesystem, runAsNonRoot
```

## 3. Decisions importantes

- **API create vs update** : `POST /api/services` (create, 409 si doublon) / `PUT /api/services/:name` (update, 404 si inexistant). Jamais d'upsert.
- **RuleAction** : chaque regle a un champ `action: mock|proxy` (default mock, backward-compatible via serde default). Quand `is_mocked=false` au niveau service, c'est proxy direct sans evaluer les regles.
- **Format template** : `{` et `}` sont des accolades JSON/XML litterales. `{{expr}}` est une variable template, `{{expr | pipe}}` avec transformation. Le frontend ne valide JAMAIS comme du JSON standard — il utilise `templateToTestJson()` qui comprend cette grammaire.
- **Source de verite pendant l'edition** : les Fields JS `[{key, fieldType, source, value, pipe, asNumber}]`. Le template string est un format de serialisation genere uniquement a la sauvegarde.
- **tpl-utils.js** : module unique partage entre JsonResponseBuilder, XmlResponseBuilder et RuleForm. Toute logique de parsing/validation/serialisation passe par ce module. Ne jamais re-dupliquer ces fonctions dans les composants.
- **Routes internes protegees** : `/`, `/index.html`, `/api/*`, `/assets/*`, `/favicon.ico` — aucun service utilisateur ne peut les capturer.
- **Noms reserves** : `api`, `auth`, `index.html`, `assets`, `favicon.ico` — refuses a la creation de service.
- **Mode sombre** : CSS vars `[data-theme="dark"]`, toggle + localStorage + media query.
- **Auth Keycloak (desactivable)** : `AUTH_ENABLED=true|false`. Quand desactive, le middleware injecte un `AuthUser::anonymous()` avec full access — zero changement comportemental.
- **Groupes internes** : `Group { name, admins, members }` stocke dans le YAML. Chaque service a un `group_name: Option<String>`. Tout le monde peut creer un groupe (le createur est automatiquement admin). Les admins du groupe peuvent modifier/supprimer le groupe et gerer les membres. Le super-admin peut tout controler. Unicite du nom case-insensitive.
- **Auth middleware** : implemente comme middleware Axum (pas extracteur `FromRequestParts`) a cause d'une incompatibilite edition 2024/axum-core 0.4.5. L'`AuthUser` est injecte dans les extensions et extrait via `Extension<AuthUser>` dans les handlers.
- **Health endpoint** : `GET /api/health` exempt d'auth, utilise par les probes K8s.
- **Store Arc snapshot** : `Arc<RwLock<Arc<MockConfig>>>` — snapshot O(1) sans deep clone, optimal pour les lectures dans l'intercept.
- **Proxy streaming** : corps request et response transferes en streaming (zero buffering), via `BodyStream` + `bytes_stream()` + `Body::from_stream()`.
- **Methode HTTP sur Rule** : retiree du Service, ajoutee sur Rule avec `method` (defaut "ANY") + `sub_path` optionnel. Le service matche sur le path seul.
- **listen_path optionnel** : si vide, `build_effective_pattern` genere `/{name}/*` (catch-all). Securite assuree par noms reserves + `is_internal_route()`.
- **WSDL configurable** : `WsdlMode` enum (Auto|Proxy|Mock) sur Service. Auto/Proxy = bypass vers le vrai backend. Mock = appliquer les regles.
- **Scripts rhai** : champ `script: Option<String>` sur Rule. Engine sandboxe (10K ops, 1MB strings). Resultat accessible via `{{script}}` ou `{{script.champ}}`.
- **UI groupee** : les services s'affichent dans des accordeons par `group_name`. RGAA AA : `aria-expanded`, `role="region"`, clavier. Recherche ouvre les groupes correspondants.
- **CSS centralise** : les classes `.btn`, `.form-field`, `.form-error`, `.badge`, `.modal-*` sont definies uniquement dans `app.css`. Les composants ne les redefinissent plus. Variables mappees vers les tokens Aurora/ZAC via `tokens-aurora.css` importe dans `main.js`.
- **Composants reutilises** : `ToggleSwitch.svelte` est le seul toggle (pas de HTML inline). Les toggles script et chaos dans RuleForm utilisent ce composant.
- **Permissions groupes** : tout le monde peut creer un groupe (le createur est auto-admin). Les admins du groupe peuvent modifier/supprimer le groupe et gerer les membres. Le super-admin controle tout. Unicite du nom case-insensitive.

## 4. Etat d'avancement

**Tout fonctionne** : 162 tests backend + 107 tests frontend = 269 tests verts. Zero warning Svelte. Build frontend : 41Ko CSS + 150Ko JS (gzip 55Ko total).

**Toutes les evolutions planifiees (A-G) sont implementees** + groupes de services + path optionnel + WSDL configurable + tokens Aurora/ZAC + composants reutilises + doc rhai + breadcrumb RGAA + serde sans retrocompat + `untrack()` idiomatique.

## 5. Points sensibles a ne pas casser

1. **`tpl-utils.js`** — ne jamais re-dupliquer de logique template dans les composants.
2. **`templateToTestJson`** — remplace `{{var}}` par `"__var__"` pour valider le JSON. Respecte le contexte `inString`.
3. **`is_internal_route()`** dans intercept.rs — verifie AVANT le matching des services.
4. **`match_path`** — retourne `None` si pattern_segs est vide (securite).
5. **POST vs PUT** — creation = POST, modification = PUT. Ne pas revenir a un upsert.
6. **RuleAction default** — `#[serde(default)]` garantit la retrocompatibilite YAML.
7. **Mode switch** — les boutons de mode utilisent `{#key modeKey}` pour forcer le re-render.
8. **Auth middleware vs extracteur** — NE PAS utiliser `FromRequestParts` (incompatibilite edition 2024/axum-core 0.4.5).
9. **AUTH_ENABLED=false** — le middleware retourne `AuthUser::anonymous()` avec `is_super_admin: true`.
10. **Groups/group_name/wsdl_mode** — `#[serde(default)]` sur tous. Les YAML existants deserialisent correctement.
11. **Store Arc** — `snapshot()` retourne `Arc<MockConfig>` (clone O(1)). Les callers API utilisent `.iter().find().cloned()` au lieu de `.into_iter()`.
12. **Proxy streaming** — NE PAS re-introduire de `to_bytes()` ou `.bytes()` dans proxy.rs. Le streaming est critique pour la tenue en charge sous 64Mo.
13. **build_effective_pattern** — si listen_path vide → `/{name}/*`. Ne pas casser ce comportement catch-all.
14. **CSS centralise dans app.css** — NE PAS redefinir `.btn`, `.form-field`, `.form-error`, `.badge`, `.modal-*` dans les composants Svelte. Tout passe par `app.css`.
15. **Groupes ouverts a tous** — `create_group` n'a PAS de `require_super_admin`. Le createur est auto-admin. `update_group` et `delete_group` verifient `can_manage_group`.
16. **Pas de retrocompat serde** — pas de `#[serde(default)]` sur les champs obligatoires (Service, Rule). Les YAML doivent fournir TOUS les champs. Pas de version deployee a supporter.
17. **Breadcrumb RGAA** — `<nav aria-label="Fil d'Ariane">` avec `<ol>` et `aria-current="page"` sur la page active. Visible sur toutes les vues secondaires (logs, groupes, ajout, detail).
18. **Commentaires maintenabilite** — chaque fichier critique a un commentaire en tete expliquant son role et ses dependances. Cible : quelqu'un qui ne connait ni Rust ni Svelte puisse trouver ou modifier quoi.
19. **Zero warning Svelte** — les `state_referenced_locally` sont resolus avec `untrack()` de Svelte (pas de `svelte-ignore`). `untrack()` lit la prop une fois sans creer de souscription reactive. Le build doit produire ZERO warning.

## 6. Tests en place

| Fichier | Nb | Contenu |
|---------|-----|---------|
| `src/engine/template.rs` | 37 | Pipes, fake data, round-trip JSON, variables, syntaxe `{{}}` |
| `src/engine/matcher.rs` | 17 | Conditions, first-match, operateurs, method, sub_path |
| `src/engine/renderer.rs` | 16 | Fragments, chaos, fake data |
| `src/engine/script.rs` | 6 | Script execution, context, map/string results |
| `src/engine/proxy.rs` | 4 | Client creation, forward, URL construction |
| `src/server/intercept.rs` | 22 | Matching, securite, proxy path, WSDL bypass, catch-all |
| `src/server/validation.rs` | 14 | Noms reserves, paths, doublons regles, method validation |
| `src/models/mod.rs` | 12 | Serde round-trip, groups, WsdlMode, deserialization complete |
| `src/store/mod.rs` | 9 | Persistence, atomic write, concurrence, Arc snapshot |
| `src/auth/mod.rs` | 11 | Config, role resolution, access control, visible_services, group permissions |
| `src/auth/keycloak.rs` | 6 | URL construction, token parsing, trailing slash |
| `frontend/src/tests/tpl-utils.test.js` | 55 | Round-trip Fields<->Template, validation JSON/XML, preview, buildExpr, script |
| `frontend/src/tests/ServiceForm.test.js` | 10 | Validation noms reserves/doublons, path optionnel |
| `frontend/src/tests/ServiceList.test.js` | 9 | Groupes, recherche, filtrage par groupe |
| `frontend/src/tests/RuleForm.test.js` | 5 | Unicite noms de regle |
| `frontend/src/tests/ServiceCard.test.js` | 8 | Affichage, toggle, badge |
| `frontend/src/tests/RuleList.test.js` | 7 | Drag-drop, boutons, badges |

## 7. Outils et pieges a eviter

| Piege | Solution |
|-------|----------|
| `cargo` non dans le PATH | Utiliser `& "$env:USERPROFILE\.cargo\bin\cargo.exe"` en PowerShell |
| `fireEvent.input` ne met pas a jour `$state` en Svelte 5 | Setter `el.value = x` puis `fireEvent.input(el)` |
| `fireEvent.click` sur submit button bloque par `required` | Utiliser `fireEvent.submit(container.querySelector('form'))` |
| Radios natifs desynchronises apres annulation de mode | Utiliser des `<button>` avec `role="radio"` + `{#key modeKey}` |
| `replace_all: true` dans Edit | DANGEREUX si la chaine apparait plusieurs fois |
| Warnings Svelte `state_referenced_locally` | Benin, pattern `const initial = rule` est intentionnel |
| `FromRequestParts` en edition 2024 | Incompatible avec axum-core 0.4.5, utiliser middleware + Extension |
| `{nom}` dans les attributs Svelte | Svelte l'interprete comme une expression, utiliser backticks ou concaténation |
| SmartString + String add conflict avec rhai | Utiliser `format!()` au lieu de l'operateur `+` pour concatener |

## 8. Evolutions futures

### CSS Design System
Le CSS est actuellement duplique dans chaque composant (~8 fichiers repetent `.btn`, `.btn-primary`, `.form-field`, etc.). Un design system CSS sera fourni pour centraliser tout dans `app.css`. Les composants utiliseront les classes globales au lieu de les redefinir localement.

### Composants partages
Plusieurs patterns HTML sont repetes 40+ fois et pourraient etre extraits :
- `FormField.svelte` — label + input + hint
- `ConfirmDialog.svelte` — dialog de confirmation suppression
- `RemovableList.svelte` — liste avec boutons de suppression

### Documentation maintenabilite
Pour les equipes qui ne connaissent ni Rust ni Svelte :
- Ajouter des commentaires "WHY" dans le code critique
- Documenter les patterns Svelte 5 ($state, $derived, $effect, $props)
- Enrichir les hints rhai dans l'UI (liens vers doc, exemples concrets)

### Support MOM / Messaging (Etude de faisabilite)

**Kafka** — Faisable via la crate `rdkafka` (wrapper librdkafka). Un consumer Kafka ecoute un topic, matche les messages contre les regles, et publie la reponse mockee sur un topic de reponse. Impact : binaire plus gros (~2-5Mo), dep native (librdkafka), config supplementaire (KAFKA_BROKERS, topics).

**JMS** — Pas de crate Rust mature pour JMS. JMS est un protocole Java (Jakarta EE). Options : (1) utiliser le protocole AMQP sous-jacent via `lapin` (crate AMQP Rust), (2) wrapper Java via JNI (complexe), (3) simplement documenter que JMS n'est pas supporte nativement et recommander un bridge HTTP→JMS.

**Mail (SMTP)** — Faisable via la crate `lettre` pour l'envoi et `mailin` ou `smtp-server` pour la reception. Un mock SMTP ecoute un port, intercepte les mails entrants, et les logge. Impact modere.

**Recommandation** : Commencer par Kafka (le plus demande en entreprise), documenter JMS comme non-supporte, et evaluer le mail en phase 2.

### Stockage hautes performances
Le stockage actuel (YAML fichier + Arc<RwLock<Arc<MockConfig>>>) est rapide en lecture (snapshot O(1)) mais l'ecriture atomique sur disque via rename est bloquante. Pour des volumes tres importants :
- **Option 1** : Migrer vers SQLite (via `rusqlite` ou `sqlx`). Lectures/ecritures microseconde, WAL mode pour concurrence. Inconvenient : complexite schema.
- **Option 2** : Migrer vers `sled` (BTree embarque Rust, zero-dep). Microseconde en lecture/ecriture, crash-safe. Inconvenient : API bas niveau.
- **Option 3** : Garder le YAML mais deplacer l'ecriture dans un channel async (write-behind). Les mutations sont appliquees en memoire instantanement et persistees en arriere-plan. Le plus simple et suffisant pour le use case actuel.
