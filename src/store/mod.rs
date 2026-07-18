// Persistance de la config (services + groupes) en YAML sur disque.
// Architecture : Arc<RwLock<Arc<MockConfig>>> pour lectures O(1) sans clone.
// snapshot() retourne un Arc (clone du pointeur, pas des donnees).
// Les mutations (update/replace) clonent les donnees, les modifient, mettent
// a jour l'Arc en memoire INSTANTANEMENT, puis delegent l'ecriture disque
// effective (tmp write + rename) a une tache de fond via un channel
// (write-behind, cf WriterHandle plus bas) — le thread de requete HTTP n'est
// donc plus jamais bloque par l'I/O disque elle-meme.
//
// Backup/rollback : avant chaque ecrasement du fichier de config, l'ancien
// contenu est copie dans {data_dir}/backups/. C'est event-driven (declenche
// par la mutation elle-meme), PAS une tache de fond/cron — coherent avec le
// choix fait pour le ping (src/server/ping.rs). Rotation immediate apres
// coup pour ne garder que BACKUP_MAX_COUNT fichiers (defaut 5), necessaire
// vu la contrainte PVC 64Mi en K8s.
//
// IMPORTANT (write-behind) : le backup reste SYNCHRONE et s'execute AVANT la
// mise en queue de l'ecriture asynchrone (voir prepare_and_backup(), appelee
// sous le verrou d'ecriture avant tout appel a WriterHandle::send_write()).
// Une mutation ne passe donc jamais sans sauvegarde prealable — casser cet
// ordre viderait le mecanisme de rollback de son utilite.
//
// Backup pre-reset protege : avant un reset complet (DELETE /api/config/reset),
// backup_before_reset() copie la config courante dans backups/protected/. Ce
// sous-repertoire est EXEMPT de rotate_backups() (qui ne liste que les
// fichiers directement dans backups/, pas ses sous-dossiers) — un reset ne
// peut donc jamais se faire "avaler" par le quota BACKUP_MAX_COUNT classique
// a cause d'ecritures ulterieures. Il n'est purge que par expiration
// (PROTECTED_BACKUP_MAX_AGE_MS, 30 jours), verifiee de facon opportuniste a
// chaque ecriture normale (toujours event-driven, jamais de timer/cron) :
// l'interpretation retenue est qu'une ecriture survenant >=30 jours apres le
// reset est le signal que plus personne ne depend de cet ancien etat — pas de
// comptage d'activite plus fin que ca.
use crate::models::MockConfig;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};

static BACKUP_SEQ: AtomicU64 = AtomicU64::new(0);

const PROTECTED_BACKUP_MAX_AGE_MS: u128 = 30 * 24 * 60 * 60 * 1000;

// Capacite du channel d'ecriture asynchrone (write-behind). Bornee
// volontairement : le but de cette evolution est de ne plus bloquer le
// thread de requete sur l'I/O disque, PAS d'autoriser une file d'attente
// illimitee qui grossirait sans controle en cas de pic d'ecritures (sobriete
// ressources). Chaque mutation flush individuellement (pas de
// vrai batching, voir WriteJob et le commentaire sur run()) : avec ce
// pattern, une capacite de 64 absorbe largement les pics realistes. Au-dela,
// send_write().await applique un backpressure naturel (le call site attend
// qu'une place se libere) plutot que de perdre des ecritures ou de laisser
// la memoire grossir indefiniment — c'est un choix delibere, pas une limite
// subie. Constante fixe (pas de variable d'env comme BACKUP_MAX_COUNT) : il
// s'agit d'une soupape de securite interne, pas d'un reglage utilisateur.
const WRITE_QUEUE_CAPACITY: usize = 64;

/// Un job pousse dans le channel du writer asynchrone.
/// - `Write` : contenu YAML deja serialise a persister sur `path`.
/// - `Barrier` : ne fait aucune ecriture, signale juste au demandeur (via le
///   oneshot) que tous les jobs pousses AVANT lui ont ete traites. Utilise par
///   `flush()` (tests deterministes sans sleep, et drain a l'arret gracieux
///   du serveur) — jamais par le chemin de mutation normal.
enum WriteJob {
    Write { path: PathBuf, yaml: String },
    Barrier(oneshot::Sender<()>),
}

#[derive(Default)]
struct WriterStats {
    last_write_ok_ms: AtomicU64,
    last_error: std::sync::RwLock<Option<WriterError>>,
}

/// Derniere erreur d'ecriture disque rencontree par la tache de fond.
/// Expose via GET /api/health pour la detection d'un backlog/probleme
/// disque en observabilite K8s.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WriterError {
    pub message: String,
    pub at_ms: u64,
}

/// Etat observable du write-behind, expose via GET /api/health.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WriteQueueStatus {
    pub pending: usize,
    pub capacity: usize,
    pub last_write_ok_ms: Option<u64>,
    pub last_error: Option<WriterError>,
}

/// Handle cote "producteur" du write-behind : chaque `MockStore` en detient
/// un clone (Sender + stats partages), la tache de fond en detient le
/// `Receiver` correspondant (voir `WriterHandle::spawn`).
#[derive(Clone)]
struct WriterHandle {
    tx: mpsc::Sender<WriteJob>,
    stats: Arc<WriterStats>,
}

impl WriterHandle {
    fn spawn() -> Self {
        let (tx, rx) = mpsc::channel(WRITE_QUEUE_CAPACITY);
        let stats = Arc::new(WriterStats::default());
        tokio::spawn(Self::run(rx, stats.clone()));
        Self { tx, stats }
    }

