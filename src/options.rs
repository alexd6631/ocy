use std::{collections::HashSet, path::PathBuf};

use eyre::{Context, Result};
use gumdrop::Options;

#[derive(Debug, Options)]
pub struct OcyOptions {
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "ignore this path")]
    pub ignores: Vec<PathBuf>,

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
    /// `--ignores` is ordinary user error, not a bug.
    pub fn ignores_set(&self) -> Result<HashSet<PathBuf>> {
        self.ignores
            .iter()
            .map(|p| {
                p.canonicalize()
                    .with_context(|| format!("cannot resolve ignored path {}", p.display()))
            })
            .collect()
    }
}
