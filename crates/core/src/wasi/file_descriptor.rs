//! File Descriptor Management
//!
//! Manages virtual file descriptors for WASM actors.

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::PathBuf;

pub type Fd = u32;

pub const FD_STDIN: Fd = 0;
pub const FD_STDOUT: Fd = 1;
pub const FD_STDERR: Fd = 2;
pub const FD_FIRST_AVAILABLE: Fd = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdFlags {
    None,
    Append,
    Truncate,
    Create,
    CreateAndTruncate,
    Directory,
}

impl FdFlags {
    pub fn can_read(&self) -> bool {
        matches!(self, Self::None | Self::Directory)
    }

    pub fn can_write(&self) -> bool {
        matches!(
            self,
            Self::Append | Self::Truncate | Self::Create | Self::CreateAndTruncate
        )
    }

    pub fn should_create(&self) -> bool {
        matches!(self, Self::Create | Self::CreateAndTruncate)
    }

    pub fn should_truncate(&self) -> bool {
        matches!(self, Self::Truncate | Self::CreateAndTruncate)
    }

    pub fn is_directory(&self) -> bool {
        matches!(self, Self::Directory)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenFlags {
    Read,
    Write,
    ReadWrite,
    Directory,
}

#[derive(Debug, Clone)]
pub struct VirtualFile {
    pub path: PathBuf,
    pub flags: OpenFlags,
    pub position: u64,
    pub data: Vec<u8>,
    pub is_directory: bool,
    pub directory_entries: Vec<String>,
    pub directory_position: usize,
}

impl VirtualFile {
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

    pub fn can_read(&self) -> bool {
        matches!(
            self.flags,
            OpenFlags::Read | OpenFlags::ReadWrite | OpenFlags::Directory
        )
    }

    pub fn can_write(&self) -> bool {
        matches!(self.flags, OpenFlags::Write | OpenFlags::ReadWrite)
    }

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

#[derive(Debug, Clone)]
pub struct PreopenDirectory {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug)]
pub struct FileDescriptorTable {
    descriptors: HashMap<Fd, VirtualFile>,
    next_fd: Fd,
    preopens: Vec<PreopenDirectory>,
}

impl FileDescriptorTable {
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

    pub fn open(&mut self, path: PathBuf, flags: OpenFlags) -> Result<Fd> {
        let fd = self.allocate_fd();
        let file = VirtualFile::new(path, flags);
        self.descriptors.insert(fd, file);
        Ok(fd)
    }

    pub fn open_directory(&mut self, path: PathBuf) -> Result<Fd> {
        let fd = self.allocate_fd();
        let file = VirtualFile::new_directory(path);
        self.descriptors.insert(fd, file);
        Ok(fd)
    }

    pub fn get(&self, fd: Fd) -> Result<&VirtualFile> {
        self.descriptors.get(&fd).ok_or_else(|| {
            Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid file descriptor",
            ))
        })
    }

    pub fn get_mut(&mut self, fd: Fd) -> Result<&mut VirtualFile> {
        self.descriptors.get_mut(&fd).ok_or_else(|| {
            Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid file descriptor",
            ))
        })
    }

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

    pub fn read(&mut self, fd: Fd, buf: &mut [u8]) -> Result<usize> {
        self.get_mut(fd)?.read(buf)
    }

    pub fn write(&mut self, fd: Fd, buf: &[u8]) -> Result<usize> {
        self.get_mut(fd)?.write(buf)
    }

    pub fn seek(&mut self, fd: Fd, pos: SeekFrom) -> Result<u64> {
        self.get_mut(fd)?.seek(pos)
    }

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

    pub fn is_valid(&self, fd: Fd) -> bool {
        self.descriptors.contains_key(&fd)
    }
}

impl Default for FileDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Unknown,
    BlockDevice,
    CharacterDevice,
    Directory,
    RegularFile,
    SocketDgram,
    SocketStream,
    Symlink,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rights: u64 {
        const FD_DATASYNC = 1 << 0;
        const FD_READ = 1 << 1;
        const FD_SEEK = 1 << 2;
        const FD_FDSTAT_SET_FLAGS = 1 << 3;
        const FD_SYNC = 1 << 4;
        const FD_TELL = 1 << 5;
        const FD_WRITE = 1 << 6;
        const FD_ADVISE = 1 << 7;
        const FD_ALLOCATE = 1 << 8;
        const PATH_CREATE_DIRECTORY = 1 << 9;
        const PATH_CREATE_FILE = 1 << 10;
        const PATH_LINK_SOURCE = 1 << 11;
        const PATH_LINK_TARGET = 1 << 12;
        const PATH_OPEN = 1 << 13;
        const FD_READDIR = 1 << 14;
        const PATH_READLINK = 1 << 15;
        const PATH_RENAME_SOURCE = 1 << 16;
        const PATH_RENAME_TARGET = 1 << 17;
        const PATH_FILESTAT_GET = 1 << 18;
        const PATH_FILESTAT_SET_SIZE = 1 << 19;
        const PATH_FILESTAT_SET_TIMES = 1 << 20;
        const FD_FILESTAT_GET = 1 << 21;
        const FD_FILESTAT_SET_SIZE = 1 << 22;
        const FD_FILESTAT_SET_TIMES = 1 << 23;
        const PATH_SYMLINK = 1 << 24;
        const PATH_REMOVE_DIRECTORY = 1 << 25;
        const PATH_UNLINK_FILE = 1 << 26;
        const POLL_FD_READWRITE = 1 << 27;
        const SOCK_SHUTDOWN = 1 << 28;
    }
}

#[derive(Debug, Clone)]
pub struct FdStat {
    pub file_type: FileType,
    pub flags: OpenFlags,
    pub rights_base: Rights,
    pub rights_inheriting: Rights,
}

#[derive(Debug, Clone)]
pub struct FileStat {
    pub device: u64,
    pub inode: u64,
    pub file_type: FileType,
    pub link_count: u64,
    pub size: u64,
    pub access_time: u64,
    pub modification_time: u64,
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