    /// Boucle de la tache de fond : consomme le channel jusqu'a sa
    /// fermeture. PAS de polling/timer — purement event-driven (un `recv()`
    /// qui attend le prochain message), coherent avec le reste du projet
    /// (ping, backups). Choix assume : flush par mutation individuelle
    /// plutot qu'un vrai batching (regrouper plusieurs jobs avant d'ecrire)
    /// — un batching reduirait le nombre d'ecritures mais elargirait la
    /// fenetre de risque en cas de crash (plus de mutations en memoire sans
    /// contrepartie sur disque). Le gain recherche ici est de ne plus
    /// bloquer le thread de requete, pas de reduire le nombre d'IO.
    ///
    /// Une erreur d'ecriture est loguee (niveau error) et enregistree dans
    /// `stats.last_error`, MAIS ne fait pas paniquer la tache : le call site
    /// HTTP a deja repondu succes a ce stade (la mutation est appliquee en
    /// memoire), donc la tache continue de traiter les jobs suivants plutot
    /// que d'abandonner tout write-behind futur pour une erreur transitoire.
    async fn run(mut rx: mpsc::Receiver<WriteJob>, stats: Arc<WriterStats>) {
        while let Some(job) = rx.recv().await {
            match job {
                WriteJob::Write { path, yaml } => match MockStore::write_to_disk(&path, &yaml) {
                    Ok(()) => {
                        stats
                            .last_write_ok_ms
                            .store(MockStore::now_ms() as u64, Ordering::Relaxed);
                    }
                    Err(e) => {
                        tracing::error!(
                            path = %path.display(),
                            error = %e,
                            "write-behind: echec de la persistance disque — l'etat en memoire est en avance sur le disque, perte possible si le processus s'arrete avant la prochaine ecriture reussie"
                        );
                        *stats.last_error.write().unwrap() = Some(WriterError {
                            message: e.to_string(),
                            at_ms: MockStore::now_ms() as u64,
                        });
                    }
                },
                WriteJob::Barrier(done) => {
                    let _ = done.send(());
                }
            }
        }
        tracing::warn!("write-behind: tache d'ecriture arretee (channel ferme)");
    }

    /// Pousse une ecriture dans la file. `send().await` applique un
    /// backpressure naturel si la file est pleine (voir WRITE_QUEUE_CAPACITY)
    /// — c'est le seul cas ou une mutation peut de nouveau attendre sur le
    /// write-behind, et c'est volontaire (soupape de securite plutot qu'une
    /// file illimitee). Si le Receiver a ete abandonne (tache de fond morte,
    /// ne devrait pas arriver en fonctionnement normal), on logue au lieu de
    /// paniquer : la mutation reste appliquee en memoire.
    async fn send_write(&self, path: PathBuf, yaml: String) {
        if self
            .tx
            .send(WriteJob::Write { path, yaml })
            .await
            .is_err()
        {
            tracing::error!(
                "write-behind: tache d'ecriture indisponible, mutation appliquee en memoire uniquement (pas persistee)"
            );
        }
    }

    /// Attend que tous les jobs pousses AVANT cet appel aient ete traites
    /// (succes ou echec). Utilise par les tests (assertions deterministes
    /// sans sleep) et par l'arret gracieux du serveur (drainer la file avant
    /// de quitter, cf main.rs).
    async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(WriteJob::Barrier(tx)).await.is_ok() {
            let _ = rx.await;
        }
    }

    fn status(&self) -> WriteQueueStatus {
        let pending = WRITE_QUEUE_CAPACITY.saturating_sub(self.tx.capacity());
        let last_write_ok_ms = match self.stats.last_write_ok_ms.load(Ordering::Relaxed) {
            0 => None,
            v => Some(v),
        };
        let last_error = self.stats.last_error.read().unwrap().clone();
        WriteQueueStatus {
            pending,
            capacity: WRITE_QUEUE_CAPACITY,
            last_write_ok_ms,
            last_error,
        }
    }
}

#[derive(Clone)]
pub struct MockStore {
    config: Arc<RwLock<Arc<MockConfig>>>,
    path: PathBuf,
    writer: WriterHandle,
}

