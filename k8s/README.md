# Kubernetes manifests

A [Kustomize](https://kustomize.io) base with everything Mimicway needs, and two ways to route traffic to it. Mimicway does not depend on any routing technology: use the overlay that matches your cluster, or keep the base and add your own routing.

| Directory | Content |
|---|---|
| `base/` | ConfigMap (environment), PersistentVolumeClaim (64 MiB for the configuration and its backups), Deployment (one pod, probes, strict security context), Service (port 80 to 7342). No namespace, no routing. |
| `ingress/` | The base in the `mimicway` namespace, exposed by a standard `Ingress` on `mimicway.example.com`. |
| `gloo-edge/` | The base in the `mimicway` namespace, exposed through Gloo Edge (`Upstream`, `RouteTable`, `VirtualService`). |

## Deploy

```bash
# Build and push the image, or use a published one.
docker build -t <registry>/mimicway:<version> .
docker push <registry>/mimicway:<version>

# Point the overlay at it, then apply.
cd k8s/ingress
kustomize edit set image mimicway=<registry>/mimicway:<version>
kubectl create namespace mimicway
kubectl apply -k .

kubectl -n mimicway get pods -l app.kubernetes.io/name=mimicway
kubectl -n mimicway logs -l app.kubernetes.io/name=mimicway
```

Edit the host name in `ingress/ingress.yaml` (or `gloo-edge/virtualservice.yaml`), and the settings in `base/configmap.yaml` (see the configuration table of the [README](../README.md#configuration)). To use another namespace, change it in the overlay's `kustomization.yaml`; for Gloo Edge, also in the three Gloo resources, which refer to it in their specification.

## What any routing must provide

Whatever exposes Mimicway (these overlays, a Helm chart of your own, a platform that generates routes from labels…) only has to meet three conditions:

1. **Mimicway is served from the root of a host.** Its UI loads `/assets/…`, `/runtime-config.json` and `/api/…` from the root, so it cannot be moved under a path prefix such as `/mimicway` (planned, see the [roadmap](../ROADMAP.md)).
2. **The pod's port (`PORT`, 7342) is reachable** from the routing layer, through the Service.
3. **`/api/health` answers the probes**; it never requires authentication.

The UI and the API are always served by the same pod. If your platform exposes them on two different hosts, set `API_BASE_URL` on the UI's host and `CORS_ALLOWED_ORIGINS` on the API's, as described in [Split UI and API](../README.md#split-ui-and-api); both routes then point to the same Service.

## Why one pod, and why `Recreate`

The configuration is one file on one volume, written by one process. A second replica, or a rolling update that starts the new pod before the old one stops, would let two processes write the same file and lose changes. The Deployment therefore runs a single replica with the `Recreate` strategy: on an update, the old pod receives `SIGTERM`, writes its pending changes and stops, then the new one starts. Expect a few seconds of downtime per update.

## Pod security

- `runAsNonRoot`, user 1000, no privilege escalation, every Linux capability dropped; `fsGroup: 1000` makes the data volume writable whatever owner the storage class gives it.
- Read-only root file system: only `/data` (the volume) is writable.
- Memory limit 256 MiB: idle, Mimicway uses a few MiB, but request bodies up to 10 MiB are buffered to evaluate rules.
- Mimicway serves plain HTTP: terminate TLS in your ingress (commented example in `ingress/ingress.yaml`).
- To restrict where proxied services may connect, add a `NetworkPolicy` for the pod's egress (see the hardening checklist of the [security model](../docs/en/security.md#hardening-checklist)).

## Persistence

`mock-config.yaml` lives on the volume mounted at `/data`, next to `backups/`. Writes are atomic (temporary file, then rename), so a crash never leaves a half-written configuration.
