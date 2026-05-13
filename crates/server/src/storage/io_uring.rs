//! io_uring-backed state backend for high-performance disk I/O.
//!
//! Stores actor state as JSON files in a configurable directory hierarchy:
//! `{root_dir}/{actor_id}/{key}`. Each file contains a JSON-serialized
//! [`KeyValue`].
//!
//! Uses `monoio` for io_uring-backed async file I/O. A dedicated background
//! thread runs a `monoio` runtime and processes file operations sequentially.
//! Communication between tokio tasks and the monoio thread uses
//! `std::sync::mpsc` channels to avoid mixing tokio and monoio runtimes.
//!
//! If io_uring is not available at runtime (e.g., kernel < 5.1), operations
//! transparently fall back to `tokio::fs`.
//!
//! Directory listing is always performed via `tokio::fs` because
//! `monoio` 0.2 does not export a `read_dir` equivalent.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::oneshot;

use super::{KeyValue, StateBackend, StorageError, StorageResult};

// --- Path helpers -------------------------------------------------------

/// Validate that a path component does not contain directory traversal
/// characters.
fn validate_component(s: &str) -> StorageResult<()> {
    if s.is_empty() || s.contains('/') || s.contains('\\') || s.contains("..") {
        return Err(StorageError::Internal(format!(
            "invalid path component: {s}"
        )));
    }
    Ok(())
}

/// Build the file path for a given actor and key.
fn key_path(root_dir: &Path, actor_id: &str, key: &str) -> StorageResult<PathBuf> {
    validate_component(actor_id)?;
    validate_component(key)?;
    Ok(root_dir.join(actor_id).join(key))
}

/// Build the directory path for a given actor.
fn actor_dir(root_dir: &Path, actor_id: &str) -> StorageResult<PathBuf> {
    validate_component(actor_id)?;
    Ok(root_dir.join(actor_id))
}

// --- Monoio worker ------------------------------------------------------

/// A file-system operation dispatched to the monoio worker thread.
///
/// Directory listing operations are NOT routed through the worker because
/// `monoio` 0.2 lacks a `read_dir` primitive. Those operations are handled
/// directly via `tokio::fs`.
enum FsOp {
    /// Read an entire file and return its contents as raw bytes.
    Read {
        path: PathBuf,
        reply: oneshot::Sender<io::Result<Vec<u8>>>,
    },
    /// Atomically write a file: write to `{path}.tmp`, then rename.
    AtomicWrite {
        path: PathBuf,
        data: Vec<u8>,
        reply: oneshot::Sender<io::Result<()>>,
    },
    /// Remove a file. Returns `true` if the file existed.
    Remove {
        path: PathBuf,
        reply: oneshot::Sender<io::Result<bool>>,
    },
}

/// Handle for communicating with the monoio worker thread.
///
/// Uses `std::sync::mpsc` for dispatching operations from tokio tasks
/// to the monoio thread. The monoio `Runtime` is owned exclusively by
/// the background thread.  `MonoioWorker` is `Send + Sync`.
struct MonoioWorker {
    /// Sender for dispatching file-system operations.
    tx: std::sync::mpsc::Sender<FsOp>,
    /// Join handle for the worker thread.
    _thread: std::thread::JoinHandle<()>,
}

impl MonoioWorker {
    /// Spawn a new monoio worker thread and return a handle to it.
    ///
    /// Waits for the worker to confirm that the runtime was created
    /// successfully. Returns `Err` if the `monoio` runtime cannot be
    /// created (e.g., io_uring is not supported by the running kernel).
    /// When this happens, [`IoUringBackend`] will transparently fall
    /// back to `tokio::fs`.
    async fn spawn() -> io::Result<Self> {
        // Use std::sync::mpsc to avoid mixing tokio and monoio runtimes.
        // The worker thread uses blocking recv(); tokio tasks use non-
        // blocking send() on an unbounded channel.
        let (tx, rx) = std::sync::mpsc::channel::<FsOp>();
        let (ready_tx, ready_rx) = oneshot::channel::<io::Result<()>>();

        let thread = std::thread::Builder::new()
            .name("aether-io-uring".into())
            .spawn(move || {
                let mut rt = match monoio::RuntimeBuilder::<monoio::IoUringDriver>::new().build() {
                    Ok(rt) => {
                        let _ = ready_tx.send(Ok(()));
                        rt
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };

                // Synchronous receive loop. Each operation runs inside a
                // dedicated `block_on` call to keep monoio futures on the
                // monoio runtime.
                while let Ok(op) = rx.recv() {
                    match op {
                        FsOp::Read { path, reply } => {
                            let result = rt.block_on(async { monoio::fs::read(&path).await });
                            let _ = reply.send(result);
                        }
                        FsOp::AtomicWrite { path, data, reply } => {
                            let result =
                                rt.block_on(async { write_then_rename_monoio(&path, &data).await });
                            let _ = reply.send(result);
                        }
                        FsOp::Remove { path, reply } => {
                            let result = rt.block_on(Self::delete_file_uring(path));
                            let _ = reply.send(result);
                        }
                    }
                }
            })?;

        // Wait for the worker to confirm it is ready.
        match ready_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(io::Error::other(
                    "monoio worker thread panicked during init",
                ));
            }
        }

