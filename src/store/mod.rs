// Persistance de la config (services + groupes) en YAML sur disque.
// Architecture : Arc<RwLock<Arc<MockConfig>>> pour lectures O(1) sans clone.
// snapshot() retourne un Arc (clone du pointeur, pas des donnees).
// Les ecritures (update/replace) clonent les donnees, les modifient,
// puis font un atomic write (ecriture tmp + rename) pour eviter la corruption.
//
// Backup/rollback : avant chaque ecrasement du fichier de config, l'ancien
// contenu est copie dans {data_dir}/backups/. C'est event-driven (declenche
// par l'ecriture elle-meme), PAS une tache de fond/cron — coherent avec le
// choix fait pour le ping (src/server/ping.rs). Rotation immediate apres
// coup pour ne garder que BACKUP_MAX_COUNT fichiers (defaut 5), necessaire
// vu la contrainte PVC 64Mi en K8s (cf CLAUDE.md).
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
use tokio::sync::RwLock;

static BACKUP_SEQ: AtomicU64 = AtomicU64::new(0);

const PROTECTED_BACKUP_MAX_AGE_MS: u128 = 30 * 24 * 60 * 60 * 1000;

#[derive(Clone)]
pub struct MockStore {
    config: Arc<RwLock<Arc<MockConfig>>>,
    path: PathBuf,
}

impl MockStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(Arc::new(MockConfig::empty()))),
            path,
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

        let config = if file.exists() {
            let content =
                std::fs::read_to_string(&file).map_err(|e| StoreError::Io(e.to_string()))?;
            serde_yaml::from_str(&content).map_err(|e| StoreError::Yaml(e.to_string()))?
        } else {
            let empty = MockConfig::empty();
            Self::atomic_write(&file, &empty)?;
            empty
        };

        tracing::info!(path = %file.display(), services = config.services.len(), "config loaded");

        Ok(Self {
            config: Arc::new(RwLock::new(Arc::new(config))),
            path: file,
        })
    }

    pub async fn snapshot(&self) -> Arc<MockConfig> {
        self.config.read().await.clone()
    }

    pub async fn replace(&self, config: MockConfig) -> Result<(), StoreError> {
        Self::atomic_write(&self.path, &config)?;
        *self.config.write().await = Arc::new(config);
        Ok(())
    }

    pub async fn update<F>(&self, f: F) -> Result<Arc<MockConfig>, StoreError>
    where
        F: FnOnce(&mut MockConfig),
    {
        let mut guard = self.config.write().await;
        let mut cfg = (**guard).clone();
        f(&mut cfg);
        Self::atomic_write(&self.path, &cfg)?;
        *guard = Arc::new(cfg);
        Ok(guard.clone())
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

    fn atomic_write(path: &Path, config: &MockConfig) -> Result<(), StoreError> {
        let yaml = serde_yaml::to_string(config).map_err(|e| StoreError::Yaml(e.to_string()))?;

        let parent = path.parent().ok_or_else(|| {
            StoreError::Io("config path has no parent directory".into())
        })?;

        Self::purge_expired_protected_backups(parent)?;
        Self::backup_before_overwrite(path, parent)?;

        let tmp_path = parent.join(".mock-config.yaml.tmp");
        std::fs::write(&tmp_path, yaml.as_bytes()).map_err(|e| StoreError::Io(e.to_string()))?;
        std::fs::rename(&tmp_path, path).map_err(|e| StoreError::Io(e.to_string()))?;

        Ok(())
    }

    /// Purge les backups pre-reset (backups/protected/) plus vieux que
    /// PROTECTED_BACKUP_MAX_AGE_MS. Opportuniste : declenche par l'ecriture
    /// en cours, pas de timer. Age lu depuis le timestamp encode dans le nom
    /// de fichier (`pre-reset-{ts}.yaml`), pas depuis les metadonnees disque.
    fn purge_expired_protected_backups(parent: &Path) -> Result<(), StoreError> {
        let protected_dir = parent.join("backups").join("protected");
        if !protected_dir.exists() {
            return Ok(());
        }

        let now = Self::now_ms();
        for entry in std::fs::read_dir(&protected_dir).map_err(|e| StoreError::Io(e.to_string()))? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
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

#[derive(Debug, Clone)]
pub enum StoreError {
    Io(String),
    Yaml(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(msg) => write!(f, "IO error: {msg}"),
            StoreError::Yaml(msg) => write!(f, "YAML error: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{*, WsdlMode};

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
                    script: None,
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

    #[test]
    fn backup_max_count_default() {
        unsafe { std::env::remove_var("BACKUP_MAX_COUNT") };
        assert_eq!(MockStore::backup_max_count(), 5);
    }

    #[test]
    fn backup_max_count_from_env() {
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

    #[test]
    fn data_path_default() {
        unsafe { std::env::remove_var("DATA_PATH") };
        let p = MockStore::data_path();
        assert_eq!(p, PathBuf::from("./data"));
    }

    #[test]
    fn data_path_from_env() {
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
}
