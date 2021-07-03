use std::sync::Arc;

use glob::Pattern;

use crate::filesystem::FileInfo;

pub struct Matcher {
    pub name: Arc<str>,
    pub to_match: Pattern,
    pub clean_strategy: CleanStrategy,
}
pub enum CleanStrategy {
    Remove(RemovalPattern),
    RunCommand(Arc<str>),
}

pub struct RemovalPattern(Pattern);

impl Matcher {
    pub fn with_remove_strategy(name: Arc<str>, to_match: Pattern, to_remove: Pattern) -> Self {
        let clean_strategy = CleanStrategy::Remove(RemovalPattern(to_remove));
        Self {
            name,
            to_match,
            clean_strategy,
        }
    }

    pub fn with_command_strategy(name: Arc<str>, to_match: Pattern, command: String) -> Self {
        let clean_strategy = CleanStrategy::RunCommand(command.into());
        Self {
            name,
            to_match,
            clean_strategy,
        }
    }

    pub fn any_entry_match(&self, entries: &[FileInfo]) -> bool {
        entries.iter().any(|e| self.to_match.matches(&e.name))
    }
}

impl RemovalPattern {
    pub fn find_files_to_remove(&self, entries: Vec<FileInfo>) -> (Vec<FileInfo>, Vec<FileInfo>) {
        entries.into_iter().partition(|e| self.0.matches(&e.name))
    }
}
