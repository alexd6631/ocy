use crate::models::{FileInfo, SimpleFileKind};
use glob::{Pattern, PatternError};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Why a [`Rule`] could not be built.
#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    #[error("rule `{name}` has no markers, so it would match every directory scanned")]
    NoMarkers { name: String },

    #[error("rule `{name}` reclaims nothing")]
    NoTargets { name: String },

    #[error("rule `{name}` has an empty target path")]
    EmptyTarget { name: String },

    #[error("rule `{name}` has an invalid pattern `{pattern}`")]
    InvalidPattern {
        name: String,
        pattern: String,
        #[source]
        source: PatternError,
    },
}

/// A path to reclaim, relative to the directory whose markers matched.
///
/// Components are matched one directory level at a time, so a target may reach into a
/// subdirectory -- `.angular/cache` reclaims only the cache, not the whole `.angular`
/// directory. Every component is a glob, which is what makes `cmake-build-*` expressible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub components: Vec<Pattern>,
    /// The kind the entry must have, or [`None`] to accept any.
    pub kind: Option<SimpleFileKind>,
}

impl Target {
    /// A target that reclaims a directory at `path`, relative to the project directory.
    pub fn directory(path: &str) -> Result<Self, PatternError> {
        Ok(Self {
            components: Self::parse(path)?,
            kind: Some(SimpleFileKind::Directory),
        })
    }

    /// A target that reclaims a file at `path`, relative to the project directory.
    pub fn file(path: &str) -> Result<Self, PatternError> {
        Ok(Self {
            components: Self::parse(path)?,
            kind: Some(SimpleFileKind::File),
        })
    }

    /// A target that reclaims whatever is at `path`, whether file, directory or symlink.
    pub fn any(path: &str) -> Result<Self, PatternError> {
        Ok(Self {
            components: Self::parse(path)?,
            kind: None,
        })
    }

    fn parse(path: &str) -> Result<Vec<Pattern>, PatternError> {
        path.split('/')
            .filter(|component| !component.is_empty())
            .map(Pattern::new)
            .collect()
    }
}

/// What a rule reclaims once its markers have matched.
#[derive(Debug, Clone)]
pub enum CleanAction {
    /// Reclaim these paths from inside the matched directory.
    Remove(Vec<Target>),

    /// Reclaim the matched directory itself.
    ///
    /// This is what makes self-describing artifacts expressible: a Python virtual
    /// environment is identified by the `pyvenv.cfg` it *contains*, not by anything
    /// beside it, so no sibling-only rule can name it.
    RemoveSelf,

    /// Run the project's own clean command in the matched directory.
    Run(Arc<str>),

    /// Reclaim the administrative directories of git worktrees that no longer exist.
    ///
    /// This is what `git worktree prune` removes. It needs to read each record's `gitdir`
    /// pointer and check whether the checkout is still there, which no glob can express.
    RemoveStaleWorktrees,
}

/// A cleanup rule: what identifies a project, and what may be reclaimed from it.
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: Arc<str>,
    markers: Vec<Pattern>,
    /// The one directory this rule applies to, or [`None`] to apply wherever the markers
    /// match.
    anchor: Option<PathBuf>,
    action: CleanAction,
}

impl Rule {
    /// A rule that reclaims `targets` from any directory containing all of `markers`.
    pub fn remove(name: &str, markers: &[&str], targets: &[&str]) -> Result<Self, RuleError> {
        Self::new(
            name,
            markers,
            CleanAction::Remove(directory_targets(name, targets)?),
        )
    }

    /// A rule that reclaims `targets` from one specific directory.
    ///
    /// A shared tool cache is identified by where it is rather than by anything it holds:
    /// `~/.cache/sccache` contains nothing but hex-named shards, a shape that describes
    /// every other content-addressed store just as well. Anchoring to the path is exact,
    /// so such a rule cannot fire anywhere it was not meant to -- which is what makes
    /// reclaiming something outside any project tree safe to offer at all.
    ///
    /// The anchor is not a scan root. It is still the walk that has to reach the
    /// directory, so a cache outside the tree `ocy` was pointed at stays untouched.
    pub fn remove_at(name: &str, anchor: PathBuf, targets: &[&str]) -> Result<Self, RuleError> {
        Ok(Self {
            name: name.into(),
            // The anchor is what identifies the directory, so there is nothing to look
            // for inside it and nothing for the empty-marker check to protect against.
            markers: Vec::new(),
            anchor: Some(anchor),
            action: CleanAction::Remove(directory_targets(name, targets)?),
        })
    }

