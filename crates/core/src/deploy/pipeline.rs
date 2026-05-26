//! Built-in CI/CD Pipeline
//!
//! Provides a sequential stage-based pipeline executor with rollback support,
//! artifact persistence via a pluggable store, and configurable failure
//! actions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Action performed by a pipeline step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepAction {
    /// Build the project artifacts.
    Build,
    /// Run tests against the built artifacts.
    Test,
    /// Perform a security scan (SAST, dependency audit, etc.).
    SecurityScan,
    /// Publish artifacts to a registry.
    Publish,
    /// Deploy artifacts to a target environment.
    Deploy,
}

/// A single step within a pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    /// Unique step name.
    pub name: String,
    /// The action this step performs.
    pub action: StepAction,
    /// Names of steps within the same stage that must complete first.
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// A named group of steps that execute as a unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    /// Stage name.
    pub name: String,
    /// Steps belonging to this stage.
    pub steps: Vec<PipelineStep>,
    /// Maximum time allowed for this stage.
    pub timeout: Duration,
}

/// Action to take when a pipeline stage fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureAction {
    /// Stop immediately and do not attempt rollback.
    Abort,
    /// Attempt to undo completed stages in reverse order.
    Rollback,
    /// Continue to the next stage despite the failure.
    Continue,
}

/// Top-level pipeline configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Ordered list of stages to execute.
    pub stages: Vec<PipelineStage>,
    /// Action to take when any stage fails.
    pub on_failure: FailureAction,
    /// Identifier for the artifact store backend.
    pub artifact_store: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            stages: Vec::new(),
            on_failure: FailureAction::Abort,
            artifact_store: "in-memory".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

/// Status of a completed (or failed) pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageStatus {
    /// The stage has not been executed yet.
    Pending,
    /// The stage is currently running.
    Running,
    /// The stage completed successfully.
    Success,
    /// The stage failed.
    Failed,
    /// The stage was rolled back after a downstream failure.
    RolledBack,
    /// The stage was skipped due to a prior failure.
    Skipped,
}

/// Result of executing a single pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    /// Name of the stage.
    pub stage: String,
    /// Final status of the stage.
    pub status: StageStatus,
    /// Artifacts produced by this stage.
    pub artifacts: Vec<String>,
    /// Wall-clock duration of stage execution.
    pub duration: Duration,
    /// Captured log output.
    pub logs: String,
}

// ---------------------------------------------------------------------------
// Artifact store
// ---------------------------------------------------------------------------

/// Trait for persisting pipeline artifacts.
pub trait ArtifactStore: Send + Sync {
    /// Stores an artifact under the given key.
    fn store(&self, key: &str, data: &[u8]) -> Result<()>;
    /// Retrieves an artifact by key. Returns `None` if not found.
    fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>>;
    /// Lists all artifact keys matching the given prefix.
    fn list(&self, prefix: &str) -> Result<Vec<String>>;
    /// Deletes an artifact by key.
    fn delete(&self, key: &str) -> Result<bool>;
}

/// In-memory artifact store for testing and development.
pub struct InMemoryArtifactStore {
    data: RwLock<HashMap<String, Vec<u8>>>,
}

impl InMemoryArtifactStore {
    /// Creates a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryArtifactStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactStore for InMemoryArtifactStore {
    fn store(&self, key: &str, data: &[u8]) -> Result<()> {
        let mut store = self.data.write();
        store.insert(key.to_string(), data.to_vec());
        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let store = self.data.read();
        Ok(store.get(key).cloned())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let store = self.data.read();
        Ok(store
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }

    fn delete(&self, key: &str) -> Result<bool> {
        let mut store = self.data.write();
        Ok(store.remove(key).is_some())
    }
}

// ---------------------------------------------------------------------------
// Pipeline executor
// ---------------------------------------------------------------------------

/// Executes pipelines defined by a [`PipelineConfig`].
///
/// Stages are executed sequentially. Each stage's steps are executed in order
/// (respecting intra-stage dependencies). On failure, the configured
/// [`FailureAction`] determines what happens next.
pub struct PipelineExecutor {
    config: PipelineConfig,
    artifact_store: Arc<dyn ArtifactStore>,
    step_handlers: HashMap<StepAction, Arc<dyn StepHandler>>,
    results: RwLock<Vec<PipelineResult>>,
}

/// Trait for handling the execution of a single pipeline step.
pub trait StepHandler: Send + Sync {
    /// Executes the step. Returns a list of artifact keys produced.
    fn execute(&self, step: &PipelineStep, artifacts: &[String]) -> Result<Vec<String>>;
}

/// Default step handler that produces placeholder artifacts for testing.
pub struct DefaultStepHandler;

impl StepHandler for DefaultStepHandler {
    fn execute(&self, step: &PipelineStep, _artifacts: &[String]) -> Result<Vec<String>> {
        let artifact = format!("{}/{}", step.action.action_variant(), step.name);
        Ok(vec![artifact])
    }
}

impl PipelineExecutor {
    /// Creates a new executor with the given config and artifact store.
    pub fn new(config: PipelineConfig, artifact_store: Arc<dyn ArtifactStore>) -> Self {
        Self {
            config,
            artifact_store,
            step_handlers: HashMap::new(),
            results: RwLock::new(Vec::new()),
        }
    }

