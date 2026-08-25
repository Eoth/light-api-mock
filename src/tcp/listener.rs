// Ecoute TCP brute, une tache tokio par service configure, en parallele du
// serveur HTTP principal (jamais sur le meme port : un protocole binaire ne
// se multiplexe pas par chemin d'URL, cf la note d'etude "TCP brut" du
// handoff de reprise). Un service TCP existe hors du routeur Axum -- pas
// d'auth, pas de CORS, pas de middleware HTTP : ce sont des concepts qui ne
// s'appliquent pas a une socket brute.
//
// MOCK UNIQUEMENT (pas de relais/proxy) -- decision explicite, cf
// `tcp::mod` : un relais qui ne fait que retransmettre n'ajoute aucune
// valeur de mock, et route le trafic vers un intermediaire qui de toute
// facon rejoint la meme cible reelle -- l'appelant peut s'y connecter
// directement, ce qui rend le detour par lightMock a la fois inutile et
// contraire au principe de sobriete (un saut reseau de plus pour un
// resultat identique).
use crate::tcp::config::TcpService;
use crate::tcp::{hex, matcher};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_MAX_MESSAGE_SIZE: usize = 16 * 1024;

/// Meme idiome que `request_log::max_body_size`/`message_log::max_body_size`
/// (env var, defaut 16 Ko) : borne la lecture initiale d'une connexion pour
/// eviter qu'un client hostile ne fasse grossir la memoire sans limite avant
/// tout matching (cf le principe de limites de ressources explicites pour le
/// parsing d'entree non fiable).
pub fn max_message_size() -> usize {
    std::env::var("TCP_MOCK_MAX_MESSAGE_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_MESSAGE_SIZE)
}

/// Statut d'un service TCP au demarrage -- expose via `GET /tcp/status`
/// (`server/api.rs`) pour que l'operateur puisse voir sans grep-er les logs
/// si un port a effectivement pu etre bind. Fige a l'instant du demarrage,
/// comme la config elle-meme (pas de re-verification periodique).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TcpServiceStatus {
    pub name: String,
    pub listen_port: u16,
    pub listening: bool,
    pub error: Option<String>,
}

/// Demarre un accept-loop par service TCP configure au demarrage du
/// processus. Config figee pour toute la duree du process (pas d'API pour
/// la modifier a chaud dans cette premiere tranche, cf `tcp::config`) :
/// chaque tache capture directement le `TcpService` dont elle a besoin, pas
/// de re-lecture d'un store partage a chaque connexion.
///
/// Un service dont le port ne peut pas etre bind (deja pris, permissions...)
/// est logge en erreur et simplement ignore -- ne doit jamais empecher le
/// demarrage du serveur HTTP principal ni des autres services TCP.
pub async fn spawn_tcp_services(
    config: &crate::tcp::config::TcpConfig,
) -> (Vec<JoinHandle<()>>, Vec<TcpServiceStatus>) {
    let mut handles = Vec::with_capacity(config.services.len());
    let mut statuses = Vec::with_capacity(config.services.len());

    for service in &config.services {
        let addr = format!("0.0.0.0:{}", service.listen_port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(
                    service = %service.name,
                    addr = %addr,
                    error = %e,
                    "tcp-mock: failed to bind, service skipped"
                );
                statuses.push(TcpServiceStatus {
                    name: service.name.clone(),
                    listen_port: service.listen_port,
                    listening: false,
                    error: Some(e.to_string()),
                });
                continue;
            }
        };
        tracing::info!(service = %service.name, addr = %addr, "tcp-mock: listening");
        statuses.push(TcpServiceStatus {
            name: service.name.clone(),
            listen_port: service.listen_port,
            listening: true,
            error: None,
        });

        let service = Arc::new(service.clone());
        handles.push(tokio::spawn(async move {
            loop {
                let (stream, peer) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::warn!(service = %service.name, error = %e, "tcp-mock: accept failed");
                        continue;
                    }
                };
                let service = service.clone();
                tokio::spawn(async move {
                    tracing::debug!(service = %service.name, peer = %peer, "tcp-mock: connection accepted");
                    handle_connection(stream, service).await;
                });
            }
        }));
    }

    (handles, statuses)
}