    /// [`Rule::remove_at`], for targets that are not plain directories.
    ///
    /// A system package cache keeps its downloads as loose files beside a lock and a
    /// working directory the package manager needs to keep, so directories alone cannot
    /// say what to reclaim from it.
    pub fn remove_targets_at(
        name: &str,
        anchor: PathBuf,
        targets: Vec<Target>,
    ) -> Result<Self, RuleError> {
        if targets.is_empty() {
            Err(RuleError::NoTargets {
                name: name.to_string(),
            })
        } else {
            Ok(Self {
                name: name.into(),
                // As for `remove_at`: the anchor alone identifies the directory.
                markers: Vec::new(),
                anchor: Some(anchor),
                action: CleanAction::Remove(targets),
            })
        }
    }

    /// A rule that reclaims `targets`, for targets that are not plain directories.
    pub fn remove_targets(
        name: &str,
        markers: &[&str],
        targets: Vec<Target>,
    ) -> Result<Self, RuleError> {
        if targets.is_empty() {
            Err(RuleError::NoTargets {
                name: name.to_string(),
            })
        } else {
            Self::new(name, markers, CleanAction::Remove(targets))
        }
    }

    /// A rule that reclaims the matched directory itself.
    pub fn remove_self(name: &str, markers: &[&str]) -> Result<Self, RuleError> {
        Self::new(name, markers, CleanAction::RemoveSelf)
    }

    /// A rule that prunes the records of git worktrees whose checkout is gone.
    pub fn prune_stale_worktrees(name: &str, markers: &[&str]) -> Result<Self, RuleError> {
        Self::new(name, markers, CleanAction::RemoveStaleWorktrees)
    }

    /// A rule that runs `command` in the matched directory.
    pub fn run(name: &str, markers: &[&str], command: &str) -> Result<Self, RuleError> {
        Self::new(name, markers, CleanAction::Run(command.into()))
    }

    fn new(name: &str, markers: &[&str], action: CleanAction) -> Result<Self, RuleError> {
        // A rule without markers matches every directory. For `RemoveSelf` that would
        // propose deleting the entire tree, so it is rejected rather than trusted.
        if markers.is_empty() {
            Err(RuleError::NoMarkers {
                name: name.to_string(),
            })
        } else {
            let markers = markers
                .iter()
                .map(|marker| {
                    Pattern::new(marker).map_err(|source| RuleError::InvalidPattern {
                        name: name.to_string(),
                        pattern: (*marker).to_string(),
                        source,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Self {
                name: name.into(),
                markers,
                anchor: None,
                action,
            })
        }
    }

    pub fn action(&self) -> &CleanAction {
        &self.action
    }

    /// The single directory this rule applies to, for a rule that names one.
    pub fn anchor(&self) -> Option<&Path> {
        self.anchor.as_deref()
    }

    /// Whether `dir`, holding `entries`, is a project this rule applies to.
    ///
    /// Every marker must be present. Alternatives are expressed as globs
    /// (`build.gradle*`) or as separate rules, so there is no any-of mode to configure.
    /// An anchored rule additionally applies to nothing but its own directory.
    pub fn matches(&self, dir: &Path, entries: &[FileInfo]) -> bool {
        self.anchor
            .as_ref()
            .is_none_or(|anchor| anchor.as_path() == dir)
            && self
                .markers
                .iter()
                .all(|marker| entries.iter().any(|entry| marker.matches(&entry.name)))
    }
}

/// Parse `targets` as directory paths relative to the directory a rule matched.
fn directory_targets(name: &str, targets: &[&str]) -> Result<Vec<Target>, RuleError> {
    if targets.is_empty() {
        return Err(RuleError::NoTargets {
            name: name.to_string(),
        });
    }

    targets
        .iter()
        .map(|target| {
            Target::directory(target).map_err(|source| RuleError::InvalidPattern {
                name: name.to_string(),
                pattern: (*target).to_string(),
                source,
            })
        })
        .collect()
}

/// The width of the widest rule name, for column alignment.
pub fn widest_name(rules: &[Rule]) -> usize {
    rules
        .iter()
        .map(|rule| rule.name.chars().count())
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