impl MockStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(Arc::new(MockConfig::empty()))),
            path,
            writer: WriterHandle::spawn(),
        }
    }

    pub fn data_path() -> PathBuf {
        std::env::var("DATA_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"))
    }

    pub fn config_file(data_dir: &Path) -> PathBuf {
        data_dir.join("mock-config.yaml")
    }

    pub fn backup_max_count() -> usize {
        std::env::var("BACKUP_MAX_COUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5)
    }

    pub async fn load_or_init(data_dir: &Path) -> Result<Self, StoreError> {
        let file = Self::config_file(data_dir);
        std::fs::create_dir_all(data_dir).map_err(|e| StoreError::Io(e.to_string()))?;

        // Bootstrap : ecriture synchrone (pas de write-behind ici). C'est le
        // seul chemin d'ecriture qui reste bloquant intentionnellement — il
        // s'execute avant que le serveur ne commence a accepter du trafic,
        // donc aucune requete HTTP n'attend dessus, et le contrat "le fichier
        // existe des le retour de load_or_init" doit rester vrai sans flush().
        let config = if file.exists() {
            let content =
                std::fs::read_to_string(&file).map_err(|e| StoreError::Io(e.to_string()))?;
            serde_yaml::from_str(&content).map_err(|e| StoreError::Yaml(e.to_string()))?
        } else {
            let empty = MockConfig::empty();
            let yaml = serde_yaml::to_string(&empty).map_err(|e| StoreError::Yaml(e.to_string()))?;
            Self::write_to_disk(&file, &yaml)?;
            empty
        };

        tracing::info!(path = %file.display(), services = config.services.len(), "config loaded");

        Ok(Self {
            config: Arc::new(RwLock::new(Arc::new(config))),
            path: file,
            writer: WriterHandle::spawn(),
        })
    }

    pub async fn snapshot(&self) -> Arc<MockConfig> {
        self.config.read().await.clone()
    }

    /// Applique la mutation instantanement en memoire, puis delegue
    /// l'ecriture disque a la tache de fond (write-behind). Le verrou
    /// d'ecriture est tenu du debut (backup synchrone) jusqu'a la mise en
    /// queue incluse : ca serialise les mutations concurrentes dans le meme
    /// ordre que les jobs arrivent dans le channel (FIFO, un seul
    /// consommateur), donc l'ordre des ecritures sur disque respecte
    /// toujours l'ordre des mutations.
    pub async fn replace(&self, config: MockConfig) -> Result<(), StoreError> {
        let mut guard = self.config.write().await;
        let yaml = Self::prepare_and_backup(&self.path, &config)?;
        *guard = Arc::new(config);
        self.writer.send_write(self.path.clone(), yaml).await;
        Ok(())
    }

    pub async fn update<F>(&self, f: F) -> Result<Arc<MockConfig>, StoreError>
    where
        F: FnOnce(&mut MockConfig),
    {
        let mut guard = self.config.write().await;
        let mut cfg = (**guard).clone();
        f(&mut cfg);
        let yaml = Self::prepare_and_backup(&self.path, &cfg)?;
        *guard = Arc::new(cfg);
        self.writer.send_write(self.path.clone(), yaml).await;
        Ok(guard.clone())
    }

    /// Attend que toutes les mutations deja soumises aient ete persistees
    /// sur disque (succes ou echec). A utiliser pour des assertions de test
    /// deterministes, ou a l'arret gracieux du serveur pour drainer la file
    /// avant de quitter (cf main.rs). Ne fait PAS partie du chemin de
    /// mutation normal — les handlers HTTP ne l'appellent jamais.
    pub async fn flush(&self) {
        self.writer.flush().await;
    }

    /// Etat observable du write-behind (taille de file, derniere ecriture
    /// reussie, derniere erreur), expose par GET /api/health.
    pub fn writer_status(&self) -> WriteQueueStatus {
        self.writer.status()
    }

    /// Liste les backups disponibles (backups/ + backups/protected/), triees
    /// des plus recents aux plus anciens. Lecture de metadonnees fichier
    /// uniquement (nom, taille, mtime via DirEntry::metadata()) — le contenu
    /// YAML n'est jamais charge pour construire cette liste.
    pub async fn list_backups(&self) -> Result<Vec<BackupInfo>, StoreError> {
        let parent = self.path.parent().ok_or_else(|| {
            StoreError::Io("config path has no parent directory".into())
        })?;

        let backups_dir = parent.join("backups");
        let mut result = Vec::new();
        Self::collect_backups_dir(&backups_dir, false, &mut result)?;
        Self::collect_backups_dir(&backups_dir.join("protected"), true, &mut result)?;

        result.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
        Ok(result)
    }

    fn collect_backups_dir(
        dir: &Path,
        protected: bool,
        out: &mut Vec<BackupInfo>,
    ) -> Result<(), StoreError> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir).map_err(|e| StoreError::Io(e.to_string()))? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let Some(filename) = path.file_name().and_then(|n| n.to_str()) else { continue };

            let created_at_ms = Self::extract_backup_timestamp(&path, protected)
                .unwrap_or_else(|| {
                    meta.modified()
                        .ok()
                        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                }) as u64;

            out.push(BackupInfo {
                filename: filename.to_string(),
                protected,
                size_bytes: meta.len(),
                created_at_ms,
            });
        }
        Ok(())
    }

    fn extract_backup_timestamp(path: &Path, protected: bool) -> Option<u128> {
        if protected {
            Self::extract_protected_timestamp(path)
        } else {
            path.file_stem()?
                .to_str()?
                .strip_prefix("mock-config-")?
                .split('-')
                .next()?
                .parse::<u128>()
                .ok()
        }
    }

    /// Restaure la config depuis un fichier de backup (backups/ ou
    /// backups/protected/). `filename` doit deja avoir ete valide par
    /// l'appelant (`validate_backup_filename`, anti-traversal) — cette
    /// methode ne refait pas cette verification, elle se contente de
    /// chercher le nom tel quel dans les deux repertoires de backups.
    /// Reutilise `replace()` pour la reecriture : `atomic_write()` cree donc
    /// automatiquement un backup de l'etat courant AVANT d'ecraser avec le
    /// contenu restaure, sans logique dupliquee.
    pub async fn restore_from_backup(&self, filename: &str) -> Result<(), StoreError> {
        let parent = self.path.parent().ok_or_else(|| {
            StoreError::Io("config path has no parent directory".into())
        })?;

        let backups_dir = parent.join("backups");
        let candidate = backups_dir.join(filename);
        let protected_candidate = backups_dir.join("protected").join(filename);

        let source = if candidate.is_file() {
            candidate
        } else if protected_candidate.is_file() {
            protected_candidate
        } else {
            return Err(StoreError::NotFound(format!(
                "backup file not found: {filename}"
            )));
        };

        let content =
            std::fs::read_to_string(&source).map_err(|e| StoreError::Io(e.to_string()))?;
        let config: MockConfig =
            serde_yaml::from_str(&content).map_err(|e| StoreError::Yaml(e.to_string()))?;

        tracing::info!(path = %source.display(), "config restore from backup");
        self.replace(config).await
    }

    /// Sauvegarde protegee avant un reset complet. A appeler explicitement
    /// AVANT `replace(MockConfig::empty())` dans le handler de reset — ne
    /// fait pas partie du chemin d'ecriture normal (atomic_write), donc les
    /// ecritures ordinaires ne creent jamais de backup "protected".
    pub async fn backup_before_reset(&self) -> Result<(), StoreError> {
        if !self.path.exists() {
            return Ok(());
        }
        let parent = self.path.parent().ok_or_else(|| {
            StoreError::Io("config path has no parent directory".into())
        })?;

        let protected_dir = parent.join("backups").join("protected");
        std::fs::create_dir_all(&protected_dir).map_err(|e| StoreError::Io(e.to_string()))?;

        let dest = protected_dir.join(format!("pre-reset-{}.yaml", Self::now_ms()));
        std::fs::copy(&self.path, &dest).map_err(|e| StoreError::Io(e.to_string()))?;
        tracing::info!(path = %dest.display(), "pre-reset backup created (protected, 30j)");
        Ok(())
    }

    /// Partie SYNCHRONE de l'ecriture : serialise en YAML, purge les backups
    /// proteges expires, puis sauvegarde le fichier existant AVANT tout
    /// remplacement. Appelee sous le verrou d'ecriture, avant la mise en
    /// queue de l'ecriture asynchrone — voir le commentaire en tete de
    /// fichier sur la contrainte "backup avant write-behind".
    fn prepare_and_backup(path: &Path, config: &MockConfig) -> Result<String, StoreError> {
        let yaml = serde_yaml::to_string(config).map_err(|e| StoreError::Yaml(e.to_string()))?;

        let parent = path.parent().ok_or_else(|| {
            StoreError::Io("config path has no parent directory".into())
        })?;

        Self::purge_expired_protected_backups(parent)?;
        Self::backup_before_overwrite(path, parent)?;

        Ok(yaml)
    }

    /// Partie ASYNCHRONE (write-behind) : ecrit le YAML deja serialise sur
    /// disque via tmp write + rename. Appelee uniquement par la tache de
    /// fond du writer (WriterHandle::run) — sauf pour l'ecriture de bootstrap
    /// dans load_or_init(), qui reste volontairement synchrone (cf commentaire
    /// sur load_or_init).
    fn write_to_disk(path: &Path, yaml: &str) -> Result<(), StoreError> {
        let parent = path.parent().ok_or_else(|| {
            StoreError::Io("config path has no parent directory".into())
        })?;

        let tmp_path = parent.join(".mock-config.yaml.tmp");
        std::fs::write(&tmp_path, yaml.as_bytes()).map_err(|e| StoreError::Io(e.to_string()))?;
        std::fs::rename(&tmp_path, path).map_err(|e| StoreError::Io(e.to_string()))?;

        Ok(())
    }

    /// Purge les backups pre-reset (backups/protected/) plus vieux que
    /// PROTECTED_BACKUP_MAX_AGE_MS. Opportuniste : declenche par l'ecriture
    /// en cours, pas de timer. Age lu depuis le timestamp encode dans le nom
    /// de fichier (`pre-reset-{ts}.yaml`), pas depuis les metadonnees disque.
    ///
    /// Appelee sous le verrou d'ecriture, sur CHAQUE mutation (§3/§5 point 22,
    /// "opportuniste a chaque ecriture normale") : `backups/protected/` n'est
    /// borne QUE par cette purge (pas de rotation par quantite comme
    /// `backups/`), donc en usage reel (E2E ou dev quotidien qui reset
    /// souvent, jamais 30 jours sans mutation) ce dossier peut compter des
    /// centaines/milliers de fichiers avant sa premiere expiration. `entry
    /// .file_type()` (plutot que `entry.path().is_file()`, qui refait un
    /// `stat()` par fichier) reutilise le type deja renvoye par l'enumeration
    /// du repertoire (gratuit sur Windows via WIN32_FIND_DATAW, pas d'appel
    /// systeme supplementaire) : evite N stats couteux inutiles a CHAQUE
    /// mutation de l'appli des que ce dossier grossit. Diagnostique via un
    /// dossier `backups/protected/` de dev local a >1300 entrees (accumule
    /// par des mois de sessions E2E, chaque `beforeEach` faisant un
    /// `DELETE /api/config/reset`) : cf le commentaire du test
    /// `purge_expired_protected_backups_correct_with_many_entries` plus bas
    /// pour la mesure avant/apres.
    fn purge_expired_protected_backups(parent: &Path) -> Result<(), StoreError> {
        let protected_dir = parent.join("backups").join("protected");
        if !protected_dir.exists() {
            return Ok(());
        }

        let now = Self::now_ms();
        for entry in std::fs::read_dir(&protected_dir).map_err(|e| StoreError::Io(e.to_string()))? {
            let Ok(entry) = entry else { continue };
            let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
            if !is_file {
                continue;
            }
            let path = entry.path();
            if let Some(ts) = Self::extract_protected_timestamp(&path) {
                if now.saturating_sub(ts) >= PROTECTED_BACKUP_MAX_AGE_MS {
                    std::fs::remove_file(&path).map_err(|e| StoreError::Io(e.to_string()))?;
                    tracing::info!(path = %path.display(), "expired pre-reset backup purged");
                }
            }
        }
        Ok(())
    }

    fn extract_protected_timestamp(path: &Path) -> Option<u128> {
        path.file_stem()?
            .to_str()?
            .strip_prefix("pre-reset-")?
            .parse::<u128>()
            .ok()
    }

    /// Copie le fichier de config existant (avant ecrasement) dans un
    /// repertoire de backups, puis purge les plus anciens au-dela de
    /// `backup_max_count()`. Ne fait rien si `path` n'existe pas encore
    /// (premier ecrit, rien a sauvegarder).
    fn backup_before_overwrite(path: &Path, parent: &Path) -> Result<(), StoreError> {
        if !path.exists() {
            return Ok(());
        }

        let backups_dir = parent.join("backups");
        std::fs::create_dir_all(&backups_dir).map_err(|e| StoreError::Io(e.to_string()))?;

        let seq = BACKUP_SEQ.fetch_add(1, Ordering::Relaxed);
        let backup_name = format!("mock-config-{}-{:06}.yaml", Self::now_ms(), seq);
        std::fs::copy(path, backups_dir.join(&backup_name))
            .map_err(|e| StoreError::Io(e.to_string()))?;

        Self::rotate_backups(&backups_dir)?;
        Ok(())
    }

    fn rotate_backups(backups_dir: &Path) -> Result<(), StoreError> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(backups_dir)
            .map_err(|e| StoreError::Io(e.to_string()))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|p| p.is_file())
            .collect();

        // Le nom encode timestamp+seq avec largeur fixe : le tri lexicographique
        // correspond a l'ordre chronologique (plus recent = plus grand).
        files.sort();

        let max = Self::backup_max_count();
        if files.len() > max {
            for old in &files[..files.len() - max] {
                std::fs::remove_file(old).map_err(|e| StoreError::Io(e.to_string()))?;
            }
        }
        Ok(())
    }

    fn now_ms() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }
}

