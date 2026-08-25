// Mock/proxy TCP brut pour des protocoles binaires non-HTTP (LDAP, SMTP,
// protocole maison...). Feature Cargo "tcp-proxy", NON active par defaut :
// ce module ne compile meme pas sinon (voir le `#[cfg(feature = ...)]` sur
// la declaration `pub mod tcp;` dans main.rs), donc zero impact sur le
// binaire/tests tant que non activee -- meme contrat que `messaging/` pour
// Kafka. Schema (TcpService/TcpRule) ET comportement (matching, ecoute,
// proxy) entierement dans ce module, INDEPENDANT de `models::MockConfig` --
// voir le commentaire en tete de `config.rs` pour la raison (rayon d'impact
// sur les tests HTTP existants).
//
// Etat de cette premiere tranche : moteur fonctionnel (ecoute par service,
// matching prefixe/regex sur octets bruts, mock ou proxy transparent),
// configurable UNIQUEMENT via un fichier YAML ecrit a la main
// (`{DATA_PATH}/tcp-config.yaml`) aujourd'hui -- pas encore d'endpoints API
// ni d'UI (a construire dans une passe suivante si ce chantier est
// confirme). Le port ecoute par service est fige au demarrage du processus
// (pas de rebind a chaud, pas de rechargement du fichier a chaud). Chaque
// connexion mock est request/response unique (lit un message, repond, ne
// maintient pas de session multi-echanges) -- suffisant pour un protocole
// binaire simple requete/reponse, PAS pour rejouer une sequence complete
// (ex. LDAP bind + search + unbind sur la meme connexion) : cf handoff,
// limitation documentee plutot que silencieuse.
pub mod config;
pub mod hex;
pub mod listener;
pub mod matcher;

pub use listener::{TcpServiceStatus, spawn_tcp_services};
