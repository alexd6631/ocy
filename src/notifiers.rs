use crate::palette::rule_color;
use crate::utils::{SIZE_COLUMN_WIDTH, format_opt_file_size, format_path, format_path_truncate};
use colored::Colorize;
use eyre::Report;
use indicatif::{ProgressBar, ProgressStyle};
use ocy_core::{
    cleaner::CleanerNotifier,
    models::{FileInfo, RemovalAction, RemovalCandidate},
    walker::WalkNotifier,
};
use std::{
    io::IsTerminal,
    path::Path,
    sync::{Mutex, PoisonError},
    time::Duration,
};

/// Whether the animated progress display should be used at all.
///
/// An animated bar redraws in place on stderr. That is wrong in two situations: when
/// logging is on, because the log lines land on the same stream and fight the redraw; and
/// when stderr is not a terminal, because the control sequences are meaningless in a file.
/// In both cases the results are still printed -- only the animation is dropped.
pub fn progress_is_useful() -> bool {
    std::io::stderr().is_terminal() && log::max_level() == log::LevelFilter::Off
}

fn spinner(animated: bool) -> ProgressBar {
    if animated {
        let bar = ProgressBar::new_spinner();
        bar.enable_steady_tick(Duration::from_millis(50));
        bar
    } else {
        ProgressBar::hidden()
    }
}

fn bar(animated: bool, size: usize) -> ProgressBar {
    if animated {
        let bar = ProgressBar::new(size as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                // SAFETY: the template is a literal, so it either parses on every run or
                // on none; a malformed one would fail the first test that renders a bar.
                .template("{spinner} {bar:40} {pos:>7}/{len:7} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        bar.enable_steady_tick(Duration::from_millis(50));
        bar
    } else {
        ProgressBar::hidden()
    }
}

/// Print a result line, stepping around the progress display if one is drawn.
///
/// [`ProgressBar::println`] emits nothing once the draw target is hidden, which would
/// silently swallow every result whenever output is piped. `suspend` runs the closure
/// either way, so results survive redirection.
fn emit(progress_bar: &ProgressBar, line: String) {
    progress_bar.suspend(|| println!("{line}"));
}

pub struct LoggingCleanerNotifier<'a> {
    base_path: &'a Path,
    pub progress_bar: ProgressBar,
}

impl<'a> LoggingCleanerNotifier<'a> {
    pub fn new(base_path: &'a Path, size: usize, animated: bool) -> Self {
        Self {
            base_path,
            progress_bar: bar(animated, size),
        }
    }
}

impl<'a> CleanerNotifier for &LoggingCleanerNotifier<'a> {
    fn notify_removal_started(&self, candidate: &RemovalCandidate) {
        self.progress_bar.set_message(format!(
            "{} {}",
            format_clean_action(candidate, ActionLabel::Start),
            format_candidate(self.base_path, candidate)
        ));
    }

    fn notify_removal_success(&self, candidate: RemovalCandidate) {
        self.progress_bar.inc(1);
        emit(
            &self.progress_bar,
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
        emit(
            &self.progress_bar,
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
    /// Width of the rule-name column, taken from the widest name in the active rule set.
    ///
    /// Candidates stream out as they are found, so the width cannot be derived from the
    /// results; deriving it from the rules keeps the columns aligned regardless of which
    /// rules happen to fire. This is issue #3.
    name_width: usize,
    pub progress_bar: ProgressBar,
    /// Behind a lock rather than a [`RefCell`](std::cell::RefCell): the walk reports from
    /// several threads at once.
    pub to_remove: Mutex<Vec<RemovalCandidate>>,
}

impl<'a> VecWalkNotifier<'a> {
    pub fn new(base_path: &'a Path, name_width: usize, animated: bool) -> Self {
        Self {
            base_path,
            name_width,
            progress_bar: spinner(animated),
            to_remove: Mutex::default(),
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
        // Pad before colouring: the escape sequences are not printable width, so padding
        // an already-coloured string is the classic way to get ragged columns.
        let name = format!(
            "{:>width$}",
            candidate.matcher_name,
            width = self.name_width
        );
        let size = format!(
            "{:>width$}",
            format_opt_file_size(candidate.file_size()),
            width = SIZE_COLUMN_WIDTH
        );

        emit(
            &self.progress_bar,
            format!(
                "{} {} {}",
                name.color(rule_color(&candidate.matcher_name)),
                size.cyan(),
                format_candidate(self.base_path, &candidate),
            ),
        );

        // Recovered rather than unwrapped: a panic in one branch of the walk must not
        // throw away the candidates every other branch has already found.
        self.to_remove
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(candidate);
    }

    fn notify_fail_to_scan(&self, e: &FileInfo, report: Report) {
        emit(
            &self.progress_bar,
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
            format!("`{command}` in `{path_str}`")
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
