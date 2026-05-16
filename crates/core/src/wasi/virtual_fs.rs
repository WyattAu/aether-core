//! Virtual Filesystem Abstraction
//!
//! Provides filesystem abstraction for WASM actors with capability-based
//! access control and path sandboxing.

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// File type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Regular file
    RegularFile,
    /// Directory
    Directory,
    /// Symbolic link
    Symlink,
    /// Unknown type
    Unknown,
}

/// File metadata structure
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// Type of the file
    pub file_type: FileType,
    /// Size in bytes
    pub size: u64,
    /// Last modified time in nanoseconds
    pub modified_ns: u64,
    /// Last accessed time in nanoseconds
    pub accessed_ns: u64,
    /// Creation time in nanoseconds
    pub created_ns: u64,
}

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Entry name
    pub name: String,
    /// Entry file type
    pub file_type: FileType,
}

/// Virtual filesystem trait for filesystem abstraction
#[async_trait]
pub trait VirtualFs: Send + Sync {
    /// Read file contents
    async fn read(&self, path: &Path) -> Result<Vec<u8>>;
    /// Write file contents
    async fn write(&self, path: &Path, data: &[u8]) -> Result<()>;
    /// Read directory entries
    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
    /// Create a directory
    async fn create_dir(&self, path: &Path) -> Result<()>;
    /// Remove a directory
    async fn remove_dir(&self, path: &Path) -> Result<()>;
    /// Remove a file
    async fn remove_file(&self, path: &Path) -> Result<()>;
    /// Get file metadata
    async fn metadata(&self, path: &Path) -> Result<FileMetadata>;
    /// Check if file exists
    async fn exists(&self, path: &Path) -> bool;
    /// Rename a file or directory
    async fn rename(&self, from: &Path, to: &Path) -> Result<()>;
}

/// Filesystem capabilities for capability-based access control
#[derive(Debug, Clone)]
pub struct FsCapabilities {
    /// Can read files
    pub can_read: bool,
    /// Can write files
    pub can_write: bool,
    /// Can delete files
    pub can_delete: bool,
}

impl FsCapabilities {
    /// Create FsCapabilities from a CapabilitySet
    pub fn from_capability_set(caps: CapabilitySet) -> Self {
        Self {
            can_read: caps.has_fs_read(),
            can_write: caps.has_fs_write(),
            can_delete: caps.has_fs_delete(),
        }
    }
}

/// Sandbox configuration for path-based access control
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// List of allowed path prefixes
    pub allowed_paths: Vec<PathBuf>,
    /// Filesystem capabilities
    pub capabilities: FsCapabilities,
}

impl SandboxConfig {
    /// Create a new sandbox configuration
    pub fn new(allowed_paths: Vec<PathBuf>, capabilities: FsCapabilities) -> Self {
        Self {
            allowed_paths,
            capabilities,
        }
    }

    /// Check if a path is allowed within the sandbox
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        let canonical_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => {
                for allowed in &self.allowed_paths {
                    if path.starts_with(allowed) || path == allowed {
                        return true;
                    }
                }
                return false;
            }
        };

        for allowed in &self.allowed_paths {
            if let Ok(canonical_allowed) = std::fs::canonicalize(allowed) {
                if canonical_path.starts_with(&canonical_allowed) {
                    return true;
                }
            } else if canonical_path.starts_with(allowed) {
                return true;
            }
        }
        false
    }

    /// Check read capability for a path
    pub fn check_read(&self, path: &Path) -> Result<()> {
        if !self.capabilities.can_read {
            return Err(Error::capability_denied_simple("fs:read not granted"));
        }
        if !self.is_path_allowed(path) {
            return Err(Error::capability_denied_simple(format!(
                "path {:?} not in sandbox",
                path
            )));
        }
        Ok(())
    }

    /// Check write capability for a path
    pub fn check_write(&self, path: &Path) -> Result<()> {
        if !self.capabilities.can_write {
            return Err(Error::capability_denied_simple("fs:write not granted"));
        }
        if !self.is_path_allowed(path) {
            return Err(Error::capability_denied_simple(format!(
                "path {:?} not in sandbox",
                path
            )));
        }
        Ok(())
    }

    /// Check delete capability for a path
    pub fn check_delete(&self, path: &Path) -> Result<()> {
        if !self.capabilities.can_delete {
            return Err(Error::capability_denied_simple("fs:delete not granted"));
        }
        if !self.is_path_allowed(path) {
            return Err(Error::capability_denied_simple(format!(
                "path {:?} not in sandbox",
                path
            )));
        }
        Ok(())
    }
}

