use eyre::Context;
use eyre::Result;

use std::{
    fmt::Debug,
    fs::{self, DirEntry},
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub enum SimpleFileKind {
    File,
    Directory,
}
#[derive(Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub name: String,
    pub kind: SimpleFileKind,
}

impl FileInfo {
    fn new(path: PathBuf, name: String, kind: SimpleFileKind) -> Self {
        Self { path, name, kind }
    }
}

pub trait FileSystem {
    fn current_directory(&self) -> Result<FileInfo>;

    fn list_files(&self, file: &FileInfo) -> Result<Vec<FileInfo>>;

    fn file_size(&self, file: &FileInfo) -> Result<u64>;
}

pub trait FileSystemClean {
    fn remove_file(&self, file: &FileInfo) -> Result<()>;
}
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn current_directory(&self) -> Result<FileInfo> {
        let path_buf = std::env::current_dir()?;
        Ok(FileInfo::new(
            path_buf,
            "".into(),
            SimpleFileKind::Directory,
        ))
    }

    fn list_files(&self, file: &FileInfo) -> Result<Vec<FileInfo>> {
        fs::read_dir(&file.path)?
            .map(|e| {
                e.context("failed to read dir entry")
                    .and_then(|e| map_entry_to_simple_file(e))
            })
            .collect()
    }

    fn file_size(&self, file: &FileInfo) -> Result<u64> {
        RealFileSystem::get_size(&file.path)
    }
}

impl RealFileSystem {
    pub fn get_size<P>(path: P) -> Result<u64>
    where
        P: AsRef<Path>,
    {
        let mut result = 0;

        if path.as_ref().is_dir() {
            for entry in fs::read_dir(&path)? {
                let _path = entry?.path();
                if _path.is_file() {
                    result += _path.metadata()?.len();
                } else {
                    result += RealFileSystem::get_size(_path)?;
                }
            }
        } else {
            result = path.as_ref().metadata()?.len();
        }
        Ok(result)
    }
}

fn map_entry_to_simple_file(entry: DirEntry) -> Result<FileInfo> {
    let path = entry.path();

    let name = entry
        .file_name()
        .into_string()
        .map_err(|_| eyre::eyre!("Cannot convert os string"))?;

    let file_type = entry.file_type()?;

    let kind = if file_type.is_dir() {
        SimpleFileKind::Directory
    } else {
        SimpleFileKind::File
    };

    Ok(FileInfo::new(path, name, kind))
}

impl FileSystemClean for RealFileSystem {
    fn remove_file(&self, file: &FileInfo) -> Result<()> {
        if file.kind == SimpleFileKind::Directory {
            std::fs::remove_dir_all(&file.path)?;
        } else {
            std::fs::remove_file(&file.path)?;
        }
        Ok(())
    }
}

pub struct MockFileSystemClean;

impl FileSystemClean for MockFileSystemClean {
    fn remove_file(&self, _file: &FileInfo) -> Result<()> {
        Ok(())
        //        Err(eyre::eyre!("Failed"))
    }
}