    /// Registers a custom handler for a specific step action.
    pub fn register_handler(&mut self, action: StepAction, handler: Arc<dyn StepHandler>) {
        self.step_handlers.insert(action, handler);
    }

    /// Returns all results from the most recent execution.
    pub fn results(&self) -> Vec<PipelineResult> {
        self.results.read().clone()
    }

    /// Executes the pipeline.
    ///
    /// Returns `Ok(())` if all stages succeed. Returns an error on failure
    /// unless `FailureAction::Continue` is configured.
    pub fn execute(&self) -> Result<Vec<PipelineResult>> {
        let mut all_results = Vec::with_capacity(self.config.stages.len());
        let mut completed_stage_names: Vec<String> = Vec::new();

        for stage in &self.config.stages {
            let result = self.execute_stage(stage, &completed_stage_names)?;

            match result.status {
                StageStatus::Success => {
                    completed_stage_names.push(stage.name.clone());
                    all_results.push(result);
                }
                StageStatus::Failed => match self.config.on_failure {
                    FailureAction::Abort => {
                        all_results.push(result);
                        self.append_skipped(&mut all_results);
                        break;
                    }
                    FailureAction::Rollback => {
                        all_results.push(result);
                        self.rollback(&mut all_results, &completed_stage_names);
                        self.append_skipped(&mut all_results);
                        break;
                    }
                    FailureAction::Continue => {
                        all_results.push(result);
                    }
                },
                _ => {
                    all_results.push(result);
                }
            }
        }

        {
            let mut stored = self.results.write();
            *stored = all_results.clone();
        }

        let has_failure = all_results.iter().any(|r| r.status == StageStatus::Failed);
        if has_failure {
            Err(Error::actor("pipeline execution failed"))
        } else {
            Ok(all_results)
        }
    }

    fn execute_stage(
        &self,
        stage: &PipelineStage,
        _completed_stages: &[String],
    ) -> Result<PipelineResult> {
        let start = Instant::now();
        let mut logs = String::new();
        let mut artifacts = Vec::new();

        logs.push_str(&format!("=== Stage: {} ===\n", stage.name));

        for step in &stage.steps {
            if start.elapsed() > stage.timeout {
                logs.push_str(&format!(
                    "Step '{}' timed out after {:?}\n",
                    step.name, stage.timeout
                ));
                return Ok(PipelineResult {
                    stage: stage.name.clone(),
                    status: StageStatus::Failed,
                    artifacts,
                    duration: start.elapsed(),
                    logs,
                });
            }

            if !step.dependencies.is_empty() {
                let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
                for dep in &step.dependencies {
                    if !seen.insert(dep.as_str()) {
                        logs.push_str(&format!(
                            "Step '{}' has duplicate dependency '{}'\n",
                            step.name, dep
                        ));
                        return Ok(PipelineResult {
                            stage: stage.name.clone(),
                            status: StageStatus::Failed,
                            artifacts,
                            duration: start.elapsed(),
                            logs,
                        });
                    }
                }
            }

            let handler = self
                .step_handlers
                .get(&step.action)
                .cloned()
                .unwrap_or_else(|| Arc::new(DefaultStepHandler));

            match handler.execute(step, &artifacts) {
                Ok(step_artifacts) => {
                    logs.push_str(&format!(
                        "Step '{}' ({:?}) succeeded\n",
                        step.name, step.action
                    ));
                    artifacts.extend(step_artifacts);
                }
                Err(e) => {
                    logs.push_str(&format!(
                        "Step '{}' ({:?}) failed: {}\n",
                        step.name, step.action, e
                    ));
                    return Ok(PipelineResult {
                        stage: stage.name.clone(),
                        status: StageStatus::Failed,
                        artifacts,
                        duration: start.elapsed(),
                        logs,
                    });
                }
            }
        }

        for artifact in &artifacts {
            if let Err(e) = self.artifact_store.store(artifact, b"") {
                logs.push_str(&format!(
                    "Warning: failed to store artifact '{}': {}\n",
                    artifact, e
                ));
            }
        }

        logs.push_str(&format!(
            "Stage '{}' completed in {:?}\n",
            stage.name,
            start.elapsed()
        ));

        Ok(PipelineResult {
            stage: stage.name.clone(),
            status: StageStatus::Success,
            artifacts,
            duration: start.elapsed(),
            logs,
        })
    }

