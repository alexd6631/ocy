use std::sync::Arc;

use glob::Pattern;

use crate::filesystem::FileInfo;

pub struct Matcher {
    pub name: Arc<str>,
    pub to_match: Pattern,
    pub to_remove: Pattern,
}

impl Matcher {
    pub fn new(name: Arc<str>, to_match: Pattern, to_remove: Pattern) -> Self {
        Self {
            name,
            to_match,
            to_remove,
        }
    }

    pub fn any_entry_match(&self, entries: &[FileInfo]) -> bool {
        entries.iter().any(|e| self.to_match.matches(&e.name))
    }

    pub fn find_files_to_remove(&self, entries: Vec<FileInfo>) -> (Vec<FileInfo>, Vec<FileInfo>) {
        entries
            .into_iter()
            .partition(|e| self.to_remove.matches(&e.name))
    }
}
