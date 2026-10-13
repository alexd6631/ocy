use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard, OnceLock, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    filesystem::FileSystem,
    models::RemovalCandidate,
    models::{FileInfo, SimpleFileKind},
    rule::{CleanAction, Rule, Target},
};
use eyre::Report;
use eyre::Result;
use rayon::prelude::*;

/// Recover a lock whose holder panicked.
///
/// Every piece of state behind a lock here is a plain collection that is only ever added
/// to, so a panic part-way through leaves it structurally sound. Recovering keeps a panic
/// in one branch of the walk from cascading into every other branch that touches the same
/// state.
fn recover<'a, T>(
    result: Result<MutexGuard<'a, T>, PoisonError<MutexGuard<'a, T>>>,
) -> MutexGuard<'a, T> {
    result.unwrap_or_else(PoisonError::into_inner)
}

/// Version-control metadata, never descended into.
///
/// These directories hold thousands of small objects and no build output. Walking them is
/// pure cost, so they are skipped even under [`WalkOptions::walk_all`].
pub const VCS_DIRS: &[&str] = &[".git", ".svn", ".hg", ".jj", ".bzr"];

/// Package installation trees, never descended into.
///
/// A `node_modules` belonging to a project is claimed whole by the rule that matched its
/// manifest, so nothing inside one is ever reclaimed separately anyway. A `node_modules`
/// that no rule claims is a different thing entirely: an install prefix such as
/// `~/.n/lib/node_modules` holds installed *software*, and the `node_modules` inside each
/// installed package holds the dependencies that program needs in order to run.
///
/// Descending would offer to delete exactly those. An installed package carries a
/// `package.json`, so one level down it is indistinguishable from a project -- and
/// reclaiming its dependencies leaves the program on disk but broken, with no build to
/// recreate them. `npm` bundling its own dependencies that way is how the tool that would
/// reinstall them becomes the casualty.
pub const INSTALLED_DIRS: &[&str] = &["node_modules"];

#[derive(Debug, Default, Clone)]
pub struct WalkOptions {
    /// Absolute paths that are neither scanned nor reclaimed, nor anything under them.
    ///
    /// A subtree is what has to be protected: an application's own directory holds its
    /// data and its bundled dependencies the same way a project holds its build output,
    /// so ignoring the directory has to cover every level below it. Protecting only the
    /// named path would leave a scan pointed inside it free to reclaim what it finds.
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

    /// Reclaim the directory a cache symlink points at, rather than passing the link by.
    ///
    /// Only an anchored rule follows links. A shared cache is routinely linked onto a
    /// roomier disk, and the link is the only place a scan of the home directory meets
    /// it. A link inside a project is another matter: a `node_modules` linked to a
    /// sibling checkout is that checkout's, and reclaiming through it would reach out of
    /// the project into something that was never its build output.
    pub follow_links: bool,
}

/// Tracks paths already claimed by a rule, so nested candidates are not walked or re-reported.
///
/// This wrapper encapsulates the lock, to ensure it is only ever held for the length of
/// one operation and that check-then-insert cannot be split across two of them.
#[derive(Debug, Default)]
pub struct PrunedSet {
    inner: Mutex<HashSet<PathBuf>>,
}

impl PrunedSet {
    /// Creates a new empty PrunedSet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if a path is directly in the pruned set.
    pub fn contains(&self, path: &Path) -> bool {
        recover(self.inner.lock()).contains(path)
    }

    /// Take `path` for a candidate, unless it overlaps one already taken.
    ///
    /// Reporting both a directory and something inside it would count the nested bytes
    /// twice in the total and race the two deletions against each other. Rules within a
    /// directory are applied in order, so the overlap can be found in either direction:
    /// a nested target may be claimed before the parent enclosing it, or after. A path
    /// overlaps when any of its ancestors is already claimed, or when anything already
    /// claimed lies inside it.
    ///
    /// Candidates stream to the user as they are found, so the first claim stands and the
    /// overlapping one is dropped. That can leave an enclosing directory unreclaimed,
    /// which is the safe direction to err for a tool that deletes things.
    ///
    /// The check and the insert happen under one lock, which is what makes this safe to
    /// call from several branches of the walk at once: two overlapping candidates found
    /// concurrently would otherwise both pass the check and both be taken.
    pub fn claim(&self, path: &Path) -> bool {
        let mut pruned = recover(self.inner.lock());

        let overlaps = path.ancestors().any(|ancestor| pruned.contains(ancestor))
            || pruned.iter().any(|claimed| claimed.starts_with(path));

        if !overlaps {
            pruned.insert(path.to_path_buf());
        }
        !overlaps
    }
}

