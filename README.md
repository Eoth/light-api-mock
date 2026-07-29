# lightMock

Mock & Proxy Intelligent pour environnements Kubernetes.

Un seul binaire Rust qui intercepte les requetes HTTP, les mock ou les proxifie vers le vrai backend, configurable en temps reel via une interface web. Chaque service est expose sous `/{service_name}/...` (ou `/{group_code}/{service_name}/...` s'il appartient a un groupe), sans redemarrage de pod.

## Fonctionnalites

- **Namespace URL par service** : chaque service est expose sous `/{name}/{listen_path}`, pas de collision
- **Methode HTTP par regle** : chaque regle d'un service definit sa propre methode (GET, POST, PUT, ...) et un `sub_path` optionnel — le service lui-meme matche sur le path seul
- **Bascule Mock / Proxy** : au niveau service (toggle ON/OFF) et/ou au niveau regle (`action: mock|proxy`, mock partiel)
- **Moteur de regles** : conditions combinables (ET/OU) sur path params, query, headers, body JSON/XML/form
- **Testeur de regle et detecteur de conflits** : `POST /api/rule-test` rejoue un brouillon de regle contre une requete deja capturee (detail par condition, hints cross-source) ; `POST /api/rule-conflicts` avertit (non-bloquant) si une regle chevauche une regle existante a la sauvegarde
- **Templates dynamiques** : expressions `{{path.siret}}`, `{{fake.CompanyName}}`, `{{now_ms}}`, pipes `| first(9)`, `| upper`, `| replace("a","b")`, `| capitalize`, `| substr(0,5)`, `| length`, `| prepend("x")`, `| append("x")`
- **Scripts Rhai** : jusqu'a 3 blocs par regle (`pre_script`/`script`/`post_script`, independants), fonctions natives dont des generateurs deterministes par seed (`seeded_int`, `seeded_pick`) et des dates formattables (`date_now`, `date_past`, `date_future`) — voir [Scripts Rhai](#scripts-rhai)
- **Groupes de services** : regroupement visuel (accordeons) + prefixe d'URL optionnel (`/{code}/...`), gestion des permissions (admins/membres)
- **SOAP/WSDL** : mode par service (`Auto`/`Proxy`/`Mock`) pour choisir si les requetes WSDL bypassent le mock ou non
- **Mode Chaos** : injection de latence (fixe ou plage) et d'erreurs HTTP
- **Journal des requetes** : historique consultable dans l'IHM avec filtre par service
- **Ping de disponibilite** : test de connexion TCP a la demande vers `real_target_url` (jamais de requete HTTP fonctionnelle)
- **Sauvegardes et restauration** : rotation automatique des backups YAML + restauration depuis l'UI (voir [Sauvegardes et rollback](#sauvegardes-et-rollback))
- **Auth Keycloak (optionnelle)** : desactivee par defaut ; une fois activee, roles et permissions par groupe de services
- **Kafka (optionnel, feature `messaging-kafka`)** : mock/proxy applique aussi aux messages Kafka, non compile par defaut
- **Import / Export / Reset** : sauvegarde, restauration et reinitialisation de la configuration
- **Mode sombre** : theme clair/sombre (preference navigateur ou toggle manuel)
- **Zero dependance externe obligatoire** : pas de base de donnees, persistance fichier YAML (ecriture asynchrone en arriere-plan, mutation en memoire instantanee)
- **Interface accessible** : conformite RGAA niveau AA

## Sécurité et confidentialité

Section volontairement visible et explicite : lightMock est destiné à être déployé derrière un
pare-feu d'entreprise, souvent par des équipes qui doivent justifier chaque flux réseau sortant.

### Comportement réseau — exhaustif, rien de caché

**Aucune télémétrie.** lightMock n'envoie jamais de métriques d'usage, de rapport de crash, de
statistiques de version ou toute autre donnée vers un serveur de l'éditeur — il n'existe
d'ailleurs aucun serveur de l'éditeur : le logiciel est un binaire autonome, sans "phone home".

Le binaire n'émet du trafic réseau sortant que dans exactement 4 cas, tous **déclenchés par une
action explicite de l'utilisateur ou une configuration qu'il a lui-même renseignée** :

| Flux | Déclencheur | Portée | Détail |
|---|---|---|---|
| Test de connexion ("ping") | Clic manuel sur "Tester la cible" | `real_target_url` du service, configuré par l'utilisateur | **Connexion TCP pure** (`tokio::net::TcpStream::connect`, timeout 3s) — jamais de requête HTTP (pas de GET/HEAD), jamais de handshake TLS applicatif. Un cache en mémoire (TTL 2 min) évite même de répéter ce test TCP à chaque clic. |
| Proxy | Une requête entrante correspond à un service configuré en proxy (`is_mocked=false` ou règle `action=proxy`) | `real_target_url` du service concerné, configuré par l'utilisateur — **jamais une autre destination** | Requête HTTP forwardée telle quelle (streaming, sans buffering) vers la cible choisie par l'utilisateur pour ce service précis. Aucun service sans cible configurée ("purement mocké") ne déclenche jamais de proxy. |
| Authentification Keycloak | Uniquement si `AUTH_ENABLED=true` (désactivé par défaut) | `KEYCLOAK_URL` configuré par l'utilisateur | Login (ROPC), validation JWT (JWKS), rafraîchissement de token — vers le serveur Keycloak que l'utilisateur a lui-même renseigné. |
| Kafka (messaging) | Uniquement si le binaire est compilé avec `--features messaging-kafka` (**non activée par défaut**, `cargo build`/`cargo test` standards ne téléchargent ni ne compilent la dépendance Kafka) ET `KAFKA_ENABLED=true` | `KAFKA_BROKERS` configuré par l'utilisateur | Consumer/producer vers les brokers renseignés par l'utilisateur. |

**Aucun autre appel réseau sortant n'existe dans le code** — vérifiable directement : les seuls
usages de `reqwest::Client`/`TcpStream::connect` en dehors des tests se trouvent dans
`src/auth/keycloak.rs` (Keycloak), `src/engine/proxy.rs` (proxy + ping TCP) et `src/messaging/`
(Kafka, feature-gated). Aucun appel vers un domaine en dur dans le code.

### Signaler une vulnérabilité

Voir [SECURITY.md](SECURITY.md) — signalement privé via GitHub Security Advisories, délais de
réponse visés, périmètre couvert.

### Chaîne d'approvisionnement (supply chain)

**Build reproductible** : `Cargo.lock` et `frontend/package-lock.json` sont commités et
versionnés — un `cargo build`/`npm ci` reproduit exactement le même graphe de dépendances.

**Aucun pipeline CI/CD n'est fourni avec ce projet à ce jour** ; les commandes ci-dessous sont
donc à exécuter manuellement (ou à intégrer dans le pipeline CI de votre choix une fois mis en
place — voir la note en tête de chaque commande).

<a id="audit-des-dépendances"></a>
**Audit des dépendances connues (CVE)** :

```bash
# Backend (Rust)
cargo install cargo-audit
cargo audit

# Frontend (npm)
cd frontend && npm audit
```

**Génération d'un SBOM (Software Bill of Materials)** — format CycloneDX pour les deux piles :

```bash
# Backend (Rust) — CycloneDX
cargo install cargo-cyclonedx
cargo cyclonedx --format json
# Alternative SPDX, sans compiler le projet (lecture de Cargo.lock uniquement) :
#   cargo install cargo-sbom && cargo sbom > sbom.spdx.json

# Frontend (npm) — CycloneDX
cd frontend && npx @cyclonedx/cyclonedx-npm --output-file sbom.json
```

**Scan de vulnérabilités de l'image Docker** ([Trivy](https://aquasecurity.github.io/trivy/)) :

```bash
docker build -t lightmock:local .
trivy image lightmock:local
```

**Image Docker** : build multi-stage, image finale `alpine` (base minimale, pas d'outillage de
build résiduel), utilisateur non-root dédié (`app`, uid 1000, jamais `root`) — voir
[Dockerfile](Dockerfile). Les manifests Kubernetes fournis (`k8s/deployment.yaml`) renforcent
encore la posture : `runAsNonRoot`, `readOnlyRootFilesystem`, `allowPrivilegeEscalation: false`,
toutes les capabilities Linux retirées (`drop: [ALL]`).

## Prerequis

| Outil    | Version min | Notes |
|----------|-------------|-------|
| Rust     | 1.85+       | Edition 2024, toolchain MSVC sur Windows |
| Node.js  | 20+         | Pour le frontend Svelte |
| npm      | 9+          | |
| cmake + toolchain C | - | Uniquement pour compiler avec `--features messaging-kafka` (librdkafka est compilee depuis les sources) |

> **Setup automatise** : voir [scripts/bootstrap-windows.ps1](scripts/bootstrap-windows.ps1) ou [scripts/bootstrap-linux.sh](scripts/bootstrap-linux.sh)

## Demarrage rapide

### Bootstrap automatique

```powershell
# Windows PowerShell
.\scripts\bootstrap-windows.ps1
```

```bash
# Linux / macOS
chmod +x scripts/bootstrap-linux.sh && ./scripts/bootstrap-linux.sh
```

### Demarrage manuel

```bash
# 1. Frontend
cd frontend && npm install && npm run build && cd ..

# 2. Backend
cargo build --release

# 3. Lancer
# PowerShell :
$env:STATIC_DIR = "frontend/dist"; $env:DATA_PATH = "data"; .\target\release\light-mock.exe

# Bash :
STATIC_DIR=./frontend/dist DATA_PATH=./data ./target/release/light-mock
```

Ouvrir http://localhost:7342

### Dev frontend (hot-reload)

Terminal 1 : `DATA_PATH=./data ./target/release/light-mock`
Terminal 2 : `cd frontend && npm run dev` → http://localhost:5173

## Concept cle : URL namespace

Chaque service est expose sous **`/{name}/{listen_path}`** (ou **`/{group_code}/{name}/{listen_path}`** s'il appartient a un groupe). La methode HTTP n'est **pas** fixee au niveau du service : elle est definie par chaque regle (`Rule.method` + `Rule.sub_path` optionnel), ce qui permet a un meme service de repondre a plusieurs methodes/sous-chemins.

| Service name | listen_path | Regle : method + sub_path | URL finale de test |
|---|---|---|---|
| `insee` | `/v4/sirene/{siret}` | `GET`, sub_path vide | `GET /insee/v4/sirene/{siret}` |
| `auth` | `/login` | `POST`, sub_path vide | `POST /auth/login` |
| `users` | *(vide → catch-all `/*`)* | `GET`, sub_path vide | `GET /users/anything` |

En mode proxy, le prefixe `/{name}` (et `/{group_code}` le cas echeant) est strippe avant forward vers le vrai backend. Si `listen_path` est vide, un catch-all `/{name}/*` est genere automatiquement.

## API REST

| Methode | Endpoint | Description |
|---|---|---|
| GET | `/api/health` | Sonde de sante (exempt d'auth, utilise par K8s) |
| GET | `/api/config` | Configuration complete |
| PUT | `/api/config` | Remplacer toute la config |
| DELETE | `/api/config/reset` | Reinitialiser (supprime tous les services, super-admin requis) |
| GET | `/api/config/backups` | Liste des sauvegardes disponibles (super-admin requis) |
| POST | `/api/config/restore/:filename` | Restaurer une sauvegarde (super-admin requis) |
| GET | `/api/services` | Liste des services |
| POST | `/api/services` | Creer un service (409 si le nom existe deja dans le meme perimetre) |
| GET / PUT / DELETE | `/api/services/:name` | Detail / modification / suppression d'un service **sans groupe** |
| GET / PUT / DELETE | `/api/groups/:group/services/:name` | Idem, pour un service **appartenant a un groupe** |
| PUT | `/api/services/:name/toggle` (ou variante groupee) | Basculer mock/proxy |
| POST | `/api/services/:name/ping` (ou variante groupee) | Test de connexion TCP vers `real_target_url` |
| PUT | `/api/services/:name/rules/reorder` (ou variante groupee) | Reordonner les regles |
| POST | `/api/script/validate` | Valider la syntaxe d'un script Rhai (script/pre_script/post_script) |
| POST | `/api/rule-test` | Tester un brouillon de regle contre une requete deja capturee (stateless, detail par condition) |
| POST | `/api/rule-conflicts` | Detecter un chevauchement entre un brouillon de regle et les autres regles du service (stateless, avertissement non-bloquant) |
| GET | `/api/logs?limit=50` | Journal des requetes |
| GET / POST | `/api/groups` | Liste / creation d'un groupe de services |
| GET / PUT / DELETE | `/api/groups/:name` | Detail / modification / suppression d'un groupe |
| PUT | `/api/groups/:name/members` | Gestion des membres/admins d'un groupe |
| POST | `/api/auth/login`, `/api/auth/validate`, GET `/api/auth/me`, `/api/auth/status` | Authentification Keycloak (no-op si `AUTH_ENABLED=false`) |
| GET | `/api/messaging/status`, `/api/messaging/logs`, POST `/api/messaging/simulate` | Uniquement si compile avec `--features messaging-kafka` |

> Un service identifie par son nom seul (`/api/services/:name`) est toujours scope au perimetre "sans groupe" — un service qui appartient a un groupe doit etre adresse via `/api/groups/:group/services/:name`.

### Exemple : creer un service avec une regle mockee

```bash
curl -X POST http://localhost:7342/api/services \
  -H "Content-Type: application/json" \
  -d '{
    "name": "demo",
    "listen_path": "/v1/{id}",
    "real_target_url": "http://httpbin.org",
    "is_mocked": true,
    "rewrite_directory_urls": false,
    "group_name": null,
    "wsdl_mode": "auto",
    "rules": [{
      "name": "hello",
      "method": "GET",
      "sub_path": null,
      "action": "mock",
      "pre_script": null,
      "script": null,
      "post_script": null,
      "conditions": { "all_of": [], "any_of": [] },
      "response": {
        "status": 200,
        "headers": [{"name": "Content-Type", "value": "application/json"}],
        "body": [{"type": "Template", "template": "{\"id\":\"{{path.id}}\",\"message\":\"Hello from lightMock!\"}"}],
        "chaos": null
      }
    }]
  }'

# Tester : GET /demo/v1/anything
curl http://localhost:7342/demo/v1/anything
```

`Service` n'a pas de champ `method` : chaque regle definit la sienne. Tous les champs listes ci-dessus sont obligatoires (pas de valeur par defaut cote serveur) — mettre `null`/`~` explicitement pour les champs optionnels non utilises (`sub_path`, `group_name`, scripts).

## Variables d'environnement

| Variable | Defaut | Description |
|---|---|---|
| `DATA_PATH` | `./data` | Repertoire du fichier `mock-config.yaml` |
| `STATIC_DIR` | `./frontend/dist` | Assets Svelte compiles |
| `PORT` | `7342` | Port d'ecoute HTTP |
| `RUST_LOG` | `light_mock=info` | Filtre de logs (ex: `light_mock=debug`) |
| `BACKUP_MAX_COUNT` | `5` | Nombre de sauvegardes conservees dans `{DATA_PATH}/backups/` avant rotation |
| `AUTH_ENABLED` | `false` | Active l'authentification Keycloak. Si `true`, `KEYCLOAK_URL`/`KEYCLOAK_REALM`/`KEYCLOAK_CLIENT_ID` deviennent obligatoires (le demarrage echoue sinon). |
| `KEYCLOAK_URL` | *(vide)* | URL du serveur Keycloak (requis si `AUTH_ENABLED=true`). |
| `KEYCLOAK_REALM` | *(vide)* | Realm Keycloak (requis si `AUTH_ENABLED=true`). |
| `KEYCLOAK_CLIENT_ID` | *(vide)* | Client ID Keycloak (requis si `AUTH_ENABLED=true`). |
| `SUPER_ADMINS` | *(vide)* | Liste d'identifiants (CSV) ayant le role super-admin (reset complet, restauration de backups). |
| `SHOW_RESET_BUTTON` | `false` | Affiche le bouton "Reset complet" dans l'UI quand `AUTH_ENABLED=false`. **N'est pas une mesure de securite** : `require_super_admin()` reste la seule autorite reelle cote serveur ; ce flag ne pilote que l'affichage. |
| `KAFKA_ENABLED` | `false` | Active le consumer Kafka au demarrage. Sans effet si le binaire n'est pas compile avec `--features messaging-kafka`. |
| `KAFKA_BROKERS` | *(vide)* | Liste de brokers Kafka separes par des virgules (ex: `broker1:9092,broker2:9092`). |
| `KAFKA_CONSUMER_GROUP` | `lightmock` | Consumer group Kafka utilise pour ecouter `KAFKA_LISTEN_TOPIC`. |
| `KAFKA_LISTEN_TOPIC` | *(vide)* | Topic Kafka ecoute par le consumer. |
| `KAFKA_REPLY_TOPIC` | *(vide, optionnel)* | Topic sur lequel publier la reponse mockee rendue. Si absent, aucune publication n'est tentee. |
| `MESSAGE_LOG_TTL_MS` | `86400000` (24h) | Duree de retention des entrees du journal des messages Kafka avant purge. |
| `MESSAGE_LOG_MAX_BODY_SIZE` | `16384` (16 Ko) | Taille au-dela de laquelle le corps d'un message est tronque dans le journal (les metadonnees restent completes). |

## Sauvegardes et rollback

Avant chaque mutation de la configuration, l'ancien contenu de `mock-config.yaml` est copie
(de facon synchrone) dans `{DATA_PATH}/backups/` (rotation automatique, `BACKUP_MAX_COUNT`
fichiers conserves). Avant un `DELETE /api/config/reset`, une sauvegarde supplementaire est
creee dans `{DATA_PATH}/backups/protected/` : elle n'est **jamais** supprimee par la rotation
normale, seulement au bout de 30 jours (verifie a la premiere ecriture suivante, pas de tache
planifiee). L'ecriture disque elle-meme est asynchrone (write-behind) : une mutation est
appliquee en memoire instantanement, puis persistee en arriere-plan.

**Restauration via l'UI/API** (recommande) : le bouton "Sauvegardes" liste les fichiers
disponibles et permet une restauration en un clic (`GET /api/config/backups` +
`POST /api/config/restore/:filename`, reserves aux super-admins). Une restauration cree
automatiquement un nouveau backup de l'etat ecrase juste avant, donc reversible.

**Rollback manuel** (sans UI) : arreter le service (ou agir entre deux ecritures), copier le
fichier choisi depuis `backups/` ou `backups/protected/` par-dessus `mock-config.yaml`, puis
redemarrer le service.

## Tests

```bash
# Rust (262 tests par defaut)
cargo test

# Rust + Kafka (feature optionnelle, necessite cmake + toolchain C, +31 tests)
cargo test --features messaging-kafka messaging::

# Frontend unitaires (211 tests Vitest)
cd frontend && npm test

# E2E navigateur (77 tests Playwright, serveur doit tourner)
cd frontend && npm run test:e2e
```

## Architecture

```
light-mock/
  src/
    models/        # Service, Rule, RuleAction, Group, Condition, BodyFragment, FakeKind, ChaosConfig, WsdlMode
    engine/        # matcher, proxy, renderer, template (expressions + 12 pipes), script (moteur Rhai)
    auth/          # AuthConfig, client Keycloak, middleware
    messaging/     # Kafka (matcher, consumer, journal) — feature "messaging-kafka" uniquement
    store/         # Persistance YAML (Arc<RwLock<Arc<>>>, write-behind, backups/restauration)
    server/        # Axum : API REST, intercept middleware, request_log, ping, validation
  frontend/
    src/lib/
      tpl-utils.js        # Module partage : serialisation/validation/conversion templates
      rhai-functions.js   # Module partage : fonctions natives Rhai (doc + autocompletion)
      service-url.js      # Module partage : construction de l'URL de test d'un service
      api.js               # Client API REST
      components/          # Composants Svelte 5 (25 composants)
    src/tests/       # Tests unitaires Vitest
    e2e/             # Tests Playwright
  k8s/             # Manifests K8s + Gloo Edge
  scripts/         # Bootstrap Windows / Linux
  Dockerfile       # Build multi-stage
```

## Format template

Le moteur de templates lightMock utilise une syntaxe propre pour generer des reponses
dynamiques. Les accolades simples `{` `}` sont des caracteres litteraux (utiles pour du JSON/XML
brut) ; les doubles accolades `{{ }}` delimitent une expression evaluee au runtime :

| Syntaxe | Signification | Exemple |
|---|---|---|
| `{` / `}` | Accolade JSON/XML litterale | `{"key":"value"}` reste tel quel |
| `{{variable}}` | Expression evaluee au runtime | `{{path.siret}}` → `44306184100047` |
| `{{variable \| pipe}}` | Variable avec transformation | `{{path.siret \| first(9)}}` → `443061841` |

**Variables disponibles** : `path.X`, `query.X`, `header.X`, `body.X` (JSON pointer), `fake.Kind`,
`uuid`, `now_ms`, `now_iso`, `now_epoch`, `seq`, `script`/`script.X`, `pre_script`/`pre_script.X`,
`post_script`/`post_script.X` (ces trois derniers exposent le resultat des blocs de script de la
regle, voir [Scripts Rhai](#scripts-rhai)).

**Pipes** : `lower`, `upper`, `trim`, `capitalize`, `first(N)`, `last(N)`, `substr(start,len)`, `default("val")`, `replace("a","b")`, `prepend("x")`, `append("x")`, `length`

**Fake data** : `FirstName`, `LastName`, `Email`, `PhoneNumberFR`, `Integer{min,max}`, `CompanyName`, `StreetName`, `CityFR`, `PostcodeFR`, `Siren`, `Siret`, `FullAddressFR`, `DatePast`, `DateFuture`, `TimestampMs`, `BoolRandom`, `LoremSentence`, `CountryFR`, `IbanFR`

## Scripts Rhai

Chaque regle peut executer jusqu'a 3 blocs de script independants (pas de chainage entre eux,
meme contexte de requete pour chacun) : `pre_script`, `script`, `post_script`. Le sandbox limite
l'execution a 10K operations et 1 Mo de chaines. Le resultat de chaque bloc est accessible dans
le template via `{{script}}` / `{{pre_script}}` / `{{post_script}}` (valeur brute, si le script
retourne une chaine) ou `{{script.champ}}` (si le script retourne une map Rhai `#{...}`).
L'editeur de script dans l'IHM propose une autocompletion (`Ctrl+Espace` ou en tapant) sur ces
fonctions natives :

| Fonction | Description |
|---|---|
| `random_int(min, max)` | Entier aleatoire dans `[min, max]` |
| `now_ms()` | Timestamp courant en millisecondes |
| `now_iso()` | Date/heure courante au format `AAAA-MM-JJ` |
| `year()` | Annee courante |
| `uuid()` | UUID v4 |
| `fake("Kind")` | Donnee factice (memes types que le body builder, ex. `fake("Siret")`) |
| `date_now(format?)` | Date du jour ; `format` optionnel : `"iso"` (defaut, `AAAA-MM-JJ`), `"fr"` (`JJ/MM/AAAA`), `"en"` (`MM/JJ/AAAA`) |
| `date_past(jours, format?)` | Date dans le passe (`jours` avant aujourd'hui, borne a 0 si negatif/nul) |
| `date_future(jours, format?)` | Date dans le futur (`jours` apres aujourd'hui, borne a 0 si negatif/nul) |
| `seeded_int(seed, min, max)` | Entier deterministe dans `[min, max]` pour un `seed` donne (meme seed → meme valeur) |
| `seeded_pick(seed, [liste])` | Element deterministe de `liste` pour un `seed` donne |

`seed` accepte n'importe quel type Rhai (string, entier, booleen...), typiquement une donnee de
la requete (`request.path.siret`, `request.query.X`, `request.headers.X`). Utile pour mocker une
API type INSEE/SIRET ou un meme SIRET doit toujours renvoyer le meme resultat.

### Exemple : reponse deterministe par SIRET

Service `seeded-test`, `listen_path: "/entreprise/{siret}"`, une regle `GET` sans conditions :

```
script: #{ name: seeded_pick(request.path.siret, ["Dupont SARL", "Martin SAS", "Petit EURL"]), score: seeded_int(request.path.siret, 0, 100) }
```

Body de la reponse (fragment `Template`) :

```json
{"siret":"{{path.siret}}","name":"{{script.name}}","score":{{script.score}}}
```

`GET /seeded-test/entreprise/44306184100047` renvoie systematiquement le meme `name`/`score`
pour ce SIRET (verifie par un test E2E, `frontend/e2e/insee.spec.mjs`), et un resultat different
pour un autre SIRET.

## Groupes de services

Un service peut appartenir a un groupe (`group_name`) affiche comme un accordeon dans l'IHM.
Chaque groupe a un `code` technique (5 caracteres, genere automatiquement, insensible a la
casse du nom) qui prefixe l'URL des services du groupe : `/{code}/{service}/...`. Tout le monde
peut creer un groupe (le createur en devient automatiquement admin) ; les admins du groupe
peuvent le modifier/supprimer et gerer ses membres ; un super-admin peut tout controler.

## Authentification (optionnelle)

Desactivee par defaut (`AUTH_ENABLED=false`, acces complet anonyme). Une fois activee, lightMock
delegue l'authentification a Keycloak (login ROPC, validation JWT via JWKS). Les roles
determinent l'acces aux services et groupes ; `SUPER_ADMINS` (liste d'identifiants) donne un
acces complet, y compris le reset et la restauration de sauvegardes.

## Messaging Kafka (optionnel)

Feature Cargo `messaging-kafka`, **non activee par defaut** (`cargo build`/`cargo test` sans
`--features messaging-kafka` ne compilent ni ne telechargent la dependance Kafka). Une fois
activee et configuree (`KAFKA_*`, voir tableau des variables d'environnement), lightMock
consomme un topic et applique le meme moteur de regles/templates que le HTTP (sans
`pre_script`/`script`/`post_script`, qui restent HTTP-only dans cette version), avec un journal
consultable dans l'IHM et une simulation possible sans producteur Kafka reel
(`POST /api/messaging/simulate`).

## Deploiement Kubernetes

Namespace `entreprise-tools`, exposition via Gloo Edge. Voir [k8s/README.md](k8s/README.md).

```bash
docker build -t <registry>/lightmock:latest .
kubectl apply -k k8s/
```

## Troubleshooting

| Probleme | Solution |
|---|---|
| `cargo build` echoue avec `link.exe not found` | Installer VS Build Tools : `winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools"` |
| Port 7342 deja utilise | `$env:PORT = "7343"` ou tuer le processus existant |
| Frontend ne s'affiche pas | Verifier `STATIC_DIR` pointe vers `frontend/dist` (chemin absolu recommande sur Windows) |
| Requete mock retourne 404 | Verifier l'URL inclut le namespace : `/{service_name}/{path}` (et `/{group_code}/...` si le service est groupe) |
| Requete sur une methode/sous-chemin non couvert | La methode est definie par regle, pas par service : verifier qu'une regle existe pour cette methode/`sub_path` |
| `cargo build --features messaging-kafka` sur Windows : `link.exe`/`cl.exe` introuvables malgre VS installe | Git Bash place `C:\Program Files\Git\usr\bin\link.exe` (coreutils) avant le linker MSVC dans le PATH herite ; `vcvarsall.bat`/`vcvars64.bat` sont eux-memes peu fiables ici. Construire l'environnement MSVC a la main (PATH/INCLUDE/LIB pointant directement vers `VC\Tools\MSVC\<ver>\bin\Hostx64\x64` + le Windows SDK) plutot que de compter sur `vcvarsall.bat` |
| `cargo build --features messaging-kafka` : `cmake` erreur `Failed to run MSBuild ... path exceeds the OS max path length limit` | Le generateur cmake par defaut produit un arbre de build tres imbrique qui depasse 260 caracteres si le repo est deja profondement niche (ex. OneDrive). Fixer `CARGO_TARGET_DIR` sur un chemin court (ex. `C:\lm-target`) via variable d'env avant de builder |

## Licence

MIT — voir [LICENSE](LICENSE).
