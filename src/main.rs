mod notifiers;
mod options;
mod palette;
mod rules;
mod utils;

use colored::Colorize;
use eyre::{Context, Result};
use gumdrop::Options;
use ocy_core::command::RealCommandExecutor;
use ocy_core::filesystem::{FileSystem, RealFileSystem};
use ocy_core::models::FileInfo;
use ocy_core::rule::Rule;
use ocy_core::rule::widest_name;
use ocy_core::walker::{WalkOptions, Walker};
use ocy_core::{cleaner::Cleaner, models::RemovalCandidate};
use rules::{SCANNED_HIDDEN_DIRS, rule_names, standard_rules};
use std::process::ExitCode;
use std::sync::Arc;

use notifiers::{LoggingCleanerNotifier, VecWalkNotifier, progress_is_useful};
use options::OcyOptions;
use utils::{format_file_size_and_more, prompt};

fn main() -> Result<ExitCode> {
    let options = OcyOptions::parse_args_default_or_exit();

    init_logging(&options);

    print_banner();

    if options.list_rules {
        print_rules()?;
        Ok(ExitCode::SUCCESS)
    } else if options.version {
        Ok(ExitCode::SUCCESS)
    } else {
        run(&options)
    }
}

fn run(options: &OcyOptions) -> Result<ExitCode> {
    let current_directory = RealFileSystem
        .current_directory()
        .wrap_err("Cannot scan current directory")?;

    let start_dir = options.start_directory()?;
    let start_directory = RealFileSystem
        .directory_from_current(start_dir)
        .wrap_err("Cannot scan start directory")?;

    let rules = rules_for_options(options)?;

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

    let animated = progress_is_useful();
    let files = perform_walk(
        &current_directory,
        &start_directory,
        walk_options,
        rules,
        animated,
    )?;

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
            perform_clean(&current_directory, files, animated);
        }
        Ok(ExitCode::SUCCESS)
    }
}

fn rules_for_options(options: &OcyOptions) -> Result<Vec<Rule>> {
    let all_rules = standard_rules(options.allow_commands)?;
    let all_rule_names = rule_names(&all_rules);

    let selected_rules = options.selected_rules();

    for rule_name in selected_rules {
        if !all_rule_names.contains(rule_name.as_str()) {
            eyre::bail!("Unknown rule: {}", rule_name);
        }
    }

    let filtered_rules = if selected_rules.is_empty() {
        all_rules
    } else {
        all_rules
            .into_iter()
            .filter(|rule| selected_rules.contains(&rule.name.to_string()))
            .collect()
    };
    Ok(filtered_rules)
}

fn perform_walk(
    current_directory: &FileInfo,
    start_directory: &FileInfo,
    walk_options: WalkOptions,
    rules: Vec<Rule>,
    animated: bool,
) -> Result<Vec<RemovalCandidate>> {
    let notifier = VecWalkNotifier::new(&current_directory.path, widest_name(&rules), animated);
    let walker = Walker::new(RealFileSystem, rules, &notifier, walk_options);

    walker.walk_from_path(start_directory);
    Ok(notifier.to_remove.into_inner())
}

fn perform_clean(current_directory: &FileInfo, files: Vec<RemovalCandidate>, animated: bool) {
    let notifier = LoggingCleanerNotifier::new(&current_directory.path, files.len(), animated);
    let cleaner = Cleaner::new(files, RealFileSystem, RealCommandExecutor, &notifier);
    cleaner.clean();
}

fn total_size(files: &[RemovalCandidate]) -> (u64, bool) {
    let estimate = files.iter().map(|e| e.estimate_file_size()).sum();
    let has_more = files.iter().any(|e| e.file_size().is_none());
    (estimate, has_more)
}

/// Start logging, silent unless asked.
///
/// Diagnostics go to stderr, where the progress bar also draws, so the two interleave
/// while debugging.
fn init_logging(options: &OcyOptions) {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off"));

    if let Some(filter) = options.log_filter() {
        builder.filter_level(filter);
    }
    builder.init();
}

fn print_banner() {
    let version = std::env!("CARGO_PKG_VERSION");
    let banner_template = include_str!("../data/banner.txt");
    let banner = banner_template.replace("$VERSION", version);
    println!("{}", banner.yellow());
}

fn print_rules() -> Result<()> {
    let rules = rule_names(&standard_rules(true)?);
    let mut rules: Vec<Arc<str>> = rules.into_iter().collect();
    rules.sort();

    println!("Supported rules:");
    for rule in rules {
        println!(" - {}", rule);
    }
    Ok(())
}
