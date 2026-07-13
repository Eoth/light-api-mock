# Ping de disponibilité

Sur la fiche d'un [service](services.md), un bouton **"Tester la cible (réseau uniquement)"** permet de vérifier rapidement si le vrai backend (`real_target_url`) est joignable sur le réseau.

*(Capture manquante — aucun scénario E2E existant ne déclenche le bouton "Tester la cible" [ce comportement n'est couvert que par des tests unitaires Vitest sur `UrlHealthBadge.svelte`] ; à réaliser manuellement, cf `frontend/e2e/README.md` section captures.)*

## Ce que fait (et ne fait pas) ce test

Le ping se contente d'ouvrir une **connexion réseau brute** (une simple connexion TCP) vers l'adresse et le port du backend, avec un délai d'expiration court. Il **n'envoie jamais de vraie requête HTTP** (pas de `GET`, pas de `HEAD`, aucun appel à une route de l'API cible) :

- ✅ "Joignable" = le réseau permet d'atteindre cette adresse.
- ❌ Cela ne dit **rien** sur la santé fonctionnelle de l'API (elle peut très bien être en panne applicative tout en étant "joignable" réseau).

Ce choix est volontaire : le ping ne doit jamais déclencher d'effet de bord sur le vrai backend(pas de log applicatif généré côté cible, pas de consommation de quota d'API, etc.).

## Les différents statuts affichés

| Statut | Signification |
|---|---|
| Non testé | Aucun test n'a encore été lancé pour ce service |
| Test en cours | La vérification réseau est en train de s'exécuter |
| Accessible | La connexion réseau a réussi |
| Inaccessible | La connexion réseau a échoué (backend éteint, mauvaise adresse, pare-feu...) |
| Expiré | Le dernier résultat date de plus de 2 minutes — relancez un test pour un résultat à jour |

Un clic répété dans les 2 minutes qui suivent un test réutilise directement le dernier résultat connu, sans relancer un nouveau test réseau.

## Prérequis et limites

- Aucun prérequis particulier : disponible dès l'installation de base.
- Uniquement disponible si le service a une `real_target_url` renseignée.
- Le résultat n'est **pas sauvegardé** dans la configuration du service : c'est un statut temporaire, qui repart à zéro si l'application lightMock redémarre.
- Si deux services portant le même nom existent dans des groupes différents, le badge de disponibilité peut, pendant une courte fenêtre (2 minutes), afficher le résultat de l'un pour l'autre — un détail d'affichage mineur, sans conséquence fonctionnelle (ce badge n'affecte jamais le comportement réel du service).
