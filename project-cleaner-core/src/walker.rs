use std::sync::Arc;

use crate::{
    filesystem::{FileInfo, FileSystem, SimpleFileKind},
    matcher::Matcher,
};
use eyre::Report;
use eyre::Result;

pub struct Walker<FS: FileSystem, N: WalkNotifier> {
    fs: FS,
    matchers: Vec<Matcher>,
    notifier: N,
}

#[derive(Debug)]
pub struct DeletionCandidate {
    pub matcher_name: Arc<str>,
    pub file_info: FileInfo,
    pub file_size: Option<u64>,
}

impl DeletionCandidate {
    pub fn new(matcher_name: Arc<str>, file_info: FileInfo, file_size: Option<u64>) -> Self {
        Self {
            matcher_name,
            file_info,
            file_size,
        }
    }
}

pub trait WalkNotifier {
    fn notify_candidate_for_removal(&self, candidate: DeletionCandidate);
    fn notify_fail_to_scan(&self, e: &FileInfo, report: Report);
}

impl<FS: FileSystem, N: WalkNotifier> Walker<FS, N> {
    pub fn new(fs: FS, matchers: Vec<Matcher>, notifier: N) -> Self {
        Self {
            fs,
            matchers,
            notifier,
        }
    }

    pub fn simple_walk(&self) {
        let current = self.fs.current_directory().unwrap();

        self.process_dir(&current);
    }

    fn process_dir(&self, file: &FileInfo) {
        match self.process_entries(file) {
            Ok(children) => {
                children.iter().for_each(|d| self.process_dir(&d));
            }
            Err(report) => self.notifier.notify_fail_to_scan(file, report),
        }
    }

    fn process_entries(&self, file: &FileInfo) -> Result<Vec<FileInfo>> {
        let mut entries = self.fs.list_files(&file)?;

        for matcher in &self.matchers {
            if entries.iter().any(|e| matcher.to_match.matches(&e.name)) {
                let (to_remove, remaining): (Vec<_>, Vec<_>) = entries
                    .into_iter()
                    .partition(|e| matcher.to_remove.matches(&e.name));
                to_remove.into_iter().for_each(|e| {
                    let candidate = DeletionCandidate::new(
                        matcher.name.clone(),
                        e,
                        self.fs.file_size(file).ok(),
                    );
                    self.notifier.notify_candidate_for_removal(candidate);
                });
                entries = remaining;
            }
        }
        entries.retain(|e| e.kind == SimpleFileKind::Directory && e.name != ".git");
        Ok(entries)
    }
}
