# lightMock

Mock & Proxy Intelligent pour environnements Kubernetes.

Un seul binaire Rust qui intercepte les requetes HTTP, les mock ou les proxifie vers le vrai backend, le tout configurable en temps reel via une interface web sans redemarrage de pod.

## Fonctionnalites

- **Bascule Mock / Proxy** : chaque service peut etre bascule ON (mock) ou OFF (proxy transparent) depuis l'IHM, sans redemarrage
- **Moteur de regles** : conditions combinables (ET/OU) sur query params, headers, body JSON/XML/SOAP/form
- **Reponses dynamiques** : fragments composables (texte fixe, UUID, choix aleatoire, donnees fictives)
- **Mode Chaos** : injection de latence et d'erreurs HTTP pour tester la resilience
- **Reverse Proxy transparent** : nettoyage des prefixes de routage, forwarding des headers et body
- **Zero dependance externe** : pas de base de donnees, persistance fichier YAML atomique
- **Interface accessible** : conformite RGAA niveau AA (labels, aria, focus, contrastes)

## Prerequis

| Outil    | Version minimale |
|----------|------------------|
| Rust     | 1.75+            |
| Node.js  | 20+              |
| npm      | 9+               |

Pour le deploiement K8s : `kubectl` + acces au cluster avec le namespace `entreprise-tools`.

---

## Lancer l'application (dev local)

Il y a **deux facons** de lancer lightMock en local :

### Option A : Tout-en-un (recommande pour tester)

Le backend Rust sert directement l'IHM. Un seul processus, une seule URL.

```bash
# 1. Build du frontend (une seule fois, ou apres modif frontend)
cd frontend
npm install
npm run build
cd ..

# 2. Build du backend
cargo build --release

# 3. Lancer (Windows PowerShell)
$env:STATIC_DIR = "frontend/dist"
$env:DATA_PATH = "data"
.\target\release\light-mock.exe

# 3. Lancer (Linux / macOS / Git Bash)
STATIC_DIR=./frontend/dist DATA_PATH=./data ./target/release/light-mock
```

