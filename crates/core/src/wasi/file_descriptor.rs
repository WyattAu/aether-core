//! File Descriptor Management
//!
//! Manages virtual file descriptors for WASM actors.

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::PathBuf;

/// Virtual file descriptor number.
pub type Fd = u32;

/// Standard input file descriptor.
pub const FD_STDIN: Fd = 0;
/// Standard output file descriptor.
pub const FD_STDOUT: Fd = 1;
/// Standard error file descriptor.
pub const FD_STDERR: Fd = 2;
/// First non-standard file descriptor available for allocation.
pub const FD_FIRST_AVAILABLE: Fd = 3;

/// Flags controlling file descriptor behavior on open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdFlags {
    /// No special flags.
    None,
    /// Open for appending.
    Append,
    /// Truncate the file to zero length on open.
    Truncate,
    /// Create the file if it does not exist.
    Create,
    /// Create the file and truncate it.
    CreateAndTruncate,
    /// Open as a directory.
    Directory,
}

impl FdFlags {
    /// Returns `true` if the file can be read with these flags.
    pub fn can_read(&self) -> bool {
        matches!(self, Self::None | Self::Directory)
    }

    /// Returns `true` if the file can be written with these flags.
    pub fn can_write(&self) -> bool {
        matches!(
            self,
            Self::Append | Self::Truncate | Self::Create | Self::CreateAndTruncate
        )
    }

    /// Returns `true` if the file should be created on open.
    pub fn should_create(&self) -> bool {
        matches!(self, Self::Create | Self::CreateAndTruncate)
    }

    /// Returns `true` if the file should be truncated on open.
    pub fn should_truncate(&self) -> bool {
        matches!(self, Self::Truncate | Self::CreateAndTruncate)
    }

    /// Returns `true` if the descriptor refers to a directory.
    pub fn is_directory(&self) -> bool {
        matches!(self, Self::Directory)
    }
}

/// Flags specifying how a file is opened for reading and/or writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenFlags {
    /// Open for reading only.
    Read,
    /// Open for writing only.
    Write,
    /// Open for both reading and writing.
    ReadWrite,
    /// Open as a directory for enumeration.
    Directory,
}

/// An in-memory virtual file backing a file descriptor.
#[derive(Debug, Clone)]
pub struct VirtualFile {
    /// Virtual filesystem path of the file.
    pub path: PathBuf,
    /// Access mode flags.
    pub flags: OpenFlags,
    /// Current read/write cursor position in bytes.
    pub position: u64,
    /// In-memory file contents.
    pub data: Vec<u8>,
    /// Whether this file represents a directory.
    pub is_directory: bool,
    /// Names of entries within a directory.
    pub directory_entries: Vec<String>,
    /// Current enumeration position within directory entries.
    pub directory_position: usize,
}

impl VirtualFile {
    /// Creates a new empty virtual file with the given path and flags.
    pub fn new(path: PathBuf, flags: OpenFlags) -> Self {
        Self {
            path,
            flags,
            position: 0,
            data: Vec::new(),
            is_directory: false,
            directory_entries: Vec::new(),
            directory_position: 0,
        }
    }

    /// Creates a new virtual directory at the given path.
    pub fn new_directory(path: PathBuf) -> Self {
        Self {
            path,
            flags: OpenFlags::Directory,
            position: 0,
            data: Vec::new(),
            is_directory: true,
            directory_entries: Vec::new(),
            directory_position: 0,
        }
    }

    /// Returns `true` if the file was opened for reading.
    pub fn can_read(&self) -> bool {
        matches!(
            self.flags,
            OpenFlags::Read | OpenFlags::ReadWrite | OpenFlags::Directory
        )
    }

    /// Returns `true` if the file was opened for writing.
    pub fn can_write(&self) -> bool {
        matches!(self.flags, OpenFlags::Write | OpenFlags::ReadWrite)
    }

