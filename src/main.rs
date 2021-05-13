mod matchers;
mod notifiers;
mod options;
mod utils;

use colored::Colorize;
use eyre::{Context, Result};
use gumdrop::Options;
use matchers::standard_matchers;
use std::{collections::HashSet, path::PathBuf, process::exit};

use ocy_core::filesystem::{FileInfo, FileSystem, RealFileSystem};
use ocy_core::walker::Walker;
use ocy_core::{cleaner::Cleaner, walker::RemovalCandidate};

use notifiers::{LoggingCleanerNotifier, VecWalkNotifier};
use options::OcyOptions;
use utils::{format_file_size, prompt};

fn main() -> Result<()> {
    let options = OcyOptions::parse_args_default_or_exit();

    print_banner();

    if options.version {
        exit(0);
    }

    let ignores = options.get_ignores_set();

    let current_directory = RealFileSystem
        .current_directory()
        .wrap_err("Cannot scan current directory")?;

    let files = perform_walk(&current_directory, ignores, options.walk_all);
    if files.is_empty() {
        println!("No projects found");
        exit(1);
    }
    println!();

    let total_size = total_size(&files);

    if prompt(&format!(
        "Reclaim {} (y/N) ? ",
        format_file_size(total_size).cyan()
    )) {
        perform_clean(&current_directory, files);
    }

    Ok(())
}

fn perform_walk(
    current_directory: &FileInfo,
    ignores: HashSet<PathBuf>,
    walk_all: bool,
) -> Vec<RemovalCandidate> {
    let fs = RealFileSystem;
    let matchers = standard_matchers();
    let notifier = VecWalkNotifier::new(&current_directory.path);
    let walker = Walker::new(fs, matchers, &notifier, ignores, walk_all);

    walker.walk_from_path(current_directory);
    notifier.to_remove.into_inner()
}

fn perform_clean(current_directory: &FileInfo, files: Vec<RemovalCandidate>) {
    let fs = RealFileSystem;
    let notifier = LoggingCleanerNotifier::new(&current_directory.path, files.len());
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
