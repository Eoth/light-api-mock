# k8s/ - Manifests Kubernetes

Deploiement de reference dans le namespace `entreprise-tools`. Les manifests de ce dossier
illustrent un chemin de deploiement (Gloo Edge) mais lightMock n'impose aucune technologie
d'exposition — voir "Contrat de deploiement" ci-dessous.

## Contrat de deploiement

Ce que lightMock exige de son environnement de deploiement se resume a **3 conditions**,
independamment de la maniere dont on les satisfait :

1. **Le frontend doit pouvoir joindre `/runtime-config.json` et l'API a l'URL qu'il expose.**
   Concretement : le navigateur qui a charge la SPA doit pouvoir atteindre `GET /runtime-config.json`
   (sans authentification, cf `CLAUDE.md` §5 point 88) sur le meme Host que celui qui a servi
   la SPA, et cette reponse doit annoncer une URL d'API (`api_base_url`) reellement joignable
   depuis ce navigateur — vide si front et API partagent le meme Host (cas par defaut), ou une
   URL absolue si l'infrastructure les separe. Voir `README.md`, section "Deploiement : URL de
   l'API independante du Host du frontend", pour le detail complet de ce mecanisme
   (`API_BASE_URL`) — non duplique ici.
2. **Le backend doit etre joignable sur le port configure** (`PORT`, defaut `7342`) depuis
   quel que soit le point d'entree que l'infrastructure place devant le pod.
3. **`/api/health` doit etre atteignable** pour que les probes Kubernetes (liveness/readiness,
   cf `deployment.yaml`) fonctionnent — exempt d'authentification par conception.

**Comment on satisfait ce contrat est entierement a la discretion de l'equipe qui deploie et de
son infrastructure.** lightMock n'a aucune opinion sur le mecanisme d'exposition : un
`VirtualService` Gloo Edge ecrit a la main, un simple label sur un Ingress/RouteTable que
l'automatisation de la plateforme reprend automatiquement dans une ressource deja existante, un
chart Helm maison, un `Ingress` Kubernetes basique, ou tout autre mecanisme propre au cluster
cible — tous sont valides tant que les 3 conditions ci-dessus sont vraies. Les exemples de ce
fichier (Gloo Edge, Ingress basique) sont des **illustrations d'implementations possibles**, pas
une prescription : n'en reproduisez la forme exacte que si elle correspond a la facon dont votre
cluster gere reellement le routage. Si votre plateforme attribue deja des ressources de routage
par convention/label, suivez cette convention plutot que ces exemples.

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

Les 3 fichiers `Gloo *` sont specifiques a l'exemple d'implementation Gloo Edge decrit plus bas
— ignorez-les (et retirez-les de `kustomization.yaml`) si votre cluster expose lightMock
autrement (Ingress simple, automatisation de plateforme...). `pvc.yaml`/`configmap.yaml`/
`deployment.yaml`/`service.yaml` restent necessaires quel que soit le mecanisme d'exposition.

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

## Exemple d'implementation 1 : Gloo Edge (celui fourni dans ce dossier)

Les fichiers `upstream.yaml`/`routetable.yaml`/`virtualservice.yaml` de ce dossier montrent
**une facon possible** de satisfaire le contrat ci-dessus avec Gloo Edge, en ecrivant les
ressources de routage a la main. C'est l'implementation choisie pour ce depot de reference, pas
une exigence de lightMock : sur un cluster ou les equipes n'ecrivent jamais de `VirtualService`
elles-memes (une automatisation de la plateforme les genere a partir d'un label sur
l'Ingress/RouteTable, par exemple), suivez cette convention a la place et ignorez ces 3
manifests — le Service K8s (`service.yaml`) reste le seul point d'ancrage necessaire.

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

### Upstream
Reference le Service Kubernetes `lightmock` dans `entreprise-tools` sur le port 80.

### RouteTable
Intercepte les requetes avec le prefixe `/lightmock` et les redirige vers l'upstream avec un `prefixRewrite` vers `/` pour que le backend recive des chemins propres.

A adapter si lightMock doit etre accessible depuis la racine ou un autre prefixe.

### VirtualService
Expose lightMock sur un domaine (par defaut `lightmock.example.com`).

A adapter selon votre configuration DNS/domaine interne. Pour ajouter a un VirtualService existant, integrer la RouteTable en `delegateAction` dans les routes du VS parent.

## Exemple d'implementation 2 : Ingress Kubernetes basique (sans service mesh)

Le contrat de deploiement ne suppose aucun service mesh ni gateway avance — un `Ingress`
standard suffit des lors que les 3 conditions du contrat sont satisfaites. Exemple minimal
exposant le pod `lightmock` (via `service.yaml`, inchange) sur un seul domaine, front et API
co-localises (donc `API_BASE_URL` reste vide, comportement par defaut) :

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: lightmock
  namespace: entreprise-tools
spec:
  rules:
    - host: lightmock.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: lightmock
                port:
                  number: 80
```

Rien de plus n'est requis : `service.yaml` route deja vers le port 7342 du pod, qui sert a la
fois `/api/*`, `/api/health` et les assets statiques de la SPA sur ce meme port (cf
"Architecture reseau" de l'exemple Gloo ci-dessus pour le detail des chemins internes, valable
quel que soit le mecanisme d'exposition). Si un jour front et API doivent etre separes sur deux
`Ingress`/domaines distincts, le meme principe que l'exemple Gloo "Front et back exposes
separement" ci-dessous s'applique : deux ressources de routage pointant vers le meme Service,
plus `API_BASE_URL` configure sur celle qui sert le front.

## Securite du Pod

- `readOnlyRootFilesystem: true` : seul `/data` (PVC) est writable
- `runAsNonRoot: true`, `runAsUser: 1000`
- `allowPrivilegeEscalation: false`
- `capabilities: drop ALL`
- Replicas: 1 (ecriture exclusive sur le PVC, pas de conflit)

## Persistence

Le fichier `mock-config.yaml` est stocke sur le PVC monte en `/data`. L'ecriture est atomique (temp file + rename) pour garantir l'integrite en cas de crash.

## Front et back exposes separement (illustration Gloo Edge)

Le pod lightMock sert TOUJOURS l'API (`/api/*`) et le frontend (assets statiques) depuis le
**meme** processus (cf point 1 du contrat de deploiement, en tete de ce fichier) — mais rien
n'empeche l'infrastructure d'exposer ce meme pod via **deux entrees de routage distinctes**,
potentiellement sur des domaines differents (cas reel rapporte : un `VirtualService` pour le
front, un `RouteTable`/`Upstream` separe pour le back). Le frontend, servi par la premiere
entree, doit alors savoir joindre l'API via la seconde — c'est exactement ce que `API_BASE_URL`
resout (voir `README.md`, "Deploiement : URL de l'API independante du Host du frontend").

**Ceci reste une illustration parmi d'autres** (meme principe applicable a deux `Ingress`
distincts, deux `RouteTable` derriere une automatisation de plateforme, etc.) — le contrat ne
prescrit que le resultat (le frontend doit pouvoir joindre `/runtime-config.json` et l'API a
l'URL qu'il expose), pas la ressource Gloo specifique ci-dessous. Exemple concret : le meme
`Upstream`/pod `lightmock` (inchange, cf `upstream.yaml`) expose via deux domaines Gloo Edge,
l'un pour le front, l'autre pour l'API :

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
