use super::stale_worktree_records;
use std::fs;
use std::path::{Path, PathBuf};

/// Build a `.git` directory holding one worktree record pointing at `checkout`.
fn git_dir_with_record(root: &Path, name: &str, checkout: &Path) -> PathBuf {
    let git_dir = root.join(".git");
    let record = git_dir.join("worktrees").join(name);
    fs::create_dir_all(&record).unwrap();
    fs::write(record.join("gitdir"), format!("{}\n", checkout.display())).unwrap();
    git_dir
}

#[test]
fn finds_a_record_whose_checkout_is_gone() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let git_dir = git_dir_with_record(temp.path(), "feature", &temp.path().join("gone/.git"));

    let stale = stale_worktree_records(&git_dir);

    assert_eq!(1, stale.len(), "got {stale:?}");
    assert!(stale[0].ends_with("worktrees/feature"));
    Ok(())
}

#[test]
fn leaves_a_live_worktree_alone() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let checkout = temp.path().join("live");
    fs::create_dir_all(&checkout)?;
    fs::write(checkout.join(".git"), "gitdir: whatever")?;
    let git_dir = git_dir_with_record(temp.path(), "live", &checkout.join(".git"));

    assert!(stale_worktree_records(&git_dir).is_empty());
    Ok(())
}

/// `git worktree prune` skips locked records, and so must this.
#[test]
fn leaves_a_locked_record_alone() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let git_dir = git_dir_with_record(temp.path(), "parked", &temp.path().join("gone/.git"));
    fs::write(git_dir.join("worktrees/parked/locked"), "on a usb stick")?;

    assert!(stale_worktree_records(&git_dir).is_empty());
    Ok(())
}

/// A record that cannot be verified must not be proposed for deletion.
#[test]
fn leaves_a_record_without_a_pointer_alone() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let git_dir = temp.path().join(".git");
    fs::create_dir_all(git_dir.join("worktrees").join("odd"))?;

    assert!(stale_worktree_records(&git_dir).is_empty());
    Ok(())
}

#[test]
fn a_repository_without_worktrees_yields_nothing() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    fs::create_dir_all(temp.path().join(".git"))?;

    assert!(stale_worktree_records(&temp.path().join(".git")).is_empty());
    Ok(())
}