**Resultat :**
- IHM : [http://localhost:3000](http://localhost:3000)
- API : [http://localhost:3000/api/services](http://localhost:3000/api/services)

### Option B : Dev frontend avec hot-reload

Pour modifier les composants Svelte avec rechargement instantane.
Il faut **deux terminaux** :

**Terminal 1 — Backend Rust** (API + moteur de mock) :
```bash
# Build + lancer le serveur Rust
cargo build --release

# Windows PowerShell :
$env:DATA_PATH = "data"
.\target\release\light-mock.exe

# Linux / macOS / Git Bash :
DATA_PATH=./data ./target/release/light-mock
```
Le backend ecoute sur `http://localhost:3000`.

**Terminal 2 — Frontend Svelte** (IHM avec hot-reload) :
```bash
cd frontend
npm install    # premiere fois uniquement
npm run dev
```
Le frontend ecoute sur `http://localhost:5173` et proxifie automatiquement les appels `/api/*` vers le backend sur le port 3000.

**Resultat :**
- IHM avec hot-reload : [http://localhost:5173](http://localhost:5173)
- API directe (pour Bruno) : [http://localhost:3000/api/services](http://localhost:3000/api/services)

---

## Tester l'API avec Bruno

L'API est accessible sur `http://localhost:3000`. Base URL a configurer dans Bruno : `http://localhost:3000`.

### Requetes de base

**Lister les services** — `GET http://localhost:3000/api/services`
> Reponse : `[]` (vide au demarrage)

**Creer un service mock** — `PUT http://localhost:3000/api/services/demo`
> Headers : `Content-Type: application/json`
> Body :
```json
{
  "name": "demo",
  "listen_path": "/demo/*",
  "real_target_url": "http://httpbin.org",
  "is_mocked": true,
  "rewrite_directory_urls": false,
  "rules": [
    {
      "name": "hello",
      "conditions": { "all_of": [], "any_of": [] },
      "response": {
        "status": 200,
        "headers": [{ "name": "Content-Type", "value": "application/json" }],
        "body": [{ "type": "Literal", "value": "{\"message\": \"Hello from lightMock!\"}" }],
        "chaos": null
      }
    }
  ]
}
```
> Reponse : le service cree (200)

**Tester le mock** — `GET http://localhost:3000/demo/test`
> Reponse : `{"message": "Hello from lightMock!"}`

**Basculer en mode proxy** — `PUT http://localhost:3000/api/services/demo/toggle`
> Body : `{"is_mocked": false}`
> Maintenant `GET /demo/get` est proxifie vers `http://httpbin.org/get`

**Rebasculer en mode mock** — `PUT http://localhost:3000/api/services/demo/toggle`
> Body : `{"is_mocked": true}`

**Supprimer un service** — `DELETE http://localhost:3000/api/services/demo`
> Reponse : 204 No Content

---

## API REST (reference complete)

| Methode | Endpoint                              | Description                        |
|---------|---------------------------------------|------------------------------------|
| GET     | `/api/config`                         | Configuration complete             |
| PUT     | `/api/config`                         | Remplacer toute la configuration   |
| GET     | `/api/services`                       | Liste des services                 |
| GET     | `/api/services/:name`                | Detail d'un service                |
| PUT     | `/api/services/:name`                | Creer ou modifier un service       |
| DELETE  | `/api/services/:name`                | Supprimer un service               |
| PUT     | `/api/services/:name/toggle`         | Basculer mock/proxy                |
| PUT     | `/api/services/:name/rules/reorder`  | Reordonner les regles              |

## Variables d'environnement

| Variable     | Defaut              | Description                                      |
|-------------|---------------------|--------------------------------------------------|
| `DATA_PATH`  | `./data`            | Repertoire du fichier `mock-config.yaml`          |
| `STATIC_DIR` | `./frontend/dist`   | Repertoire des assets Svelte compiles             |
| `PORT`       | `3000`              | Port d'ecoute HTTP                                |
| `RUST_LOG`   | `light_mock=info`   | Filtre de logs (ex: `light_mock=debug`)           |

## Deploiement Kubernetes

Le deploiement cible le namespace `entreprise-tools` avec exposition via Gloo Edge.

```bash
# Build et push de l'image
docker build -t <registry>/lightmock:latest .
docker push <registry>/lightmock:latest

# Appliquer les manifests (adapter l'image dans deployment.yaml)
kubectl apply -k k8s/
```

Voir [k8s/README.md](k8s/README.md) pour le detail des manifests et la configuration Gloo.

## Architecture

```
light-mock/
  src/
    models/     # Structures de donnees (Route, Rule, Condition, Response)
    engine/     # Moteur de matching, proxy client, renderer de reponses
    store/      # Persistance YAML atomique (Arc<RwLock<>>)
    server/     # Serveur Axum (API REST, interception, middleware)
    main.rs     # Point d'entree
  frontend/     # SPA Svelte 5 + Vite 6
  k8s/          # Manifests Kubernetes + Gloo Edge
  Dockerfile    # Build multi-stage (Node + Rust + Alpine)
```

Voir les README dans chaque sous-dossier pour plus de details.

## Stack technique

**Backend** : Rust (axum 0.7, tokio, reqwest, serde_yaml, serde_json, quick-xml, regex, fastrand)

**Frontend** : Svelte 5 + Vite 6 (SPA pure, composants maison RGAA AA)

**Infra** : Docker multi-stage, Kubernetes (Deployment replicas:1, PVC, Gloo Edge)

## Tests

```bash
# Tests unitaires Rust (58 tests)
cargo test

# Smoke tests d'integration (necessite le binaire compile + frontend build)
# Windows PowerShell :
.\smoke-test.ps1
```
