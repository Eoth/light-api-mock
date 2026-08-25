// Mock TCP brut pour des protocoles binaires non-HTTP simples, un message ->
// une reponse fixe (LDAP, SMTP, protocole maison...). Feature Cargo
// "tcp-mock", NON active par defaut : ce module ne compile meme pas sinon
// (voir le `#[cfg(feature = ...)]` sur la declaration `pub mod tcp;` dans
// main.rs), donc zero impact sur le binaire/tests tant que non activee --
// meme contrat que `messaging/` pour Kafka. Schema (TcpService/TcpRule) ET
// comportement (matching, ecoute) entierement dans ce module, INDEPENDANT
// de `models::MockConfig` -- voir le commentaire en tete de `config.rs`
// pour la raison (rayon d'impact sur les tests HTTP existants).
//
// AUCUN MODE PROXY/RELAIS -- decision explicite de l'utilisateur, retire de
// la premiere version de cette tranche. Raison : un relais TCP brut qui ne
// fait que retransmettre sans matching n'ajoute AUCUNE valeur de mock, et
// fait passer le trafic par un intermediaire (lightMock) qui de toute facon
// rejoint la meme cible reelle -- l'appelant peut s'y connecter directement.
// Faire transiter ce trafic par lightMock sans contrepartie fonctionnelle
// est un saut reseau superflu, contraire au principe de sobriete (§3 du
// handoff de reprise) : autant aller directement a la source que de passer
// par un intermediaire qui ira a la meme source. Consequence dans le code :
// `TcpService` n'a pas de cible reelle configurable, `TcpRule` n'a pas de
// notion d'action proxy, `handle_connection` (listener.rs) ne fait qu'une
// chose si rien ne matche : fermer la connexion.
//
// Ce que ce module PERMET reellement aujourd'hui : mocker un protocole
// binaire simple ou chaque connexion est UN message envoye par le client
// suivi d'UNE reponse fixe attendue (ex. un ping/heartbeat proprietaire, un
// handshake fixe, un petit RPC binaire sans etat). Le matching porte sur des
// octets bruts (prefixe hexadecimal ou regex binaire), aucun decodage
// protocolaire.
//
// Ce que ce module NE PERMET PAS, et pourquoi ce n'est pas juste "pas encore
// fait" mais une limitation structurelle de ce qui a ete construit :
//   - LDAP/AD (bind + search + unbind) : un vrai client LDAP enchaine
//     plusieurs messages BER sur la MEME connexion, chacun attendant sa
//     propre reponse -- ce module ne gere qu'un seul message par connexion
//     (pas de boucle lecture/reponse). Meme avec une boucle, chaque BIND
//     encode un message-id qui change a chaque appel et une longueur BER
//     recalculee selon le contenu : deux BIND consecutifs du meme client ne
//     produisent PAS les memes octets, ce qui rend un matching prefixe/regex
//     sur octets bruts fragile des qu'on sort d'un cas a un seul appel fige.
//     Il faudrait au minimum une boucle multi-echanges par connexion, et un
//     minimum de conscience du framing BER (savoir ou un message finit dans
//     le flux) -- pas construit ici, et pas juste un oubli : plus gros que
//     le perimetre volontairement retenu pour cette tranche.
//   - SMTP (mail) : meme categorie de probleme (EHLO/MAIL FROM/RCPT TO/DATA
//     = plusieurs echanges sur une connexion), meme cause (pas de boucle
//     multi-echanges). Le framing texte ASCII de SMTP est plus simple que le
//     BER de LDAP, mais le prerequis (boucle par connexion) est identique et
//     manque tout autant.
//   - Tout protocole a etat sur plusieurs messages, de facon generale.
//   - Le mode "proxifier vers un vrai serveur" : retire (voir ci-dessus),
//     jamais reintroduit sans une vraie contrepartie de mock/matching.
//
// Reconfigurable a chaud via l'API REST (`GET/POST/PUT/DELETE /tcp/services`,
// `server/api.rs`, feature-gated) : `TcpRuntime::replace()` persiste la
// nouvelle config sur disque puis relance TOUS les listeners (pas de diff
// fin service par service -- volume attendu faible, cout negligeable). Le
// serveur HTTP principal n'est jamais affecte par ce redemarrage, qui ne
// touche que les ecoutes TCP. Pas encore d'UI Svelte au-dessus de cette API.
pub mod config;
pub mod hex;
pub mod listener;
pub mod matcher;
pub mod runtime;
pub mod validation;

pub use listener::{TcpServiceStatus, spawn_tcp_services};
pub use runtime::TcpRuntime;
