use std::{collections::HashSet, path::PathBuf};

use gumdrop::Options;

#[derive(Debug, Options)]
pub struct OcyOptions {
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "ignore this path")]
    pub ignores: Vec<PathBuf>,

    #[options(help = "print version")]
    pub version: bool,

    #[options(short = "a", long ="all", help = "walk into hidden dirs")]
    pub walk_all: bool
}

impl OcyOptions {
    pub fn get_ignores_set(&self) -> HashSet<PathBuf> {
        self.ignores
            .iter()
            .map(|p| p.canonicalize().unwrap())
            .collect::<HashSet<_>>()
    }
}
