use crate::utils::{format_opt_file_size, format_path, format_path_truncate};
use colored::Colorize;
use eyre::Report;
use indicatif::{ProgressBar, ProgressStyle};
use ocy_core::{
    cleaner::CleanerNotifier,
    models::{FileInfo, RemovalAction, RemovalCandidate},
    walker::WalkNotifier,
};
use std::{cell::RefCell, path::Path};
pub struct LoggingCleanerNotifier<'a> {
    base_path: &'a Path,
    pub progress_bar: ProgressBar,
}

impl<'a> LoggingCleanerNotifier<'a> {
    pub fn new(base_path: &'a Path, size: usize) -> Self {
        let progress_bar = ProgressBar::new(size as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner} {bar:40} {pos:>7}/{len:7} {msg}")
                .progress_chars("#>-"),
        );
        progress_bar.enable_steady_tick(50);
        Self {
            base_path,
            progress_bar,
        }
    }
}

impl<'a> CleanerNotifier for &LoggingCleanerNotifier<'a> {
    fn notify_removal_started(&self, candidate: &RemovalCandidate) {
        self.progress_bar.set_message(format!(
            "{} {}",
            format_clean_action(&candidate, ActionLabel::Start),
            format_candidate(self.base_path, &candidate)
        ));
    }

    fn notify_removal_success(&self, candidate: RemovalCandidate) {
        self.progress_bar.inc(1);
        self.progress_bar.println(
            format!(
                "{} {}",
                format_clean_action(&candidate, ActionLabel::Success),
                format_candidate(self.base_path, &candidate)
            )
            .green()
            .to_string(),
        );
    }

    fn notify_removal_failed(&self, candidate: RemovalCandidate, report: Report) {
        self.progress_bar.inc(1);
        self.progress_bar.println(
            format!(
                "{} {}: {}",
                format_clean_action(&candidate, ActionLabel::Failed),
                format_candidate(self.base_path, &candidate),
                report
            )
            .red()
            .to_string(),
        );
    }

    fn notify_removal_finish(&self) {
        self.progress_bar.disable_steady_tick();
        self.progress_bar.finish_and_clear();
    }
}

#[derive(Debug)]
pub struct VecWalkNotifier<'a> {
    base_path: &'a Path,
    pub progress_bar: ProgressBar,
    pub to_remove: RefCell<Vec<RemovalCandidate>>,
}

impl<'a> VecWalkNotifier<'a> {
    pub fn new(base_path: &'a Path) -> Self {
        let progress_bar = ProgressBar::new_spinner();
        progress_bar.enable_steady_tick(50);
        Self {
            base_path,
            progress_bar,
            to_remove: RefCell::default(),
        }
    }
}

impl<'a> WalkNotifier for &VecWalkNotifier<'a> {
    fn notify_entered_directory(&self, dir: &FileInfo) {
        self.progress_bar.set_message(format!(
            "Scanning {}",
            format_path_truncate(self.base_path, &dir.path)
        ));
    }

    fn notify_candidate_for_removal(&self, candidate: RemovalCandidate) {
        self.progress_bar.println(format!(
            "{:>9} {:>9} {}",
            candidate.matcher_name.green(),
            format_opt_file_size(candidate.file_size()).cyan(),
            format_candidate(self.base_path, &candidate),
        ));

        self.to_remove.borrow_mut().push(candidate);
    }

    fn notify_fail_to_scan(&self, e: &FileInfo, report: Report) {
        self.progress_bar.println(
            format!(
                "Failed to scan {}: {}",
                format_path(self.base_path, &e.path),
                report
            )
            .red()
            .to_string(),
        );
    }

    fn notify_walk_finish(&self) {
        self.progress_bar.disable_steady_tick();
        self.progress_bar.finish_and_clear();
    }
}

fn format_candidate(base_path: &Path, candidate: &RemovalCandidate) -> String {
    match &candidate.action {
        RemovalAction::Delete { file_info, .. } => format_path(base_path, &file_info.path),
        RemovalAction::RunCommand { work_dir, command } => {
            let path_str = format_path(base_path, &work_dir.path);
            format!("`{}` in `{}`", &command, path_str)
        }
    }
}

enum ActionLabel {
    Start,
    Success,
    Failed,
}

fn format_clean_action(candidate: &RemovalCandidate, label: ActionLabel) -> &'static str {
    match &candidate.action {
        RemovalAction::Delete { .. } => match label {
            ActionLabel::Start => "Removing",
            ActionLabel::Success => "Removed",
            ActionLabel::Failed => "Failed to remove",
        },
        RemovalAction::RunCommand { .. } => match label {
            ActionLabel::Start => "Executing",
            ActionLabel::Success => "Executed",
            ActionLabel::Failed => "Failed to execute",
        },
    }
}
