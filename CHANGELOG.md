# Changelog

Tous les changements notables de ce projet sont documentés dans ce fichier.

Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et ce projet
adhère au [Semantic Versioning](https://semver.org/lang/fr/). Tant que la version reste en
`0.x`, l'API (REST comme configuration YAML) peut encore évoluer de façon non rétrocompatible
entre versions mineures.

## [Unreleased]

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
