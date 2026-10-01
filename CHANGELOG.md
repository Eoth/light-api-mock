# Changelog

Tous les changements notables de ce projet sont documentés dans ce fichier.

Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et ce projet
adhère au [Semantic Versioning](https://semver.org/lang/fr/). Tant que la version reste en
`0.x`, l'API (REST comme configuration YAML) peut encore évoluer de façon non rétrocompatible
entre versions mineures.

## [Unreleased]

### Security
- Rule scripts (Rhai) can no longer `import` script files from the server's disk (Rhai's default module resolver was active), `eval` is disabled, `print`/`debug` go to the debug log instead of stdout, and the call depth is bounded. `random_int`, `seeded_int`, `date_past`/`date_future` and `parse_date` no longer panic on extreme arguments; `seeded_int` returns the same values as before for the same seed.
- The binary listens on `127.0.0.1` by default instead of every interface (`BIND_ADDRESS`, set to `0.0.0.0` in the Docker image and the Kubernetes manifests). Without authentication, a binary started on a workstation exposed its configuration API and its proxy rules to the whole local network. When listening on loopback only, the management API also refuses requests addressed to another host name (DNS rebinding). **Breaking** for native installs reached from other machines: set `BIND_ADDRESS=0.0.0.0`.
- The management API no longer answers cross-origin requests from any website (`Access-Control-Allow-Origin: *`), and refuses state-changing requests that the browser marks as cross-site: any page open in a developer's browser could read and rewrite a local lightMock's configuration. Allowed origins are listed in `CORS_ALLOWED_ORIGINS` (needed only when the UI is served from another origin than the API, see `API_BASE_URL`). Mocked and proxied services still accept every origin. **Breaking** for split deployments that relied on the open CORS policy: set `CORS_ALLOWED_ORIGINS` on the API.
- With `AUTH_ENABLED=true`, only the management API (`/api/...`) requires a token, as documented. The mocked and proxied service routes required one too, which broke every mock for the applications under test and forwarded the lightMock token to the real backends on proxied rules.

### Fixed
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
