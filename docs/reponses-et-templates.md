# Réponses dynamiques et templates

Une fois qu'une [règle](regles-de-matching.md) a matché, lightMock doit produire une réponse :
un code de statut HTTP, des en-têtes, et un corps (JSON ou XML) qui peut être **statique** ou
**dynamique** (contenir des valeurs calculées à chaque requête).

## Construire le corps de la réponse

Deux façons de construire le corps, au choix :

### 1. Le builder guidé (recommandé pour débuter)

Un éditeur visuel où vous ajoutez des champs un par un (nom, type, valeur), sans écrire de JSON/XML
à la main. Chaque champ peut être :

- une **valeur fixe** (texte, nombre, booléen tel quel),
- une **variable** qui sera remplacée à chaque requête (voir "Variables disponibles" plus bas),
- une **donnée factice** générée automatiquement (voir "Données factices" plus bas),
- un **objet** ou un **tableau d'objets**, pour construire une structure imbriquée.

Pour naviguer dans une structure profondément imbriquée sans se perdre, un fil d'Ariane (chemin
cliquable, ex. `racine > adresse > ville`) au-dessus de l'éditeur permet de "rentrer" dans un
sous-niveau et d'en ressortir en un clic.

*(Capture manquante — aucun scénario E2E existant ne navigue dans le builder JSON guidé avec le
fil d'Ariane ; à réaliser manuellement, cf `frontend/e2e/README.md` section captures.)*

### 2. Le mode "exemple d'abord" (coller un exemple existant)

Pour les corps JSON déjà complexes, il est souvent plus rapide de **coller un exemple réel** de
réponse (par exemple, une réponse déjà obtenue du vrai backend) : lightMock détecte
automatiquement tous les champs et vous permet ensuite de remplacer certaines valeurs par des
variables ou des données factices, champ par champ.

*(Capture manquante — aucun scénario E2E existant n'utilise le mode "exemple d'abord"
(`JsonPasteBuilder`) ; à réaliser manuellement, cf `frontend/e2e/README.md` section captures.)*

> Ce mode "coller un exemple" n'est disponible que pour le JSON pour le moment ; le XML se
> construit uniquement via le builder guidé.

## La syntaxe des templates : `{{ }}`

Que vous utilisiez le builder guidé ou le mode "exemple d'abord", le résultat final est un
**template** : un texte JSON ou XML dans lequel certains passages entre doubles accolades sont
évalués à chaque requête.

| Écriture | Signification |
|---|---|
| `{` et `}` (accolade simple) | Caractères littéraux, comme dans n'importe quel JSON/XML — **pas** une variable |
| `{{variable}}` | Une expression évaluée à chaque requête |
| `{{variable \| transformation}}` | Une variable, ensuite transformée (voir "Transformations" ci-dessous) |

Exemple : `{"siret":"{{path.siret}}"}` renvoie le paramètre de chemin `siret` de la requête reçue.

### Variables disponibles

| Variable | Contenu |
|---|---|
| `path.X` | Valeur du paramètre de chemin `X` (ex. `{id}` dans l'URL du service) |
| `query.X` | Valeur du paramètre de requête `X` (`?X=...`) |
| `header.X` | Valeur de l'en-tête HTTP `X` |
| `body.X` | Valeur au chemin JSON `X` dans le corps de la requête reçue |
| `fake.NomDuType` | Une donnée factice générée (voir plus bas) |
| `uuid` | Un identifiant unique généré |
| `now_ms` / `now_iso` / `now_epoch` | La date/heure actuelle, sous différents formats |
| `seq` | Un compteur d'appels |
| `script` / `pre_script` / `post_script` | Résultat d'un [script Rhai](scripts-rhai.md) associé à la règle, si vous en avez écrit un |

### Transformations (pipes)

Une variable peut être transformée avant d'être insérée : `lower`, `upper`, `trim`,
`capitalize`, `first(N)` (les N premiers caractères), `last(N)`, `substr(debut,longueur)`,
`default("valeur")` (valeur de secours si vide), `replace("a","b")`, `prepend("x")` (ajoute devant),
`append("x")` (ajoute derrière), `length`.

Exemple : `{{path.siret | first(9)}}` ne garde que les 9 premiers caractères du SIRET reçu.

### Données factices (`fake.*`)

Pratique pour peupler une réponse avec des données qui ont l'air réalistes sans devoir les saisir
à la main : prénom, nom, email, téléphone français, entreprise, adresse, ville, code postal,
SIREN/SIRET, adresse complète, date passée/future, horodatage, booléen aléatoire, phrase de
remplissage ("lorem"), pays, IBAN français, et un entier dans une plage donnée
(`Integer{min,max}`).

*(Capture manquante — aucun scénario E2E existant ne sélectionne un type `fake.*` dans un champ
du builder ; à réaliser manuellement, cf `frontend/e2e/README.md` section captures.)*

## Mode Chaos : simuler des pannes et des lenteurs

Pour tester la robustesse d'une application face à un backend capricieux, chaque réponse mockée
peut activer un "mode chaos" :

- **Latence** : un délai fixe, ou une plage aléatoire (entre un minimum et un maximum) avant de
  répondre.
- **Taux d'erreur** : un pourcentage de requêtes qui reçoivent, à la place de la réponse normale,
  une erreur HTTP (code configurable, `500` par défaut).

*(Capture manquante — aucun scénario E2E existant ne configure le mode Chaos via l'interface (les
tests existants postent `chaos: null` directement via l'API) ; à réaliser manuellement, cf
`frontend/e2e/README.md` section captures.)*

## Prérequis et limites

- Aucun prérequis particulier : disponible dès l'installation de base.
- Le mode "coller un exemple" (paste) n'est pas encore éditable en profondeur après détection
  (renommage/ajout/suppression de champs) — pour ces retouches fines, repassez par le builder
  guidé habituel.
- Le mode "coller un exemple" existe seulement pour JSON, pas pour XML.