/// Metadonnees d'un fichier de backup (jamais le contenu YAML), exposees par
/// `GET /api/config/backups`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BackupInfo {
    pub filename: String,
    pub protected: bool,
    pub size_bytes: u64,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone)]
pub enum StoreError {
    Io(String),
    Yaml(String),
    NotFound(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(msg) => write!(f, "IO error: {msg}"),
            StoreError::Yaml(msg) => write!(f, "YAML error: {msg}"),
            StoreError::NotFound(msg) => write!(f, "Not found: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{*, WsdlMode};

    // Process-wide env vars (BACKUP_MAX_COUNT, DATA_PATH) are mutated by
    // several tests below; cargo test runs test fns in parallel OS threads,
    // so without serialization one test's set_var/remove_var can leak into
    // another's assertion window (pre-existing flakiness, unrelated to
    // write-behind). Any test touching these env vars must hold this lock
    // for its whole body.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lightmock-test-{}", fastrand::u64(..)));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_config() -> MockConfig {
        MockConfig {
            services: vec![Service {
                name: "svc-a".into(),

                listen_path: "/svc-a/*".into(),
                real_target_url: "http://svc-a:8080".into(),
                is_mocked: true,
                rewrite_directory_urls: false,
                group_name: None,
                wsdl_mode: WsdlMode::default(),
                rules: vec![Rule {
                    name: "default".into(),
                    method: "GET".into(),
                    sub_path: None,
                    action: RuleAction::default(),
                    pre_script: None,
                    script: None,
                    post_script: None,
                    response_mode: None,
                    conditions: ConditionGroup::default(),
                    response: MockResponse {
                        status: 200,
                        headers: vec![],
                        body: vec![BodyFragment::Literal {
                            value: "ok".into(),
                        }],
                        chaos: None,
                    },
                }],
            }],
            groups: vec![],
        }
    }

    #[tokio::test]
    async fn load_creates_empty_config_if_missing() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        let config = store.snapshot().await;
        assert!(config.services.is_empty());
        assert!(MockStore::config_file(&dir).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn load_reads_existing_config() {
        let dir = temp_dir();
        let file = MockStore::config_file(&dir);
        let cfg = sample_config();
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        std::fs::write(&file, &yaml).unwrap();

        let store = MockStore::load_or_init(&dir).await.unwrap();
        let loaded = store.snapshot().await;
        assert_eq!(loaded.services.len(), 1);
        assert_eq!(loaded.services[0].name, "svc-a");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn replace_persists_to_disk() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();

        let cfg = sample_config();
        store.replace(cfg.clone()).await.unwrap();
        // write-behind: wait for the queued disk write before reading the file.
        store.flush().await;

        let on_disk: MockConfig = serde_yaml::from_str(
            &std::fs::read_to_string(MockStore::config_file(&dir)).unwrap(),
        )
        .unwrap();
        assert_eq!(on_disk.services.len(), 1);

        let in_mem = store.snapshot().await;
        assert_eq!(*in_mem, on_disk);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn update_modifies_in_place() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        let updated = store
            .update(|cfg| {
                cfg.services[0].is_mocked = false;
                cfg.services.push(Service {
                    name: "svc-b".into(),
    
                    listen_path: "/svc-b/*".into(),
                    real_target_url: "http://svc-b:9090".into(),
                    is_mocked: false,
                    rewrite_directory_urls: false,
                    group_name: None,
                    wsdl_mode: WsdlMode::default(),
                    rules: vec![],
                });
            })
            .await
            .unwrap();

        assert_eq!(updated.services.len(), 2);
        assert!(!updated.services[0].is_mocked);
        assert_eq!(updated.services[1].name, "svc-b");

        // write-behind: wait for the queued disk write before reading the file.
        store.flush().await;
        let on_disk: MockConfig = serde_yaml::from_str(
            &std::fs::read_to_string(MockStore::config_file(&dir)).unwrap(),
        )
        .unwrap();
        assert_eq!(on_disk, *updated);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn atomic_write_no_partial_file() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();
        store.flush().await;

        let tmp_path = dir.join(".mock-config.yaml.tmp");
        assert!(!tmp_path.exists(), "temp file should be cleaned up after rename");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn backup_created_on_write() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        // load_or_init writes the initial empty config; no prior file to back up yet.
        store.replace(sample_config()).await.unwrap();

        let backups_dir = dir.join("backups");
        let count = std::fs::read_dir(&backups_dir).unwrap().count();
        assert!(count >= 1, "expected at least one backup after overwriting an existing config");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn backup_rotation_keeps_max_n() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = temp_dir();
        unsafe { std::env::set_var("BACKUP_MAX_COUNT", "3") };

        let store = MockStore::load_or_init(&dir).await.unwrap();
        for i in 0..6 {
            let mut cfg = sample_config();
            cfg.services[0].name = format!("svc-{i}");
            store.replace(cfg).await.unwrap();
        }

        let backups_dir = dir.join("backups");
        let count = std::fs::read_dir(&backups_dir).unwrap().count();
        assert_eq!(count, 3, "backups should be capped at BACKUP_MAX_COUNT");

        unsafe { std::env::remove_var("BACKUP_MAX_COUNT") };
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn protected_backup_created_before_reset() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        store.backup_before_reset().await.unwrap();

        let protected_dir = dir.join("backups").join("protected");
        let count = std::fs::read_dir(&protected_dir).unwrap().count();
        assert_eq!(count, 1, "expected exactly one pre-reset backup");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn protected_backup_exempt_from_normal_rotation() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = temp_dir();
        unsafe { std::env::set_var("BACKUP_MAX_COUNT", "2") };

        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();
        store.backup_before_reset().await.unwrap();
        store.replace(MockConfig::empty()).await.unwrap();

        // Many writes after the reset, well beyond BACKUP_MAX_COUNT=2 —
        // the protected pre-reset backup must survive all of them.
        for i in 0..8 {
            let mut cfg = sample_config();
            cfg.services[0].name = format!("svc-{i}");
            store.replace(cfg).await.unwrap();
        }

        let protected_dir = dir.join("backups").join("protected");
        let count = std::fs::read_dir(&protected_dir).unwrap().count();
        assert_eq!(count, 1, "pre-reset backup must not be rotated away by normal quota");

        let backups_dir = dir.join("backups");
        let normal_count = std::fs::read_dir(&backups_dir)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().path().is_file())
            .count();
        assert_eq!(normal_count, 2, "normal backups still capped at BACKUP_MAX_COUNT");

        unsafe { std::env::remove_var("BACKUP_MAX_COUNT") };
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn protected_backup_purged_after_one_month_on_next_write() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        let protected_dir = dir.join("backups").join("protected");
        std::fs::create_dir_all(&protected_dir).unwrap();
        let old_ts = MockStore::now_ms() - PROTECTED_BACKUP_MAX_AGE_MS - 1_000;
        std::fs::write(
            protected_dir.join(format!("pre-reset-{old_ts}.yaml")),
            b"services: []\ngroups: []\n",
        )
        .unwrap();

        // Any subsequent write is the "activity 1 month later" signal.
        store.replace(MockConfig::empty()).await.unwrap();

        let count = std::fs::read_dir(&protected_dir).unwrap().count();
        assert_eq!(count, 0, "pre-reset backup older than 30 days should be purged");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn protected_backup_kept_if_not_yet_expired() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        let protected_dir = dir.join("backups").join("protected");
        std::fs::create_dir_all(&protected_dir).unwrap();
        let recent_ts = MockStore::now_ms() - 1_000;
        std::fs::write(
            protected_dir.join(format!("pre-reset-{recent_ts}.yaml")),
            b"services: []\ngroups: []\n",
        )
        .unwrap();

        store.replace(MockConfig::empty()).await.unwrap();

        let count = std::fs::read_dir(&protected_dir).unwrap().count();
        assert_eq!(count, 1, "pre-reset backup under 30 days old must be kept");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Regression pour la cause racine du flaky E2E write-behind (sujets
    /// 24/27, cf CLAUDE.md §7) : `backups/protected/` n'a AUCUNE rotation par
    /// quantite (contrairement a `backups/`, cape a BACKUP_MAX_COUNT) — seule
    /// `purge_expired_protected_backups` (appelee sous verrou, sur CHAQUE
    /// mutation) le borne, par age. En dev local/E2E ou `DELETE
    /// /api/config/reset` est appele avant quasi chaque test (donc chaque run
    /// de la suite ajoute des dizaines de backups proteges, jamais vieux de
    /// 30 jours entre deux sessions), ce dossier avait grossi a >1300
    /// entrees sur cette machine — reproduit ici avec 1500 entrees fraiches +
    /// 1 expiree. Avant le passage de `path.is_file()` (un `stat()` par
    /// fichier) a `entry.file_type()` (deja connu de l'enumeration du
    /// repertoire, gratuit sur Windows), cette fonction faisait ~1500 appels
    /// systeme synchrones SOUS LE VERROU D'ECRITURE a CHAQUE mutation de
    /// l'appli (pas seulement les resets) — mesure directement responsable
    /// (avec la latence intrinseque d'un dossier synchronise OneDrive) de
    /// ralentissements suffisants pour occasionnellement depasser la fenetre
    /// de polling (5s) des tests E2E write-behind. Ce test ne mesure pas le
    /// temps (fragile en CI) mais verifie la CORRECTION a l'echelle : seule
    /// l'entree expiree est purgee parmi 1501, aucune des fraiches n'est
    /// touchee.
    #[tokio::test]
    async fn purge_expired_protected_backups_correct_with_many_entries() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        let protected_dir = dir.join("backups").join("protected");
        std::fs::create_dir_all(&protected_dir).unwrap();

        // now_ms() lue UNE SEULE FOIS avant la boucle : appelee a chaque
        // iteration, l'horloge reelle peut avancer pendant les 1500 ecritures
        // et faire coincider deux (i, now_ms()) differents sur le meme
        // fresh_ts (collision de nom de fichier -> moins de 1500 fichiers
        // reellement crees). Purement arithmetique ici, aucune ambiguite.
        let base_now = MockStore::now_ms();
        for i in 0..1500 {
            let fresh_ts = base_now - 1_000 - i;
            std::fs::write(
                protected_dir.join(format!("pre-reset-{fresh_ts}.yaml")),
                b"services: []\ngroups: []\n",
            )
            .unwrap();
        }
        let old_ts = MockStore::now_ms() - PROTECTED_BACKUP_MAX_AGE_MS - 1_000;
        std::fs::write(
            protected_dir.join(format!("pre-reset-{old_ts}.yaml")),
            b"services: []\ngroups: []\n",
        )
        .unwrap();

        store.replace(MockConfig::empty()).await.unwrap();

        let remaining = std::fs::read_dir(&protected_dir).unwrap().count();
        assert_eq!(remaining, 1500, "only the single expired entry should be purged out of 1501");
        assert!(
            !protected_dir.join(format!("pre-reset-{old_ts}.yaml")).exists(),
            "the expired entry itself must be gone"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn backup_max_count_default() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("BACKUP_MAX_COUNT") };
        assert_eq!(MockStore::backup_max_count(), 5);
    }

    #[test]
    fn backup_max_count_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("BACKUP_MAX_COUNT", "12") };
        assert_eq!(MockStore::backup_max_count(), 12);
        unsafe { std::env::remove_var("BACKUP_MAX_COUNT") };
    }

