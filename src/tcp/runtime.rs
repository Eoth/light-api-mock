// Etat vivant du mock TCP : config actuelle + taches d'ecoute + statut,
// derriere un verrou partage (meme forme que MockStore, en plus simple --
// pas de write-behind/backup/rotation, le volume d'ecriture attendu ici est
// bien plus faible que la config HTTP, cf `tcp::config`).
//
// `replace()` est le seul point d'entree de mutation (utilise par l'API
// CRUD, `server/api.rs`) : persiste sur disque (ecriture atomique tmp+
// rename, meme technique que `MockStore::write_to_disk`) PUIS relance les
// listeners -- arrete proprement les anciens avant de rebind les nouveaux,
// pour que le mock TCP puisse etre reconfigure sans redemarrer tout le
// processus (donc sans couper le serveur HTTP principal).
use crate::tcp::config::TcpConfig;
use crate::tcp::listener::{TcpServiceStatus, spawn_tcp_services};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

struct Inner {
    config: TcpConfig,
    handles: Vec<JoinHandle<()>>,
    statuses: Vec<TcpServiceStatus>,
}

#[derive(Clone)]
pub struct TcpRuntime {
    data_dir: PathBuf,
    inner: Arc<RwLock<Inner>>,
}

impl TcpRuntime {
    /// Charge `{data_dir}/tcp-config.yaml` (absent/mal forme -> config vide,
    /// cf `TcpConfig::load`) et demarre les listeners correspondants. Point
    /// d'entree unique, appele une fois au demarrage (`main.rs`).
    pub async fn load_and_spawn(data_dir: &Path) -> Self {
        let config = TcpConfig::load(data_dir);
        let (handles, statuses) = spawn_tcp_services(&config).await;
        Self {
            data_dir: data_dir.to_path_buf(),
            inner: Arc::new(RwLock::new(Inner {
                config,
                handles,
                statuses,
            })),
        }
    }

    pub async fn snapshot_config(&self) -> TcpConfig {
        self.inner.read().await.config.clone()
    }

    pub async fn statuses(&self) -> Vec<TcpServiceStatus> {
        self.inner.read().await.statuses.clone()
    }

    /// Remplace la config integralement, persiste, puis relance TOUS les
    /// listeners (pas de diff fin "seul ce qui a change repart") : plus
    /// simple et plus sur pour cette premiere tranche -- le volume de
    /// services TCP attendu est faible (poignee d'entrees), le cout d'un
    /// arret/redemarrage complet des listeners a chaque mutation est
    /// negligeable. `handle.abort()` PUIS `handle.await` (pas juste abort) :
    /// necessaire pour que le port soit reellement libere avant le rebind
    /// suivant -- await sur un handle aborte ne retourne qu'une fois la
    /// tache (et la `TcpListener` qu'elle possede) effectivement detruite,
    /// pas juste la demande de cancellation envoyee.
    pub async fn replace(&self, new_config: TcpConfig) -> std::io::Result<()> {
        let yaml = serde_yaml::to_string(&new_config)
            .expect("TcpConfig n'a aucun champ pouvant echouer a la serialisation YAML");
        Self::write_to_disk(&TcpConfig::config_file(&self.data_dir), &yaml)?;

        let mut guard = self.inner.write().await;
        for handle in guard.handles.drain(..) {
            handle.abort();
            let _ = handle.await;
        }
        let (handles, statuses) = spawn_tcp_services(&new_config).await;
        guard.config = new_config;
        guard.handles = handles;
        guard.statuses = statuses;
        Ok(())
    }

    fn write_to_disk(path: &Path, yaml: &str) -> std::io::Result<()> {
        let parent = path
            .parent()
            .expect("tcp-config.yaml a toujours un parent (DATA_PATH)");
        std::fs::create_dir_all(parent)?;
        let tmp_path = parent.join(".tcp-config.yaml.tmp");
        std::fs::write(&tmp_path, yaml.as_bytes())?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcp::config::{TcpMatcher, TcpRule, TcpService};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    fn temp_dir() -> PathBuf {
        let dir = crate::server::test_support::temp_data_dir("tcp-runtime-test");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn load_and_spawn_starts_empty_with_no_config_file() {
        let dir = temp_dir();
        let runtime = TcpRuntime::load_and_spawn(&dir).await;
        assert!(runtime.snapshot_config().await.services.is_empty());
        assert!(runtime.statuses().await.is_empty());
    }

    #[tokio::test]
    async fn replace_persists_config_to_disk() {
        let dir = temp_dir();
        let runtime = TcpRuntime::load_and_spawn(&dir).await;

        let config = TcpConfig {
            services: vec![TcpService {
                name: "svc".into(),
                listen_port: 0,
                rules: vec![],
            }],
        };
        runtime.replace(config.clone()).await.unwrap();

        let on_disk = std::fs::read_to_string(TcpConfig::config_file(&dir)).unwrap();
        let parsed: TcpConfig = serde_yaml::from_str(&on_disk).unwrap();
        assert_eq!(parsed, config);
    }

    #[tokio::test]
    async fn replace_updates_live_statuses() {
        let dir = temp_dir();
        let runtime = TcpRuntime::load_and_spawn(&dir).await;
        assert!(runtime.statuses().await.is_empty());

        let config = TcpConfig {
            services: vec![TcpService {
                name: "svc".into(),
                listen_port: 0,
                rules: vec![],
            }],
        };
        runtime.replace(config).await.unwrap();

        let statuses = runtime.statuses().await;
        assert_eq!(statuses.len(), 1);
        assert!(statuses[0].listening);
    }

    #[tokio::test]
    async fn replace_on_same_port_frees_it_before_rebinding() {
        // Le cas a risque documente dans replace() : remplacer la config
        // d'un service SANS changer son port doit reussir (l'ancien
        // listener doit avoir libere le port avant que le nouveau essaie de
        // le reprendre), pas echouer en "address already in use".
        let dir = temp_dir();
        let runtime = TcpRuntime::load_and_spawn(&dir).await;

        // Port reel (pas 0) : necessaire pour reutiliser EXACTEMENT le meme
        // port au deuxieme replace() ci-dessous.
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let config = TcpConfig {
            services: vec![TcpService {
                name: "svc".into(),
                listen_port: port,
                rules: vec![TcpRule {
                    name: "r1".into(),
                    matcher: TcpMatcher::Any,
                    response_hex: crate::tcp::hex::encode(b"v1"),
                }],
            }],
        };
        runtime.replace(config).await.unwrap();
        assert!(runtime.statuses().await[0].listening);

        // Meme port, regle differente : le rebind doit reussir.
        let config2 = TcpConfig {
            services: vec![TcpService {
                name: "svc".into(),
                listen_port: port,
                rules: vec![TcpRule {
                    name: "r2".into(),
                    matcher: TcpMatcher::Any,
                    response_hex: crate::tcp::hex::encode(b"v2"),
                }],
            }],
        };
        runtime.replace(config2).await.unwrap();
        let statuses = runtime.statuses().await;
        assert!(
            statuses[0].listening,
            "rebind on the same port should succeed: {statuses:?}"
        );

        // Verifie que c'est bien la NOUVELLE regle qui repond (le nouveau
        // listener a bien pris le relais, pas l'ancien qui zombierait).
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"ping").await.unwrap();
        let mut resp = [0u8; 2];
        client.read_exact(&mut resp).await.unwrap();
        assert_eq!(&resp, b"v2");
    }
}