/// Une connexion = lit un message, matche, repond si une regle matche, puis
/// ferme. Pas de boucle multi-echanges (cf limitation documentee dans
/// `tcp::mod`) : suffisant pour un protocole simple requete/reponse, pas
/// pour une sequence de plusieurs messages sur la meme connexion. Aucun
/// repli proxy si rien ne matche : sans regle, la connexion ferme
/// simplement (pas de destination a atteindre, il n'y en a plus).
async fn handle_connection(mut stream: TcpStream, service: Arc<TcpService>) {
    let mut buf = vec![0u8; max_message_size()];
    let n = match tokio::time::timeout(READ_TIMEOUT, stream.read(&mut buf)).await {
        Ok(Ok(0)) => return,
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            tracing::debug!(service = %service.name, error = %e, "tcp-mock: read failed");
            return;
        }
        Err(_) => {
            tracing::debug!(service = %service.name, "tcp-mock: read timeout, closing");
            return;
        }
    };
    let data = &buf[..n];

    let Some(rule) = matcher::match_rule(&service.rules, data) else {
        tracing::debug!(service = %service.name, "tcp-mock: no rule matched, closing");
        return;
    };

    match hex::decode(&rule.response_hex) {
        Ok(response) => {
            if let Err(e) = stream.write_all(&response).await {
                tracing::debug!(
                    service = %service.name, rule = %rule.name, error = %e,
                    "tcp-mock: mock write failed"
                );
            }
        }
        Err(e) => {
            tracing::error!(
                service = %service.name, rule = %rule.name, error = %e,
                "tcp-mock: invalid response_hex in matched rule, closing without reply"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcp::config::{TcpMatcher, TcpRule};

    fn svc(rules: Vec<TcpRule>) -> Arc<TcpService> {
        Arc::new(TcpService {
            name: "svc".into(),
            listen_port: 0,
            rules,
        })
    }

    #[tokio::test]
    async fn mock_rule_replies_with_configured_bytes() {
        let service = svc(vec![TcpRule {
            name: "hello".into(),
            matcher: TcpMatcher::Any,
            response_hex: hex::encode(b"pong"),
        }]);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, service).await;
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"ping").await.unwrap();
        let mut resp = [0u8; 4];
        client.read_exact(&mut resp).await.unwrap();
        assert_eq!(&resp, b"pong");
    }

    #[tokio::test]
    async fn no_matching_rule_closes_connection_without_reply() {
        let service = svc(vec![TcpRule {
            name: "specific".into(),
            matcher: TcpMatcher::Prefix(hex::encode(b"X")),
            response_hex: hex::encode(b"nope-should-not-see-this"),
        }]);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, service).await;
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"ping").await.unwrap();
        let mut resp = Vec::new();
        let n = tokio::time::timeout(Duration::from_millis(300), client.read_to_end(&mut resp))
            .await
            .unwrap_or(Ok(0))
            .unwrap_or(0);
        assert_eq!(n, 0);
        assert!(resp.is_empty());
    }

    #[tokio::test]
    async fn no_rules_at_all_closes_connection_without_reply() {
        let service = svc(vec![]);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, service).await;
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"ping").await.unwrap();
        let mut resp = Vec::new();
        let n = tokio::time::timeout(Duration::from_millis(300), client.read_to_end(&mut resp))
            .await
            .unwrap_or(Ok(0))
            .unwrap_or(0);
        assert_eq!(n, 0);
    }

    #[test]
    fn max_message_size_defaults_to_16kb() {
        // SAFETY-note (pas d'unsafe ici) : simple lecture d'env var absente ;
        // cette variable n'est fixee par aucun autre test de la suite.
        unsafe { std::env::remove_var("TCP_MOCK_MAX_MESSAGE_SIZE") };
        assert_eq!(max_message_size(), 16 * 1024);
    }

    #[tokio::test]
    async fn spawn_reports_listening_true_for_a_free_port() {
        // Port 0 = attribue par l'OS, toujours libre par construction ; on ne
        // peut donc pas exprimer "port deja pris" avec ce meme mecanisme (cf
        // le test suivant, qui reserve explicitement un port avant coup).
        let config = crate::tcp::config::TcpConfig {
            services: vec![TcpService {
                name: "free-port".into(),
                listen_port: 0,
                rules: vec![],
            }],
        };
        let (_handles, statuses) = spawn_tcp_services(&config).await;
        assert_eq!(statuses.len(), 1);
        assert!(statuses[0].listening);
        assert!(statuses[0].error.is_none());
    }

    #[tokio::test]
    async fn spawn_reports_listening_false_when_port_already_taken() {
        // Reserve un port reel en le bindant nous-memes AVANT d'appeler
        // spawn_tcp_services avec ce meme port -- garantit un conflit
        // deterministe (contrairement a un port fixe au hasard, qui pourrait
        // etre libre sur la machine de CI). "0.0.0.0" et pas "127.0.0.1" :
        // spawn_tcp_services bind sur "0.0.0.0:{port}" (toutes interfaces),
        // reserver seulement l'interface loopback ne cree pas toujours un
        // conflit reel selon l'OS (observe sur Windows).
        let reserved = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let taken_port = reserved.local_addr().unwrap().port();

        let config = crate::tcp::config::TcpConfig {
            services: vec![TcpService {
                name: "taken-port".into(),
                listen_port: taken_port,
                rules: vec![],
            }],
        };
        let (handles, statuses) = spawn_tcp_services(&config).await;
        assert_eq!(statuses.len(), 1);
        assert!(!statuses[0].listening);
        assert!(statuses[0].error.is_some());
        // Aucune tache d'ecoute demarree pour ce service en echec.
        assert!(handles.is_empty());

        drop(reserved);
    }
}