enum MemoryNode {
    File(Vec<u8>),
    Directory(HashMap<String, Arc<RwLock<MemoryNode>>>),
}

/// In-memory filesystem for testing
pub struct MemoryFs {
    root: Arc<RwLock<MemoryNode>>,
    sandbox: SandboxConfig,
}

impl MemoryFs {
    /// Create a new in-memory filesystem with sandbox configuration
    pub fn new(sandbox: SandboxConfig) -> Self {
        Self {
            root: Arc::new(RwLock::new(MemoryNode::Directory(HashMap::new()))),
            sandbox,
        }
    }

    fn get_current_time_ns() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    async fn get_node(&self, path: &Path) -> Result<Arc<RwLock<MemoryNode>>> {
        let components: Vec<&str> = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .filter(|s| !s.is_empty() && *s != "/")
            .collect();

        let mut current = self.root.clone();
        for component in components {
            let next = {
                let guard = current.read().await;
                match &*guard {
                    MemoryNode::Directory(children) => {
                        children.get(component).cloned().ok_or_else(|| {
                            Error::io(std::io::Error::new(
                                std::io::ErrorKind::NotFound,
                                "file not found",
                            ))
                        })?
                    }
                    MemoryNode::File(_) => {
                        return Err(Error::io(std::io::Error::new(
                            std::io::ErrorKind::NotADirectory,
                            "not a directory",
                        )));
                    }
                }
            };
            current = next;
        }
        Ok(current)
    }

    async fn get_parent_and_name(&self, path: &Path) -> Result<(Arc<RwLock<MemoryNode>>, String)> {
        let parent = path.parent().ok_or_else(|| {
            Error::io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no parent directory",
            ))
        })?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid filename",
                ))
            })?
            .to_string();

        let parent_node = if parent.as_os_str().is_empty() || parent == Path::new("/") {
            self.root.clone()
        } else {
            self.get_node(parent).await?
        };

        Ok((parent_node, name))
    }
}

