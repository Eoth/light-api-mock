# lightMock

Mock & Proxy Intelligent pour environnements Kubernetes.

Un seul binaire Rust qui intercepte les requetes HTTP, les mock ou les proxifie vers le vrai backend, configurable en temps reel via une interface web. Chaque service est expose sous `/{service_name}/...` avec une methode HTTP explicite, sans redemarrage de pod.

## Fonctionnalites

- **Namespace URL par service** : chaque service est expose sous `/{name}/{listen_path}`, pas de collision
- **Methode HTTP explicite** : un service = une methode (GET, POST, PUT, etc.)
- **Bascule Mock / Proxy** : toggle ON/OFF depuis l'IHM, sans redemarrage
- **Moteur de regles** : conditions combinables (ET/OU) sur path params, query, headers, body JSON/XML/form
- **Templates dynamiques** : expressions `{path.siret}`, `{fake.CompanyName}`, `{now_ms}`, pipes `| first(9)`, `| upper`
- **Mode Chaos** : injection de latence (fixe ou plage) et d'erreurs HTTP
- **Journal des requetes** : historique consultable dans l'IHM (service, methode, path, mode, status)
- **Import / Export** : sauvegarde et restauration de la configuration en JSON
- **Zero dependance externe** : pas de base de donnees, persistance fichier YAML atomique
- **Interface accessible** : conformite RGAA niveau AA

## Prerequis

| Outil    | Version min | Notes |
|----------|-------------|-------|
| Rust     | 1.75+       | Avec toolchain MSVC sur Windows |
| Node.js  | 20+         | Pour le frontend Svelte |
| npm      | 9+          | |

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

Chaque service est expose sous **`/{name}/{listen_path}`**. Exemples :

| Service name | Methode | listen_path | URL finale de test |
|---|---|---|---|
| `insee` | GET | `/v4/sirene/{siret}` | `GET /insee/v4/sirene/{siret}` |
| `auth` | POST | `/login` | `POST /auth/login` |
| `users` | GET | `/*` | `GET /users/anything` |

En mode proxy, le prefixe `/{name}` est strippe avant forward vers le vrai backend.

## API REST

| Methode | Endpoint | Description |
|---|---|---|
| GET | `/api/config` | Configuration complete |
| PUT | `/api/config` | Remplacer toute la config |
| GET | `/api/services` | Liste des services |
| GET | `/api/services/:name` | Detail d'un service |
| PUT | `/api/services/:name` | Creer / modifier un service |
| DELETE | `/api/services/:name` | Supprimer un service |
| PUT | `/api/services/:name/toggle` | Basculer mock/proxy |
| PUT | `/api/services/:name/rules/reorder` | Reordonner les regles |
| GET | `/api/logs?limit=50` | Journal des requetes |

### Exemple : creer un service

```bash
curl -X PUT http://localhost:7342/api/services/demo \
  -H "Content-Type: application/json" \
  -d '{
    "name": "demo",
    "method": "GET",
    "listen_path": "/*",
    "real_target_url": "http://httpbin.org",
    "is_mocked": true,
    "rules": [{
      "name": "hello",
      "conditions": { "all_of": [], "any_of": [] },
      "response": {
        "status": 200,
        "headers": [{"name": "Content-Type", "value": "application/json"}],
        "body": [{"type": "Template", "template": "{{\"message\":\"Hello from lightMock!\"}}"}],
        "chaos": null
      }
    }]
  }'

# Tester : GET /demo/anything
curl http://localhost:7342/demo/anything
```

## Variables d'environnement

| Variable | Defaut | Description |
|---|---|---|
| `DATA_PATH` | `./data` | Repertoire du fichier `mock-config.yaml` |
| `STATIC_DIR` | `./frontend/dist` | Assets Svelte compiles |
| `PORT` | `7342` | Port d'ecoute HTTP |
| `RUST_LOG` | `light_mock=info` | Filtre de logs (ex: `light_mock=debug`) |

## Tests

```bash
# Rust (90 tests)
cargo test -- --test-threads=1

# Frontend unitaires (35 tests Vitest)
cd frontend && npm test

# E2E navigateur (19 tests Playwright, serveur doit tourner)
cd frontend && npm run test:e2e
```

## Architecture

```
light-mock/
  src/
    models/        # Service, Rule, Condition, BodyFragment, FakeKind, ChaosConfig
    engine/        # matcher, proxy, renderer, template (expressions + pipes)
    store/         # Persistance YAML atomique (Arc<RwLock<>>)
    server/        # Axum : API REST, intercept middleware, request_log
  frontend/        # SPA Svelte 5 + Vite 6
  k8s/             # Manifests K8s + Gloo Edge
  scripts/         # Bootstrap Windows / Linux
  Dockerfile       # Build multi-stage
```

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
| Tests Rust flaky sur env var | Utiliser `cargo test -- --test-threads=1` |
| Requete mock retourne 404 | Verifier l'URL inclut le namespace : `/{service_name}/{path}` |
| Requete POST sur un service GET | Chaque service a une methode fixe, creer un 2e service pour POST |

## Licence

MIT
