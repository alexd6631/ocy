use crate::models::FileInfo;
use crate::models::SimpleFileKind;
use eyre::Context;
use eyre::Report;
use eyre::Result;
use std::collections::HashSet;
use std::fs::{self, DirEntry, Metadata};
use std::path::Path;

/// The contents of a directory, together with any per-entry failures.
///
/// Reading a directory is not all-or-nothing: a single entry that cannot be stat'ed must
/// not hide its siblings, because a hidden sibling is silently unreclaimed disk space.
#[derive(Debug, Default)]
pub struct DirListing {
    pub entries: Vec<FileInfo>,
    pub errors: Vec<Report>,
}

pub trait FileSystem {
    fn current_directory(&self) -> Result<FileInfo>;

    fn list_files(&self, file: &FileInfo) -> Result<DirListing>;

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

    fn list_files(&self, file: &FileInfo) -> Result<DirListing> {
        let read_dir = fs::read_dir(&file.path)
            .with_context(|| format!("cannot read directory {}", file.path.display()))?;

        Ok(read_dir.fold(DirListing::default(), |mut listing, entry| {
            match entry
                .context("failed to read dir entry")
                .and_then(|entry| describe_entry(&entry))
            {
                Ok(info) => listing.entries.push(info),
                Err(report) => listing.errors.push(report),
            }
            listing
        }))
    }

    fn file_size(&self, file: &FileInfo) -> Result<u64> {
        RealFileSystem::reclaimable_size(&file.path)
    }
}

impl RealFileSystem {
    /// The number of bytes actually freed by deleting `path`.
    ///
    /// Symbolic links are never followed. Deleting a link removes only the link, so
    /// descending into its target would attribute another tree's bytes to this candidate
    /// and promise space that the clean cannot deliver. Not following them also makes
    /// link cycles harmless.
    ///
    /// On Unix the figure is allocated blocks rather than apparent length, so sparse files
    /// are not overstated, and each inode is counted once so hard links are not
    /// double-counted.
    pub fn reclaimable_size<P>(path: P) -> Result<u64>
    where
        P: AsRef<Path>,
    {
        let mut seen_inodes = HashSet::new();
        let mut pending = vec![path.as_ref().to_path_buf()];
        let mut total = 0;

        while let Some(current) = pending.pop() {
            let metadata = fs::symlink_metadata(&current)
                .with_context(|| format!("cannot stat {}", current.display()))?;

            if metadata.is_dir() {
                let read_dir = fs::read_dir(&current)
                    .with_context(|| format!("cannot read directory {}", current.display()))?;
                for entry in read_dir {
                    pending.push(entry?.path());
                }
            }

            if counts_towards_total(&metadata, &mut seen_inodes) {
                total += allocated_bytes(&metadata);
            }
        }

        Ok(total)
    }
}

fn describe_entry(entry: &DirEntry) -> Result<FileInfo> {
    let file_type = entry
        .file_type()
        .with_context(|| format!("cannot determine type of {}", entry.path().display()))?;

    let kind = if file_type.is_symlink() {
        SimpleFileKind::Symlink
    } else if file_type.is_dir() {
        SimpleFileKind::Directory
    } else {
        SimpleFileKind::File
    };

    Ok(FileInfo::new(
        entry.path(),
        entry.file_name().to_string_lossy().into_owned(),
        kind,
    ))
}

#[cfg(unix)]
fn allocated_bytes(metadata: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.blocks() * 512
}

#[cfg(not(unix))]
fn allocated_bytes(metadata: &Metadata) -> u64 {
    metadata.len()
}

/// Whether this entry's bytes have not already been counted through another hard link.
#[cfg(unix)]
fn counts_towards_total(metadata: &Metadata, seen_inodes: &mut HashSet<(u64, u64)>) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() <= 1 || seen_inodes.insert((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn counts_towards_total(_metadata: &Metadata, _seen_inodes: &mut HashSet<(u64, u64)>) -> bool {
    true
}

impl FileSystemClean for RealFileSystem {
    fn remove_file(&self, file: &FileInfo) -> Result<()> {
        match file.kind {
            SimpleFileKind::Directory => fs::remove_dir_all(&file.path)
                .with_context(|| format!("cannot remove directory {}", file.path.display())),
            SimpleFileKind::File | SimpleFileKind::Symlink => fs::remove_file(&file.path)
                .with_context(|| format!("cannot remove {}", file.path.display())),
        }
    }
}

#[cfg(test)]
mod tests;
