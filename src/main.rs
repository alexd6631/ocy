mod matchers;
mod notifiers;
mod options;
mod utils;

use colored::Colorize;
use eyre::{Context, Result};
use gumdrop::Options;
use matchers::{standard_matchers, widest_name};
use ocy_core::command::RealCommandExecutor;
use std::{collections::HashSet, path::PathBuf, process::ExitCode};

use ocy_core::filesystem::{FileSystem, RealFileSystem};
use ocy_core::models::FileInfo;
use ocy_core::walker::Walker;
use ocy_core::{cleaner::Cleaner, models::RemovalCandidate};

use notifiers::{LoggingCleanerNotifier, VecWalkNotifier};
use options::OcyOptions;
use utils::{format_file_size_and_more, prompt};

fn main() -> Result<ExitCode> {
    let options = OcyOptions::parse_args_default_or_exit();

    print_banner();

    if options.version {
        Ok(ExitCode::SUCCESS)
    } else {
        run(&options)
    }
}

fn run(options: &OcyOptions) -> Result<ExitCode> {
    let ignores = options.ignores_set()?;

    let current_directory = RealFileSystem
        .current_directory()
        .wrap_err("Cannot scan current directory")?;

    let files = perform_walk(
        &current_directory,
        ignores,
        options.walk_all,
        options.allow_commands,
    );

    // An empty scan is a successful scan: there was simply nothing to reclaim.
    if files.is_empty() {
        println!("No projects found");
        Ok(ExitCode::SUCCESS)
    } else {
        println!();
        let (total_size, has_more) = total_size(&files);
        let total = format_file_size_and_more(total_size, has_more);

        if options.dry_run {
            println!("Would reclaim {} (dry run)", total.cyan());
            Ok(ExitCode::SUCCESS)
        } else {
            if prompt(&format!("Reclaim {} (y/N) ? ", total.cyan()))? {
                perform_clean(&current_directory, files);
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn perform_walk(
    current_directory: &FileInfo,
    ignores: HashSet<PathBuf>,
    walk_all: bool,
    allow_commands: bool,
) -> Vec<RemovalCandidate> {
    let fs = RealFileSystem;
    let matchers = standard_matchers(allow_commands);
    let notifier = VecWalkNotifier::new(&current_directory.path, widest_name(&matchers));
    let walker = Walker::new(fs, matchers, &notifier, ignores, walk_all);

    walker.walk_from_path(current_directory);
    notifier.to_remove.into_inner()
}

fn perform_clean(current_directory: &FileInfo, files: Vec<RemovalCandidate>) {
    let fs = RealFileSystem;
    let ce = RealCommandExecutor;
    let notifier = LoggingCleanerNotifier::new(&current_directory.path, files.len());
    let cleaner = Cleaner::new(files, fs, ce, &notifier);
    cleaner.clean();
}

fn total_size(files: &[RemovalCandidate]) -> (u64, bool) {
    let estimate = files.iter().map(|e| e.estimate_file_size()).sum();
    let has_more = files.iter().any(|e| e.file_size().is_none());
    (estimate, has_more)
}

fn print_banner() {
    let version = std::env!("CARGO_PKG_VERSION");
    let banner_template = include_str!("../data/banner.txt");
    let banner = banner_template.replace("$VERSION", version);
    println!("{}", banner.yellow());
}