pub struct Walker<FS: FileSystem, N: WalkNotifier> {
    fs: FS,
    rules: Vec<Rule>,
    notifier: N,
    options: WalkOptions,
    /// Paths already claimed by a rule, so nested candidates are not walked or re-reported.
    pruned: PrunedSet,
    /// Settled once before the walk starts, so the branches can read it without locking.
    root_device: OnceLock<Option<u64>>,
    /// Directories already walked, so a worktree reached both by descent and by its git
    /// record is scanned once and counted once.
    visited: Mutex<HashSet<PathBuf>>,
    /// Checkouts of linked worktrees found during the walk, scanned once it finishes.
    pending_worktrees: Mutex<Vec<FileInfo>>,
    directories_scanned: AtomicUsize,
    candidates_found: AtomicUsize,
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

/// The walk shares one `&Walker` across rayon's threads, so every branch of it has to be
/// able to read the walker at the same time as every other.
impl<FS: FileSystem + Sync, N: WalkNotifier + Sync> Walker<FS, N> {
    pub fn new(fs: FS, rules: Vec<Rule>, notifier: N, options: WalkOptions) -> Self {
        Self {
            fs,
            rules,
            notifier,
            options,
            pruned: PrunedSet::new(),
            root_device: OnceLock::new(),
            visited: Mutex::default(),
            pending_worktrees: Mutex::default(),
            directories_scanned: AtomicUsize::new(0),
            candidates_found: AtomicUsize::new(0),
        }
    }

    pub fn walk_from_path(&self, path: &FileInfo) {
        if self.options.one_file_system {
            // Only ever set here, before any branch of the walk can read it.
            let _ = self.root_device.set(self.fs.device_id(path));
        }

        log::info!(
            "scanning {} with {} rules",
            path.path.display(),
            self.rules.len()
        );
        self.process_dir(path, 0);
        self.process_pending_worktrees(&path.path);
        log::info!(
            "scanned {} directories, found {} candidates",
            self.directories_scanned.load(Ordering::Relaxed),
            self.candidates_found.load(Ordering::Relaxed)
        );

        self.notifier.notify_walk_finish();
    }

    /// Walk the linked worktrees discovered during the main walk.
    ///
    /// Deferred rather than recursed into on the spot, so that a worktree nested inside
    /// the tree is reached by ordinary descent first and skipped here as already visited.
    /// Only checkouts below `root` are followed: `ocy` was asked to clean one directory,
    /// and a worktree parked in `/tmp` is outside what was asked for.
    fn process_pending_worktrees(&self, root: &Path) {
        loop {
            // Popped in its own statement: as the scrutinee of a `while let`, the lock
            // would be held for the whole body, and walking a worktree can queue more.
            let next = recover(self.pending_worktrees.lock()).pop();
            let Some(worktree) = next else {
                break;
            };

            if worktree.path.starts_with(root) {
                log::debug!("following linked worktree {}", worktree.path.display());
                self.process_dir(&worktree, 0);
            } else {
                log::debug!(
                    "skipping worktree outside the scan root: {}",
                    worktree.path.display()
                );
            }
        }
    }

    fn process_dir(&self, file: &FileInfo, depth: usize) {
        // TODO consider using is_already_claimed
        if self.is_ignored(&file.path) || self.pruned.contains(&file.path) {
            return;
        }
        if !recover(self.visited.lock()).insert(file.path.clone()) {
            return;
        }

        match self.process_entries(file, depth) {
            // Sibling directories are independent of one another, and the walk spends
            // most of its time waiting on the filesystem rather than working, so reading
            // several at once is close to free. Nested `for_each`es compose: rayon steals
            // work across the whole tree rather than one level of it.
            Ok(DirOutcome::Descend(children)) => children
                .par_iter()
                .for_each(|child| self.process_dir(child, depth + 1)),
            Ok(DirOutcome::Reclaimed) => (),
            Err(report) => self.notifier.notify_fail_to_scan(file, report),
        }
    }

