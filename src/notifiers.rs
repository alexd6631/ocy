use crate::filesize::format_file_size;
use colored::Colorize;
use eyre::Report;
use project_cleaner_core::{
    cleaner::CleanerNotifier,
    filesystem::FileInfo,
    walker::{DeletionCandidate, WalkNotifier},
};
use std::cell::RefCell;
pub struct LoggingCleanerNotifier;

impl CleanerNotifier for LoggingCleanerNotifier {
    fn notify_removal_success(&self, candidate: DeletionCandidate) {
        println!(
            "{}",
            format!("Deleted {:?}", candidate.file_info.path).green()
        );
    }

    fn notify_removal_failed(&self, candidate: DeletionCandidate, report: Report) {
        eprintln!(
            "{}",
            format!(
                "Failed to remove {:?}: {}",
                candidate.file_info.path, report
            )
            .red()
        );
    }
}

#[derive(Debug, Default)]
pub struct VecWalkNotifier {
    pub to_remove: RefCell<Vec<DeletionCandidate>>,
}

impl WalkNotifier for &VecWalkNotifier {
    fn notify_candidate_for_removal(&self, candidate: DeletionCandidate) {
        println!(
            "[{:>6}] {:>9} {:?}",
            candidate.matcher_name.green(),
            format_file_size(candidate.file_size.unwrap_or(0)).blue(),
            candidate.file_info.path,
        );

        self.to_remove.borrow_mut().push(candidate);
    }

    fn notify_fail_to_scan(&self, e: &FileInfo, report: Report) {
        println!(
            "{}",
            format!("Failed to scan {:?}: {}", e.path, report).red()
        );
    }
}
