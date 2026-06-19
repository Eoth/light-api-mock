# k8s/ - Manifests Kubernetes

Deploiement dans le namespace `entreprise-tools` avec exposition via Gloo Edge.

## Fichiers

| Fichier              | Type            | Description                                       |
|----------------------|-----------------|---------------------------------------------------|
| `pvc.yaml`           | PVC             | Volume 64Mi pour la persistance YAML              |
| `configmap.yaml`     | ConfigMap       | Variables d'env (DATA_PATH, STATIC_DIR, PORT)     |
| `deployment.yaml`    | Deployment      | Pod unique (replicas: 1), probes, security context |
| `service.yaml`       | Service         | ClusterIP port 80 -> 3000                         |
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
Service ClusterIP (port 80 -> 3000)
  |
  v
Pod lightMock (port 3000)
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
