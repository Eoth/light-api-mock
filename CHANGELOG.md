# Changelog

Tous les changements notables de ce projet sont documentés dans ce fichier.

Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et ce projet
adhère au [Semantic Versioning](https://semver.org/lang/fr/). Tant que la version reste en
`0.x`, l'API (REST comme configuration YAML) peut encore évoluer de façon non rétrocompatible
entre versions mineures.

## [Unreleased]

### Security
- With authentication enabled, the management API enforces group boundaries everywhere: moving a service needs the right to create it in its destination (a member could push services into any group, or out of all groups); `GET /api/config` and `GET /api/logs` only return what the user can access (they exposed every team's services, targets and captured traffic); a group update is validated like a creation (name, 5-character code, uniqueness) and a renamed group keeps its services. Checks and changes now happen atomically, so concurrent requests cannot create duplicates.
- Requests whose path holds a `.` or `..` segment (literal or `%2e`-encoded) are refused with 400: URL parsing resolved them, so `/service/../admin` reached paths of the real backend outside the base path configured for the service. The proxy now gives up on a backend after 10 s without a connection or 120 s without data (`PROXY_READ_TIMEOUT_SECS`) and answers 504; a backend that accepted the connection and never answered held the request forever.
- Credential headers (`Authorization`, `Proxy-Authorization`, `Cookie`, `Set-Cookie`, `X-Api-Key`, `X-Auth-Token`, `X-Amz-Security-Token`, plus any name listed in `REDACT_HEADERS`) are replaced by `[redacted]` in the request log, the traffic observation and the rule tester captures, and are never copied into a suggested rule. `GET /api/logs` returned the bearer tokens and session cookies of the applications under test, and a suggestion saved the real backend's `Set-Cookie` into the configuration. Proxy target URLs lose their `user:password@` part in logs.
- Access tokens are validated locally only, against the realm's published keys: any asymmetric algorithm Keycloak signs with (RSA, RSA-PSS, ECDSA, EdDSA), issuer, expiry, and the client the token was issued to (`azp` or `aud`). The fallback to Keycloak's `userinfo` endpoint is removed: it accepted tokens issued to any client of the realm, so another application's token opened lightMock, and every invalid token cost a call to Keycloak. A rotated key is picked up at once; unknown key ids trigger at most one key-set fetch per 30 s. Calls to Keycloak time out (5 s to connect, 10 s overall); an unreachable Keycloak answers 503 and error responses no longer carry internal details. **Breaking** when Keycloak issues tokens under another URL than `KEYCLOAK_URL` (public hostname vs in-cluster service): set `KEYCLOAK_ISSUER`. The login now reports the user name found in the token.
- Rule scripts (Rhai) can no longer `import` script files from the server's disk (Rhai's default module resolver was active), `eval` is disabled, `print`/`debug` go to the debug log instead of stdout, and the call depth is bounded. `random_int`, `seeded_int`, `date_past`/`date_future` and `parse_date` no longer panic on extreme arguments; `seeded_int` returns the same values as before for the same seed.
- The binary listens on `127.0.0.1` by default instead of every interface (`BIND_ADDRESS`, set to `0.0.0.0` in the Docker image and the Kubernetes manifests). Without authentication, a binary started on a workstation exposed its configuration API and its proxy rules to the whole local network. When listening on loopback only, the management API also refuses requests addressed to another host name (DNS rebinding). **Breaking** for native installs reached from other machines: set `BIND_ADDRESS=0.0.0.0`.
- The management API no longer answers cross-origin requests from any website (`Access-Control-Allow-Origin: *`), and refuses state-changing requests that the browser marks as cross-site: any page open in a developer's browser could read and rewrite a local lightMock's configuration. Allowed origins are listed in `CORS_ALLOWED_ORIGINS` (needed only when the UI is served from another origin than the API, see `API_BASE_URL`). Mocked and proxied services still accept every origin. **Breaking** for split deployments that relied on the open CORS policy: set `CORS_ALLOWED_ORIGINS` on the API.
- With `AUTH_ENABLED=true`, only the management API (`/api/...`) requires a token, as documented. The mocked and proxied service routes required one too, which broke every mock for the applications under test and forwarded the lightMock token to the real backends on proxied rules.

### Fixed
- Regular expressions of rule conditions and raw TCP matchers are compiled once and cached instead of on every evaluated request, with a 1 MiB limit on their compiled size; a rule whose expression does not compile is refused when saved instead of silently never matching.
- A service could be renamed onto another service of the same group, or created in a group that does not exist; an ungrouped service named like a group code (or the reverse) made `/{code}/...` route to two places. All are refused. Creating a service returns the one of the requested group when another group has a namesake. The ping cache no longer mixes up namesake services of different groups. Request log entries carry the service's `group_name`.
- `PUT /api/config` (and the UI import) refuses a service that refers to a group the configuration does not define; such a service silently lost its group URL prefix and permissions.
- Traffic observation and rule suggestions now work for services that belong to a group; they never started for them (the group was looked up by code instead of name).
- Non-ASCII path parameters are decoded as UTF-8: `/users/%C3%A9t%C3%A9` gives `été` (it gave `Ã©tÃ©`), in any script; patterns written with non-ASCII characters match their percent-encoded requests. `%` followed by anything but two hex digits is kept as is.
- SIGTERM (Kubernetes, Docker, systemd) now triggers the graceful shutdown that drains pending configuration writes; only Ctrl+C did, so a pod stop could lose the last changes.

### Added
- `SECURITY.md` : politique de signalement de vulnérabilité (canal, périmètre, délais visés).
- `LICENSE` : licence MIT explicite (le README l'annonçait déjà, le fichier manquait).
- README : section "Sécurité et confidentialité" (comportement réseau exhaustif — absence de
  télémétrie, portée exacte des 4 flux sortants possibles), et documentation des commandes
  d'audit de dépendances (`cargo audit`/`npm audit`), de génération de SBOM (CycloneDX) et de
  scan de l'image Docker (Trivy).
- Métadonnées de packaging (`license`, `repository`, `description`) dans `Cargo.toml` et
  `frontend/package.json`, pour une meilleure qualité de SBOM généré.

## [0.1.0] - 2026-07-29

### Added
- Première version versionnée de lightMock : mock & proxy HTTP intelligent, un seul binaire Rust
  (Axum) servant une UI Svelte 5, déployable en Kubernetes, Docker Compose ou nativement.
- Moteur de règles first-match (conditions ET/OU sur path/query/header/body JSON/XML/form),
  bascule mock/proxy par service et par règle, testeur de règle et détecteur de conflits.
- Templates dynamiques (`{{variable | pipe}}`), 19 types de données factices, scripts Rhai
  sandboxés (jusqu'à 3 blocs par règle, fonctions natives déterministes par seed).
- Groupes de services (accordéons, préfixe d'URL, permissions admins/membres), mode SOAP/WSDL
  par service, mode Chaos (latence/erreurs injectées).
- Journal des requêtes, ping de disponibilité (connexion TCP pure, jamais de requête HTTP),
  sauvegardes YAML automatiques avec rotation et restauration depuis l'UI.
- Authentification Keycloak optionnelle (désactivée par défaut), rôles et permissions par
  groupe de services.
- Support Kafka optionnel (feature Cargo `messaging-kafka`, non compilée par défaut).
- Interface Svelte 5 accessible (RGAA niveau AA), mode sombre.

Voir l'historique des commits pour le détail des décisions d'architecture ayant mené à cette
version.

[Unreleased]: https://github.com/eoth/light-api-mock/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/eoth/light-api-mock/releases/tag/v0.1.0