    #[tokio::test]
    async fn replace_empty_config() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();
        store.replace(MockConfig::empty()).await.unwrap();

        let snapshot = store.snapshot().await;
        assert!(snapshot.services.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn list_backups_empty_dir() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        // load_or_init only writes the initial config, no prior file to back up.
        let backups = store.list_backups().await.unwrap();
        assert!(backups.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn list_backups_includes_normal_and_protected() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();
        store.replace(sample_config()).await.unwrap();
        store.backup_before_reset().await.unwrap();

        let backups = store.list_backups().await.unwrap();
        assert_eq!(backups.iter().filter(|b| !b.protected).count(), 2);
        assert_eq!(backups.iter().filter(|b| b.protected).count(), 1);
        assert!(backups.iter().all(|b| b.size_bytes > 0));
        assert!(backups.iter().all(|b| b.created_at_ms > 0));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn list_backups_sorted_most_recent_first() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        for i in 0..3 {
            let mut cfg = sample_config();
            cfg.services[0].name = format!("svc-{i}");
            store.replace(cfg).await.unwrap();
        }

        let backups = store.list_backups().await.unwrap();
        assert!(backups.len() >= 2);
        for pair in backups.windows(2) {
            assert!(pair[0].created_at_ms >= pair[1].created_at_ms);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn restore_from_backup_replaces_config() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();
        // write-behind: flush so "svc-a" actually lands on disk before the
        // next replace() backs up "whatever is currently on disk" — without
        // this, the backup below could capture the stale bootstrap content.
        store.flush().await;

        let mut other = sample_config();
        other.services[0].name = "restored-svc".into();
        store.replace(other).await.unwrap();

        let backups = store.list_backups().await.unwrap();
        let first_backup = backups
            .iter()
            .find(|b| !b.protected)
            .expect("expected at least one normal backup");

        store.restore_from_backup(&first_backup.filename).await.unwrap();

        let restored = store.snapshot().await;
        assert_eq!(restored.services[0].name, "svc-a");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn restore_from_backup_finds_protected_file() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();
        // write-behind: flush so "svc-a" actually lands on disk before
        // backup_before_reset() copies "whatever is currently on disk" into
        // backups/protected/ — otherwise it would copy the stale bootstrap
        // content instead.
        store.flush().await;
        store.backup_before_reset().await.unwrap();
        store.replace(MockConfig::empty()).await.unwrap();

        let backups = store.list_backups().await.unwrap();
        let protected = backups
            .iter()
            .find(|b| b.protected)
            .expect("expected a protected backup");

        store.restore_from_backup(&protected.filename).await.unwrap();

        let restored = store.snapshot().await;
        assert_eq!(restored.services.len(), 1);
        assert_eq!(restored.services[0].name, "svc-a");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn restore_from_backup_unknown_filename_errors() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        let err = store
            .restore_from_backup("mock-config-9999999999999-000042.yaml")
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn restore_from_backup_creates_safety_backup_of_current_state_first() {
        // Ce test compte les backups non-proteges dans backups/ (soumis a
        // rotation par BACKUP_MAX_COUNT) : sans tenir ENV_MUTEX, une mutation
        // concurrente de cette variable par un autre test (ex.
        // backup_rotation_keeps_max_n, protected_backup_exempt_from_normal_rotation,
        // backup_max_count_from_env) peut plafonner le nombre de backups a une
        // valeur trop basse pendant la fenetre du test, rendant la comparaison
        // avant/apres fausse de facon intermittente (meme pitfall que tout
        // test Rust qui mute un env var process-wide sans serialisation).
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("BACKUP_MAX_COUNT") };
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        let mut other = sample_config();
        other.services[0].name = "before-restore".into();
        store.replace(other).await.unwrap();

        let backups_before = store.list_backups().await.unwrap();
        let target = backups_before
            .iter()
            .find(|b| !b.protected)
            .unwrap()
            .filename
            .clone();

        store.restore_from_backup(&target).await.unwrap();

        // restore_from_backup() -> replace() -> prepare_and_backup() must have backed
        // up "before-restore" (the state overwritten by the restore) before queuing
        // the restored content for write-behind — never lose the pre-restore state.
        let backups_after = store.list_backups().await.unwrap();
        assert!(backups_after.iter().filter(|b| !b.protected).count() > backups_before.iter().filter(|b| !b.protected).count());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn data_path_default() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("DATA_PATH") };
        let p = MockStore::data_path();
        assert_eq!(p, PathBuf::from("./data"));
    }

    #[test]
    fn data_path_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("DATA_PATH", "/mnt/pvc/lightmock") };
        let p = MockStore::data_path();
        assert_eq!(p, PathBuf::from("/mnt/pvc/lightmock"));
        unsafe { std::env::remove_var("DATA_PATH") };
    }

    #[tokio::test]
    async fn concurrent_reads_dont_block() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        let s1 = store.clone();
        let s2 = store.clone();
        let (r1, r2) = tokio::join!(s1.snapshot(), s2.snapshot());
        assert_eq!(r1, r2);
        std::fs::remove_dir_all(&dir).ok();
    }

    // --------------- Write-behind ---------------

    #[tokio::test]
    async fn write_behind_flush_persists_to_disk() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();

        store.replace(sample_config()).await.unwrap();
        store.flush().await;

        let on_disk: MockConfig = serde_yaml::from_str(
            &std::fs::read_to_string(MockStore::config_file(&dir)).unwrap(),
        )
        .unwrap();
        assert_eq!(on_disk.services.len(), 1);
        let status = store.writer_status();
        assert!(status.last_write_ok_ms.is_some());
        assert!(status.last_error.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Le backup de l'etat ecrase doit exister sur disque immediatement apres
    /// replace(), MEME SANS flush() — il est cree de facon synchrone par
    /// prepare_and_backup() avant que l'ecriture ne soit seulement mise en
    /// queue. Ne pas confondre avec la persistance du NOUVEAU contenu
    /// (mock-config.yaml lui-meme), qui elle est asynchrone.
    #[tokio::test]
    async fn backup_is_synchronous_before_write_is_queued() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();

        store.replace(sample_config()).await.unwrap();
        // Deliberement PAS de flush() ici : on verifie que le backup de
        // l'etat precedent est deja sur disque independamment de l'etat
        // d'avancement de la tache de fond.
        let backups = store.list_backups().await.unwrap();
        assert_eq!(
            backups.iter().filter(|b| !b.protected).count(),
            1,
            "backup must be created synchronously, before the async write is even queued"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn concurrent_updates_preserve_order_and_match_disk() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();
        store.replace(sample_config()).await.unwrap();

        let mut handles = Vec::new();
        for i in 0..10 {
            let s = store.clone();
            handles.push(tokio::spawn(async move {
                s.update(move |cfg| {
                    cfg.services.push(Service {
                        name: format!("concurrent-{i}"),
                        listen_path: format!("/concurrent-{i}/*"),
                        real_target_url: "http://x:8080".into(),
                        is_mocked: true,
                        rewrite_directory_urls: false,
                        group_name: None,
                        wsdl_mode: WsdlMode::default(),
                        rules: vec![],
                    });
                })
                .await
                .unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        store.flush().await;

        let on_disk: MockConfig = serde_yaml::from_str(
            &std::fs::read_to_string(MockStore::config_file(&dir)).unwrap(),
        )
        .unwrap();
        let in_mem = store.snapshot().await;
        assert_eq!(*in_mem, on_disk, "disk must match memory once flushed");
        assert_eq!(on_disk.services.len(), 11, "the original service + all 10 concurrent updates");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn write_queue_status_reports_bounded_capacity() {
        let dir = temp_dir();
        let store = MockStore::load_or_init(&dir).await.unwrap();

        let status = store.writer_status();
        assert_eq!(status.capacity, WRITE_QUEUE_CAPACITY);

        for i in 0..20 {
            let mut cfg = sample_config();
            cfg.services[0].name = format!("svc-{i}");
            store.replace(cfg).await.unwrap();
            assert!(store.writer_status().pending <= WRITE_QUEUE_CAPACITY);
        }
        store.flush().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Une erreur d'ecriture disque (chemin invalide) est loguee et exposee
    /// via writer_status(), mais ne fait PAS mourir la tache de fond : une
    /// mutation ulterieure vers un chemin redevenu valide doit toujours
    /// aboutir.
    #[tokio::test]
    async fn write_error_is_reported_and_task_keeps_running() {
        let dir = temp_dir();
        let bogus_path = dir.join("missing-subdir").join("mock-config.yaml");
        let store = MockStore::new(bogus_path.clone());

        store.replace(sample_config()).await.unwrap();
        store.flush().await;

        let status = store.writer_status();
        assert!(status.last_error.is_some(), "write to a missing directory must be reported as an error");
        assert!(status.last_write_ok_ms.is_none());

        std::fs::create_dir_all(bogus_path.parent().unwrap()).unwrap();
        store.replace(sample_config()).await.unwrap();
        store.flush().await;

        let status2 = store.writer_status();
        assert!(status2.last_write_ok_ms.is_some(), "task must keep processing jobs after a prior write error");
        assert!(bogus_path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
