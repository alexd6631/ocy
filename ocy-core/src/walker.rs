use std::{cell::RefCell, collections::HashSet, path::Path, path::PathBuf};

use crate::{
    filesystem::FileSystem,
    models::RemovalCandidate,
    models::{FileInfo, SimpleFileKind},
    rule::{CleanAction, Rule, Target},
};
use eyre::Report;
use eyre::Result;

/// Version-control metadata, never descended into.
///
/// These directories hold thousands of small objects and no build output. Walking them is
/// pure cost, so they are skipped even under [`WalkOptions::walk_all`].
pub const VCS_DIRS: &[&str] = &[".git", ".svn", ".hg", ".jj", ".bzr"];

#[derive(Debug, Default, Clone)]
pub struct WalkOptions {
    /// Absolute paths that are neither scanned nor reclaimed.
    pub ignores: HashSet<PathBuf>,

    /// Descend into every hidden directory, not only [`WalkOptions::scanned_hidden`].
    pub walk_all: bool,

    /// Hidden directories descended into even when `walk_all` is false.
    ///
    /// Build output routinely hides behind a leading dot -- `.venv`, `.gradle`, `.next`.
    /// Skipping every dotted directory by default means missing most of it.
    pub scanned_hidden: HashSet<String>,

    /// Maximum depth below the scan root, or [`None`] for unlimited.
    pub max_depth: Option<usize>,

    /// Do not cross onto another filesystem, so a scan cannot wander onto network mounts.
    pub one_file_system: bool,
}

pub struct Walker<FS: FileSystem, N: WalkNotifier> {
    fs: FS,
    rules: Vec<Rule>,
    notifier: N,
    options: WalkOptions,
    /// Paths already claimed by a rule, so nested candidates are not walked or re-reported.
    pruned: RefCell<HashSet<PathBuf>>,
    root_device: RefCell<Option<u64>>,
}

pub trait WalkNotifier {
    fn notify_entered_directory(&self, dir: &FileInfo);
    fn notify_candidate_for_removal(&self, candidate: RemovalCandidate);
    fn notify_fail_to_scan(&self, e: &FileInfo, report: Report);
    fn notify_walk_finish(&self);
}

/// What the walk should do with a directory once its rules have been applied.
enum DirOutcome {
    /// Continue into these child directories.
    Descend(Vec<FileInfo>),
    /// The directory is itself a candidate; there is nothing below it worth visiting.
    Reclaimed,
}

impl<FS: FileSystem, N: WalkNotifier> Walker<FS, N> {
    pub fn new(fs: FS, rules: Vec<Rule>, notifier: N, options: WalkOptions) -> Self {
        Self {
            fs,
            rules,
            notifier,
            options,
            pruned: RefCell::default(),
            root_device: RefCell::default(),
        }
    }

    pub fn walk_from_path(&self, path: &FileInfo) {
        if self.options.one_file_system {
            *self.root_device.borrow_mut() = self.fs.device_id(path);
        }
        self.process_dir(path, 0);
        self.notifier.notify_walk_finish();
    }

    fn process_dir(&self, file: &FileInfo, depth: usize) {
        if self.is_ignored(&file.path) || self.pruned.borrow().contains(&file.path) {
            return;
        }

        match self.process_entries(file, depth) {
            Ok(DirOutcome::Descend(children)) => children
                .iter()
                .for_each(|child| self.process_dir(child, depth + 1)),
            Ok(DirOutcome::Reclaimed) => (),
            Err(report) => self.notifier.notify_fail_to_scan(file, report),
        }
    }

    fn process_entries(&self, dir: &FileInfo, depth: usize) -> Result<DirOutcome> {
        self.notifier.notify_entered_directory(dir);

        let listing = self.fs.list_files(dir)?;
        listing
            .errors
            .into_iter()
            .for_each(|report| self.notifier.notify_fail_to_scan(dir, report));
        let mut entries = listing.entries;

        for rule in &self.rules {
            if !rule.matches(&entries) {
                continue;
            }

            match rule.action() {
                // The scan root is never proposed for deletion: running ocy from inside a
                // venv must not offer to delete the directory being scanned.
                CleanAction::RemoveSelf if depth > 0 => {
                    self.emit_removal(rule, dir.clone());
                    return Ok(DirOutcome::Reclaimed);
                }
                CleanAction::RemoveSelf => (),
                CleanAction::Remove(targets) => {
                    let claimed = self.claim_targets(rule, &entries, targets);
                    entries.retain(|entry| !claimed.contains(&entry.path));
                    self.pruned.borrow_mut().extend(claimed);
                }
                CleanAction::Run(command) => {
                    self.notifier
                        .notify_candidate_for_removal(RemovalCandidate::new_cmd(
                            rule.name.clone(),
                            dir.clone(),
                            command.clone(),
                        ));
                }
                CleanAction::RemoveStaleWorktrees => self.claim_stale_worktrees(rule, dir),
            }
        }

        entries.retain(|entry| self.is_walkable(entry, depth));
        Ok(DirOutcome::Descend(entries))
    }

