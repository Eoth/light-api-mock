# Authentification

Par défaut, lightMock est **ouvert à tous** — n'importe qui ayant accès à l'URL peut tout voir et
tout modifier, sans avoir besoin de se connecter. C'est volontaire pour un usage en environnement
de développement/test fermé. L'authentification peut être **activée** si besoin (par exemple pour
un environnement partagé entre plusieurs équipes).

## Fonctionnement une fois activée

Quand l'authentification est activée, lightMock délègue la vérification de l'identité à
**Keycloak** (un serveur d'authentification externe déjà utilisé par ailleurs dans votre
organisation, le cas échéant) : un écran de connexion apparaît, et l'accès est ensuite lié à
l'utilisateur connecté.

<!-- SCREENSHOT: écran de connexion Keycloak -->

Les droits d'accès dépendent alors :

- Des **rôles** de l'utilisateur (quels services/groupes il peut voir ou modifier).
- De son appartenance aux [groupes de services](groupes.md) (admin de groupe, membre...).
- D'une liste de **super-administrateurs**, qui ont accès à tout, y compris aux actions
  sensibles (réinitialisation complète, restauration de sauvegardes).

<!-- SCREENSHOT: badge utilisateur connecté dans la barre de navigation -->

## Prérequis et limites

- **Désactivée par défaut.** Son activation est une décision d'administration système (variable
  d'environnement au démarrage de lightMock), pas une option que l'on bascule depuis l'interface —
  si vous ne savez pas si elle est active sur votre instance, regardez si un écran de connexion
  apparaît à l'ouverture.
- Nécessite un serveur Keycloak déjà en place et accessible depuis lightMock ; sans cette
  configuration renseignée correctement, lightMock refuse de démarrer si l'authentification est
  activée (pour éviter de tourner accidentellement "à moitié protégé").
- Un bouton "Reset" (réinitialisation complète, voir [Administration](administration.md)) peut être
  affiché ou masqué indépendamment de l'authentification — mais sa présence à l'écran n'est
  **jamais** la seule protection réelle : l'autorisation est toujours vérifiée côté serveur.
