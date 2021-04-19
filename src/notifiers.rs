use crate::utils::format_file_size;
use colored::Colorize;
use eyre::Report;
use indicatif::ProgressBar;
use ocy_core::{
    cleaner::CleanerNotifier,
    filesystem::FileInfo,
    walker::{RemovalCandidate, WalkNotifier},
};
use std::cell::RefCell;
pub struct LoggingCleanerNotifier {
    pub progress_bar: ProgressBar,
}

impl LoggingCleanerNotifier {
    pub fn new() -> Self {
        let progress_bar = ProgressBar::new_spinner();
        progress_bar.enable_steady_tick(50);
        Self { progress_bar }
    }
}

impl CleanerNotifier for &LoggingCleanerNotifier {
    fn notify_removal_started(&self, candidate: &RemovalCandidate) {
        self.progress_bar.set_message(&format!("Removing {:?}", candidate.file_info.path));    
    }

    fn notify_removal_success(&self, candidate: RemovalCandidate) {
        self.progress_bar.println(format!(
            "{}",
            format!("Removed {:?}", candidate.file_info.path).green()
        ));
    }

    fn notify_removal_failed(&self, candidate: RemovalCandidate, report: Report) {
        self.progress_bar.println(format!(
            "{}",
            format!(
                "Failed to remove {:?}: {}",
                candidate.file_info.path, report
            )
            .red()
        ));
    }

    fn notify_removal_finish(&self) {
        self.progress_bar.disable_steady_tick();
        self.progress_bar.finish_and_clear();
    }
}

#[derive(Debug)]
pub struct VecWalkNotifier {
    pub progress_bar: ProgressBar,
    pub to_remove: RefCell<Vec<RemovalCandidate>>,
}

impl VecWalkNotifier {
    pub fn new() -> Self {
        let progress_bar = ProgressBar::new_spinner();
        progress_bar.enable_steady_tick(50);
        Self {
            progress_bar,
            to_remove: Default::default(),
        }
    }
}

impl WalkNotifier for &VecWalkNotifier {
    fn notify_entered_directory(&self, dir: &FileInfo) {
        self.progress_bar.set_message(&format!("Scanning {:?}", dir.path));    
    }

    fn notify_candidate_for_removal(&self, candidate: RemovalCandidate) {
        self.progress_bar.println(format!(
            "{:>9} {:>9} {:?}",
            candidate.matcher_name.green(),
            format_file_size(candidate.file_size.unwrap_or(0)).cyan(),
            candidate.file_info.path,
        ));

        self.to_remove.borrow_mut().push(candidate);
    }

    fn notify_fail_to_scan(&self, e: &FileInfo, report: Report) {
        self.progress_bar.println(format!(
            "{}",
            format!("Failed to scan {:?}: {}", e.path, report).red()
        ));
    }

    fn notify_walk_finish(&self) {
        self.progress_bar.disable_steady_tick();
        self.progress_bar.finish_and_clear();
    }
}
