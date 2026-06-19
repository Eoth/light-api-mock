# src/ - Backend Rust

## Structure des modules

### `models/`
Structures de donnees serialisables (Serde) representant la configuration :
- `MockConfig` : racine contenant la liste des services
- `Service` : chemin d'ecoute, URL cible, flag `is_mocked`, regles
- `Rule` : nom, groupe de conditions, reponse mockee
- `ConditionGroup` : combinaison `all_of` (ET) et `any_of` (OU)
- `Condition` : source (QueryParam, Header, JsonPointer, XPath, FormField, BodyRaw) + operateur (Eq, Contains, Regex, Exists)
- `MockResponse` : status HTTP, headers, fragments de body, config chaos
- `BodyFragment` : Literal, Uuid, PickFrom, FakeData
- `ChaosConfig` : delay_ms, error_rate, error_status

### `engine/`
Logique metier sans dependance au framework web :
- `matcher.rs` : evaluation first-match des regles, extraction de valeurs depuis query params, headers, JSON (RFC 6901), XML/SOAP (XPath simplifie), formulaires, body brut
- `proxy.rs` : reverse proxy transparent via reqwest (nettoyage headers hop-by-hop, forwarding body)
- `renderer.rs` : rendu des fragments dynamiques (UUID v4, donnees fictives FR, choix aleatoire) + mode chaos (latence, erreurs)

### `store/`
Persistance fichier YAML avec coherence garantie :
- `MockStore` : `Arc<RwLock<MockConfig>>` pour acces concurrent
- Ecriture atomique : fichier temporaire + `rename(2)`
- Chemin configurable via `DATA_PATH`

### `server/`
Serveur HTTP Axum :
- `api.rs` : routes CRUD REST (`/api/config`, `/api/services/:name`, toggle, reorder)
- `intercept.rs` : middleware qui intercepte les requetes non-API, les route vers le mock ou le proxy selon la config
- `mod.rs` : assemblage du routeur (API + middleware intercept + ServeDir pour les assets)

## Lancer les tests

```bash
cargo test
```
