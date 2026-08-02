use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use eyre::{Context, Result};
use gumdrop::Options;
use log::LevelFilter;

/// Separator accepted inside a single `--ignore` value.
const IGNORE_SEPARATOR: char = ',';

#[derive(Debug, Default, Options)]
pub struct OcyOptions {
    #[options(help = "print help message")]
    help: bool,

    #[options(free, help = "start directory (defaults to current directory)")]
    pub start_dir: Vec<String>,

    #[options(
        short = "r",
        long = "rule",
        help = "apply only these rules (repeatable)"
    )]
    pub rules: Vec<String>,

    #[options(long = "rules", help = "list all available rules")]
    pub list_rules: bool,

    /// Held as written rather than as a [`PathBuf`], because the value is split before it
    /// is a path. Repeated once per path, so the flag is singular despite collecting a list.
    #[options(
        short = "i",
        long = "ignore",
        meta = "PATH[,PATH...]",
        help = "ignore path(s), repeatable"
    )]
    pub ignores: Vec<String>,

    /// Short form is `-V`, leaving `-v` free for verbosity as most CLIs do.
    #[options(short = "V", long = "version", help = "print version")]
    pub version: bool,

    #[options(
        short = "v",
        long = "verbose",
        count,
        help = "log more; repeat for debug and trace"
    )]
    pub verbose: u32,

    #[options(short = "q", long = "quiet", help = "suppress all logging")]
    pub quiet: bool,

    #[options(short = "a", long = "all", help = "walk into hidden dirs")]
    pub walk_all: bool,

    #[options(
        short = "n",
        long = "dry-run",
        help = "report what would be reclaimed, then exit without deleting"
    )]
    pub dry_run: bool,

    #[options(
        long = "allow-commands",
        help = "allow rules that run a project's own clean command (e.g. `make clean`)"
    )]
    pub allow_commands: bool,

    #[options(
        long = "max-depth",
        meta = "N",
        help = "do not descend deeper than this many levels"
    )]
    pub max_depth: Option<usize>,

    #[options(
        short = "x",
        long = "one-file-system",
        help = "do not cross onto another filesystem"
    )]
    pub one_file_system: bool,
}

impl OcyOptions {
    /// The log filter these options ask for, or [`None`] to defer to `RUST_LOG`.
    ///
    /// An explicit flag always wins over the environment. `RUST_LOG` is frequently
    /// exported once and forgotten, and a `-v` that silently did nothing because of it
    /// would be the more surprising outcome.
    pub fn log_filter(&self) -> Option<LevelFilter> {
        if self.quiet {
            Some(LevelFilter::Off)
        } else {
            match self.verbose {
                0 => None,
                1 => Some(LevelFilter::Info),
                2 => Some(LevelFilter::Debug),
                _ => Some(LevelFilter::Trace),
            }
        }
    }

    /// The ignore paths, canonicalised.
    ///
    /// A path that cannot be resolved is an error rather than a panic: mistyping
    /// `--ignore` is ordinary user error, not a bug.
    pub fn ignores_set(&self) -> Result<HashSet<PathBuf>> {
        self.ignores
            .iter()
            .map(|value| resolve_ignore(value))
            .collect::<Result<Vec<_>>>()
            .map(|paths| paths.into_iter().flatten().collect())
    }

    /// Returns the start directory if specified.
    ///
    /// Returns `Ok(None)` if no start directory was provided (use current directory).
    /// Returns `Ok(Some(path))` if a single start directory was provided.
    /// Returns an error if multiple start directories were provided.
    pub fn start_directory(&self) -> Result<Option<&str>> {
        match self.start_dir.len() {
            0 => Ok(None),
            1 => Ok(Some(&self.start_dir[0])),
            _ => eyre::bail!("Only one start directory can be specified"),
        }
    }

    /// Returns the slice of selected rule names.
    pub fn selected_rules(&self) -> &[String] {
        &self.rules
    }
}

/// Resolve one `--ignore` value into canonical paths.
///
/// Both `--ignore a --ignore b` and `--ignore a,b` are accepted, and they compose.
///
/// A comma is legal in a filename and the shell offers no way to protect one -- quoting
/// `"a,b"` still arrives as the bytes `a,b` -- so the value is split only when it does not
/// already name something that exists. A directory genuinely called `a,b` therefore
/// resolves as itself, and the ambiguous case resolves in favour of the real path, which
/// is the reading that cannot surprise anyone into ignoring the wrong thing.
fn resolve_ignore(value: &str) -> Result<Vec<PathBuf>> {
    let value = value.trim();

    if let Ok(path) = Path::new(value).canonicalize() {
        Ok(vec![path])
    } else {
        value
            .split(IGNORE_SEPARATOR)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(|path| {
                Path::new(path)
                    .canonicalize()
                    .with_context(|| format!("cannot resolve ignored path {path}"))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
