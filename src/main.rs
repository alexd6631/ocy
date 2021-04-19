mod matchers;
mod notifiers;
mod utils;

use colored::Colorize;
use matchers::standard_matchers;
use notifiers::{LoggingCleanerNotifier, VecWalkNotifier};
use ocy_core::filesystem::RealFileSystem;
use ocy_core::walker::Walker;
use ocy_core::{cleaner::Cleaner, walker::RemovalCandidate};
use utils::{format_file_size, prompt};

fn main() {
    print_banner();

    let files = perform_walk();
    if files.is_empty() {
        println!("No projects found");
        return;
    }
    println!();

    let total_size = total_size(&files);

    if prompt(&format!(
        "Reclaim {} (y/N) ? ",
        format_file_size(total_size).cyan()
    )) {
        perform_clean(files);
    }
}

fn perform_walk() -> Vec<RemovalCandidate> {
    let fs = RealFileSystem;
    let matchers = standard_matchers();

    let notifier = VecWalkNotifier::new();
    let walker = Walker::new(fs, matchers, &notifier);

    walker.walk_from_current_directory();
    notifier.to_remove.into_inner()
}

fn perform_clean(files: Vec<RemovalCandidate>) {
    let fs = RealFileSystem;
    let notifier = LoggingCleanerNotifier::new();
    let cleaner = Cleaner::new(files, fs, &notifier);
    cleaner.clean();
}

fn total_size(files: &[RemovalCandidate]) -> u64 {
    files.iter().map(|e| e.file_size.unwrap_or(0)).sum()
}

fn print_banner() {
    let version = std::env!("CARGO_PKG_VERSION");
    let banner_template = include_str!("../data/banner.txt");
    let banner = banner_template.replace("$VERSION", &version);
    println!("{}", banner.yellow());
}