    /// Report every target of a matched rule that actually exists.
    ///
    /// Returns the claimed paths so the caller can avoid descending into them.
    fn claim_targets(
        &self,
        rule: &Rule,
        entries: &[FileInfo],
        targets: &[Target],
    ) -> HashSet<PathBuf> {
        targets
            .iter()
            .flat_map(|target| self.resolve_target(entries, target))
            .filter(|found| !self.is_ignored(&found.path))
            .filter(|found| !self.is_already_claimed(&found.path))
            .map(|found| {
                let path = found.path.clone();
                self.pruned.borrow_mut().insert(path.clone());
                self.emit_removal(rule, found);
                path
            })
            .collect()
    }

    /// Walk a target's components one directory level at a time.
    ///
    /// The first component is matched against the already-listed entries, so the common
    /// single-component target costs no extra syscall; only a nested target such as
    /// `.angular/cache` reads further directories.
    fn resolve_target(&self, entries: &[FileInfo], target: &Target) -> Vec<FileInfo> {
        let Some((first, rest)) = target.components.split_first() else {
            return Vec::new();
        };

        let mut found: Vec<FileInfo> = entries
            .iter()
            .filter(|entry| first.matches(&entry.name))
            .cloned()
            .collect();

        for component in rest {
            found = found
                .iter()
                .filter(|entry| entry.kind == SimpleFileKind::Directory)
                .filter_map(|dir| self.fs.list_files(dir).ok())
                .flat_map(|listing| listing.entries)
                .filter(|entry| component.matches(&entry.name))
                .collect();
        }

        found.retain(|entry| target.kind.is_none_or(|kind| kind == entry.kind));
        found
    }

    /// Report the records of worktrees whose checkout is gone.
    ///
    /// This reads the records directly rather than through [`FileSystem`], because
    /// deciding staleness means following a `gitdir` pointer out of the tree being
    /// walked. The logic is covered by the tests in [`crate::git`].
    fn claim_stale_worktrees(&self, rule: &Rule, dir: &FileInfo) {
        crate::git::stale_worktree_records(&dir.path.join(".git"))
            .into_iter()
            .filter(|record| !self.is_ignored(record))
            .for_each(|record| {
                let name = record
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.emit_removal(rule, FileInfo::new(record, name, SimpleFileKind::Directory));
            });
    }

    fn emit_removal(&self, rule: &Rule, file: FileInfo) {
        let size = self.fs.file_size(&file).ok();
        self.notifier
            .notify_candidate_for_removal(RemovalCandidate::new(rule.name.clone(), file, size));
    }

    fn is_ignored(&self, path: &Path) -> bool {
        self.options.ignores.contains(path)
    }

    /// Whether this path overlaps a candidate that has already been reported.
    ///
    /// Reporting both a directory and something inside it would count the nested bytes
    /// twice in the total and race the two deletions against each other. Rules within a
    /// directory are applied in order, so the overlap can be found in either direction:
    /// a nested target may be claimed before the parent enclosing it, or after.
    ///
    /// Candidates stream to the user as they are found, so the first claim stands and the
    /// overlapping one is dropped. That can leave an enclosing directory unreclaimed,
    /// which is the safe direction to err for a tool that deletes things.
    fn is_already_claimed(&self, path: &Path) -> bool {
        let pruned = self.pruned.borrow();
        path.ancestors().any(|ancestor| pruned.contains(ancestor))
            || pruned.iter().any(|claimed| claimed.starts_with(path))
    }

    fn is_walkable(&self, file: &FileInfo, depth: usize) -> bool {
        file.kind == SimpleFileKind::Directory
            && self.within_depth(depth)
            && self.is_scannable_name(&file.name)
            && self.stays_on_one_filesystem(file)
    }

    fn within_depth(&self, depth: usize) -> bool {
        self.options
            .max_depth
            .is_none_or(|max_depth| depth < max_depth)
    }

    fn is_scannable_name(&self, name: &str) -> bool {
        if VCS_DIRS.contains(&name) {
            false
        } else if name.starts_with('.') {
            self.options.walk_all || self.options.scanned_hidden.contains(name)
        } else {
            true
        }
    }

    fn stays_on_one_filesystem(&self, file: &FileInfo) -> bool {
        match (self.options.one_file_system, *self.root_device.borrow()) {
            (true, Some(root)) => self.fs.device_id(file).is_none_or(|device| device == root),
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests;
