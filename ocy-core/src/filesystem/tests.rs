use super::{FileSystem, RealFileSystem};
use crate::models::{FileInfo, SimpleFileKind};
use std::fs;
use std::path::Path;

fn dir_info(path: &Path) -> FileInfo {
    FileInfo::new(
        path.to_path_buf(),
        path.file_name().unwrap().to_string_lossy().into_owned(),
        SimpleFileKind::Directory,
    )
}

/// A name that is not valid UTF-8 must not hide the rest of the directory.
#[cfg(unix)]
#[test]
fn lists_siblings_of_a_non_utf8_name() -> eyre::Result<()> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let temp = tempfile::tempdir()?;
    fs::write(temp.path().join("Cargo.toml"), "")?;
    fs::create_dir(temp.path().join("target"))?;
    fs::write(temp.path().join(OsStr::from_bytes(b"\xff\xfebad")), "")?;

    let listing = RealFileSystem.list_files(&dir_info(temp.path()))?;
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();

    assert!(names.contains(&"Cargo.toml"), "got {names:?}");
    assert!(names.contains(&"target"), "got {names:?}");
    assert_eq!(3, listing.entries.len(), "got {names:?}");
    assert!(listing.errors.is_empty());
    Ok(())
}

/// A symlink must contribute only its own size, never its target's.
#[cfg(unix)]
#[test]
fn does_not_follow_symlinks_when_sizing() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let big = temp.path().join("big");
    fs::create_dir(&big)?;
    fs::write(big.join("blob"), vec![0_u8; 512 * 1024])?;

    let candidate = temp.path().join("target");
    fs::create_dir(&candidate)?;
    std::os::unix::fs::symlink(&big, candidate.join("link"))?;

    let size = RealFileSystem::reclaimable_size(&candidate)?;
    let big_size = RealFileSystem::reclaimable_size(&big)?;

    assert!(big_size >= 512 * 1024, "fixture too small: {big_size}");
    assert!(
        size < 64 * 1024,
        "symlink target leaked into the estimate: {size}"
    );
    Ok(())
}

/// A symlink cycle must terminate rather than recurse until the kernel intervenes.
#[cfg(unix)]
#[test]
fn terminates_on_a_symlink_cycle() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let candidate = temp.path().join("target");
    fs::create_dir_all(candidate.join("sub"))?;
    std::os::unix::fs::symlink(temp.path(), candidate.join("sub").join("loop"))?;

    RealFileSystem::reclaimable_size(&candidate)?;
    Ok(())
}

/// Hard links to one inode are the same bytes on disk and must be counted once.
#[cfg(unix)]
#[test]
fn counts_hard_linked_content_once() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let candidate = temp.path().join("target");
    fs::create_dir(&candidate)?;
    fs::write(candidate.join("a"), vec![0_u8; 256 * 1024])?;
    fs::hard_link(candidate.join("a"), candidate.join("b"))?;

    let size = RealFileSystem::reclaimable_size(&candidate)?;

    assert!(
        size < 384 * 1024,
        "hard link counted twice: {size} for 256 KiB of content"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn reports_symlinks_as_their_own_kind() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    fs::create_dir(temp.path().join("real"))?;
    std::os::unix::fs::symlink(temp.path().join("real"), temp.path().join("link"))?;

    let listing = RealFileSystem.list_files(&dir_info(temp.path()))?;
    let link = listing.entries.iter().find(|e| e.name == "link").unwrap();

    assert_eq!(SimpleFileKind::Symlink, link.kind);
    Ok(())
}