#[async_trait]
impl VirtualFs for MemoryFs {
    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        self.sandbox.check_read(path)?;
        let node = self.get_node(path).await?;
        let guard = node.read().await;
        match &*guard {
            MemoryNode::File(data) => Ok(data.clone()),
            MemoryNode::Directory(_) => Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::IsADirectory,
                "is a directory",
            ))),
        }
    }

    async fn write(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.sandbox.check_write(path)?;

        if let Some(parent) = path.parent()
            && parent != Path::new("")
            && parent != Path::new("/")
            && self.get_node(parent).await.is_err()
        {
            return Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "parent directory does not exist",
            )));
        }

        let (parent_node, name) = self.get_parent_and_name(path).await?;
        let mut parent_guard = parent_node.write().await;

        match &mut *parent_guard {
            MemoryNode::Directory(children) => {
                children.insert(name, Arc::new(RwLock::new(MemoryNode::File(data.to_vec()))));
                Ok(())
            }
            MemoryNode::File(_) => Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "parent is not a directory",
            ))),
        }
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>> {
        self.sandbox.check_read(path)?;
        let node = self.get_node(path).await?;
        let guard = node.read().await;

        match &*guard {
            MemoryNode::Directory(children) => {
                let entries: Vec<DirEntry> = children
                    .keys()
                    .map(|name| DirEntry {
                        name: name.clone(),
                        file_type: FileType::Unknown,
                    })
                    .collect();
                Ok(entries)
            }
            MemoryNode::File(_) => Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "not a directory",
            ))),
        }
    }

    async fn create_dir(&self, path: &Path) -> Result<()> {
        self.sandbox.check_write(path)?;
        let (parent_node, name) = self.get_parent_and_name(path).await?;
        let mut parent_guard = parent_node.write().await;

        match &mut *parent_guard {
            MemoryNode::Directory(children) => {
                children.insert(
                    name,
                    Arc::new(RwLock::new(MemoryNode::Directory(HashMap::new()))),
                );
                Ok(())
            }
            MemoryNode::File(_) => Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "parent is not a directory",
            ))),
        }
    }

    async fn remove_dir(&self, path: &Path) -> Result<()> {
        self.sandbox.check_delete(path)?;
        let (parent_node, name) = self.get_parent_and_name(path).await?;

        {
            let parent_guard = parent_node.read().await;
            match &*parent_guard {
                MemoryNode::Directory(children) => {
                    let child = children.get(&name).ok_or_else(|| {
                        Error::io(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "directory not found",
                        ))
                    })?;
                    let child_guard = child.read().await;
                    match &*child_guard {
                        MemoryNode::Directory(dir_children) => {
                            if !dir_children.is_empty() {
                                return Err(Error::io(std::io::Error::new(
                                    std::io::ErrorKind::DirectoryNotEmpty,
                                    "directory not empty",
                                )));
                            }
                        }
                        MemoryNode::File(_) => {
                            return Err(Error::io(std::io::Error::new(
                                std::io::ErrorKind::NotADirectory,
                                "not a directory",
                            )));
                        }
                    }
                }
                MemoryNode::File(_) => {
                    return Err(Error::io(std::io::Error::new(
                        std::io::ErrorKind::NotADirectory,
                        "parent is not a directory",
                    )));
                }
            }
        }

        let mut parent_guard = parent_node.write().await;
        match &mut *parent_guard {
            MemoryNode::Directory(children) => {
                children.remove(&name);
            }
            MemoryNode::File(_) => {}
        }
        Ok(())
    }

    async fn remove_file(&self, path: &Path) -> Result<()> {
        self.sandbox.check_delete(path)?;
        let (parent_node, name) = self.get_parent_and_name(path).await?;

        {
            let parent_guard = parent_node.read().await;
            match &*parent_guard {
                MemoryNode::Directory(children) => {
                    let child = children.get(&name).ok_or_else(|| {
                        Error::io(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "file not found",
                        ))
                    })?;
                    let child_guard = child.read().await;
                    match &*child_guard {
                        MemoryNode::File(_) => {}
                        MemoryNode::Directory(_) => {
                            return Err(Error::io(std::io::Error::new(
                                std::io::ErrorKind::IsADirectory,
                                "is a directory",
                            )));
                        }
                    }
                }
                MemoryNode::File(_) => {
                    return Err(Error::io(std::io::Error::new(
                        std::io::ErrorKind::NotADirectory,
                        "parent is not a directory",
                    )));
                }
            }
        }

        let mut parent_guard = parent_node.write().await;
        match &mut *parent_guard {
            MemoryNode::Directory(children) => {
                children.remove(&name);
            }
            MemoryNode::File(_) => {}
        }
        Ok(())
    }

    async fn metadata(&self, path: &Path) -> Result<FileMetadata> {
        self.sandbox.check_read(path)?;
        let node = self.get_node(path).await?;
        let guard = node.read().await;

        let (file_type, size) = match &*guard {
            MemoryNode::File(data) => (FileType::RegularFile, data.len() as u64),
            MemoryNode::Directory(_) => (FileType::Directory, 0),
        };

        let time_ns = Self::get_current_time_ns();
        Ok(FileMetadata {
            file_type,
            size,
            modified_ns: time_ns,
            accessed_ns: time_ns,
            created_ns: time_ns,
        })
    }

    async fn exists(&self, path: &Path) -> bool {
        self.get_node(path).await.is_ok()
    }

    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        self.sandbox.check_read(from)?;
        self.sandbox.check_write(to)?;

        let node = self.get_node(from).await?;
        let (from_parent, from_name) = self.get_parent_and_name(from).await?;
        let (to_parent, to_name) = self.get_parent_and_name(to).await?;

        {
            let mut from_parent_guard = from_parent.write().await;
            match &mut *from_parent_guard {
                MemoryNode::Directory(children) => {
                    children.remove(&from_name);
                }
                MemoryNode::File(_) => {}
            }
        }

        let mut to_parent_guard = to_parent.write().await;
        match &mut *to_parent_guard {
            MemoryNode::Directory(children) => {
                children.insert(to_name, node);
            }
            MemoryNode::File(_) => {
                return Err(Error::io(std::io::Error::new(
                    std::io::ErrorKind::NotADirectory,
                    "target parent is not a directory",
                )));
            }
        }

        Ok(())
    }
}

/// Host filesystem wrapper with sandboxing
pub struct HostFs {
    sandbox: SandboxConfig,
}

