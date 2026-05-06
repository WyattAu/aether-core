//! Configuration hot-reload support
//!
//! Provides file watching, change detection, and configuration diffing
//! for runtime configuration updates without restart.

use crate::config::{ActorConfig, ActorKind, AetherConfig, InstanceCount};
use crate::error::{Error, Result};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const DEFAULT_DEBOUNCE_MS: u64 = 100;

/// Watches a configuration file for changes using mtime polling.
pub struct ConfigWatcher {
    path: PathBuf,
    last_modified: SystemTime,
    debounce_ms: u64,
    last_check: SystemTime,
}

impl ConfigWatcher {
    /// Creates a new watcher for the given configuration file path.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let metadata = std::fs::metadata(&path).map_err(|e| {
            Error::config(format!(
                "Failed to access config file {}: {}",
                path.display(),
                e
            ))
        })?;

        let last_modified = metadata.modified().map_err(|e| {
            Error::config(format!(
                "Failed to get modification time for {}: {}",
                path.display(),
                e
            ))
        })?;

        Ok(Self {
            path,
            last_modified,
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            last_check: SystemTime::now(),
        })
    }

    /// Sets the debounce interval in milliseconds.
    pub fn with_debounce(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Checks if the configuration file has been modified.
    ///
    /// Returns `Some(SystemTime)` with the new modification time if changed,
    /// or `None` if unchanged or within debounce window.
    pub fn check_for_changes(&mut self) -> Result<Option<SystemTime>> {
        let now = SystemTime::now();

        let elapsed = now
            .duration_since(self.last_check)
            .unwrap_or(Duration::ZERO);

        if elapsed < Duration::from_millis(self.debounce_ms) {
            return Ok(None);
        }

        self.last_check = now;

        let metadata = match std::fs::metadata(&self.path) {
            Ok(m) => m,
            Err(e) => {
                return Err(Error::config(format!(
                    "Failed to access config file {}: {}",
                    self.path.display(),
                    e
                )));
            }
        };

        let current_modified = metadata.modified().map_err(|e| {
            Error::config(format!(
                "Failed to get modification time for {}: {}",
                self.path.display(),
                e
            ))
        })?;

        if current_modified > self.last_modified {
            self.last_modified = current_modified;
            Ok(Some(current_modified))
        } else {
            Ok(None)
        }
    }

    /// Returns the path being watched.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Returns the last known modification time.
    pub fn last_modified(&self) -> SystemTime {
        self.last_modified
    }
}

/// Callback type for configuration change notifications.
pub type ConfigChangeCallback = Box<dyn Fn(&ConfigDiff) + Send + Sync>;

/// Alias for configuration change watchers.
pub type ConfigChangeWatcher = ConfigChangeCallback;

/// Manages configuration reloading with callbacks.
pub struct ConfigReloader {
    config: Arc<RwLock<AetherConfig>>,
    config_path: Option<PathBuf>,
    watchers: Vec<ConfigChangeWatcher>,
}

impl ConfigReloader {
    /// Creates a new reloader with the given shared configuration.
    pub fn new(config: Arc<RwLock<AetherConfig>>) -> Self {
        Self {
            config,
            config_path: None,
            watchers: Vec::new(),
        }
    }

    /// Sets the configuration file path for reloading.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Registers a callback to be invoked when configuration changes.
    pub fn on_change<F>(&mut self, callback: F)
    where
        F: Fn(&ConfigDiff) + Send + Sync + 'static,
    {
        self.watchers.push(Box::new(callback));
    }

    /// Reloads configuration from the configured path.
    ///
    /// Returns `Ok(true)` if the configuration was reloaded,
    /// `Ok(false)` if no path was configured or no changes detected.
    pub async fn reload(&self) -> Result<bool> {
        let path = match &self.config_path {
            Some(p) => p,
            None => return Ok(false),
        };

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| Error::config(format!("Failed to read {}: {}", path.display(), e)))?;

        let new_config = AetherConfig::from_toml(&content)?;

        let diff = {
            let old_config = self.config.read();
            ConfigDiff::compute(&old_config, &new_config)
        };

