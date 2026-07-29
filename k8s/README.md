# k8s/ - Manifests Kubernetes

Deploiement dans le namespace `entreprise-tools` avec exposition via Gloo Edge.

## Fichiers

| Fichier              | Type            | Description                                       |
|----------------------|-----------------|---------------------------------------------------|
| `pvc.yaml`           | PVC             | Volume 64Mi pour la persistance YAML              |
| `configmap.yaml`     | ConfigMap       | Variables d'env (DATA_PATH, STATIC_DIR, PORT)     |
| `deployment.yaml`    | Deployment      | Pod unique (replicas: 1), probes, security context |
| `service.yaml`       | Service         | ClusterIP port 80 -> 7342                         |
| `upstream.yaml`      | Gloo Upstream   | Reference au Service K8s pour Gloo                |
| `routetable.yaml`    | Gloo RouteTable | Route `/lightmock` -> upstream avec prefix rewrite |
| `virtualservice.yaml`| Gloo VS         | Expose le domaine et delegue a la RouteTable      |
| `kustomization.yaml` | Kustomize       | Applique tous les manifests d'un coup             |

## Deploiement

```bash
# Build et push de l'image (adapter le registry)
docker build -t <registry>/lightmock:latest .
docker push <registry>/lightmock:latest

# Mettre a jour l'image dans deployment.yaml
# image: <registry>/lightmock:latest

# Appliquer
kubectl apply -k k8s/

# Verifier
kubectl -n entreprise-tools get pods -l app.kubernetes.io/name=lightmock
kubectl -n entreprise-tools logs -l app.kubernetes.io/name=lightmock
```

## Architecture reseau

```
Client (navigateur / appli)
  |
  v
Gloo Edge (VirtualService)
  |
  v  prefixRewrite: /lightmock -> /
Gloo RouteTable
  |
  v
Upstream (lightmock:80)
  |
  v
Service ClusterIP (port 80 -> 7342)
  |
  v
Pod lightMock (port 7342)
  |-- /api/*        -> API REST de configuration
  |-- /svc-a/*      -> Mock ou Proxy selon is_mocked
  |-- /*            -> Frontend Svelte (assets statiques)
```

## Configuration Gloo

### Upstream
Reference le Service Kubernetes `lightmock` dans `entreprise-tools` sur le port 80.

### RouteTable
Intercepte les requetes avec le prefixe `/lightmock` et les redirige vers l'upstream avec un `prefixRewrite` vers `/` pour que le backend recive des chemins propres.

A adapter si lightMock doit etre accessible depuis la racine ou un autre prefixe.

### VirtualService
Expose lightMock sur un domaine (par defaut `lightmock.example.com`).

A adapter selon votre configuration DNS/domaine interne. Pour ajouter a un VirtualService existant, integrer la RouteTable en `delegateAction` dans les routes du VS parent.

## Securite du Pod

- `readOnlyRootFilesystem: true` : seul `/data` (PVC) est writable
- `runAsNonRoot: true`, `runAsUser: 1000`
- `allowPrivilegeEscalation: false`
- `capabilities: drop ALL`
- Replicas: 1 (ecriture exclusive sur le PVC, pas de conflit)

## Persistence

Le fichier `mock-config.yaml` est stocke sur le PVC monte en `/data`. L'ecriture est atomique (temp file + rename) pour garantir l'integrite en cas de crash.

## Front et back exposes separement (ingress/VirtualService distincts)

Le pod lightMock sert TOUJOURS l'API (`/api/*`) et le frontend (assets statiques) depuis le
**meme** processus (cf `## Architecture reseau` ci-dessus) — mais rien n'empeche l'infrastructure
d'exposer ce meme pod via **deux entrees de routage distinctes**, potentiellement sur des domaines
differents (cas reel rapporte : un `VirtualService` pour le front, un `RouteTable`/`Upstream`
separe pour le back). Le frontend, servi par la premiere entree, doit alors savoir joindre l'API
via la seconde — c'est exactement ce que `API_BASE_URL` resout (voir `README.md`, "Deploiement :
URL de l'API independante du Host du frontend").

Exemple : le meme `Upstream`/pod `lightmock` (inchange, cf `upstream.yaml`) expose via deux
domaines Gloo Edge, l'un pour le front, l'autre pour l'API :

```yaml
# VirtualService pour le FRONT (domaine public de la SPA)
apiVersion: gateway.solo.io/v1
kind: VirtualService
metadata:
  name: lightmock-front
  namespace: entreprise-tools
spec:
  virtualHost:
    domains:
      - lightmock.example.com
    routes:
      - matchers:
          - prefix: /
        delegateAction:
          ref:
            name: lightmock
            namespace: entreprise-tools
---
# VirtualService pour le BACK (domaine public de l'API, memes RouteTable/Upstream)
apiVersion: gateway.solo.io/v1
kind: VirtualService
metadata:
  name: lightmock-api
  namespace: entreprise-tools
spec:
  virtualHost:
    domains:
      - lightmock-api.example.com
    routes:
      - matchers:
          - prefix: /
        delegateAction:
          ref:
            name: lightmock
            namespace: entreprise-tools
```

Puis, dans `configmap.yaml` (le ConfigMap consomme par le pod qui sert le front) :

```yaml
API_BASE_URL: "https://lightmock-api.example.com"
```

Le frontend, quel que soit le domaine depuis lequel il a ete charge, appellera alors toujours
`/api/*` sur `https://lightmock-api.example.com` — jamais son propre Host. Aucune modification du
`Deployment`/`Upstream`/PVC n'est necessaire : c'est le meme pod unique (`replicas: 1`) qui
repond aux deux domaines.

**Verifier** : `curl https://lightmock.example.com/runtime-config.json` doit renvoyer
`{"api_base_url":"https://lightmock-api.example.com"}`.