    /// Seeks to the given position and returns the new offset.
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(offset) => {
                self.position = offset;
            }
            SeekFrom::Current(offset) => {
                if offset >= 0 {
                    self.position = self.position.saturating_add(offset as u64);
                } else {
                    self.position = self.position.saturating_sub((-offset) as u64);
                }
            }
            SeekFrom::End(offset) => {
                let end = self.data.len() as u64;
                if offset >= 0 {
                    self.position = end.saturating_add(offset as u64);
                } else {
                    self.position = end.saturating_sub((-offset) as u64);
                }
            }
        }
        Ok(self.position)
    }

    /// Reads bytes from the current position into the buffer.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.can_read() {
            return Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "file not open for reading",
            )));
        }

        if self.is_directory {
            return Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::IsADirectory,
                "is a directory",
            )));
        }

        let available = self.data.len().saturating_sub(self.position as usize);
        let to_read = buf.len().min(available);

        buf[..to_read]
            .copy_from_slice(&self.data[self.position as usize..self.position as usize + to_read]);
        self.position += to_read as u64;

        Ok(to_read)
    }

    /// Writes bytes from the buffer at the current position.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if !self.can_write() {
            return Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "file not open for writing",
            )));
        }

        if self.is_directory {
            return Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::IsADirectory,
                "is a directory",
            )));
        }

        let end_pos = self.position as usize + buf.len();
        if end_pos > self.data.len() {
            self.data.resize(end_pos, 0);
        }

        self.data[self.position as usize..end_pos].copy_from_slice(buf);
        self.position = end_pos as u64;

        Ok(buf.len())
    }
}

/// A pre-opened directory available to the WASM guest at startup.
#[derive(Debug, Clone)]
pub struct PreopenDirectory {
    /// Host-side path of the preopened directory.
    pub path: PathBuf,
    /// Guest-side name used to reference the directory.
    pub name: String,
}

/// Table mapping file descriptor numbers to virtual files.
#[derive(Debug)]
pub struct FileDescriptorTable {
    descriptors: HashMap<Fd, VirtualFile>,
    next_fd: Fd,
    preopens: Vec<PreopenDirectory>,
}

impl FileDescriptorTable {
    /// Creates a new table pre-populated with stdin, stdout, and stderr.
    pub fn new() -> Self {
        let mut descriptors = HashMap::new();

        descriptors.insert(
            FD_STDIN,
            VirtualFile::new(PathBuf::from("/dev/stdin"), OpenFlags::Read),
        );
        descriptors.insert(
            FD_STDOUT,
            VirtualFile::new(PathBuf::from("/dev/stdout"), OpenFlags::Write),
        );
        descriptors.insert(
            FD_STDERR,
            VirtualFile::new(PathBuf::from("/dev/stderr"), OpenFlags::Write),
        );

        Self {
            descriptors,
            next_fd: FD_FIRST_AVAILABLE,
            preopens: Vec::new(),
        }
    }

    /// Adds a pre-opened directory and returns its file descriptor.
    pub fn add_preopen(&mut self, name: &str, path: PathBuf) -> Fd {
        let fd = self.allocate_fd();
        let mut file = VirtualFile::new_directory(path.clone());
        file.path = path.clone();
        self.descriptors.insert(fd, file);
        self.preopens.push(PreopenDirectory {
            path,
            name: name.to_string(),
        });
        fd
    }

    /// Returns the list of pre-opened directories.
    pub fn get_preopens(&self) -> &[PreopenDirectory] {
        &self.preopens
    }

    fn allocate_fd(&mut self) -> Fd {
        let fd = self.next_fd;
        self.next_fd = self.next_fd.wrapping_add(1);

        while self.descriptors.contains_key(&self.next_fd) {
            self.next_fd = self.next_fd.wrapping_add(1);
            if self.next_fd == fd {
                break;
            }
        }

        fd
    }

    /// Opens a new virtual file and returns its file descriptor.
    pub fn open(&mut self, path: PathBuf, flags: OpenFlags) -> Result<Fd> {
        let fd = self.allocate_fd();
        let file = VirtualFile::new(path, flags);
        self.descriptors.insert(fd, file);
        Ok(fd)
    }

    /// Opens a new virtual directory and returns its file descriptor.
    pub fn open_directory(&mut self, path: PathBuf) -> Result<Fd> {
        let fd = self.allocate_fd();
        let file = VirtualFile::new_directory(path);
        self.descriptors.insert(fd, file);
        Ok(fd)
    }

