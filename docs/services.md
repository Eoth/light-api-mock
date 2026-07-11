# Services et routage

Un **service** est l'unité de base dans lightMock : il représente une API que vous voulez
simuler ou relayer. Chaque service que vous créez devient immédiatement accessible sur sa propre
URL, sans redémarrer quoi que ce soit.

## Créer un service

Depuis l'écran d'accueil, le bouton **"+ Ajouter un service"** ouvre un formulaire avec :

- **Nom** : identifie le service et sert de premier segment de son URL (voir plus bas). Uniquement
  lettres, chiffres, tirets et underscores (pas d'espace ni de caractères spéciaux).
- **Chemin d'écoute** (`listen_path`) : la portion d'URL après le nom du service, par exemple
  `/v1/utilisateurs/{id}`. Elle peut contenir des paramètres entre accolades (`{id}`) qui seront
  réutilisables dans les réponses. Si elle est laissée vide, le service répond à **n'importe quel
  chemin** en dessous de son nom.
- **URL cible réelle** (`real_target_url`) : l'adresse du vrai backend, utilisée quand une requête
  est relayée en mode proxy (voir ci-dessous) et par le [ping de disponibilité](ping-de-disponibilite.md).
- **Mock actif** : interrupteur qui bascule le service entre mode simulé et mode relais (voir
  ci-dessous).
- **Type de service** : REST (par défaut) ou SOAP — voir la section dédiée plus bas.
- **Groupe** (optionnel) : rattache le service à un [groupe de services](groupes.md).

<!-- SCREENSHOT: formulaire de création d'un nouveau service -->

## Comment l'URL est construite

Chaque service est exposé sous son propre "espace de noms" pour éviter toute collision entre
services :

```
/{nom-du-service}/{chemin-d-ecoute}
```

ou, si le service appartient à un groupe :

```
/{code-du-groupe}/{nom-du-service}/{chemin-d-ecoute}
```

| Nom du service | Chemin d'écoute | URL finale à appeler |
|---|---|---|
| `insee` | `/v4/sirene/{siret}` | `GET /insee/v4/sirene/44306184100047` |
| `auth` | `/login` | `POST /auth/login` |
| `utilisateurs` | *(vide)* | `GET /utilisateurs/n-importe-quoi` (chemin libre) |

L'URL exacte à utiliser pour tester un service est toujours affichée dans sa fiche détail — pas
besoin de la recalculer à la main.

<!-- SCREENSHOT: fiche détail d'un service avec l'URL de test affichée -->

## Mock ou Proxy : deux modes, à deux niveaux

lightMock peut soit **répondre lui-même** à une requête (mode *Mock*, avec une réponse que vous
avez configurée), soit **la transmettre au vrai backend** et renvoyer sa réponse telle quelle
(mode *Proxy*). Ce choix existe à deux niveaux :

- **Au niveau du service** : l'interrupteur "Mock actif" bascule TOUT le service en mode proxy pur
  (aucune règle n'est évaluée, chaque requête part directement vers `real_target_url`) ou en mode
  simulé (les règles du service sont évaluées, voir [Règles de correspondance](regles-de-matching.md)).
- **Au niveau d'une règle** : même quand le service est en mode simulé, chaque règle individuelle
  peut elle-même être réglée sur "mock" (répondre avec le contenu configuré) ou "proxy" (relayer
  cette requête précise vers le vrai backend). Cela permet un **mock partiel** : par exemple,
  simuler uniquement les cas d'erreur et laisser tout le reste passer vers le vrai service.

Dans les deux cas, quand une requête est relayée (proxy), elle part avec sa méthode, ses
paramètres, ses en-têtes et son corps intacts — rien n'est modifié ni perdu en cours de route.

## Type de service : REST ou SOAP

Le sélecteur "Type de service" configure automatiquement le comportement vis-à-vis des requêtes
techniques SOAP (WSDL — le fichier qui décrit une API SOAP) :

- **REST** (par défaut) : comportement standard, sans traitement SOAP particulier.
- **SOAP** : les requêtes de description WSDL peuvent soit être **relayées telles quelles vers le
  vrai backend** (`Proxy`/`Auto`, pratique pour laisser un client SOAP découvrir le vrai contrat
  d'API), soit être **prises en charge par vos règles mockées** (`Mock`, si vous voulez simuler
  aussi la description du service).

## Suppression, modification, clonage

- **Modifier** un service ouvre le même formulaire pré-rempli.
- **Cloner** un service pré-remplit un nouveau formulaire à partir d'un service existant (nom
  suggéré en `{nom}-copie`, modifiable) — pratique pour créer rapidement une variante.
- **Supprimer** un service retire uniquement ce service précis : si un autre service du même nom
  existe dans un groupe différent, il n'est pas affecté (deux services peuvent porter le même nom
  tant qu'ils sont dans des groupes différents, ou l'un sans groupe et l'autre dans un groupe).

## Rechercher un service

La liste des services propose un champ de recherche qui filtre par nom et ouvre automatiquement
le groupe correspondant si le service trouvé appartient à un groupe replié.

## Prérequis et limites

- Aucun prérequis particulier : cette fonctionnalité est disponible dès l'installation de base.
- Le nom d'un service doit être unique **dans son périmètre** (sans groupe, ou au sein d'un même
  groupe) — deux services identiques dans deux groupes différents sont autorisés et bien
  distingués partout dans l'interface.
- Certains noms sont réservés par lightMock lui-même (`api`, `auth`, `assets`...) et ne peuvent
  pas être utilisés comme nom de service, pour éviter tout conflit avec l'interface.
