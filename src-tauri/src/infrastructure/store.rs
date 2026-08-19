use crate::{
    domain::{ModPreset, ModpackProfile},
    error::AppResult,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoredSettings {
    pub include_optional_dependencies: bool,
    pub download_concurrency: u8,
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            include_optional_dependencies: false,
            download_concurrency: 3,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Database {
    pub profiles: Vec<ModpackProfile>,
    pub presets: Vec<ModPreset>,
    pub settings: StoredSettings,
}

pub struct JsonStore {
    path: PathBuf,
    data: RwLock<Database>,
}

impl JsonStore {
    pub async fn load(path: PathBuf) -> AppResult<Self> {
        let data = match tokio::fs::read_to_string(&path).await {
            Ok(raw) => serde_json::from_str(&raw)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Database::default(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            path,
            data: RwLock::new(data),
        })
    }

    pub async fn snapshot(&self) -> Database {
        self.data.read().await.clone()
    }

    pub async fn update<R>(&self, mutate: impl FnOnce(&mut Database) -> R) -> AppResult<R> {
        let mut data = self.data.write().await;
        let result = mutate(&mut data);
        self.persist(&data).await?;
        Ok(result)
    }

    async fn persist(&self, data: &Database) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = self.path.with_extension("json.part");
        let bytes = serde_json::to_vec_pretty(data)?;
        tokio::fs::write(&temporary, bytes).await?;
        if tokio::fs::try_exists(&self.path).await? {
            tokio::fs::remove_file(&self.path).await?;
        }
        tokio::fs::rename(temporary, &self.path).await?;
        Ok(())
    }

    #[cfg(test)]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serializes_updates_without_losing_data() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::load(directory.path().join("state.json"))
            .await
            .unwrap();
        store
            .update(|data| data.settings.download_concurrency = 5)
            .await
            .unwrap();
        let loaded = JsonStore::load(store.path().to_owned()).await.unwrap();
        assert_eq!(loaded.snapshot().await.settings.download_concurrency, 5);
        assert!(
            !loaded
                .snapshot()
                .await
                .settings
                .include_optional_dependencies
        );
    }
}