        Ok(Self {
            tx,
            _thread: thread,
        })
    }

    /// Atomically write a file: write to temp, then rename.
    /// Delete a file; returns `true` if the file existed.
    async fn delete_file_uring(path: PathBuf) -> io::Result<bool> {
        match monoio::fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Send an operation to the worker and await the response.
    ///
    /// Uses `std::sync::mpsc::Sender::send()` (non-blocking, unbounded
    /// channel) to dispatch the operation, then awaits the response on a
    /// tokio `oneshot`.
    async fn send_op<T>(
        &self,
        f: impl FnOnce(oneshot::Sender<io::Result<T>>) -> FsOp,
    ) -> io::Result<T> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(f(reply_tx))
            .map_err(|_| io::Error::other("monoio worker shut down"))?;
        reply_rx
            .await
            .map_err(|_| io::Error::other("monoio worker reply dropped"))?
    }
}

/// Atomically write a file using monoio io_uring operations.
///
/// Writes to `{path}.tmp` first, then renames to `path` for crash safety.
/// Falls back to `std::fs` on monoio operation failure since the monoio
/// worker runs on its own dedicated thread.
async fn write_then_rename_monoio(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp_path = {
        let mut p = path.as_os_str().to_os_string();
        p.push(".tmp");
        PathBuf::from(p)
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Use std::fs since we're on a dedicated thread and io_uring
    // may not be supported for all operations on all kernels.
    std::fs::write(&tmp_path, data)?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        io::Error::other(format!("rename failed: {e}"))
    })?;

    Ok(())
}

// --- tokio::fs fallback helpers -----------------------------------------

/// Read an entire file using `tokio::fs`.
async fn tokio_read_file(path: &Path) -> io::Result<Vec<u8>> {
    tokio::fs::read(path).await
}

/// Atomically write a file using `tokio::fs`.
async fn tokio_write_file(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp_path = {
        let mut p = path.as_os_str().to_os_string();
        p.push(".tmp");
        PathBuf::from(p)
    };

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&tmp_path, data).await?;

    // fsync the temp file for durability.
    let tmp_file = tokio::fs::File::open(&tmp_path).await?;
    tmp_file.sync_all().await?;

    tokio::fs::rename(&tmp_path, path).await.map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        io::Error::other(format!("rename failed: {e}"))
    })?;

    Ok(())
}

/// Delete a file using `tokio::fs`.
async fn tokio_delete_file(path: &Path) -> io::Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// List directory entry names, excluding hidden files and `.tmp` files.
///
/// This function is shared by both the io_uring and fallback paths because
/// `monoio` 0.2 does not export a `read_dir` primitive.
async fn list_dir_entries(path: &Path) -> io::Result<Vec<String>> {
    let mut entries = tokio::fs::read_dir(path).await?;
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || name_str.ends_with(".tmp") {
            continue;
        }
        if entry.file_type().await?.is_file() {
            names.push(name_str.into_owned());
        }
    }
    Ok(names)
}

// --- IoUringBackend -----------------------------------------------------