    /// Returns an immutable reference to the virtual file for a given fd.
    pub fn get(&self, fd: Fd) -> Result<&VirtualFile> {
        self.descriptors.get(&fd).ok_or_else(|| {
            Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid file descriptor",
            ))
        })
    }

    /// Returns a mutable reference to the virtual file for a given fd.
    pub fn get_mut(&mut self, fd: Fd) -> Result<&mut VirtualFile> {
        self.descriptors.get_mut(&fd).ok_or_else(|| {
            Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid file descriptor",
            ))
        })
    }

    /// Closes a file descriptor, preventing further use.
    pub fn close(&mut self, fd: Fd) -> Result<()> {
        if fd < FD_FIRST_AVAILABLE {
            return Err(Error::io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "cannot close standard file descriptor",
            )));
        }

        self.descriptors.remove(&fd).ok_or_else(|| {
            Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid file descriptor",
            ))
        })?;

        Ok(())
    }

    /// Reads from the file referenced by the given fd.
    pub fn read(&mut self, fd: Fd, buf: &mut [u8]) -> Result<usize> {
        self.get_mut(fd)?.read(buf)
    }

    /// Writes to the file referenced by the given fd.
    pub fn write(&mut self, fd: Fd, buf: &[u8]) -> Result<usize> {
        self.get_mut(fd)?.write(buf)
    }

    /// Seeks within the file referenced by the given fd.
    pub fn seek(&mut self, fd: Fd, pos: SeekFrom) -> Result<u64> {
        self.get_mut(fd)?.seek(pos)
    }

    /// Returns file descriptor metadata for the given fd.
    pub fn fdstat(&self, fd: Fd) -> Result<FdStat> {
        let file = self.get(fd)?;
        Ok(FdStat {
            file_type: if file.is_directory {
                FileType::Directory
            } else {
                FileType::RegularFile
            },
            flags: file.flags,
            rights_base: Rights::all(),
            rights_inheriting: Rights::all(),
        })
    }

    /// Returns `true` if the file descriptor is currently open.
    pub fn is_valid(&self, fd: Fd) -> bool {
        self.descriptors.contains_key(&fd)
    }
}

impl Default for FileDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}

/// File type classification for WASI file metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Unknown or unspecified file type.
    Unknown,
    /// Block device.
    BlockDevice,
    /// Character device.
    CharacterDevice,
    /// Directory.
    Directory,
    /// Regular file.
    RegularFile,
    /// Datagram socket.
    SocketDgram,
    /// Stream socket.
    SocketStream,
    /// Symbolic link.
    Symlink,
}

bitflags::bitflags! {
    /// WASI file rights bitflags controlling permitted operations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rights: u64 {
        /// Synchronize data to disk.
        const FD_DATASYNC = 1 << 0;
        /// Read from the file descriptor.
        const FD_READ = 1 << 1;
        /// Seek within the file descriptor.
        const FD_SEEK = 1 << 2;
        /// Set file descriptor flags.
        const FD_FDSTAT_SET_FLAGS = 1 << 3;
        /// Synchronize file to disk.
        const FD_SYNC = 1 << 4;
        /// Get the current offset within the file descriptor.
        const FD_TELL = 1 << 5;
        /// Write to the file descriptor.
        const FD_WRITE = 1 << 6;
        /// Advise on file access patterns.
        const FD_ADVISE = 1 << 7;
        /// Allocate space in the file.
        const FD_ALLOCATE = 1 << 8;
        /// Create a directory.
        const PATH_CREATE_DIRECTORY = 1 << 9;
        /// Create a file.
        const PATH_CREATE_FILE = 1 << 10;
        /// Create a hard link (source).
        const PATH_LINK_SOURCE = 1 << 11;
        /// Create a hard link (target).
        const PATH_LINK_TARGET = 1 << 12;
        /// Open a path.
        const PATH_OPEN = 1 << 13;
        /// Read directory entries.
        const FD_READDIR = 1 << 14;
        /// Read the target of a symbolic link.
        const PATH_READLINK = 1 << 15;
        /// Rename a path (source).
        const PATH_RENAME_SOURCE = 1 << 16;
        /// Rename a path (target).
        const PATH_RENAME_TARGET = 1 << 17;
        /// Get file attributes by path.
        const PATH_FILESTAT_GET = 1 << 18;
        /// Set file size by path.
        const PATH_FILESTAT_SET_SIZE = 1 << 19;
        /// Set file times by path.
        const PATH_FILESTAT_SET_TIMES = 1 << 20;
        /// Get file attributes by file descriptor.
        const FD_FILESTAT_GET = 1 << 21;
        /// Set file size by file descriptor.
        const FD_FILESTAT_SET_SIZE = 1 << 22;
        /// Set file times by file descriptor.
        const FD_FILESTAT_SET_TIMES = 1 << 23;
        /// Create a symbolic link.
        const PATH_SYMLINK = 1 << 24;
        /// Remove a directory.
        const PATH_REMOVE_DIRECTORY = 1 << 25;
        /// Unlink (delete) a file.
        const PATH_UNLINK_FILE = 1 << 26;
        /// Poll a file descriptor for read/write readiness.
        const POLL_FD_READWRITE = 1 << 27;
        /// Shut down a socket.
        const SOCK_SHUTDOWN = 1 << 28;
    }
}

