//! Read-only file access that sees a packed root's files.
//!
//! A resource root may keep its `data/` and `assets/` files only in its data file
//! ([`crate::prepared_files`]), with no file of that name in the folder: a content
//! root is one, so that building it creates one file where it created a link per
//! file. These functions stand in for `std::fs`'s and `Path`'s read-only ones and
//! answer from the data file for a path it holds, and from the disk for every other
//! path, so a caller need not know which kind of root, or whether a root at all, the
//! path is in. Whatever reads a resource by path goes through here.
use crate::prepared_files::{self, PreparedFiles};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

/// What the data file above `path` says of it: the file's bytes are there, or the
/// path is a folder of files there. `None`: ask the disk.
enum Held {
    File(Arc<PreparedFiles>, String),
    Folder(Arc<PreparedFiles>, String),
}

fn held(path: &Path) -> Option<Held> {
    let (files, rel) = prepared_files::find(path)?;
    if files.has(&rel) {
        Some(Held::File(files, rel))
    } else if files.has_folder(&rel) {
        Some(Held::Folder(files, rel))
    } else {
        None
    }
}

/// `std::fs::read`.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    prepared_files::read(path.as_ref())
}

/// `std::fs::read_to_string`.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    prepared_files::read_to_string(path.as_ref())
}

/// `Path::exists`.
pub fn exists(path: impl AsRef<Path>) -> bool {
    held(path.as_ref()).is_some() || path.as_ref().exists()
}

/// `Path::is_file`.
pub fn is_file(path: impl AsRef<Path>) -> bool {
    match held(path.as_ref()) {
        Some(Held::File(..)) => true,
        Some(Held::Folder(..)) => false,
        None => path.as_ref().is_file(),
    }
}

/// `Path::is_dir`.
pub fn is_dir(path: impl AsRef<Path>) -> bool {
    match held(path.as_ref()) {
        Some(Held::File(..)) => false,
        Some(Held::Folder(..)) => true,
        None => path.as_ref().is_dir(),
    }
}

/// `Path`'s questions under names of their own, so that a caller's `.exists()` reads
/// through here by a change of name. Also on what a listing gives, where the answer
/// is the value's own.
pub trait RPath {
    fn r_exists(&self) -> bool;
    fn r_is_file(&self) -> bool;
    fn r_is_dir(&self) -> bool;
}
impl RPath for Path {
    fn r_exists(&self) -> bool {
        exists(self)
    }
    fn r_is_file(&self) -> bool {
        is_file(self)
    }
    fn r_is_dir(&self) -> bool {
        is_dir(self)
    }
}
impl RPath for PathBuf {
    fn r_exists(&self) -> bool {
        exists(self)
    }
    fn r_is_file(&self) -> bool {
        is_file(self)
    }
    fn r_is_dir(&self) -> bool {
        is_dir(self)
    }
}
macro_rules! own_answers {
    ($($t:ty),*) => {$(
        impl RPath for $t {
            fn r_exists(&self) -> bool {
                true
            }
            fn r_is_file(&self) -> bool {
                self.is_file()
            }
            fn r_is_dir(&self) -> bool {
                self.is_dir()
            }
        }
    )*};
}
own_answers!(fs::Metadata, fs::FileType, Metadata, FileType);

/// `std::fs::FileType`, of a file on the disk or in a data file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileType {
    dir: bool,
    symlink: bool,
}
impl FileType {
    pub fn is_dir(&self) -> bool {
        self.dir
    }
    pub fn is_file(&self) -> bool {
        !self.dir && !self.symlink
    }
    pub fn is_symlink(&self) -> bool {
        self.symlink
    }
}
impl From<fs::FileType> for FileType {
    fn from(t: fs::FileType) -> Self {
        FileType {
            dir: t.is_dir(),
            symlink: t.is_symlink(),
        }
    }
}

