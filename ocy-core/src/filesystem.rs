use crate::models::FileInfo;
use crate::models::SimpleFileKind;
use eyre::Context;
use eyre::Report;
use eyre::Result;
use std::collections::HashSet;
use std::fs::{self, DirEntry, Metadata};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::Path;

/// The contents of a directory, together with any per-entry failures.
///
/// Reading a directory is not all-or-nothing. A hidden sibling is silently unreclaimed
/// disk space, so one bad entry must not discard the rest of the listing.
///
/// Two different things could discard it, and they are handled differently. A name that
/// does not decode as UTF-8 is not an error at all -- see [`FileInfo::name`], which keeps
/// the decoded form for matching and the original [`std::path::PathBuf`] for every
/// filesystem operation. `errors` covers the remaining case: an entry whose type cannot be
/// determined, which is rare and genuinely unusable.
#[derive(Debug, Default)]
pub struct DirListing {
    pub entries: Vec<FileInfo>,
    pub errors: Vec<Report>,
}

pub trait FileSystem {
    fn current_directory(&self) -> Result<FileInfo>;

    fn list_files(&self, file: &FileInfo) -> Result<DirListing>;

    fn file_size(&self, file: &FileInfo) -> Result<u64>;

    /// The filesystem this entry lives on, where that is knowable.
    ///
    /// [`None`] means the question cannot be answered, which callers treat as "do not
    /// restrict" rather than as a mount-point crossing.
    fn device_id(&self, _file: &FileInfo) -> Option<u64> {
        None
    }
}

pub trait FileSystemClean {
    fn remove_file(&self, file: &FileInfo) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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

    #[cfg(unix)]
    fn device_id(&self, file: &FileInfo) -> Option<u64> {
        fs::symlink_metadata(&file.path).ok().map(|m| m.dev())
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

/// Describe one directory entry.
///
/// Only the type lookup can fail. The name is decoded lossily rather than validated: rules
/// match on names, and a name that does not decode cannot match one anyway, so refusing it
/// would discard a perfectly good entry -- and, before this was split out, its siblings too.
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
    metadata.blocks() * 512
}

#[cfg(not(unix))]
fn allocated_bytes(metadata: &Metadata) -> u64 {
    metadata.len()
}

/// Whether this entry's bytes have not already been counted through another hard link.
#[cfg(unix)]
fn counts_towards_total(metadata: &Metadata, seen_inodes: &mut HashSet<(u64, u64)>) -> bool {
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
            SimpleFileKind::File => fs::remove_file(&file.path)
                .with_context(|| format!("cannot remove {}", file.path.display())),
            SimpleFileKind::Symlink => remove_symlink(&file.path)
                .with_context(|| format!("cannot remove link {}", file.path.display())),
        }
    }
}

/// Remove a symbolic link itself, never its target.
#[cfg(not(windows))]
fn remove_symlink(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)
}

/// Remove a symbolic link itself, never its target.
///
/// Windows needs the directory form of the call for a link that points at a directory --
/// `DeleteFile` fails on one, and `RemoveDirectory` fails on the file form. Both symlinks
/// and junctions carry `FILE_ATTRIBUTE_DIRECTORY` when they name a directory, so the
/// attribute decides which call to make. This never recurses, so the target is untouched.
#[cfg(windows)]
fn remove_symlink(path: &Path) -> std::io::Result<()> {
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

    if fs::symlink_metadata(path)?.file_attributes() & FILE_ATTRIBUTE_DIRECTORY == 0 {
        fs::remove_file(path)
    } else {
        fs::remove_dir(path)
    }
}

#[cfg(test)]
mod tests;