impl HostFs {
    /// Create a new host filesystem wrapper with sandbox configuration
    pub fn new(sandbox: SandboxConfig) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl VirtualFs for HostFs {
    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        self.sandbox.check_read(path)?;
        tokio::fs::read(path).await.map_err(Error::io)
    }

    async fn write(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.sandbox.check_write(path)?;
        tokio::fs::write(path, data).await.map_err(Error::io)
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>> {
        self.sandbox.check_read(path)?;
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(path).await.map_err(Error::io)?;

        while let Some(entry) = dir.next_entry().await.map_err(Error::io)? {
            let file_type = entry.file_type().await.map_err(Error::io)?;
            let ft = if file_type.is_dir() {
                FileType::Directory
            } else if file_type.is_file() {
                FileType::RegularFile
            } else if file_type.is_symlink() {
                FileType::Symlink
            } else {
                FileType::Unknown
            };

            entries.push(DirEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                file_type: ft,
            });
        }

        Ok(entries)
    }

    async fn create_dir(&self, path: &Path) -> Result<()> {
        self.sandbox.check_write(path)?;
        tokio::fs::create_dir(path).await.map_err(Error::io)
    }

    async fn remove_dir(&self, path: &Path) -> Result<()> {
        self.sandbox.check_delete(path)?;
        tokio::fs::remove_dir(path).await.map_err(Error::io)
    }

    async fn remove_file(&self, path: &Path) -> Result<()> {
        self.sandbox.check_delete(path)?;
        tokio::fs::remove_file(path).await.map_err(Error::io)
    }

    async fn metadata(&self, path: &Path) -> Result<FileMetadata> {
        self.sandbox.check_read(path)?;
        let meta = tokio::fs::metadata(path).await.map_err(Error::io)?;

        let file_type = if meta.is_dir() {
            FileType::Directory
        } else if meta.is_file() {
            FileType::RegularFile
        } else if meta.is_symlink() {
            FileType::Symlink
        } else {
            FileType::Unknown
        };

        let modified_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let accessed_ns = meta
            .accessed()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let created_ns = meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Ok(FileMetadata {
            file_type,
            size: meta.len(),
            modified_ns,
            accessed_ns,
            created_ns,
        })
    }

    async fn exists(&self, path: &Path) -> bool {
        tokio::fs::try_exists(path).await.unwrap_or(false)
    }

    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        self.sandbox.check_read(from)?;
        self.sandbox.check_write(to)?;
        tokio::fs::rename(from, to).await.map_err(Error::io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_caps() -> FsCapabilities {
        FsCapabilities {
            can_read: true,
            can_write: true,
            can_delete: true,
        }
    }

    #[tokio::test]
    async fn test_memory_fs_basic() {
        let sandbox = SandboxConfig::new(vec![PathBuf::from("/")], test_caps());
        let fs = MemoryFs::new(sandbox);

        fs.write(Path::new("/test.txt"), b"hello").await.unwrap();
        let data = fs.read(Path::new("/test.txt")).await.unwrap();
        assert_eq!(data, b"hello");
    }

    #[tokio::test]
    async fn test_memory_fs_directory() {
        let sandbox = SandboxConfig::new(vec![PathBuf::from("/")], test_caps());
        let fs = MemoryFs::new(sandbox);

        fs.create_dir(Path::new("/mydir")).await.unwrap();
        let meta = fs.metadata(Path::new("/mydir")).await.unwrap();
        assert_eq!(meta.file_type, FileType::Directory);
    }

    #[tokio::test]
    async fn test_sandbox_path_restriction() {
        let sandbox = SandboxConfig::new(
            vec![PathBuf::from("/allowed")],
            FsCapabilities {
                can_read: true,
                can_write: true,
                can_delete: true,
            },
        );

        assert!(sandbox.is_path_allowed(Path::new("/allowed/file.txt")));
        assert!(!sandbox.is_path_allowed(Path::new("/forbidden/file.txt")));
    }

    #[tokio::test]
    async fn test_capability_denied() {
        let sandbox = SandboxConfig::new(
            vec![PathBuf::from("/")],
            FsCapabilities {
                can_read: false,
                can_write: true,
                can_delete: true,
            },
        );
        let fs = MemoryFs::new(sandbox);

        let result = fs.read(Path::new("/test.txt")).await;
        assert!(matches!(result, Err(Error::Capability { .. })));
    }
}