/// `std::fs::Metadata`, of a file on the disk or in a data file (whose files are as
/// old as the data file).
#[derive(Clone, Debug)]
pub struct Metadata {
    file_type: FileType,
    len: u64,
    modified: Option<SystemTime>,
}
impl Metadata {
    pub fn file_type(&self) -> FileType {
        self.file_type
    }
    pub fn is_dir(&self) -> bool {
        self.file_type.is_dir()
    }
    pub fn is_file(&self) -> bool {
        self.file_type.is_file()
    }
    pub fn len(&self) -> u64 {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn modified(&self) -> io::Result<SystemTime> {
        self.modified
            .ok_or_else(|| io::ErrorKind::Unsupported.into())
    }
}
impl From<fs::Metadata> for Metadata {
    fn from(m: fs::Metadata) -> Self {
        Metadata {
            file_type: m.file_type().into(),
            len: m.len(),
            modified: m.modified().ok(),
        }
    }
}
fn held_metadata(files: &PreparedFiles, len: Option<u64>) -> Metadata {
    Metadata {
        file_type: FileType {
            dir: len.is_none(),
            symlink: false,
        },
        len: len.unwrap_or(0),
        modified: files.modified(),
    }
}

/// `std::fs::metadata`.
pub fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    match held(path.as_ref()) {
        Some(Held::File(files, rel)) => Ok(held_metadata(&files, files.len_of(&rel))),
        Some(Held::Folder(files, _)) => Ok(held_metadata(&files, None)),
        None => fs::metadata(path).map(Into::into),
    }
}

/// `std::fs::DirEntry`.
#[derive(Debug)]
pub struct DirEntry {
    path: PathBuf,
    /// A data file's entry: what it is, its length and age.
    held: Option<Metadata>,
    /// The disk's entry, which knows what it is without asking again.
    disk: Option<fs::DirEntry>,
}
impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
    pub fn file_name(&self) -> OsString {
        self.path.file_name().unwrap_or_default().to_owned()
    }
    pub fn file_type(&self) -> io::Result<FileType> {
        match (&self.held, &self.disk) {
            (Some(m), _) => Ok(m.file_type()),
            (None, Some(entry)) => entry.file_type().map(Into::into),
            (None, None) => fs::symlink_metadata(&self.path).map(|m| m.file_type().into()),
        }
    }
    pub fn metadata(&self) -> io::Result<Metadata> {
        match (&self.held, &self.disk) {
            (Some(m), _) => Ok(m.clone()),
            (None, Some(entry)) => entry.metadata().map(Into::into),
            (None, None) => fs::metadata(&self.path).map(Into::into),
        }
    }
}

/// `std::fs::ReadDir`, in name order.
#[derive(Debug)]
pub struct ReadDir(std::vec::IntoIter<DirEntry>);
impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Ok)
    }
}

/// `std::fs::read_dir`: a folder's files and folders, the disk's and the data file's
/// together.
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<ReadDir> {
    let path = path.as_ref();
    let mut entries: BTreeMap<OsString, DirEntry> = BTreeMap::new();
    let on_disk = fs::read_dir(path).map(|read| {
        for entry in read.flatten() {
            entries.insert(
                entry.file_name(),
                DirEntry {
                    path: entry.path(),
                    held: None,
                    disk: Some(entry),
                },
            );
        }
    });
    match held(path) {
        Some(Held::Folder(files, rel)) => {
            for (name, len) in files.children(&rel) {
                entries.insert(
                    OsString::from(&name),
                    DirEntry {
                        path: path.join(&name),
                        held: Some(held_metadata(&files, len)),
                        disk: None,
                    },
                );
            }
        }
        Some(Held::File(..)) => return Err(io::ErrorKind::NotADirectory.into()),
        None => on_disk?,
    }
    Ok(ReadDir(
        entries.into_values().collect::<Vec<_>>().into_iter(),
    ))
}

