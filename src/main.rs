mod matchers;
mod notifiers;
mod utils;

use colored::Colorize;
use matchers::standard_matchers;
use notifiers::{LoggingCleanerNotifier, VecWalkNotifier};
use ocy_core::cleaner::Cleaner;
use ocy_core::filesystem::RealFileSystem;
use ocy_core::walker::Walker;
use utils::{format_file_size, prompt};

fn main() {
    print_banner();

    let fs = RealFileSystem;
    let matchers = standard_matchers();
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

    if prompt(&format!(
        "Reclaim {} (y/N) ? ",
        format_file_size(total_size).cyan()
    )) {
        let cleaner = Cleaner::new(files, RealFileSystem, LoggingCleanerNotifier);
        cleaner.clean();
    }
}

fn print_banner() {
    let banner = include_str!("../data/banner.txt");
    println!("{}", banner.yellow());
}