//! io_uring-backed storage backend using monoio.
//!
//! Stores blobs as files in a configurable directory: `{root_dir}/{key}`.
//!
//! Uses `monoio` for io_uring-backed async file I/O. A dedicated background
//! thread runs a `monoio` runtime and processes file operations via
//! `std::sync::mpsc` channels to avoid mixing tokio and monoio runtimes.
//!
//! If io_uring is not available at runtime (e.g., kernel < 5.1), operations
//! transparently fall back to `tokio::fs`.
//!
//! Directory listing always uses `tokio::fs` because `monoio` 0.2 does not
//! export a `read_dir` primitive.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::oneshot;

use super::{StorageBackend, StorageResult, key_path};

enum FsOp {
    Read {
        path: PathBuf,
        reply: oneshot::Sender<io::Result<Vec<u8>>>,
    },
    AtomicWrite {
        path: PathBuf,
        data: Vec<u8>,
        reply: oneshot::Sender<io::Result<()>>,
    },
    Remove {
        path: PathBuf,
        reply: oneshot::Sender<io::Result<bool>>,
    },
}

struct MonoioWorker {
    tx: std::sync::mpsc::Sender<FsOp>,
    _thread: std::thread::JoinHandle<()>,
}

impl MonoioWorker {
    fn spawn() -> io::Result<Self> {
        let (tx, rx) = std::sync::mpsc::channel::<FsOp>();
        let (ready_tx, ready_rx) = oneshot::channel::<io::Result<()>>();

        let thread = std::thread::Builder::new()
            .name("aether-io-uring-storage".into())
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
                            let result = rt.block_on(async {
                                match monoio::fs::remove_file(&path).await {
                                    Ok(()) => Ok(true),
                                    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
                                    Err(e) => Err(e),
                                }
                            });
                            let _ = reply.send(result);
                        }
                    }
                }
            })?;

        match tokio::task::block_in_place(|| ready_rx.blocking_recv()) {
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

async fn write_then_rename_monoio(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp_path = {
        let mut p = path.as_os_str().to_os_string();
        p.push(".tmp");
        PathBuf::from(p)
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&tmp_path, data)?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        io::Error::other(format!("rename failed: {e}"))
    })?;

    Ok(())
}

async fn tokio_read_file(path: &Path) -> io::Result<Vec<u8>> {
    tokio::fs::read(path).await
}

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

    let tmp_file = tokio::fs::File::open(&tmp_path).await?;
    tmp_file.sync_all().await?;

    tokio::fs::rename(&tmp_path, path).await.map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        io::Error::other(format!("rename failed: {e}"))
    })?;

    Ok(())
}

async fn tokio_delete_file(path: &Path) -> io::Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// io_uring-backed storage backend for high-performance disk I/O.
///
/// Stores blobs as files in a configurable directory: `{root_dir}/{key}`.
/// Uses `monoio` for io_uring-backed async file I/O via a dedicated
/// background thread. If io_uring is not available at runtime, operations
/// transparently fall back to `tokio::fs`.
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
/// use aether_core::storage::{StorageBackend, io_uring::IoUringStorage};
///
/// # async fn example() -> aether_core::Result<()> {
/// let backend = IoUringStorage::open(std::path::Path::new("/var/lib/aether/state")).await?;
/// backend.write("actor-1:counter", b"42").await?;
/// let data = backend.read("actor-1:counter").await?;
/// # Ok(())
/// # }
/// ```
pub struct IoUringStorage {
    root_dir: PathBuf,
    uring_available: bool,
    worker: Option<Arc<MonoioWorker>>,
}

impl IoUringStorage {
    async fn read_raw(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let path = key_path(&self.root_dir, key)?;

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
            Err(e) => Err(crate::error::Error::storage_read(format!(
                "read failed for {path:?}: {e}"
            ))),
        }
    }

    async fn write_raw(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let path = key_path(&self.root_dir, key)?;

        let result = if let Some(ref worker) = self.worker {
            worker
                .send_op(|reply| FsOp::AtomicWrite {
                    path,
                    data: data.to_vec(),
                    reply,
                })
                .await
        } else {
            tokio_write_file(&path, data).await
        };

        result.map_err(|e| crate::error::Error::storage_write(format!("write failed: {e}")))?;

        Ok(())
    }

    /// Check whether io_uring acceleration is active.
    #[must_use]
    pub fn is_uring_enabled(&self) -> bool {
        self.uring_available
    }
}

