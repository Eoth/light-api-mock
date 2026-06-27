// Persistance de la config (services + groupes) en YAML sur disque.
// Architecture : Arc<RwLock<Arc<MockConfig>>> pour lectures O(1) sans clone.
// snapshot() retourne un Arc (clone du pointeur, pas des donnees).
// Les ecritures (update/replace) clonent les donnees, les modifient,
// puis font un atomic write (ecriture tmp + rename) pour eviter la corruption.
use crate::models::MockConfig;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

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

    fn atomic_write(path: &Path, config: &MockConfig) -> Result<(), StoreError> {
        let yaml = serde_yaml::to_string(config).map_err(|e| StoreError::Yaml(e.to_string()))?;

        let parent = path.parent().ok_or_else(|| {
            StoreError::Io("config path has no parent directory".into())
        })?;

        let tmp_path = parent.join(".mock-config.yaml.tmp");
        std::fs::write(&tmp_path, yaml.as_bytes()).map_err(|e| StoreError::Io(e.to_string()))?;
        std::fs::rename(&tmp_path, path).map_err(|e| StoreError::Io(e.to_string()))?;

        Ok(())
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
                    method: "ANY".into(),
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
