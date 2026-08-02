use std::collections::HashSet;
use std::sync::Arc;

use ocy_core::rule::{Rule, RuleError};

/// Hidden directories the walk descends into by default.
///
/// Only directories that can *contain* something reclaimable need listing. A hidden
/// directory that is itself a target -- `.next`, `.gradle`, `.terraform` -- is claimed by
/// its own rule without ever being descended into, and a nested target such as
/// `.angular/cache` is resolved directly rather than walked.
/// Registered git worktrees do not need listing here whatever they are called: the walk
/// follows each repository's own records to its checkouts. This list is only for hidden
/// directories that nothing else can lead the walk to.
pub const SCANNED_HIDDEN_DIRS: &[&str] = &[
    // Python virtual environments are self-marked, so the walk has to look inside.
    ".venv",
    // Checkouts parked here by hand, which have no git record to follow.
    ".worktrees",
];

/// The built-in rule set.
///
/// Command rules are gated behind `allow_commands`: a rule such as `make clean` runs a
/// script the scanned directory controls, so enabling it for an ordinary scan would
/// execute arbitrary code from any tree that happens to contain a `Makefile`.
pub fn standard_rules(allow_commands: bool) -> Result<Vec<Rule>, RuleError> {
    // Alternative markers are separate rules, because a rule requires all of its markers.
    const PYTHON_CACHES: &[&str] = &[
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
        ".tox",
        ".nox",
        ".hypothesis",
        "build",
        "dist",
    ];
    const DOTNET_OUTPUT: &[&str] = &["bin", "obj"];

    let rules = vec![
        // Rust.
        Rule::remove("Cargo", &["Cargo.toml"], &["target"])?,
        // JVM. The glob covers both the Groovy and Kotlin build scripts.
        Rule::remove("Gradle", &["build.gradle*"], &["build", ".gradle"])?,
        Rule::remove("Maven", &["pom.xml"], &["target"])?,
        Rule::remove("SBT", &["build.sbt"], &["target", "project/target"])?,
        // JavaScript and TypeScript.
        Rule::remove(
            "NodeJS",
            &["package.json"],
            &[
                "node_modules",
                ".next",
                ".nuxt",
                ".turbo",
                ".svelte-kit",
                ".parcel-cache",
            ],
        )?,
        Rule::remove("Angular", &["angular.json"], &[".angular/cache"])?,
        // Python. A virtual environment is identified by the `pyvenv.cfg` it contains,
        // which is why it needs a self-marked rule rather than a sibling one.
        Rule::remove_self("Python venv", &["pyvenv.cfg"])?,
        Rule::remove("Python", &["*.py"], &["__pycache__"])?,
        Rule::remove("Python", &["pyproject.toml"], PYTHON_CACHES)?,
        Rule::remove("Python", &["setup.py"], PYTHON_CACHES)?,
        // .NET.
        Rule::remove(".NET", &["*.csproj"], DOTNET_OUTPUT)?,
        Rule::remove(".NET", &["*.fsproj"], DOTNET_OUTPUT)?,
        // C and C++.
        Rule::remove(
            "CMake",
            &["CMakeLists.txt"],
            &["cmake-build-*", "CMakeFiles"],
        )?,
        // Zig.
        Rule::remove(
            "Zig",
            &["build.zig"],
            &[".zig-cache", "zig-cache", "zig-out"],
        )?,
        // Swift and Objective-C.
        Rule::remove("SwiftPM", &["Package.swift"], &[".build"])?,
        Rule::remove("XCode", &["*.xcodeproj"], &["DerivedData"])?,
        // Elixir.
        Rule::remove("Elixir", &["mix.exs"], &["_build", "deps"])?,
        // Haskell.
        Rule::remove("Cabal", &["*.cabal"], &["dist-newstyle"])?,
        Rule::remove("Stack", &["stack.yaml"], &[".stack-work"])?,
        // Dart and Flutter.
        Rule::remove("Flutter/Dart", &["pubspec.yaml"], &["build", ".dart_tool"])?,
        // Unity, which needs both markers to avoid matching any directory named `Assets`.
        Rule::remove(
            "Unity",
            &["Assets", "ProjectSettings"],
            &["Library", "Temp", "Obj", "Logs"],
        )?,
        // Infrastructure.
        Rule::remove("Terraform", &["*.tf"], &[".terraform"])?,
        // PHP.
        Rule::remove("Composer", &["composer.json"], &["vendor"])?,
        // Git worktree records left behind by a deleted checkout.
        Rule::prune_stale_worktrees("Git worktree", &[".git"])?,
    ];

    let command_rules = allow_commands
        .then(|| Rule::run("Make", &["Makefile"], "make clean"))
        .transpose()?
        .into_iter();

    Ok(rules.into_iter().chain(command_rules).collect())
}

/// Extract rule names from a slice of rules.
pub fn rule_names(rules: &[Rule]) -> HashSet<Arc<str>> {
    rules.iter().map(|rule| rule.name.clone()).collect()
}

#[cfg(test)]
mod tests;
