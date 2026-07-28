mod notifiers;
mod options;
mod rules;
mod utils;

use colored::Colorize;
use eyre::{Context, Result};
use gumdrop::Options;
use ocy_core::command::RealCommandExecutor;
use ocy_core::filesystem::{FileSystem, RealFileSystem};
use ocy_core::models::FileInfo;
use ocy_core::rule::widest_name;
use ocy_core::walker::{WalkOptions, Walker};
use ocy_core::{cleaner::Cleaner, models::RemovalCandidate};
use rules::{SCANNED_HIDDEN_DIRS, standard_rules};
use std::process::ExitCode;

use notifiers::{LoggingCleanerNotifier, VecWalkNotifier};
use options::OcyOptions;
use utils::{format_file_size_and_more, prompt};

fn main() -> Result<ExitCode> {
    let options = OcyOptions::parse_args_default_or_exit();

    // Silent unless `RUST_LOG` asks for output, so the progress display stays clean in
    // normal use. Diagnostics go to stderr; the progress bar draws there too, so expect
    // the two to interleave while debugging.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

    print_banner();

    if options.version {
        Ok(ExitCode::SUCCESS)
    } else {
        run(&options)
    }
}

fn run(options: &OcyOptions) -> Result<ExitCode> {
    let current_directory = RealFileSystem
        .current_directory()
        .wrap_err("Cannot scan current directory")?;

    let walk_options = WalkOptions {
        ignores: options.ignores_set()?,
        walk_all: options.walk_all,
        scanned_hidden: SCANNED_HIDDEN_DIRS
            .iter()
            .map(|d| (*d).to_string())
            .collect(),
        max_depth: options.max_depth,
        one_file_system: options.one_file_system,
    };

    if walk_options.one_file_system && RealFileSystem.device_id(&current_directory).is_none() {
        // Better to refuse than to let a safety flag silently do nothing.
        eyre::bail!("--one-file-system is not supported on this platform");
    }

    let files = perform_walk(&current_directory, walk_options, options.allow_commands)?;

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
        } else if prompt(&format!("Reclaim {} (y/N) ? ", total.cyan()))? {
            perform_clean(&current_directory, files);
        }
        Ok(ExitCode::SUCCESS)
    }
}

fn perform_walk(
    current_directory: &FileInfo,
    walk_options: WalkOptions,
    allow_commands: bool,
) -> Result<Vec<RemovalCandidate>> {
    let rules = standard_rules(allow_commands)?;
    let notifier = VecWalkNotifier::new(&current_directory.path, widest_name(&rules));
    let walker = Walker::new(RealFileSystem, rules, &notifier, walk_options);

    walker.walk_from_path(current_directory);
    Ok(notifier.to_remove.into_inner())
}

fn perform_clean(current_directory: &FileInfo, files: Vec<RemovalCandidate>) {
    let notifier = LoggingCleanerNotifier::new(&current_directory.path, files.len());
    let cleaner = Cleaner::new(files, RealFileSystem, RealCommandExecutor, &notifier);
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