        if !diff.is_empty() {
            {
                let mut config = self.config.write();
                *config = new_config;
            }

            for watcher in &self.watchers {
                watcher(&diff);
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Reloads configuration from a string, computing and applying the diff.
    ///
    /// Returns `Ok(true)` if changes were applied, `Ok(false)` if no changes.
    pub fn reload_from_str(&self, content: &str) -> Result<bool> {
        let new_config = AetherConfig::from_toml(content)?;

        let diff = {
            let old_config = self.config.read();
            ConfigDiff::compute(&old_config, &new_config)
        };

        if !diff.is_empty() {
            {
                let mut config = self.config.write();
                *config = new_config;
            }

            for watcher in &self.watchers {
                watcher(&diff);
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Returns a clone of the current configuration.
    pub fn current_config(&self) -> AetherConfig {
        self.config.read().clone()
    }
}

/// Describes a change to an actor's configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorConfigChange {
    /// Name of the actor.
    pub name: String,
    /// Old image (if changed).
    pub image_changed: Option<(String, String)>,
    /// Old instance count (if changed).
    pub instances_changed: Option<(InstanceCount, InstanceCount)>,
    /// Old kind (if changed).
    pub kind_changed: Option<(ActorKind, ActorKind)>,
    /// Capabilities changed flag.
    pub capabilities_changed: bool,
}

impl ActorConfigChange {
    /// Returns true if any field changed.
    pub fn has_changes(&self) -> bool {
        self.image_changed.is_some()
            || self.instances_changed.is_some()
            || self.kind_changed.is_some()
            || self.capabilities_changed
    }
}

/// Describes the differences between two configurations.
#[derive(Debug, Clone, Default)]
pub struct ConfigDiff {
    /// Names of actors that were added.
    pub actors_added: Vec<String>,
    /// Names of actors that were removed.
    pub actors_removed: Vec<String>,
    /// Actors with modified configurations.
    pub actors_modified: Vec<ActorConfigChange>,
}

impl ConfigDiff {
    /// Computes the difference between old and new configurations.
    pub fn compute(old: &AetherConfig, new: &AetherConfig) -> Self {
        let old_names: HashSet<&str> = old.actor.iter().map(|a| a.name.as_str()).collect();
        let new_names: HashSet<&str> = new.actor.iter().map(|a| a.name.as_str()).collect();

        let actors_added: Vec<String> = new_names
            .difference(&old_names)
            .map(|s| (*s).to_string())
            .collect();

        let actors_removed: Vec<String> = old_names
            .difference(&new_names)
            .map(|s| (*s).to_string())
            .collect();

        let mut actors_modified = Vec::new();

        for new_actor in &new.actor {
            if let Some(old_actor) = old.actor.iter().find(|a| a.name == new_actor.name) {
                let change = Self::compare_actors(old_actor, new_actor);
                if change.has_changes() {
                    actors_modified.push(change);
                }
            }
        }

        Self {
            actors_added,
            actors_removed,
            actors_modified,
        }
    }

    fn volumes_differ(
        old: &std::collections::HashMap<String, crate::config::VolumeConfig>,
        new: &std::collections::HashMap<String, crate::config::VolumeConfig>,
    ) -> bool {
        if old.len() != new.len() {
            return true;
        }
        for (key, old_vol) in old {
            match new.get(key) {
                None => return true,
                Some(new_vol) => {
                    if old_vol.path != new_vol.path
                        || old_vol.size != new_vol.size
                        || old_vol.read_only != new_vol.read_only
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn compare_actors(old: &ActorConfig, new: &ActorConfig) -> ActorConfigChange {
        let image_changed = if old.image != new.image {
            Some((old.image.clone(), new.image.clone()))
        } else {
            None
        };

        let instances_changed = if old.instances != new.instances {
            Some((old.instances.clone(), new.instances.clone()))
        } else {
            None
        };

        let kind_changed = if old.kind != new.kind {
            Some((old.kind, new.kind))
        } else {
            None
        };

        let volumes_changed =
            Self::volumes_differ(&old.capabilities.volumes, &new.capabilities.volumes);

        let capabilities_changed = old.capabilities.networking != new.capabilities.networking
            || old.capabilities.env != new.capabilities.env
            || volumes_changed
            || old.capabilities.extras != new.capabilities.extras;

        ActorConfigChange {
            name: new.name.clone(),
            image_changed,
            instances_changed,
            kind_changed,
            capabilities_changed,
        }
    }

    /// Returns true if there are any changes.
    pub fn is_empty(&self) -> bool {
        self.actors_added.is_empty()
            && self.actors_removed.is_empty()
            && self.actors_modified.is_empty()
    }

    /// Returns the total number of changes.
    pub fn total_changes(&self) -> usize {
        self.actors_added.len() + self.actors_removed.len() + self.actors_modified.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CapabilityConfig;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_actor(name: &str, image: &str) -> ActorConfig {
        ActorConfig {
            name: name.to_string(),
            kind: ActorKind::Wasm,
            image: image.to_string(),
            instances: InstanceCount::Fixed(1),
            capabilities: CapabilityConfig::default(),
        }
    }

    fn make_config(actors: Vec<ActorConfig>) -> AetherConfig {
        AetherConfig {
            project: Default::default(),
            actor: actors,
            observability: None,
        }
    }

    #[test]
    fn test_config_watcher_new() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "[project]\nname = \"test\"").unwrap();

        let watcher = ConfigWatcher::new(temp.path());
        assert!(watcher.is_ok());

        let watcher = watcher.unwrap();
        assert_eq!(watcher.path(), temp.path());
    }

    #[test]
    fn test_config_watcher_nonexistent_file() {
        let result = ConfigWatcher::new("/nonexistent/config.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_watcher_no_change() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "[project]\nname = \"test\"").unwrap();

        let mut watcher = ConfigWatcher::new(temp.path()).unwrap();

        std::thread::sleep(Duration::from_millis(150));

        let result = watcher.check_for_changes();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_config_watcher_with_change() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "[project]\nname = \"test\"").unwrap();

        let mut watcher = ConfigWatcher::new(temp.path()).unwrap().with_debounce(0);

        std::thread::sleep(Duration::from_millis(10));

        temp.write_all(b"\n[[actor]]\nname = \"new\"").unwrap();
        temp.flush().unwrap();

        std::thread::sleep(Duration::from_millis(10));

        let result = watcher.check_for_changes();
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_config_diff_empty() {
        let config = make_config(vec![make_actor("test", "test.wasm")]);
        let diff = ConfigDiff::compute(&config, &config);

        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);
    }

    #[test]
    fn test_config_diff_actors_added() {
        let old = make_config(vec![]);
        let new = make_config(vec![make_actor("api", "api.wasm")]);

        let diff = ConfigDiff::compute(&old, &new);

        assert!(!diff.is_empty());
        assert_eq!(diff.actors_added, vec!["api"]);
        assert!(diff.actors_removed.is_empty());
        assert!(diff.actors_modified.is_empty());
    }

    #[test]
    fn test_config_diff_actors_removed() {
        let old = make_config(vec![make_actor("api", "api.wasm")]);
        let new = make_config(vec![]);

        let diff = ConfigDiff::compute(&old, &new);

        assert!(!diff.is_empty());
        assert!(diff.actors_added.is_empty());
        assert_eq!(diff.actors_removed, vec!["api"]);
        assert!(diff.actors_modified.is_empty());
    }

    #[test]
    fn test_config_diff_actors_modified_image() {
        let old = make_config(vec![make_actor("api", "v1.wasm")]);
        let new = make_config(vec![make_actor("api", "v2.wasm")]);

        let diff = ConfigDiff::compute(&old, &new);

        assert!(!diff.is_empty());
        assert!(diff.actors_added.is_empty());
        assert!(diff.actors_removed.is_empty());
        assert_eq!(diff.actors_modified.len(), 1);

        let change = &diff.actors_modified[0];
        assert_eq!(change.name, "api");
        assert_eq!(
            change.image_changed,
            Some(("v1.wasm".to_string(), "v2.wasm".to_string()))
        );
        assert!(change.instances_changed.is_none());
        assert!(change.kind_changed.is_none());
        assert!(!change.capabilities_changed);
    }

    #[test]
    fn test_config_diff_actors_modified_instances() {
        let mut old_actor = make_actor("api", "api.wasm");
        old_actor.instances = InstanceCount::Fixed(1);

        let mut new_actor = make_actor("api", "api.wasm");
        new_actor.instances = InstanceCount::Fixed(3);

        let old = make_config(vec![old_actor]);
        let new = make_config(vec![new_actor]);

        let diff = ConfigDiff::compute(&old, &new);

        assert_eq!(diff.actors_modified.len(), 1);
        let change = &diff.actors_modified[0];
        assert!(change.instances_changed.is_some());
    }

    #[test]
    fn test_config_diff_actors_modified_kind() {
        let mut old_actor = make_actor("api", "api.wasm");
        old_actor.kind = ActorKind::Wasm;

        let mut new_actor = make_actor("api", "api.wasm");
        new_actor.kind = ActorKind::Oci;

        let old = make_config(vec![old_actor]);
        let new = make_config(vec![new_actor]);

        let diff = ConfigDiff::compute(&old, &new);

        assert_eq!(diff.actors_modified.len(), 1);
        let change = &diff.actors_modified[0];
        assert_eq!(change.kind_changed, Some((ActorKind::Wasm, ActorKind::Oci)));
    }

    #[test]
    fn test_config_diff_multiple_changes() {
        let old = make_config(vec![
            make_actor("api", "v1.wasm"),
            make_actor("worker", "worker.wasm"),
        ]);

        let new = make_config(vec![
            make_actor("api", "v2.wasm"),
            make_actor("scheduler", "scheduler.wasm"),
        ]);

        let diff = ConfigDiff::compute(&old, &new);

        assert_eq!(diff.actors_added, vec!["scheduler"]);
        assert_eq!(diff.actors_removed, vec!["worker"]);
        assert_eq!(diff.actors_modified.len(), 1);
        assert_eq!(diff.total_changes(), 3);
    }

    #[test]
    fn test_config_reloader_new() {
        let config = Arc::new(RwLock::new(AetherConfig::default()));
        let reloader = ConfigReloader::new(config.clone());

        assert_eq!(reloader.current_config().actor.len(), 0);
    }

    #[test]
    fn test_config_reloader_reload_from_str_no_change() {
        let config = Arc::new(RwLock::new(AetherConfig::default()));
        let reloader = ConfigReloader::new(config);

        let toml = "";
        let result = reloader.reload_from_str(toml);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_config_reloader_reload_from_str_with_change() {
        let config = Arc::new(RwLock::new(AetherConfig::default()));
        let reloader = ConfigReloader::new(config.clone());

        let toml = r#"
[[actor]]
name = "new-actor"
kind = "wasm"
image = "new.wasm"
"#;
        let result = reloader.reload_from_str(toml);
        assert!(result.is_ok());
        assert!(result.unwrap());

        let updated = config.read();
        assert_eq!(updated.actor.len(), 1);
        assert_eq!(updated.actor[0].name, "new-actor");
    }

    #[test]
    fn test_config_reloader_callback_invoked() {
        let config = Arc::new(RwLock::new(AetherConfig::default()));
        let mut reloader = ConfigReloader::new(config);

        let callback_invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = callback_invoked.clone();

        reloader.on_change(move |_diff| {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        let toml = r#"
[[actor]]
name = "new"
kind = "wasm"
image = "new.wasm"
"#;

        let result = reloader.reload_from_str(toml);
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(callback_invoked.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_config_reloader_reload_no_path() {
        let config = Arc::new(RwLock::new(AetherConfig::default()));
        let reloader = ConfigReloader::new(config);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(reloader.reload());
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_config_reloader_reload_from_file() {
        let mut temp = NamedTempFile::new().unwrap();
        write!(
            temp,
            r#"
[[actor]]
name = "file-actor"
kind = "wasm"
image = "file.wasm"
"#
        )
        .unwrap();

        let config = Arc::new(RwLock::new(AetherConfig::default()));
        let reloader = ConfigReloader::new(config.clone()).with_path(temp.path());

        let result = reloader.reload().await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        let updated = config.read();
        assert_eq!(updated.actor.len(), 1);
        assert_eq!(updated.actor[0].name, "file-actor");
    }

    #[test]
    fn test_actor_config_change_has_changes() {
        let change = ActorConfigChange {
            name: "test".to_string(),
            image_changed: None,
            instances_changed: None,
            kind_changed: None,
            capabilities_changed: false,
        };
        assert!(!change.has_changes());

        let change_with_image = ActorConfigChange {
            name: "test".to_string(),
            image_changed: Some(("old".to_string(), "new".to_string())),
            instances_changed: None,
            kind_changed: None,
            capabilities_changed: false,
        };
        assert!(change_with_image.has_changes());
    }
}