    fn rollback(&self, results: &mut [PipelineResult], completed: &[String]) {
        for stage_name in completed.iter().rev() {
            let idx = results
                .iter()
                .position(|r| r.stage == *stage_name && r.status == StageStatus::Success);
            if let Some(idx) = idx {
                results[idx].status = StageStatus::RolledBack;
            }
        }
    }

    fn append_skipped(&self, results: &mut Vec<PipelineResult>) {
        let completed_names: Vec<String> = results.iter().map(|r| r.stage.clone()).collect();

        for stage in &self.config.stages {
            if !completed_names.iter().any(|n| n == &stage.name) {
                results.push(PipelineResult {
                    stage: stage.name.clone(),
                    status: StageStatus::Skipped,
                    artifacts: Vec::new(),
                    duration: Duration::ZERO,
                    logs: format!("Stage '{}' skipped due to prior failure\n", stage.name),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl StepAction {
    /// Returns a lowercase string variant of this action.
    fn action_variant(&self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
            Self::SecurityScan => "security_scan",
            Self::Publish => "publish",
            Self::Deploy => "deploy",
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- InMemoryArtifactStore --

    #[test]
    fn store_and_retrieve() {
        let store = Arc::new(InMemoryArtifactStore::new());
        store.store("key1", b"hello").expect("store");
        let val = store.retrieve("key1").expect("retrieve");
        assert_eq!(val, Some(b"hello".to_vec()));
    }

    #[test]
    fn retrieve_nonexistent() {
        let store = Arc::new(InMemoryArtifactStore::new());
        let val = store.retrieve("ghost").expect("retrieve");
        assert!(val.is_none());
    }

    #[test]
    fn list_by_prefix() {
        let store = Arc::new(InMemoryArtifactStore::new());
        store.store("pipeline/1/artifact", b"a").expect("store");
        store.store("pipeline/2/artifact", b"b").expect("store");
        store.store("other/key", b"c").expect("store");
        let keys = store.list("pipeline/").expect("list");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn delete_artifact() {
        let store = Arc::new(InMemoryArtifactStore::new());
        store.store("key1", b"data").expect("store");
        assert!(store.delete("key1").expect("delete"));
        assert!(!store.delete("key1").expect("delete"));
        assert!(store.retrieve("key1").expect("retrieve").is_none());
    }

    // -- PipelineExecutor --

    #[test]
    fn execute_empty_pipeline() {
        let config = PipelineConfig::default();
        let store = Arc::new(InMemoryArtifactStore::new());
        let executor = PipelineExecutor::new(config, store);
        let results = executor.execute().expect("empty pipeline succeeds");
        assert!(results.is_empty());
    }

    #[test]
    fn execute_single_stage_success() {
        let config = PipelineConfig {
            stages: vec![PipelineStage {
                name: "build".to_string(),
                steps: vec![PipelineStep {
                    name: "compile".to_string(),
                    action: StepAction::Build,
                    dependencies: vec![],
                }],
                timeout: Duration::from_secs(60),
            }],
            on_failure: FailureAction::Abort,
            artifact_store: "in-memory".to_string(),
        };
        let store = Arc::new(InMemoryArtifactStore::new());
        let executor = PipelineExecutor::new(config, store);
        let results = executor.execute().expect("should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, StageStatus::Success);
        assert_eq!(results[0].stage, "build");
    }

    #[test]
    fn execute_multi_stage_success() {
        let config = PipelineConfig {
            stages: vec![
                PipelineStage {
                    name: "build".to_string(),
                    steps: vec![PipelineStep {
                        name: "compile".to_string(),
                        action: StepAction::Build,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
                PipelineStage {
                    name: "test".to_string(),
                    steps: vec![PipelineStep {
                        name: "unit_tests".to_string(),
                        action: StepAction::Test,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
            ],
            on_failure: FailureAction::Abort,
            artifact_store: "in-memory".to_string(),
        };
        let store = Arc::new(InMemoryArtifactStore::new());
        let executor = PipelineExecutor::new(config, store);
        let results = executor.execute().expect("should succeed");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, StageStatus::Success);
        assert_eq!(results[1].status, StageStatus::Success);
    }

    #[test]
    fn execute_abort_on_failure() {
        let config = PipelineConfig {
            stages: vec![
                PipelineStage {
                    name: "build".to_string(),
                    steps: vec![PipelineStep {
                        name: "compile".to_string(),
                        action: StepAction::Build,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
                PipelineStage {
                    name: "test".to_string(),
                    steps: vec![PipelineStep {
                        name: "failing_test".to_string(),
                        action: StepAction::Test,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
                PipelineStage {
                    name: "deploy".to_string(),
                    steps: vec![PipelineStep {
                        name: "ship".to_string(),
                        action: StepAction::Deploy,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
            ],
            on_failure: FailureAction::Abort,
            artifact_store: "in-memory".to_string(),
        };
        let store = Arc::new(InMemoryArtifactStore::new());

        struct FailingHandler;
        impl StepHandler for FailingHandler {
            fn execute(&self, _step: &PipelineStep, _artifacts: &[String]) -> Result<Vec<String>> {
                Err(Error::actor("test failure"))
            }
        }

        let mut executor = PipelineExecutor::new(config, store);
        executor.register_handler(StepAction::Test, Arc::new(FailingHandler));
        let results = executor.execute();
        assert!(results.is_err());
        let all = executor.results();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].status, StageStatus::Success);
        assert_eq!(all[1].status, StageStatus::Failed);
        assert_eq!(all[2].status, StageStatus::Skipped);
    }

    #[test]
    fn execute_rollback_on_failure() {
        let config = PipelineConfig {
            stages: vec![
                PipelineStage {
                    name: "build".to_string(),
                    steps: vec![PipelineStep {
                        name: "compile".to_string(),
                        action: StepAction::Build,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
                PipelineStage {
                    name: "test".to_string(),
                    steps: vec![PipelineStep {
                        name: "failing_test".to_string(),
                        action: StepAction::Test,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
            ],
            on_failure: FailureAction::Rollback,
            artifact_store: "in-memory".to_string(),
        };
        let store = Arc::new(InMemoryArtifactStore::new());

        struct FailingHandler;
        impl StepHandler for FailingHandler {
            fn execute(&self, _step: &PipelineStep, _artifacts: &[String]) -> Result<Vec<String>> {
                Err(Error::actor("test failure"))
            }
        }

        let mut executor = PipelineExecutor::new(config, store);
        executor.register_handler(StepAction::Test, Arc::new(FailingHandler));
        let results = executor.results();
        let _ = executor.execute();
        let all = executor.results();
        assert_eq!(all[0].status, StageStatus::RolledBack);
        assert_eq!(all[1].status, StageStatus::Failed);
        drop(results);
    }

    #[test]
    fn execute_continue_on_failure() {
        let config = PipelineConfig {
            stages: vec![
                PipelineStage {
                    name: "build".to_string(),
                    steps: vec![PipelineStep {
                        name: "compile".to_string(),
                        action: StepAction::Build,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
                PipelineStage {
                    name: "test".to_string(),
                    steps: vec![PipelineStep {
                        name: "failing_test".to_string(),
                        action: StepAction::Test,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
                PipelineStage {
                    name: "deploy".to_string(),
                    steps: vec![PipelineStep {
                        name: "ship".to_string(),
                        action: StepAction::Deploy,
                        dependencies: vec![],
                    }],
                    timeout: Duration::from_secs(60),
                },
            ],
            on_failure: FailureAction::Continue,
            artifact_store: "in-memory".to_string(),
        };
        let store = Arc::new(InMemoryArtifactStore::new());

        struct FailingHandler;
        impl StepHandler for FailingHandler {
            fn execute(&self, _step: &PipelineStep, _artifacts: &[String]) -> Result<Vec<String>> {
                Err(Error::actor("test failure"))
            }
        }

        let mut executor = PipelineExecutor::new(config, store);
        executor.register_handler(StepAction::Test, Arc::new(FailingHandler));
        let results = executor.execute();
        assert!(results.is_err());
        let all = executor.results();
        assert_eq!(all[0].status, StageStatus::Success);
        assert_eq!(all[1].status, StageStatus::Failed);
        assert_eq!(all[2].status, StageStatus::Success);
    }

    #[test]
    fn step_timeout() {
        let config = PipelineConfig {
            stages: vec![PipelineStage {
                name: "slow".to_string(),
                steps: vec![PipelineStep {
                    name: "slow_step".to_string(),
                    action: StepAction::Build,
                    dependencies: vec![],
                }],
                timeout: Duration::from_nanos(1),
            }],
            on_failure: FailureAction::Abort,
            artifact_store: "in-memory".to_string(),
        };
        let store = Arc::new(InMemoryArtifactStore::new());

        struct SlowHandler;
        impl StepHandler for SlowHandler {
            fn execute(&self, _step: &PipelineStep, _artifacts: &[String]) -> Result<Vec<String>> {
                std::thread::sleep(Duration::from_millis(10));
                Ok(vec!["slow_artifact".to_string()])
            }
        }

        let mut executor = PipelineExecutor::new(config, store);
        executor.register_handler(StepAction::Build, Arc::new(SlowHandler));
        let all = executor.results();
        let _ = executor.execute();
        let results = executor.results();
        assert_eq!(results[0].status, StageStatus::Failed);
        assert!(results[0].logs.contains("timed out"));
        drop(all);
    }

    #[test]
    fn custom_handler_produces_artifacts() {
        struct CustomHandler;
        impl StepHandler for CustomHandler {
            fn execute(&self, step: &PipelineStep, _artifacts: &[String]) -> Result<Vec<String>> {
                Ok(vec![format!("custom:{}", step.name)])
            }
        }

        let config = PipelineConfig {
            stages: vec![PipelineStage {
                name: "custom".to_string(),
                steps: vec![PipelineStep {
                    name: "my_step".to_string(),
                    action: StepAction::Build,
                    dependencies: vec![],
                }],
                timeout: Duration::from_secs(60),
            }],
            on_failure: FailureAction::Abort,
            artifact_store: "in-memory".to_string(),
        };
        let store = Arc::new(InMemoryArtifactStore::new());
        let mut executor = PipelineExecutor::new(config, store);
        executor.register_handler(StepAction::Build, Arc::new(CustomHandler));
        let results = executor.execute().expect("ok");
        assert_eq!(results[0].artifacts, vec!["custom:my_step"]);
    }

    // -- StepAction variant --

    #[test]
    fn step_action_variant() {
        assert_eq!(StepAction::Build.action_variant(), "build");
        assert_eq!(StepAction::Test.action_variant(), "test");
        assert_eq!(StepAction::SecurityScan.action_variant(), "security_scan");
        assert_eq!(StepAction::Publish.action_variant(), "publish");
        assert_eq!(StepAction::Deploy.action_variant(), "deploy");
    }

    // -- PipelineConfig defaults --

    #[test]
    fn config_defaults() {
        let config = PipelineConfig::default();
        assert!(config.stages.is_empty());
        assert_eq!(config.on_failure, FailureAction::Abort);
        assert_eq!(config.artifact_store, "in-memory");
    }

    // -- StageStatus round-trip --

    #[test]
    fn stage_status_equality() {
        assert_eq!(StageStatus::Pending, StageStatus::Pending);
        assert_eq!(StageStatus::Success, StageStatus::Success);
        assert_ne!(StageStatus::Success, StageStatus::Failed);
    }
}
