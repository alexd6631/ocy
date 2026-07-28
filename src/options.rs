use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use eyre::{Context, Result};
use gumdrop::Options;

/// Separator accepted inside a single `--ignore` value.
const IGNORE_SEPARATOR: char = ',';

#[derive(Debug, Default, Options)]
pub struct OcyOptions {
    #[options(help = "print help message")]
    help: bool,

    /// Held as written rather than as a [`PathBuf`], because the value is split before it
    /// is a path. Repeated once per path, so the flag is singular despite collecting a list.
    #[options(
        short = "i",
        long = "ignore",
        meta = "PATH[,PATH...]",
        help = "ignore path(s), repeatable"
    )]
    pub ignores: Vec<String>,

    #[options(help = "print version")]
    pub version: bool,

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