    fn process_entries(&self, dir: &FileInfo, depth: usize) -> Result<DirOutcome> {
        self.notifier.notify_entered_directory(dir);
        self.directories_scanned.fetch_add(1, Ordering::Relaxed);

        let listing = self.fs.list_files(dir)?;
        listing
            .errors
            .into_iter()
            .for_each(|report| self.notifier.notify_fail_to_scan(dir, report));
        let mut entries = listing.entries;

        for rule in &self.rules {
            if !rule.matches(&dir.path, &entries) {
                continue;
            }

            match rule.action() {
                // The scan root is never proposed for deletion: running ocy from inside a
                // venv must not offer to delete the directory being scanned.
                CleanAction::RemoveSelf if depth > 0 => {
                    if self.claim(rule, dir.clone()) {
                        return Ok(DirOutcome::Reclaimed);
                    }
                }
                CleanAction::RemoveSelf => (),
                CleanAction::Remove(targets) => {
                    let claimed = self.claim_targets(rule, &entries, targets);
                    entries.retain(|entry| !claimed.contains(&entry.path));
                }
                CleanAction::Run(command) => {
                    self.notifier
                        .notify_candidate_for_removal(RemovalCandidate::new_cmd(
                            rule.name.clone(),
                            dir.clone(),
                            command.clone(),
                        ));
                }
                CleanAction::RemoveStaleWorktrees => {
                    self.claim_stale_worktrees(rule, dir);
                    self.queue_linked_worktrees(dir);
                }
            }
        }

        entries.retain(|entry| self.is_walkable(entry, depth));
        Ok(DirOutcome::Descend(entries))
    }

    /// Report every target of a matched rule that actually exists.
    ///
    /// Returns the paths that were claimed, so the caller can drop them from the entries
    /// it is about to descend into.
    fn claim_targets(
        &self,
        rule: &Rule,
        entries: &[FileInfo],
        targets: &[Target],
    ) -> HashSet<PathBuf> {
        let follow_links = self.options.follow_links && rule.anchor().is_some();

        targets
            .iter()
            .flat_map(|target| self.resolve_target(entries, target, follow_links))
            .filter_map(|found| {
                let path = found.path.clone();
                self.claim(rule, found).then_some(path)
            })
            .collect()
    }

    /// Walk a target's components one directory level at a time.
    ///
    /// The first component is matched against the already-listed entries, so the common
    /// single-component target costs no extra syscall; only a nested target such as
    /// `.angular/cache` reads further directories.
    fn resolve_target(
        &self,
        entries: &[FileInfo],
        target: &Target,
        follow_links: bool,
    ) -> Vec<FileInfo> {
        let Some((first, rest)) = target.components.split_first() else {
            return Vec::new();
        };

        // A link can stand at any level: `~/.npm` is as likely to be one as
        // `~/.npm/_cacache`, and a link that is not followed stops the target short.
        // One that cannot be followed stays the link it is, for a target to take or not.
        let follow = |found: Vec<FileInfo>| -> Vec<FileInfo> {
            if follow_links {
                found
                    .into_iter()
                    .map(|entry| match entry.kind {
                        SimpleFileKind::Symlink => self.follow_link(&entry).unwrap_or(entry),
                        _ => entry,
                    })
                    .collect()
            } else {
                found
            }
        };

        let mut found = follow(
            entries
                .iter()
                .filter(|entry| first.matches(&entry.name))
                .cloned()
                .collect(),
        );

        for component in rest {
            found = follow(
                found
                    .iter()
                    .filter(|entry| entry.kind == SimpleFileKind::Directory)
                    .filter_map(|dir| self.fs.list_files(dir).ok())
                    .flat_map(|listing| listing.entries)
                    .filter(|entry| component.matches(&entry.name))
                    .collect(),
            );
        }

        found.retain(|entry| target.kind.is_none_or(|kind| kind == entry.kind));
        found
    }