#[async_trait]
impl StorageBackend for IoUringStorage {
    async fn open(path: &Path) -> StorageResult<Self> {
        let root_dir = path.to_path_buf();

        tokio::fs::create_dir_all(&root_dir).await.map_err(|e| {
            crate::error::Error::storage_write(format!(
                "failed to create storage root directory {}: {e}",
                root_dir.display()
            ))
        })?;

        let worker = match MonoioWorker::spawn() {
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

    async fn read(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.read_raw(key).await
    }

    async fn write(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.write_raw(key, data).await
    }

    async fn delete(&self, key: &str) -> StorageResult<bool> {
        let path = key_path(&self.root_dir, key)?;

        let result = if let Some(ref worker) = self.worker {
            worker.send_op(|reply| FsOp::Remove { path, reply }).await
        } else {
            tokio_delete_file(&path).await
        };

        result.map_err(|e| crate::error::Error::storage_write(format!("delete failed: {e}")))
    }

    async fn list(&self, prefix: &str) -> StorageResult<Vec<String>> {
        let mut entries = tokio::fs::read_dir(&self.root_dir)
            .await
            .map_err(|e| crate::error::Error::storage_read(format!("list_dir failed: {e}")))?;

        let mut keys = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| crate::error::Error::storage_read(format!("list_dir failed: {e}")))?
        {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') || name_str.ends_with(".tmp") {
                continue;
            }
            if !entry
                .file_type()
                .await
                .map_err(|e| crate::error::Error::storage_read(format!("list_dir failed: {e}")))?
                .is_file()
            {
                continue;
            }
            let name_owned = name_str.into_owned();
            if name_owned.starts_with(prefix) {
                keys.push(name_owned);
            }
        }
        Ok(keys)
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        Ok(self.read_raw(key).await?.is_some())
    }

    async fn close(&self) -> StorageResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::StorageBackend;
    use super::*;
    use tempfile::TempDir;

    async fn new_test_backend() -> (IoUringStorage, TempDir) {
        let dir = TempDir::new().expect("create temp dir");
        let backend = IoUringStorage::open(dir.path())
            .await
            .expect("open backend");
        (backend, dir)
    }

    #[tokio::test]
    async fn test_write_read_roundtrip() {
        let (backend, _dir) = new_test_backend().await;

        backend
            .write("key1", b"hello world")
            .await
            .expect("write failed");

        let val = backend
            .read("key1")
            .await
            .expect("read failed")
            .expect("not found");
        assert_eq!(val, b"hello world");
    }

    #[tokio::test]
    async fn test_read_nonexistent() {
        let (backend, _dir) = new_test_backend().await;
        let val = backend.read("missing").await.expect("read failed");
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let (backend, _dir) = new_test_backend().await;

        backend.write("key1", b"data").await.expect("write failed");

        let deleted = backend.delete("key1").await.expect("delete failed");
        assert!(deleted);

        let deleted = backend.delete("key1").await.expect("delete failed");
        assert!(!deleted);

        let val = backend.read("key1").await.expect("read failed");
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn test_exists() {
        let (backend, _dir) = new_test_backend().await;

        assert!(!backend.exists("key1").await.expect("exists failed"));

        backend.write("key1", b"data").await.expect("write failed");
        assert!(backend.exists("key1").await.expect("exists failed"));
    }

    #[tokio::test]
    async fn test_list_with_prefix() {
        let (backend, _dir) = new_test_backend().await;

        backend
            .write("app:counter", b"1")
            .await
            .expect("write failed");
        backend
            .write("app:config", b"cfg")
            .await
            .expect("write failed");
        backend
            .write("other:key", b"v")
            .await
            .expect("write failed");

        let keys = backend.list("app:").await.expect("list failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"app:counter".to_string()));
        assert!(keys.contains(&"app:config".to_string()));
    }

    #[tokio::test]
    async fn test_overwrite() {
        let (backend, _dir) = new_test_backend().await;

        backend.write("key1", b"first").await.expect("write failed");
        backend
            .write("key1", b"second")
            .await
            .expect("write failed");

        let val = backend
            .read("key1")
            .await
            .expect("read failed")
            .expect("not found");
        assert_eq!(val, b"second");
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
                b.write(&key, i.to_le_bytes()).await.expect("write failed");

                let val = b.read(&key).await.expect("read failed").expect("not found");
                assert_eq!(val, i.to_le_bytes());
            }));
        }

        for h in handles {
            h.await.expect("join failed");
        }

        let keys = backend.list("key-").await.expect("list failed");
        assert_eq!(keys.len(), 10);
    }

    #[tokio::test]
    async fn test_close_is_ok() {
        let (backend, _dir) = new_test_backend().await;
        backend.close().await.expect("close failed");
    }
}