/// WASI file descriptor metadata.
#[derive(Debug, Clone)]
pub struct FdStat {
    /// Type of the file referenced by this descriptor.
    pub file_type: FileType,
    /// Open flags for this descriptor.
    pub flags: OpenFlags,
    /// Base rights associated with this descriptor.
    pub rights_base: Rights,
    /// Rights inherited by new descriptors created from this one.
    pub rights_inheriting: Rights,
}

/// WASI file attributes stat structure.
#[derive(Debug, Clone)]
pub struct FileStat {
    /// Device ID.
    pub device: u64,
    /// Inode number.
    pub inode: u64,
    /// File type.
    pub file_type: FileType,
    /// Number of hard links.
    pub link_count: u64,
    /// File size in bytes.
    pub size: u64,
    /// Last access time (timestamp).
    pub access_time: u64,
    /// Last modification time (timestamp).
    pub modification_time: u64,
    /// Last status change time (timestamp).
    pub change_time: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fd_allocation() {
        let mut table = FileDescriptorTable::new();

        let fd1 = table
            .open(PathBuf::from("/test1"), OpenFlags::ReadWrite)
            .unwrap();
        let fd2 = table
            .open(PathBuf::from("/test2"), OpenFlags::ReadWrite)
            .unwrap();

        assert_eq!(fd1, FD_FIRST_AVAILABLE);
        assert_eq!(fd2, FD_FIRST_AVAILABLE + 1);
    }

    #[test]
    fn test_fd_close() {
        let mut table = FileDescriptorTable::new();

        let fd = table
            .open(PathBuf::from("/test"), OpenFlags::ReadWrite)
            .unwrap();
        assert!(table.is_valid(fd));

        table.close(fd).unwrap();
        assert!(!table.is_valid(fd));
    }

    #[test]
    fn test_read_write() {
        let mut table = FileDescriptorTable::new();

        let fd = table
            .open(PathBuf::from("/test"), OpenFlags::ReadWrite)
            .unwrap();

        let written = table.write(fd, b"hello").unwrap();
        assert_eq!(written, 5);

        table.seek(fd, SeekFrom::Start(0)).unwrap();

        let mut buf = [0u8; 10];
        let read = table.read(fd, &mut buf).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&buf[..5], b"hello");
    }

    #[test]
    fn test_seek() {
        let mut table = FileDescriptorTable::new();

        let fd = table
            .open(PathBuf::from("/test"), OpenFlags::ReadWrite)
            .unwrap();
        table.write(fd, b"hello world").unwrap();

        table.seek(fd, SeekFrom::Start(6)).unwrap();
        let mut buf = [0u8; 5];
        let read = table.read(fd, &mut buf).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&buf, b"world");
    }

    #[test]
    fn test_preopen_directories() {
        let mut table = FileDescriptorTable::new();

        let fd = table.add_preopen("data", PathBuf::from("/data"));
        assert!(fd >= FD_FIRST_AVAILABLE);

        let preopens = table.get_preopens();
        assert_eq!(preopens.len(), 1);
        assert_eq!(preopens[0].name, "data");
    }

    #[test]
    fn test_cannot_close_std_fds() {
        let mut table = FileDescriptorTable::new();

        let result = table.close(FD_STDIN);
        assert!(result.is_err());

        let result = table.close(FD_STDOUT);
        assert!(result.is_err());

        let result = table.close(FD_STDERR);
        assert!(result.is_err());
    }
}