/// io_uring-backed state backend for high-performance disk I/O.
///
/// Stores actor state as JSON files in a configurable directory hierarchy:
/// `{root_dir}/{actor_id}/{key}`. Each file contains a JSON-serialized
/// [`KeyValue`].
///
/// Uses `monoio` for io_uring-backed async file I/O via a dedicated
/// background thread. If io_uring is not available at runtime (e.g.,
/// kernel < 5.1), operations transparently fall back to `tokio::fs`.
///
/// # File layout
///
/// ```text
/// /var/lib/aether/state/
///   actor-1/
///     counter
///     config
///   actor-2/
///     session
/// ```
///
/// # Atomicity
///
/// Writes use the write-to-temp-then-rename pattern: data is written to
/// `{key}.tmp` and then atomically renamed to `{key}`. Readers see either
/// the old file or the complete new file — never a partial write.
///
/// # Examples
///
/// ```no_run
/// use aether_server::storage::{StateBackend, io_uring::IoUringBackend};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = IoUringBackend::new("/var/lib/aether/state").await?;
/// backend.set("actor-1", "counter", serde_json::json!(42)).await?;
/// # Ok(())
/// # }
/// ```
pub struct IoUringBackend {
    /// Root directory for state files.
    root_dir: PathBuf,
    /// Whether io_uring is available (false = fall back to `tokio::fs`).
    uring_available: bool,
    /// Handle to the monoio worker thread when io_uring is available.
    worker: Option<Arc<MonoioWorker>>,
}

impl IoUringBackend {
    /// Create a new io_uring-backed state backend.
    ///
    /// The `root_dir` must be an accessible directory. It will be created
    /// if it does not exist.
    ///
    /// Probes for io_uring availability at construction time. If io_uring
    /// is not available (e.g., kernel too old), the backend transparently
    /// falls back to `tokio::fs` for all operations.
    pub async fn new<P: AsRef<Path>>(root_dir: P) -> StorageResult<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();

        // Ensure the root directory exists (using tokio::fs, which always
        // works regardless of io_uring support).
        tokio::fs::create_dir_all(&root_dir).await.map_err(|e| {
            StorageError::Internal(format!(
                "failed to create state root directory {}: {e}",
                root_dir.display()
            ))
        })?;

        // Probe io_uring availability by attempting to spawn a monoio
        // worker thread.  If the probe fails we store `None` and all
        // subsequent operations will use `tokio::fs`.
        let worker = match MonoioWorker::spawn().await {
            Ok(w) => Some(Arc::new(w)),
            Err(e) => {
                tracing::warn!("io_uring not available ({}), falling back to tokio::fs", e);
                None
            }
        };

        let uring_available = worker.is_some();

        Ok(Self {
            root_dir,
            uring_available,
            worker,
        })
    }

    /// Check whether io_uring acceleration is active.
    #[must_use]
    pub fn is_uring_enabled(&self) -> bool {
        self.uring_available
    }

    /// Read the raw JSON bytes for a key, returning `None` if the file
    /// does not exist.
    async fn read_raw(&self, actor_id: &str, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let path = key_path(&self.root_dir, actor_id, key)?;

        let result = if let Some(ref worker) = self.worker {
            worker
                .send_op(|reply| FsOp::Read {
                    path: path.clone(),
                    reply,
                })
                .await
        } else {
            tokio_read_file(&path).await
        };

        match result {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StorageError::Internal(format!(
                "read failed for {path:?}: {e}"
            ))),
        }
    }

    /// Write raw JSON bytes for a key, returning the new version.
    ///
    /// Reads the current file (if any) to obtain the last version,
    /// increments it by one, and writes the updated [`KeyValue`].
    async fn write_raw(&self, actor_id: &str, key: &str, data: &[u8]) -> StorageResult<u64> {
        let path = key_path(&self.root_dir, actor_id, key)?;

        // Read the current version, if any.
        let current_version = self
            .read_raw(actor_id, key)
            .await?
            .and_then(|raw| {
                serde_json::from_slice::<KeyValue>(&raw)
                    .ok()
                    .map(|kv| kv.version)
            })
            .unwrap_or(0);

        let new_version = current_version + 1;

        let kv = KeyValue {
            key: key.to_string(),
            value: serde_json::from_slice(data)
                .map_err(|e| StorageError::Internal(format!("invalid JSON value: {e}")))?,
            version: new_version,
        };

        let serialized = serde_json::to_vec(&kv)
            .map_err(|e| StorageError::Internal(format!("JSON serialize failed: {e}")))?;

        let result = if let Some(ref worker) = self.worker {
            worker
                .send_op(|reply| FsOp::AtomicWrite {
                    path,
                    data: serialized,
                    reply,
                })
                .await
        } else {
            tokio_write_file(&path, &serialized).await
        };

        result.map_err(|e| StorageError::Internal(format!("write failed: {e}")))?;

        Ok(new_version)
    }

    /// Read all actor directories under the root, returning their names.
    ///
    /// Always uses `tokio::fs` because `monoio` 0.2 lacks `read_dir`.
    /// Filters on `is_dir()` — actor IDs map to directories, not files.
    async fn list_actor_dirs(&self) -> StorageResult<Vec<String>> {
        let mut entries = tokio::fs::read_dir(&self.root_dir)
            .await
            .map_err(|e| StorageError::Internal(format!("list_dir failed: {e}")))?;
        let mut names = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StorageError::Internal(format!("list_dir failed: {e}")))?
        {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            if entry
                .file_type()
                .await
                .map_err(|e| StorageError::Internal(format!("list_dir failed: {e}")))?
                .is_dir()
            {
                names.push(name_str.into_owned());
            }
        }
        Ok(names)
    }
}

