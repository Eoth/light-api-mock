# Politique de sécurité

## Versions supportées

lightMock suit le [versionnage sémantique](https://semver.org/lang/fr/) (voir
[CHANGELOG.md](CHANGELOG.md)). Le projet étant en version `0.x`, seule la **dernière version
publiée sur `main`** reçoit des correctifs de sécurité — il n'y a pas encore de branche de
maintenance long terme.

| Version | Supportée |
|---------|-----------|
| dernière version sur `main` | ✅ |
| toute version antérieure | ❌ |

Cette politique sera revue et étendue (branches de maintenance dédiées) au passage en `1.0.0`.

## Signaler une vulnérabilité

**Ne pas ouvrir d'issue publique** pour un problème de sécurité potentiel (fuite d'information,
contournement d'authentification, injection, traversée de répertoire, déni de service...).

Canal recommandé : [GitHub Security Advisories](https://github.com/eoth/light-api-mock/security/advisories/new)
(signalement privé, natif à GitHub, avec suivi de conversation dédié).

Si ce canal n'est pas accessible, contacter directement : **tnak49@yahoo.fr**.

Merci d'inclure, dans la mesure du possible :
- Une description du problème et de son impact
- Les étapes de reproduction (version de lightMock, configuration `AUTH_ENABLED`/`AUTH_*`,
  requête(s) concernée(s))
- Un correctif proposé si vous en avez un

### Délais visés

Ce projet est maintenu de façon bénévole/best-effort — les délais ci-dessous sont des objectifs,
pas une garantie contractuelle (SLA) :

| Étape | Délai visé |
|-------|------------|
| Accusé de réception | 5 jours ouvrés |
| Premier diagnostic (confirmé / non reproductible / besoin d'infos) | 10 jours ouvrés |
| Correctif ou plan de mitigation communiqué | Selon la sévérité — best effort, en priorité pour tout ce qui touche à l'authentification, à la traversée de fichiers ou à l'exécution de script (moteur Rhai) |

Nous demandons une divulgation coordonnée : merci de nous laisser le temps de publier un
correctif avant toute divulgation publique du détail technique.

## Ce qui est dans le périmètre

- Le binaire Rust (`src/`) : moteur de matching, proxy, rendu de template, moteur de script Rhai
  (sandbox), authentification Keycloak, persistance YAML, API REST.
- Le frontend Svelte (`frontend/src/`) : UI d'administration servie par le binaire.
- Les manifests Kubernetes fournis (`k8s/`) et le `Dockerfile`.

## Hors périmètre

- Les services tiers que l'utilisateur choisit de mocker/proxyfier (`real_target_url`) : lightMock
  n'a aucun contrôle sur leur sécurité.
- Un déploiement Keycloak/Kafka externe fourni par l'utilisateur.
- Les vulnérabilités dans les dépendances tierces (crates Rust / paquets npm) sans exploitation
  démontrée dans le contexte de lightMock — signaler plutôt en amont, ou via le processus
  `cargo audit`/`npm audit` documenté dans le [README](README.md#audit-des-dépendances).

## Voir aussi

Le [README](README.md#sécurité-et-confidentialité) documente le comportement réseau complet du
produit (absence de télémétrie, portée exacte des appels sortants), ce qui peut aider à évaluer
si un comportement observé est une anomalie ou un fonctionnement attendu avant de signaler.
