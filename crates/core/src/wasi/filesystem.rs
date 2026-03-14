//! WASI Preview 2 Filesystem API
//!
//! Implements filesystem operations for WASM actors with capability-based
//! access control and path sandboxing.
//!
//! # Overview
//!
//! This module provides filesystem operations that can be safely exposed to
//! WASM actors:
//!
//! - **[`FileSystem`]**: Main filesystem interface with sandboxed access
//! - **[`FileSystemConfig`]**: Configuration for filesystem sandboxing
//! - **[`FileSystemBuilder`]**: Builder for constructing filesystems
//!
//! # Security Model
//!
//! All filesystem access is controlled by:
//!
//! 1. **Capability Set**: Actor must have `FS_READ`, `FS_WRITE`, `FS_DELETE`
//! 2. **Sandbox Config**: Path-based access control
//! 3. **Preopen Directories**: Explicitly granted directory access
//!
//! # Example
//!
//! ```ignore
//! use aether_core::wasi::{FileSystem, FileSystemBuilder, MemoryFs};
//! use aether_core::capability::CapabilitySet;
//! use std::sync::Arc;
//! use std::path::PathBuf;
//!
//! // Create sandboxed filesystem
//! let fs = FileSystemBuilder::new()
//!     .with_vfs(Arc::new(MemoryFs::new(sandbox)))
//!     .with_allowed_path(PathBuf::from("/data"))
//!     .with_capabilities(CapabilitySet::FS_READ | CapabilitySet::FS_WRITE)
//!     .with_preopen("data", PathBuf::from("/data"))
//!     .build()?;
//!
//! // Open a file
//! let fd = fs.path_open(0, Path::new("/data/file.txt"), OpenFlags::ReadWrite).await?;
//!
//! // Write to file
//! fs.fd_write(fd, b"hello world").await?;
//!
//! // Read from file
//! let mut buf = [0u8; 11];
//! fs.fd_seek(fd, 0, 0).await?;  // Seek to start
//! fs.fd_read(fd, &mut buf).await?;
//!
//! // Close file
//! fs.fd_close(fd).await?;
//! ```

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::file_descriptor::{
    FD_FIRST_AVAILABLE, Fd, FdStat, FileDescriptorTable, FileStat, FileType, OpenFlags, Rights,
};
use super::virtual_fs::{DirEntry, FsCapabilities, SandboxConfig, VirtualFs};

/// Configuration for filesystem sandboxing.
///
/// Controls which paths are accessible and what operations are allowed.
#[derive(Debug, Clone)]
pub struct FileSystemConfig {
    /// Sandbox configuration for path-based access control
    pub sandbox: SandboxConfig,

    /// Preopen directories with names for WASI compatibility
    pub preopen_paths: Vec<(String, PathBuf)>,
}

impl FileSystemConfig {
    /// Create a new filesystem configuration.
    ///
    /// # Arguments
    ///
    /// * `allowed_paths` - List of paths the actor can access
    /// * `capabilities` - Capability set for filesystem operations
    /// * `preopen_paths` - Named preopen directories for WASI
    pub fn new(
        allowed_paths: Vec<PathBuf>,
        capabilities: CapabilitySet,
        preopen_paths: Vec<(String, PathBuf)>,
    ) -> Self {
        let fs_caps = FsCapabilities::from_capability_set(capabilities);
        Self {
            sandbox: SandboxConfig::new(allowed_paths, fs_caps),
            preopen_paths,
        }
    }
}

/// Sandboxed filesystem for WASM actors.
///
/// Provides WASI-compatible filesystem operations with capability-based
/// access control and path sandboxing.
pub struct FileSystem {
    /// Virtual filesystem implementation
    vfs: Arc<dyn VirtualFs>,

    /// File descriptor table
    fd_table: Arc<RwLock<FileDescriptorTable>>,

    /// Sandbox configuration
    sandbox: SandboxConfig,
}

impl FileSystem {
    /// Create a new filesystem with the given virtual filesystem and configuration.
    ///
    /// # Arguments
    ///
    /// * `vfs` - Virtual filesystem implementation
    /// * `config` - Filesystem configuration including sandbox and preopens
    pub fn new(vfs: Arc<dyn VirtualFs>, config: FileSystemConfig) -> Self {
        let mut fd_table = FileDescriptorTable::new();

        for (name, path) in &config.preopen_paths {
            fd_table.add_preopen(name, path.clone());
        }

        Self {
            vfs,
            fd_table: Arc::new(RwLock::new(fd_table)),
            sandbox: config.sandbox,
        }
    }