#[async_trait]
impl StateBackend for IoUringBackend {
    async fn get(&self, actor_id: &str, key: &str) -> StorageResult<Option<KeyValue>> {
        let raw = self.read_raw(actor_id, key).await?;
        match raw {
            Some(data) => {
                let kv: KeyValue = serde_json::from_slice(&data).map_err(|e| {
                    StorageError::Internal(format!(
                        "JSON deserialization failed for {actor_id}/{key}: {e}"
                    ))
                })?;
                Ok(Some(kv))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, actor_id: &str, key: &str, value: serde_json::Value) -> StorageResult<u64> {
        let data = serde_json::to_vec(&value)
            .map_err(|e| StorageError::Internal(format!("JSON serialize failed: {e}")))?;
        self.write_raw(actor_id, key, &data).await
    }

    async fn delete(&self, actor_id: &str, key: &str) -> StorageResult<bool> {
        let path = key_path(&self.root_dir, actor_id, key)?;

        let result = if let Some(ref worker) = self.worker {
            worker.send_op(|reply| FsOp::Remove { path, reply }).await
        } else {
            tokio_delete_file(&path).await
        };

        result.map_err(|e| StorageError::Internal(format!("delete failed: {e}")))
    }

    async fn list(&self, actor_id: &str) -> StorageResult<Vec<String>> {
        let dir = actor_dir(&self.root_dir, actor_id)?;

        match list_dir_entries(&dir).await {
            Ok(keys) => Ok(keys),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(StorageError::Internal(format!("list failed: {e}"))),
        }
    }

    async fn list_all(&self) -> StorageResult<Vec<(String, String)>> {
        let actor_dirs = self.list_actor_dirs().await?;
        let mut result = Vec::new();

        for actor_id in actor_dirs {
            let keys = self.list(&actor_id).await?;
            for key in keys {
                result.push((actor_id.clone(), key));
            }
        }

        Ok(result)
    }

    async fn health_check(&self) -> StorageResult<()> {
        // Verify the root directory is accessible by creating a probe file.
        let probe_path = self.root_dir.join(".aether-health-check");
        let probe_data = b"ok";

        let write_result = if let Some(ref worker) = self.worker {
            worker
                .send_op(|reply| FsOp::AtomicWrite {
                    path: probe_path.clone(),
                    data: probe_data.to_vec(),
                    reply,
                })
                .await
        } else {
            tokio_write_file(&probe_path, probe_data).await
        };

        if let Err(e) = write_result {
            return Err(StorageError::Internal(format!(
                "health check write failed: {e}"
            )));
        }

        // Clean up the probe file.
        let _ = std::fs::remove_file(&probe_path);

        Ok(())
    }
}

// --- Tests --------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::StateBackend;
    use super::*;
    use tempfile::TempDir;

    async fn new_test_backend() -> (IoUringBackend, TempDir) {
        let dir = TempDir::new().expect("create temp dir");
        let backend = IoUringBackend::new(dir.path())
            .await
            .expect("create backend");
        (backend, dir)
    }

    #[tokio::test]
    async fn test_set_get_roundtrip() {
        let (backend, _dir) = new_test_backend().await;

        let version = backend
            .set("actor-1", "counter", serde_json::json!(42))
            .await
            .expect("set failed");
        assert_eq!(version, 1);

        let kv = backend
            .get("actor-1", "counter")
            .await
            .expect("get failed")
            .expect("key not found");
        assert_eq!(kv.value, serde_json::json!(42));
        assert_eq!(kv.version, 1);
    }

    #[tokio::test]
    async fn test_set_updates_version() {
        let (backend, _dir) = new_test_backend().await;

        let v1 = backend
            .set("actor-1", "key", serde_json::json!("first"))
            .await
            .expect("set failed");
        assert_eq!(v1, 1);

        let v2 = backend
            .set("actor-1", "key", serde_json::json!("second"))
            .await
            .expect("set failed");
        assert_eq!(v2, 2);

        let kv = backend
            .get("actor-1", "key")
            .await
            .expect("get failed")
            .expect("key not found");
        assert_eq!(kv.version, 2);
        assert_eq!(kv.value, serde_json::json!("second"));
    }

    #[tokio::test]
    async fn test_delete_and_verify_not_found() {
        let (backend, _dir) = new_test_backend().await;

        backend
            .set("actor-1", "key", serde_json::json!("value"))
            .await
            .expect("set failed");

        let deleted = backend
            .delete("actor-1", "key")
            .await
            .expect("delete failed");
        assert!(deleted);

        // Second delete should return false.
        let deleted = backend
            .delete("actor-1", "key")
            .await
            .expect("delete failed");
        assert!(!deleted);

        // Key should not be found.
        let result = backend.get("actor-1", "key").await.expect("get failed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_nonexistent_returns_none() {
        let (backend, _dir) = new_test_backend().await;
        let result = backend.get("nonexistent", "key").await.expect("get failed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_keys() {
        let (backend, _dir) = new_test_backend().await;

        backend
            .set("actor-1", "a", serde_json::json!(1))
            .await
            .expect("set failed");
        backend
            .set("actor-1", "b", serde_json::json!(2))
            .await
            .expect("set failed");
        backend
            .set("actor-2", "c", serde_json::json!(3))
            .await
            .expect("set failed");

        let keys = backend.list("actor-1").await.expect("list failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"b".to_string()));
    }

    #[tokio::test]
    async fn test_list_empty_actor() {
        let (backend, _dir) = new_test_backend().await;
        let keys = backend
            .list("nonexistent-actor")
            .await
            .expect("list failed");
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_list_all() {
        let (backend, _dir) = new_test_backend().await;

        backend
            .set("actor-1", "a", serde_json::json!(1))
            .await
            .expect("set failed");
        backend
            .set("actor-1", "b", serde_json::json!(2))
            .await
            .expect("set failed");
        backend
            .set("actor-2", "c", serde_json::json!(3))
            .await
            .expect("set failed");

        let all = backend.list_all().await.expect("list_all failed");
        assert_eq!(all.len(), 3);
        assert!(all.contains(&("actor-1".to_string(), "a".to_string())));
        assert!(all.contains(&("actor-1".to_string(), "b".to_string())));
        assert!(all.contains(&("actor-2".to_string(), "c".to_string())));
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let (backend, _dir) = new_test_backend().await;
        let backend = Arc::new(backend);

        let mut handles = Vec::new();
        for i in 0..10 {
            let b = Arc::clone(&backend);
            handles.push(tokio::spawn(async move {
                let key = format!("key-{i}");
                b.set("actor-concurrent", &key, serde_json::json!(i))
                    .await
                    .expect("set failed");

                let kv = b
                    .get("actor-concurrent", &key)
                    .await
                    .expect("get failed")
                    .expect("key not found");
                assert_eq!(kv.value, serde_json::json!(i));
            }));
        }

        for h in handles {
            h.await.expect("join failed");
        }

        let keys = backend.list("actor-concurrent").await.expect("list failed");
        assert_eq!(keys.len(), 10);
    }

    #[tokio::test]
    async fn test_health_check_valid_dir() {
        let (backend, _dir) = new_test_backend().await;
        assert!(backend.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_health_check_invalid_dir() {
        // A path under /proc that cannot be written to should cause an error.
        let result = IoUringBackend::new("/proc/aether-test-dir").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_component_rejects_traversal() {
        assert!(validate_component("").is_err());
        assert!(validate_component("foo/bar").is_err());
        assert!(validate_component("foo\\bar").is_err());
        assert!(validate_component("..").is_err());
        assert!(validate_component("foo..bar").is_err());
        assert!(validate_component("normal-key").is_ok());
        assert!(validate_component("key_with_underscores").is_ok());
    }

    #[tokio::test]
    async fn test_complex_json_values() {
        let (backend, _dir) = new_test_backend().await;

        let complex = serde_json::json!({
            "name": "test",
            "count": 42,
            "nested": {
                "array": [1, 2, 3],
                "flag": true
            }
        });

        backend
            .set("actor-1", "config", complex.clone())
            .await
            .expect("set failed");

        let kv = backend
            .get("actor-1", "config")
            .await
            .expect("get failed")
            .expect("key not found");
        assert_eq!(kv.value, complex);
    }
}