    /// The directory `link` points at, if the scan may reclaim it.
    ///
    /// The target lies outside anything the walk has vetted, so it is held to what the
    /// walk would have required on the way there: nothing under an ignored directory --
    /// which includes the user's XDG state -- and nothing on another filesystem when
    /// asked to stay on one.
    fn follow_link(&self, link: &FileInfo) -> Option<FileInfo> {
        let real = self.fs.resolve_link(link)?;

        if self.is_ignored(&real.path) {
            log::debug!(
                "not following {}: {} is ignored",
                link.path.display(),
                real.path.display()
            );
            None
        } else if !self.stays_on_one_filesystem(&real) {
            log::debug!(
                "not following {}: {} is on another filesystem",
                link.path.display(),
                real.path.display()
            );
            None
        } else {
            log::debug!(
                "following {} to {}",
                link.path.display(),
                real.path.display()
            );
            Some(real)
        }
    }

    /// Report the records of worktrees whose checkout is gone.
    ///
    /// This reads the records directly rather than through [`FileSystem`], because
    /// deciding staleness means following a `gitdir` pointer out of the tree being
    /// walked. The logic is covered by the tests in [`crate::git`].
    fn claim_stale_worktrees(&self, rule: &Rule, dir: &FileInfo) {
        crate::git::stale_worktree_records(&dir.path.join(".git"))
            .into_iter()
            .for_each(|record| {
                let name = record
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.claim(rule, FileInfo::new(record, name, SimpleFileKind::Directory));
            });
    }

    /// Note the checkouts of this repository's linked worktrees for later walking.
    ///
    /// A worktree is a working copy with its own build output, and it is routinely parked
    /// under a hidden directory that the walk would otherwise never enter.
    fn queue_linked_worktrees(&self, dir: &FileInfo) {
        let found = crate::git::linked_worktree_paths(&dir.path.join(".git"));

        recover(self.pending_worktrees.lock()).extend(found.into_iter().map(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            FileInfo::new(path, name, SimpleFileKind::Directory)
        }));
    }

    /// Claim `file` for `rule` and report it, unless it must not be claimed.
    ///
    /// Every claim goes through here, so being ignored, overlapping an existing candidate
    /// and recording what has been claimed are decided in one place instead of in each
    /// caller. Returns whether the claim was taken.
    fn claim(&self, rule: &Rule, file: FileInfo) -> bool {
        if self.is_ignored(&file.path) || !self.pruned.claim(&file.path) {
            return false;
        }

        let size = match self.fs.file_size(&file) {
            Ok(size) => Some(size),
            Err(report) => {
                // The candidate is still offered; only its size is unknown.
                log::debug!("cannot size {}: {report:#}", file.path.display());
                None
            }
        };
        log::debug!(
            "rule `{}` claims {} ({})",
            rule.name,
            file.path.display(),
            size.map_or_else(
                || "size unknown".to_string(),
                |size| format!("{size} bytes")
            )
        );
        self.candidates_found.fetch_add(1, Ordering::Relaxed);
        self.notifier
            .notify_candidate_for_removal(RemovalCandidate::new(rule.name.clone(), file, size));
        true
    }

    /// Whether `path` is ignored, or lies under something that is.
    ///
    /// Checked against every ancestor rather than the path alone, so an ignored
    /// directory protects what is inside it even when the walk reaches that content
    /// directly -- as a scan root, a linked worktree checkout, or a followed symlink --
    /// without ever visiting the ignored directory itself.
    fn is_ignored(&self, path: &Path) -> bool {
        path.ancestors()
            .any(|ancestor| self.options.ignores.contains(ancestor))
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
            log::trace!("skipping {name}: version control metadata");
            false
        } else if INSTALLED_DIRS.contains(&name) {
            log::trace!("skipping {name}: installed packages, not build output");
            false
        } else if name.starts_with('.') {
            let scannable = self.options.walk_all || self.options.scanned_hidden.contains(name);
            if !scannable {
                // The most common reason a user reports something as "not found".
                log::debug!("skipping hidden {name}; use --all to descend into it");
            }
            scannable
        } else {
            true
        }
    }

    fn stays_on_one_filesystem(&self, file: &FileInfo) -> bool {
        match (
            self.options.one_file_system,
            self.root_device.get().copied().flatten(),
        ) {
            (true, Some(root)) => self.fs.device_id(file).is_none_or(|device| device == root),
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests;
