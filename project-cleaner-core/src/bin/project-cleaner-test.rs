use std::{cell::RefCell, vec};

use colored::Colorize;
use eyre::Report;
use glob::Pattern;
use humansize::{file_size_opts as options, FileSize};
use project_cleaner_core::{
    cleaner::Cleaner,
    cleaner::CleanerNotifier,
    filesystem::{FileInfo, RealFileSystem},
    matcher::Matcher,
    walker::{DeletionCandidate, WalkNotifier, Walker},
};
use std::io::Write;

fn main() {
    let fs = RealFileSystem;

    let matchers = vec![
        Matcher::new(
            "Cargo".into(),
            Pattern::new("Cargo.toml").unwrap(),
            Pattern::new("target").unwrap(),
        ),
        Matcher::new(
            "Gradle".into(),
            Pattern::new("build.gradle").unwrap(),
            Pattern::new("build").unwrap(),
        ),
        Matcher::new(
            "Maven".into(),
            Pattern::new("pom.xml").unwrap(),
            Pattern::new("target").unwrap(),
        ),
        Matcher::new(
            "NodeJS".into(),
            Pattern::new("*").unwrap(),
            Pattern::new("node_modules").unwrap(),
        ),
        Matcher::new(
            "XCode".into(),
            Pattern::new("*").unwrap(),
            Pattern::new("DerivedData").unwrap(),
        ),
        Matcher::new(
            "SBT".into(),
            Pattern::new("build.sbt").unwrap(),
            Pattern::new("target").unwrap(),
        ),
        Matcher::new(
            "SBT".into(),
            Pattern::new("plugins.sbt").unwrap(),
            Pattern::new("target").unwrap(),
        ),
    ];
    let walker_notifier = VecWalkNotifier::default();

    let walker = Walker::new(fs, matchers, &walker_notifier);
    walker.simple_walk();

    let files = walker_notifier.to_remove.into_inner();
    if files.is_empty() {
        println!("No projects found");
        return;
    }
    println!();

    let total_size = files.iter().map(|e| e.file_size.unwrap_or(0)).sum::<u64>();
    print!(
        "Reclaim {} (y/N) ? ",
        total_size.file_size(options::CONVENTIONAL).unwrap().blue()
    );
    std::io::stdout().flush().unwrap();

    let mut buffer = String::new();
    let stdin = std::io::stdin();
    stdin.read_line(&mut buffer).unwrap();

    if buffer.trim().to_ascii_lowercase() == "y" {
        let cleaner = Cleaner::new(files, RealFileSystem, LoggingCleanerNotifier);
        cleaner.clean();
    }
}

struct LoggingCleanerNotifier;

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
            format!("Failed to remove {:?}: {}", candidate.file_info.path, report).red()
        );
    }
}

#[derive(Debug, Default)]
struct VecWalkNotifier {
    pub to_remove: RefCell<Vec<DeletionCandidate>>,
}

impl WalkNotifier for &VecWalkNotifier {
    fn notify_candidate_for_removal(&self, candidate: DeletionCandidate) {
        println!(
            "[{:>6}] {:>9} {:?}",
            candidate.matcher_name.green(),
            candidate
                .file_size
                .unwrap_or(0)
                .file_size(options::CONVENTIONAL)
                .unwrap()
                .blue(),
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
