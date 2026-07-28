//! Records that git keeps for linked worktrees.

use std::fs;
use std::path::{Path, PathBuf};

/// Directory under `.git` holding one record per linked worktree.
const WORKTREES_DIR: &str = "worktrees";

/// File in a record pointing at the `.git` file inside the checkout.
const GITDIR_POINTER: &str = "gitdir";

/// Marker file that makes `git worktree prune` leave a record alone.
const LOCK_MARKER: &str = "locked";

/// Records for linked worktrees whose checkout no longer exists.
///
/// `git worktree add` writes `.git/worktrees/<name>/`, whose [`GITDIR_POINTER`] file names
/// the `.git` file inside the checkout. Deleting the checkout leaves the record behind,
/// and removing those leftovers is exactly what `git worktree prune` does.
///
/// Anything that cannot be positively confirmed as stale is left alone, so a record with
/// an unreadable pointer or an explicit lock is never returned.
pub fn stale_worktree_records(git_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(git_dir.join(WORKTREES_DIR)) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|record| is_stale(record))
        .collect()
}

/// Checkout directories of linked worktrees that still exist.
///
/// Where a worktree lives is a matter of local convention -- `.worktrees/`, `.claude/`,
/// a sibling directory -- and any of those may be hidden. Guessing the directory name
/// means missing whichever convention was not guessed, so the records are read instead.
/// Each record's [`GITDIR_POINTER`] names the `.git` file inside the checkout, whose
/// parent is the checkout itself.
pub fn linked_worktree_paths(git_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(git_dir.join(WORKTREES_DIR)) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter_map(|entry| checkout_path(&entry.path()))
        .collect()
}

fn checkout_path(record: &Path) -> Option<PathBuf> {
    let pointer = fs::read_to_string(record.join(GITDIR_POINTER)).ok()?;
    let git_file = Path::new(pointer.trim());

    if git_file.exists() {
        git_file.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn is_stale(record: &Path) -> bool {
    if record.join(LOCK_MARKER).exists() {
        false
    } else {
        match fs::read_to_string(record.join(GITDIR_POINTER)) {
            Ok(pointer) => !Path::new(pointer.trim()).exists(),
            // An unreadable pointer cannot be verified, so treat the record as live.
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests;