/// `std::fs::File`, opened to read: a file on the disk, or a data file's bytes.
#[derive(Debug)]
pub enum File {
    Disk(fs::File),
    Held(io::Cursor<Vec<u8>>),
}
impl File {
    /// `std::fs::File::open`.
    pub fn open(path: impl AsRef<Path>) -> io::Result<File> {
        match held(path.as_ref()) {
            Some(Held::File(files, rel)) => files
                .read(&rel)
                .unwrap_or_else(|| Err(io::ErrorKind::NotFound.into()))
                .map(|bytes| File::Held(io::Cursor::new(bytes))),
            Some(Held::Folder(..)) => Err(io::ErrorKind::IsADirectory.into()),
            None => fs::File::open(path).map(File::Disk),
        }
    }
    pub fn metadata(&self) -> io::Result<Metadata> {
        match self {
            File::Disk(f) => f.metadata().map(Into::into),
            File::Held(c) => Ok(Metadata {
                file_type: FileType {
                    dir: false,
                    symlink: false,
                },
                len: c.get_ref().len() as u64,
                modified: None,
            }),
        }
    }
}
impl io::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            File::Disk(f) => f.read(buf),
            File::Held(c) => c.read(buf),
        }
    }
}
impl io::Seek for File {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        match self {
            File::Disk(f) => f.seek(pos),
            File::Held(c) => c.seek(pos),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A root whose `assets/` files are only in its data file, beside a sound and a
    /// folder of sounds on the disk.
    fn packed(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("hz-rfs-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("assets/a/sounds")).unwrap();
        fs::write(root.join("horizon-resources.json"), b"{}").unwrap();
        fs::write(root.join("assets/a/sounds/s.ogg"), b"ogg").unwrap();
        let files: Vec<(String, Vec<u8>)> = vec![
            ("assets/a/lang/en_us.json".into(), b"{}".to_vec()),
            ("assets/a/textures/t.png".into(), b"png!".to_vec()),
            ("assets/b/x.json".into(), b"1".to_vec()),
        ];
        prepared_files::write_packed(
            &root,
            files.iter().map(|(n, _)| n.clone()).collect(),
            1,
            &|name| {
                files
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, b)| Ok(b.clone()))
                    .unwrap_or_else(|| Err(io::ErrorKind::NotFound.into()))
            },
        )
        .unwrap();
        root
    }

    #[test]
    fn a_packed_root_reads_as_a_folder() {
        let root = packed("reads");
        assert_eq!(read(root.join("assets/a/textures/t.png")).unwrap(), b"png!");
        assert!(root.join("assets/a/textures/t.png").r_is_file());
        assert!(!root.join("assets/a/textures/t.png").exists());
        assert!(root.join("assets/a/textures").r_is_dir());
        assert!(root.join("assets/b").r_exists());
        assert!(!root.join("assets/c").r_exists());
        assert!(!root.join("assets/a/textures/u.png").r_exists());
        assert!(root.join("assets/a/sounds/s.ogg").r_is_file());
        assert_eq!(metadata(root.join("assets/a/textures/t.png")).unwrap().len(), 4);
        let mut text = String::new();
        io::Read::read_to_string(
            &mut File::open(root.join("assets/b/x.json")).unwrap(),
            &mut text,
        )
        .unwrap();
        assert_eq!(text, "1");
        let names = |dir: &str| -> Vec<(String, bool)> {
            read_dir(root.join(dir))
                .unwrap()
                .flatten()
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        e.file_type().unwrap().is_dir(),
                    )
                })
                .collect()
        };
        assert_eq!(names("assets"), [("a".into(), true), ("b".into(), true)]);
        assert_eq!(
            names("assets/a"),
            [
                ("lang".into(), true),
                ("sounds".into(), true),
                ("textures".into(), true)
            ]
        );
        assert_eq!(names("assets/a/textures"), [("t.png".into(), false)]);
        assert_eq!(names("assets/a/sounds"), [("s.ogg".into(), false)]);
        assert!(read_dir(root.join("assets/c")).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
