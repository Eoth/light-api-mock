# Scripts Rhai (calculs avancés dans une règle)

Pour les besoins que le [builder de réponse](reponses-et-templates.md) ne couvre pas directement
(calculs, valeurs liées entre elles, données "toujours les mêmes pour une même entrée"...),
chaque règle peut exécuter un petit script écrit dans un langage simple appelé **Rhai**. Le
résultat du script devient ensuite disponible comme variable dans le corps de la réponse.

> Rhai est un mini-langage de script (syntaxe proche de JavaScript/Rust) exécuté dans un
> bac à sable : il ne peut ni accéder au disque, ni au réseau, ni consommer des ressources
> illimitées (limité à 10 000 opérations et 1 Mo de texte manipulé par exécution). Vous n'avez pas
> besoin de connaître Rhai en détail pour l'utiliser : les fonctions ci-dessous suffisent à la
> plupart des besoins, et l'éditeur les suggère automatiquement pendant la frappe.

## Trois emplacements de script, indépendants

Une règle propose jusqu'à 3 zones de script, toutes optionnelles :

- **Pré-script** (préparation)
- **Script** (le script "principal")
- **Post-script** (finalisation)

Ces trois blocs sont **totalement indépendants** : ils voient tous la même requête reçue, mais
aucun ne peut lire le résultat d'un autre. Le nommage "pré/post" est une convention pour vous
aider à organiser votre logique (par exemple séparer "préparer des données" et "les mettre en
forme"), pas un enchaînement réel.

<!-- SCREENSHOT: les 3 zones de script d'une règle (pré-script / script / post-script) -->

Chaque bloc peut retourner :
- une **valeur simple** (texte, nombre) → utilisable comme `{{script}}` / `{{pre_script}}` /
  `{{post_script}}`,
- ou une **structure avec plusieurs champs** (`#{ nom: "...", age: 30 }`) → chaque champ devient
  utilisable individuellement, ex. `{{script.nom}}`, `{{script.age}}`.

## Fonctions disponibles

| Fonction | Ce qu'elle fait |
|---|---|
| `random_int(min, max)` | Un entier aléatoire entre `min` et `max` |
| `now_ms()` | L'horodatage courant, en millisecondes |
| `now_iso()` | La date du jour au format `AAAA-MM-JJ` |
| `year()` | L'année en cours |
| `uuid()` | Un identifiant unique (UUID) |
| `fake("Kind")` | Une donnée factice (mêmes types que dans le builder de réponse, ex. `fake("Siret")`) |
| `date_now(format?)` | La date du jour, avec un format optionnel : `"iso"` (défaut, `AAAA-MM-JJ`), `"fr"` (`JJ/MM/AAAA`), `"en"` (`MM/JJ/AAAA`) |
| `date_past(jours, format?)` | Une date dans le passé, `jours` jours avant aujourd'hui |
| `date_future(jours, format?)` | Une date dans le futur, `jours` jours après aujourd'hui |
| `seeded_int(seed, min, max)` | Un entier **toujours identique pour la même `seed`**, entre `min` et `max` |
| `seeded_pick(seed, [liste])` | Un élément de `liste`, **toujours le même pour la même `seed`** |

L'éditeur affiche ces fonctions dans une liste déroulante dès que vous commencez à taper leur nom
(ou en appuyant sur `Ctrl+Espace` pour voir la liste complète), avec leur signature et leur
description — pas besoin de mémoriser ce tableau.

<!-- SCREENSHOT: autocomplétion des fonctions Rhai pendant la frappe dans l'éditeur -->

## Cas d'usage : réponse toujours identique pour une même clé

Besoin fréquent : simuler une API qui renvoie **toujours le même résultat pour une même entrée**
(par exemple un même numéro SIRET doit toujours renvoyer le même nom d'entreprise), sans pour
autant coder une vraie base de données. C'est le rôle de `seeded_int`/`seeded_pick` : la valeur
`seed` peut être n'importe quelle donnée de la requête (`request.path.siret`, `request.query.X`,
`request.headers.X`...) — pour une même valeur de `seed`, le résultat est garanti identique à
chaque appel.

**Exemple** — service `seeded-test`, chemin `/entreprise/{siret}`, règle `GET` :

Script :
```
#{ name: seeded_pick(request.path.siret, ["Dupont SARL", "Martin SAS", "Petit EURL"]), score: seeded_int(request.path.siret, 0, 100) }
```

Corps de la réponse :
```json
{"siret":"{{path.siret}}","name":"{{script.name}}","score":{{script.score}}}
```

Résultat : `GET /seeded-test/entreprise/44306184100047` renverra systématiquement le même `name`
et le même `score` pour ce SIRET précis, et des valeurs différentes (mais toujours stables) pour
un autre SIRET.

<!-- SCREENSHOT: exemple de script seeded_pick/seeded_int et son résultat testé -->

## Prérequis et limites

- Aucun prérequis particulier : disponible dès l'installation de base, aucune configuration à
  activer.
- `seeded_int`/`seeded_pick` garantissent la **stabilité** du résultat pour une même clé, mais pas
  l'absence totale de collision entre deux clés différentes (deux SIRET distincts pourraient, très
  rarement, tomber sur le même résultat) — c'est un compromis acceptable pour du mock, pas
  approprié pour un usage nécessitant une unicité garantie.
- Les scripts ne sont **pas** exécutés pour les messages [Kafka](messaging-kafka.md) simulés — ils
  restent réservés au trafic HTTP.
- La syntaxe est validée avant sauvegarde (le formulaire signale une erreur si le script ne peut
  pas s'exécuter), mais uniquement au niveau syntaxique — une erreur de logique métier (mauvaise
  valeur calculée) ne sera pas détectée automatiquement.