    /// Create a filesystem from a virtual filesystem and sandbox config.
    ///
    /// # Arguments
    ///
    /// * `vfs` - Virtual filesystem implementation
    /// * `sandbox` - Sandbox configuration for access control
    pub fn from_vfs(vfs: Arc<dyn VirtualFs>, sandbox: SandboxConfig) -> Self {
        Self {
            vfs,
            fd_table: Arc::new(RwLock::new(FileDescriptorTable::new())),
            sandbox,
        }
    }

    fn validate_fd_for_rights(&self, fd: Fd, rights: Rights) -> Result<()> {
        if fd < FD_FIRST_AVAILABLE {
            return Ok(());
        }

        let table = self
            .fd_table
            .try_read()
            .map_err(|_| Error::internal("failed to acquire fd table lock"))?;

        let stat = table.fdstat(fd)?;

        if !stat.rights_base.contains(rights) {
            return Err(Error::capability_denied_simple(format!(
                "fd {} does not have required rights",
                fd
            )));
        }

        Ok(())
    }

    /// Open a file at the given path.
    ///
    /// # Arguments
    ///
    /// * `fd` - Directory file descriptor for relative paths (use 0 for absolute)
    /// * `path` - Path to the file
    /// * `open_flags` - How to open the file
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Path is outside sandbox
    /// - Missing required capabilities
    /// - File not found (for read)
    /// - Path is a directory
    pub async fn path_open(&self, _fd: Fd, path: &Path, open_flags: OpenFlags) -> Result<Fd> {
        self.sandbox.check_read(path)?;

        if open_flags == OpenFlags::Write || open_flags == OpenFlags::ReadWrite {
            self.sandbox.check_write(path)?;
        }

        let exists = self.vfs.exists(path).await;

        match open_flags {
            OpenFlags::Write | OpenFlags::ReadWrite if !exists => {
                self.sandbox.check_write(path)?;
            }
            OpenFlags::Read | OpenFlags::Directory if !exists => {
                return Err(Error::io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "file not found",
                )));
            }
            _ => {}
        }

        let meta = if exists {
            Some(self.vfs.metadata(path).await?)
        } else {
            None
        };

        let mut table = self.fd_table.write().await;

        let fd = match meta.as_ref().map(|m| m.file_type) {
            Some(super::virtual_fs::FileType::Directory) | None
                if open_flags == OpenFlags::Directory =>
            {
                table.open_directory(path.to_path_buf())?
            }
            Some(super::virtual_fs::FileType::Directory) => {
                return Err(Error::io(std::io::Error::new(
                    std::io::ErrorKind::IsADirectory,
                    "is a directory",
                )));
            }
            _ => {
                let fd = table.open(path.to_path_buf(), open_flags)?;

                if exists {
                    let data = self.vfs.read(path).await?;
                    let file = table.get_mut(fd)?;
                    file.data = data;
                }

                fd
            }
        };

        Ok(fd)
    }

    /// Read from a file descriptor.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to read from
    /// * `buf` - Buffer to read data into
    ///
    /// # Returns
    ///
    /// Number of bytes read
    pub async fn fd_read(&self, fd: Fd, buf: &mut [u8]) -> Result<usize> {
        self.validate_fd_for_rights(fd, Rights::FD_READ)?;

        let mut table = self.fd_table.write().await;
        table.read(fd, buf)
    }

    /// Write to a file descriptor.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to write to
    /// * `buf` - Data to write
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub async fn fd_write(&self, fd: Fd, buf: &[u8]) -> Result<usize> {
        self.validate_fd_for_rights(fd, Rights::FD_WRITE)?;

        let mut table = self.fd_table.write().await;
        table.write(fd, buf)
    }

    /// Close a file descriptor.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to close
    pub async fn fd_close(&self, fd: Fd) -> Result<()> {
        let mut table = self.fd_table.write().await;
        table.close(fd)
    }

    /// Seek within a file.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor
    /// * `offset` - Offset to seek to
    /// * `whence` - 0 = start, 1 = current, 2 = end
    ///
    /// # Returns
    ///
    /// New position in file
    pub async fn fd_seek(&self, fd: Fd, offset: i64, whence: u8) -> Result<u64> {
        use std::io::SeekFrom;

        let pos = match whence {
            0 => SeekFrom::Start(offset as u64),
            1 => SeekFrom::Current(offset),
            2 => SeekFrom::End(offset),
            _ => {
                return Err(Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid whence value",
                )));
            }
        };

        let mut table = self.fd_table.write().await;
        table.seek(fd, pos)
    }

    /// Get file descriptor status.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor
    ///
    /// # Returns
    ///
    /// File descriptor status including rights
    pub async fn fd_stat(&self, fd: Fd) -> Result<FdStat> {
        let table = self.fd_table.read().await;
        table.fdstat(fd)
    }

    /// Get file status for an open file.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor
    ///
    /// # Returns
    ///
    /// File status including type, size, and timestamps
    pub async fn fd_filestat(&self, fd: Fd) -> Result<FileStat> {
        let table = self.fd_table.read().await;
        let file = table.get(fd)?;

        let meta = self.vfs.metadata(&file.path).await?;

        Ok(FileStat {
            device: 0,
            inode: 0,
            file_type: match meta.file_type {
                super::virtual_fs::FileType::RegularFile => FileType::RegularFile,
                super::virtual_fs::FileType::Directory => FileType::Directory,
                super::virtual_fs::FileType::Symlink => FileType::Symlink,
                super::virtual_fs::FileType::Unknown => FileType::Unknown,
            },
            link_count: 1,
            size: meta.size,
            access_time: meta.accessed_ns,
            modification_time: meta.modified_ns,
            change_time: meta.created_ns,
        })
    }

    /// Create a directory at the given path.
    ///
    /// # Arguments
    ///
    /// * `fd` - Directory file descriptor (unused, for WASI compatibility)
    /// * `path` - Path where directory should be created
    pub async fn path_create_directory(&self, fd: Fd, path: &Path) -> Result<()> {
        let _ = fd;
        self.sandbox.check_write(path)?;
        self.vfs.create_dir(path).await
    }

    /// Remove a directory at the given path.
    ///
    /// # Arguments
    ///
    /// * `fd` - Directory file descriptor (unused, for WASI compatibility)
    /// * `path` - Path to directory to remove
    pub async fn path_remove_directory(&self, fd: Fd, path: &Path) -> Result<()> {
        let _ = fd;
        self.sandbox.check_delete(path)?;
        self.vfs.remove_dir(path).await
    }

    /// Remove (unlink) a file at the given path.
    ///
    /// # Arguments
    ///
    /// * `fd` - Directory file descriptor (unused, for WASI compatibility)
    /// * `path` - Path to file to remove
    pub async fn path_unlink_file(&self, fd: Fd, path: &Path) -> Result<()> {
        let _ = fd;
        self.sandbox.check_delete(path)?;
        self.vfs.remove_file(path).await
    }

    /// Rename a file or directory.
    ///
    /// # Arguments
    ///
    /// * `fd` - Directory file descriptor (unused, for WASI compatibility)
    /// * `old_path` - Current path
    /// * `new_path` - New path
    pub async fn path_rename(&self, fd: Fd, old_path: &Path, new_path: &Path) -> Result<()> {
        let _ = fd;
        self.sandbox.check_read(old_path)?;
        self.sandbox.check_write(new_path)?;
        self.vfs.rename(old_path, new_path).await
    }

    /// Get file status for a path.
    ///
    /// # Arguments
    ///
    /// * `fd` - Directory file descriptor (unused, for WASI compatibility)
    /// * `path` - Path to get status for
    ///
    /// # Returns
    ///
    /// File status including type, size, and timestamps
    pub async fn path_filestat(&self, fd: Fd, path: &Path) -> Result<FileStat> {
        let _ = fd;
        self.sandbox.check_read(path)?;

        let meta = self.vfs.metadata(path).await?;

        Ok(FileStat {
            device: 0,
            inode: 0,
            file_type: match meta.file_type {
                super::virtual_fs::FileType::RegularFile => FileType::RegularFile,
                super::virtual_fs::FileType::Directory => FileType::Directory,
                super::virtual_fs::FileType::Symlink => FileType::Symlink,
                super::virtual_fs::FileType::Unknown => FileType::Unknown,
            },
            link_count: 1,
            size: meta.size,
            access_time: meta.accessed_ns,
            modification_time: meta.modified_ns,
            change_time: meta.created_ns,
        })
    }

    /// Read directory entries.
    ///
    /// # Arguments
    ///
    /// * `fd` - Directory file descriptor
    ///
    /// # Returns
    ///
    /// List of directory entries
    pub async fn fd_readdir(&self, fd: Fd) -> Result<Vec<DirEntry>> {
        let table = self.fd_table.read().await;
        let file = table.get(fd)?;

        if !file.is_directory {
            return Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "not a directory",
            )));
        }

        let path = file.path.clone();
        drop(table);

        self.vfs.read_dir(&path).await
    }

    /// Sync file data to storage.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor to sync
    pub async fn fd_sync(&self, fd: Fd) -> Result<()> {
        let table = self.fd_table.read().await;
        let file = table.get(fd)?;

        if file.is_directory {
            return Ok(());
        }

        let path = file.path.clone();
        let data = file.data.clone();
        drop(table);

        self.vfs.write(&path, &data).await
    }

    /// Allocate space in a file.
    ///
    /// # Arguments
    ///
    /// * `fd` - File descriptor
    /// * `offset` - Offset to start allocation
    /// * `len` - Length to allocate
    pub async fn fd_allocate(&self, fd: Fd, offset: u64, len: u64) -> Result<()> {
        let mut table = self.fd_table.write().await;
        let file = table.get_mut(fd)?;

        if file.is_directory {
            return Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::IsADirectory,
                "is a directory",
            )));
        }

        let new_len = (offset + len) as usize;
        if new_len > file.data.len() {
            file.data.resize(new_len, 0);
        }

        Ok(())
    }

    /// Get the list of preopen directories.
    ///
    /// # Returns
    ///
    /// List of (name, path) tuples for preopen directories
    pub fn get_preopens(&self) -> Vec<(String, PathBuf)> {
        self.fd_table
            .try_read()
            .map(|t| {
                t.get_preopens()
                    .iter()
                    .map(|p| (p.name.clone(), p.path.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Builder for constructing a FileSystem with custom configuration
pub struct FileSystemBuilder {
    vfs: Option<Arc<dyn VirtualFs>>,
    allowed_paths: Vec<PathBuf>,
    capabilities: CapabilitySet,
    preopen_paths: Vec<(String, PathBuf)>,
}

impl FileSystemBuilder {
    /// Create a new FileSystemBuilder
    pub fn new() -> Self {
        Self {
            vfs: None,
            allowed_paths: Vec::new(),
            capabilities: CapabilitySet::empty(),
            preopen_paths: Vec::new(),
        }
    }

    /// Set the virtual filesystem implementation
    pub fn with_vfs(mut self, vfs: Arc<dyn VirtualFs>) -> Self {
        self.vfs = Some(vfs);
        self
    }

    /// Add an allowed path to the sandbox
    pub fn with_allowed_path(mut self, path: PathBuf) -> Self {
        self.allowed_paths.push(path);
        self
    }

    /// Set the capability set for the filesystem
    pub fn with_capabilities(mut self, caps: CapabilitySet) -> Self {
        self.capabilities = caps;
        self
    }

    /// Add a preopen directory
    pub fn with_preopen(mut self, name: &str, path: PathBuf) -> Self {
        self.preopen_paths.push((name.to_string(), path));
        self
    }

    /// Build the FileSystem from the configured options
    pub fn build(self) -> Result<FileSystem> {
        let vfs = self
            .vfs
            .ok_or_else(|| Error::config("virtual filesystem not specified"))?;

        let config =
            FileSystemConfig::new(self.allowed_paths, self.capabilities, self.preopen_paths);

        Ok(FileSystem::new(vfs, config))
    }
}

impl Default for FileSystemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::virtual_fs::MemoryFs;
    use super::*;

    fn create_test_fs() -> FileSystem {
        let caps = CapabilitySet::FS_READ | CapabilitySet::FS_WRITE | CapabilitySet::FS_DELETE;
        let sandbox = SandboxConfig::new(
            vec![PathBuf::from("/")],
            FsCapabilities::from_capability_set(caps),
        );
        let memory_fs = Arc::new(MemoryFs::new(sandbox.clone()));
        FileSystem::from_vfs(memory_fs, sandbox)
    }

    #[tokio::test]
    async fn test_path_open_write_and_read() {
        let fs = create_test_fs();

        let fd = fs
            .path_open(0, Path::new("/test.txt"), OpenFlags::ReadWrite)
            .await
            .unwrap();

        let written = fs.fd_write(fd, b"hello world").await.unwrap();
        assert_eq!(written, 11);

        fs.fd_seek(fd, 0, 0).await.unwrap();

        let mut buf = [0u8; 11];
        let read = fs.fd_read(fd, &mut buf).await.unwrap();
        assert_eq!(read, 11);
        assert_eq!(&buf, b"hello world");

        fs.fd_close(fd).await.unwrap();
    }

    #[tokio::test]
    async fn test_path_create_directory() {
        let fs = create_test_fs();

        fs.path_create_directory(0, Path::new("/mydir"))
            .await
            .unwrap();

        let fd = fs
            .path_open(0, Path::new("/mydir"), OpenFlags::Directory)
            .await
            .unwrap();

        let stat = fs.fd_stat(fd).await.unwrap();
        assert_eq!(stat.file_type, FileType::Directory);
    }

    #[tokio::test]
    async fn test_path_remove_directory() {
        let fs = create_test_fs();

        fs.path_create_directory(0, Path::new("/mydir"))
            .await
            .unwrap();

        fs.path_remove_directory(0, Path::new("/mydir"))
            .await
            .unwrap();

        let result = fs
            .path_open(0, Path::new("/mydir"), OpenFlags::Directory)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_path_unlink_file() {
        let fs = create_test_fs();

        let fd = fs
            .path_open(0, Path::new("/test.txt"), OpenFlags::ReadWrite)
            .await
            .unwrap();
        fs.fd_write(fd, b"test").await.unwrap();
        fs.fd_sync(fd).await.unwrap();
        fs.fd_close(fd).await.unwrap();

        fs.path_unlink_file(0, Path::new("/test.txt"))
            .await
            .unwrap();

        let result = fs
            .path_open(0, Path::new("/test.txt"), OpenFlags::Read)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fd_seek() {
        let fs = create_test_fs();

        let fd = fs
            .path_open(0, Path::new("/test.txt"), OpenFlags::ReadWrite)
            .await
            .unwrap();

        fs.fd_write(fd, b"0123456789").await.unwrap();

        fs.fd_seek(fd, 5, 0).await.unwrap();
        let mut buf = [0u8; 3];
        let read = fs.fd_read(fd, &mut buf).await.unwrap();
        assert_eq!(read, 3);
        assert_eq!(&buf, b"567");

        fs.fd_seek(fd, -3, 2).await.unwrap();
        let read = fs.fd_read(fd, &mut buf).await.unwrap();
        assert_eq!(read, 3);
        assert_eq!(&buf, b"789");

        fs.fd_close(fd).await.unwrap();
    }

    #[tokio::test]
    async fn test_capability_denied() {
        let caps = CapabilitySet::empty();
        let sandbox = SandboxConfig::new(
            vec![PathBuf::from("/")],
            FsCapabilities::from_capability_set(caps),
        );
        let memory_fs = Arc::new(MemoryFs::new(sandbox.clone()));
        let fs = FileSystem::from_vfs(memory_fs, sandbox);

        let result = fs
            .path_open(0, Path::new("/test.txt"), OpenFlags::Read)
            .await;
        assert!(matches!(result, Err(Error::Capability { .. })));
    }

    #[tokio::test]
    async fn test_filesystem_builder() {
        let memory_fs = Arc::new(MemoryFs::new(SandboxConfig::new(
            vec![PathBuf::from("/")],
            FsCapabilities {
                can_read: true,
                can_write: true,
                can_delete: true,
            },
        )));

        let fs = FileSystemBuilder::new()
            .with_vfs(memory_fs)
            .with_allowed_path(PathBuf::from("/data"))
            .with_capabilities(CapabilitySet::FS_READ | CapabilitySet::FS_WRITE)
            .with_preopen("data", PathBuf::from("/data"))
            .build()
            .unwrap();

        let preopens = fs.get_preopens();
        assert_eq!(preopens.len(), 1);
        assert_eq!(preopens[0].0, "data");
    }
}
