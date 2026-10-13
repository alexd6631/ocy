use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ocy_core::rule::{Rule, RuleError, Target};

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
///
/// Shared tool caches are gated behind `clean_caches`, for a different reason: they are
/// not project output. One belongs to every project on the machine at once, so a scan of
/// one directory reclaiming it would reach far outside what was asked for.
pub fn standard_rules(allow_commands: bool, clean_caches: bool) -> Result<Vec<Rule>, RuleError> {
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
        // Rust. The second rule is what finds a target directory that `CARGO_TARGET_DIR`
        // has renamed or moved away from beside its manifest: cargo stamps every one it
        // creates with both of these, so the directory identifies itself.
        Rule::remove("Cargo", &["Cargo.toml"], &["target"])?,
        Rule::remove_self("Cargo", &["CACHEDIR.TAG", ".rustc_info.json"])?,
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
                // Only ever produced by pointing npm's cache at the project, which is
                // why the name is unambiguous where a bare `cache` would not be.
                ".npm-cache",
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
        // C and C++. A CMake build directory may be called anything -- `build`,
        // `build_debug`, `Linux-x86_64-optimize` -- and is as often a sibling of the
        // source as a child of it, so the sibling rule alone misses most of them.
        // `CMakeCache.txt` is written into the build directory itself, which makes the
        // directory self-marked whatever it was named.
        Rule::remove(
            "CMake",
            &["CMakeLists.txt"],
            &["cmake-build-*", "CMakeFiles"],
        )?,
        Rule::remove_self("CMake", &["CMakeCache.txt"])?,
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
        // WebAssembly. Keyed on `Trunk.toml` rather than on the manifest beside it: a
        // `dist` next to a `Cargo.toml` is as likely to be something the project keeps.
        Rule::remove("Trunk", &["Trunk.toml"], &["dist"])?,
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

    let cache_rules = if clean_caches {
        cache_rules()?
    } else {
        Vec::new()
    };

    Ok(rules
        .into_iter()
        .chain(command_rules)
        .chain(cache_rules)
        .collect())
}

/// Entries under a cache home that exist only to be a cache.
///
/// Every one of these is refetched or rebuilt on demand, so losing it costs time and
/// nothing else. A cache home also holds state that is not reproducible, which is why the
/// list is named out rather than inferred from the shape of what is in there.
///
/// Named generously across platforms: an anchored rule fires on an exact path, so an
/// entry that does not exist here -- `Mozilla.sccache` anywhere but macOS -- simply never
/// matches. That is cheaper and less brittle than encoding each tool's own idea of where
/// its cache belongs on which system.
const CACHE_HOME_ENTRIES: &[&str] = &[
    // Compiler caches.
    "sccache",
    "Mozilla.sccache",
    "ccache",
    "miri",
    // Rust tooling.
    "trunk",
    ".wasm-pack",
    // JavaScript tooling.
    "node-gyp",
    "pnpm",
    "yarn",
    "puppeteer",
    "deno",
    // Python tooling.
    "pip",
    "uv",
    // Go's build cache. Its *module* cache is deliberately absent: Go makes those
    // directories read-only, so removing one fails rather than reclaiming anything.
    // `go clean -modcache` is what handles that.
    "go-build",
];

/// Caches a tool keeps at a fixed place under the home directory instead.
///
/// Each entry names the cache itself rather than the directory holding it. `~/.npm` also
/// holds logs, and `~/.local/share/pnpm` also holds the binaries pnpm has installed --
/// reclaiming either wholesale would take out more than a cache.
const HOME_ENTRIES: &[&str] = &[
    // Where ccache kept its cache before 4.0, and still does when it finds one there.
    ".ccache",
    ".npm/_cacache",
    ".local/share/pnpm/store",
    ".bun/install/cache",
    ".m2/repository",
    ".ivy2/cache",
];

/// Environment variables that move a cache, and what to append to reach the cache itself.
///
/// A tool's own variable is the one thing that reliably says where its cache went;
/// reading its config file would mean a TOML parser for sccache, a second format for
/// ccache and a third for npm, each with its own way of being wrong.
///
/// The default location stays covered either way. A cache left behind at the old path is
/// still a cache, so these only add wherever the tool is being pointed now.
const MOVED_CACHES: &[(&str, &str)] = &[
    ("SCCACHE_DIR", ""),
    ("CCACHE_DIR", ""),
    ("UV_CACHE_DIR", ""),
    ("GOCACHE", ""),
    ("DENO_DIR", ""),
    ("npm_config_cache", "_cacache"),
    ("XDG_DATA_HOME", "pnpm/store"),
];

/// Where apt keeps the packages it has downloaded.
///
/// Fixed rather than read from apt's configuration, for the same reason cache locations
/// are not read from config files: `Dir::Cache` is almost never moved, and a parser for
/// apt's config format would be a lot of code to be wrong in for that case. Like every
/// anchored path, it simply never matches on a system without apt.
const APT_CACHE: &str = "/var/cache/apt";

/// What `apt-get clean` removes, relative to [`APT_CACHE`].
///
/// Every downloaded package, anything a download left half-finished, and the two binary
/// caches apt rebuilds from its package lists on its next run. The `archives/lock` file
/// and the `archives/partial` directory itself stay, as they do under `apt-get clean`.
/// The package lists in `/var/lib/apt/lists` are not a cache in this sense: without them
/// apt knows no packages at all until the next `apt update`.
const APT_CACHE_FILES: &[&str] = &["archives/*.deb", "pkgcache.bin", "srcpkgcache.bin"];
const APT_CACHE_PARTIAL: &str = "archives/partial/*";

/// The rule reclaiming apt's package cache at `anchor`.
///
/// The cache belongs to root, so only a scan run as root can actually remove anything;
/// any other scan still reports what is there, and each removal then fails on its own.
fn apt_cache_rule(anchor: PathBuf) -> Result<Rule, RuleError> {
    const NAME: &str = "APT cache";
    let invalid = |pattern: &str| {
        let pattern = pattern.to_string();
        move |source| RuleError::InvalidPattern {
            name: NAME.to_string(),
            pattern,
            source,
        }
    };

    let mut targets = APT_CACHE_FILES
        .iter()
        .map(|path| Target::file(path).map_err(invalid(path)))
        .collect::<Result<Vec<_>, _>>()?;
    targets.push(Target::any(APT_CACHE_PARTIAL).map_err(invalid(APT_CACHE_PARTIAL))?);

    Rule::remove_targets_at(NAME, anchor, targets)
}

/// Rules for the caches that tools keep once for the whole machine.
///
/// These are anchored to a path rather than matched by markers, because that is what
/// identifies them: a cache is a cache by virtue of where the tool agreed to put it, and
/// nothing inside `~/.cache/sccache` says so. Anchoring also means the rule cannot fire
/// against a project directory that happens to look similar.
///
/// Each location can be moved by an environment variable, and a scan that only knew the
/// default would quietly find nothing on a machine that had moved it.
///
/// An anchored rule is still only reached by walking, so moving a cache outside the tree
/// being scanned puts it out of reach whether or not its variable is set.
fn cache_rules() -> Result<Vec<Rule>, RuleError> {
    let Some(home) = home_directory() else {
        log::warn!("cannot locate the home directory, so no shared cache is known");
        return Ok(Vec::new());
    };

    let cargo = env_dir("CARGO_HOME").unwrap_or_else(|| home.join(".cargo"));
    let gradle = env_dir("GRADLE_USER_HOME").unwrap_or_else(|| home.join(".gradle"));

    let mut rules = [
        // Downloaded crates and the sources unpacked from them. `index` is left alone:
        // it is small, and refetching it stalls the next build of every project at once.
        anchored("Cargo registry", cargo.join("registry"), &["cache", "src"])?,
        // Bare clones of git dependencies and the working copies checked out from them.
        anchored("Cargo git", cargo.join("git"), &["checkouts", "db"])?,
        // Gradle's own `wrapper` directory is left alone: it holds the Gradle
        // distributions themselves, which is an installation rather than a cache.
        anchored("Gradle cache", gradle, &["caches", "daemon"])?,
        anchored("Tool cache", home.clone(), HOME_ENTRIES)?,
        vec![apt_cache_rule(PathBuf::from(APT_CACHE))?],
    ]
    .concat();

    for cache_home in cache_homes(&home) {
        rules.extend(anchored("Tool cache", cache_home, CACHE_HOME_ENTRIES)?);
    }

    for (variable, suffix) in MOVED_CACHES {
        let Some(moved) = env_dir(variable) else {
            continue;
        };
        let moved = if suffix.is_empty() {
            moved
        } else {
            moved.join(suffix)
        };

        match moved_cache_rule("Tool cache", &moved) {
            Some(rule) => rules.push(rule?),
            // A variable naming a filesystem root, or a path that is not valid UTF-8,
            // leaves nothing a rule could reclaim.
            None => log::warn!("ignoring {variable}: {} names no cache", moved.display()),
        }
        if let Some(rule) = linked_cache_rule("Tool cache", &moved) {
            rules.push(rule?);
        }
    }

    Ok(rules)
}

/// The cache homes in use on this platform.
///
/// `XDG_CACHE_HOME` wins where it is set, as it does for every tool that honours it.
/// macOS keeps its own cache directory, and tools there are split over which they use, so
/// both are in play on the same machine.
fn cache_homes(home: &Path) -> Vec<PathBuf> {
    let mut homes = vec![env_dir("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache"))];

    if cfg!(target_os = "macos") {
        homes.push(home.join("Library/Caches"));
    }
    homes
}

/// A rule reclaiming `path` itself, anchored on the directory that holds it.
///
/// [`Rule::remove_at`] reclaims entries *inside* its anchor, so a cache that has been
/// moved somewhere of its own -- with no sibling worth naming -- is expressed by
/// anchoring one level up. Returns [`None`] for a path with no parent or no name.
fn moved_cache_rule(rule: &str, path: &Path) -> Option<Result<Rule, RuleError>> {
    let parent = path.parent()?;
    let name = path.file_name()?.to_str()?;

    Some(Rule::remove_at(rule, parent.to_path_buf(), &[name]))
}

/// Rules reclaiming `entries` under `anchor`, and wherever a symlink has put one of them.
///
/// A cache is often a link to a roomier disk -- `~/.cache/sccache` pointing into `/work`.
/// A scan that reaches the real directory directly has to know it by its real path, so
/// the directory each entry resolves to is named on its own.
///
/// The anchor itself may sit behind a link too -- `~/.cargo/registry` is as easily moved
/// as the cache inside it. The walk does not enter links, so such an anchor is never
/// reached; the rule is restated from just outside the link, where following it is left
/// to the walk.
fn anchored(rule: &str, anchor: PathBuf, entries: &[&str]) -> Result<Vec<Rule>, RuleError> {
    let mut rules = entries
        .iter()
        .filter_map(|entry| linked_cache_rule(rule, &anchor.join(entry)))
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(outside) = outside_link(rule, &anchor, entries) {
        rules.push(outside?);
    }

    rules.insert(0, Rule::remove_at(rule, anchor, entries)?);
    Ok(rules)
}

/// `entries` of `anchor` restated from the directory holding its outermost symlinked
/// ancestor, or [`None`] when no ancestor is a link.
fn outside_link(rule: &str, anchor: &Path, entries: &[&str]) -> Option<Result<Rule, RuleError>> {
    let link = anchor
        .ancestors()
        .filter(|ancestor| {
            ancestor
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.is_symlink())
        })
        .last()?;
    let parent = link.parent()?;
    let prefix = anchor.strip_prefix(parent).ok()?;

    let targets = entries
        .iter()
        .map(|entry| prefix.join(entry).to_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    let targets: Vec<&str> = targets.iter().map(String::as_str).collect();

    Some(Rule::remove_at(rule, parent.to_path_buf(), &targets))
}

/// A rule reclaiming the directory `path` resolves to, when that is somewhere else.
///
/// Returns [`None`] when `path` does not exist, resolves to itself, or is not a directory.
fn linked_cache_rule(rule: &str, path: &Path) -> Option<Result<Rule, RuleError>> {
    let real = path.canonicalize().ok()?;

    if real == path || !real.is_dir() {
        None
    } else {
        moved_cache_rule(rule, &real)
    }
}

/// A directory named by an environment variable, treating an empty setting as unset.
fn env_dir(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The user's home directory, or [`None`] where there is no way to tell.
fn home_directory() -> Option<PathBuf> {
    std::env::home_dir()
}

/// Extract rule names from a slice of rules.
pub fn rule_names(rules: &[Rule]) -> HashSet<Arc<str>> {
    rules.iter().map(|rule| rule.name.clone()).collect()
}

/// A `--rule` value the rule itself is not named, matched ahead of the rule names.
///
/// `Cargo` is the build tool; `Rust` is the language someone scanning their own machine
/// is more likely to type. `apt` is the command everyone knows the cache by.
const RULE_SYNONYMS: &[(&str, &str)] = &[("rust", "Cargo"), ("apt", "APT cache")];

/// Whether a `--rule` value selects `rule_name`.
///
/// Every rule name is matched case-insensitively -- `--rule Cargo`, `--rule cargo` and
/// `--rule CARGO` all select the same rule -- and [`RULE_SYNONYMS`] is consulted first,
/// so `--rule rust` selects `Cargo` as well.
pub fn rule_name_matches(rule_name: &str, requested: &str) -> bool {
    let requested = RULE_SYNONYMS
        .iter()
        .find(|(synonym, _)| synonym.eq_ignore_ascii_case(requested))
        .map_or(requested, |(_, canonical)| *canonical);
    rule_name.eq_ignore_ascii_case(requested)
}

/// Hidden application state under the user's home, kept out of every rule regardless of
/// `--all`.
///
/// Hidden home directories hold installed applications' own settings and data, not build
/// output. An app that happens to bundle a
/// `package.json` beside a `node_modules` -- Discord's own native modules do -- looks
/// exactly like a project to the ordinary rules once `--all` lets the walk reach it, and
/// reclaiming its `node_modules` breaks the installed program rather than freeing
/// anything disposable.
///
/// Cache roots are deliberately excluded here because their contents are handled by the
/// explicit `--caches` rules. This makes unknown application directories safe without
/// requiring a growing list of tool names.
///
/// A location that fails to canonicalize -- nothing has created it yet -- is simply
/// left out: there is nothing under it to protect.
pub fn protected_state_dirs() -> Vec<PathBuf> {
    let Some(home) = home_directory() else {
        log::warn!("cannot locate the home directory, so no XDG state is protected");
        return Vec::new();
    };

    let cache_roots = [
        home.join(".cache"),
        env_dir("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache")),
        env_dir("CARGO_HOME").unwrap_or_else(|| home.join(".cargo")),
        env_dir("GRADLE_USER_HOME").unwrap_or_else(|| home.join(".gradle")),
        home.join(".local"),
        home.join(".npm"),
        home.join(".bun"),
    ];

    let mut protected = std::fs::read_dir(&home)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
        })
        .filter(|path| !cache_roots.iter().any(|root| path == root))
        .filter_map(|path| path.canonicalize().ok())
        .collect::<Vec<_>>();

    protected.extend(
        [
            env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            env_dir("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")),
            env_dir("XDG_STATE_HOME").unwrap_or_else(|| home.join(".local/state")),
        ]
        .into_iter()
        .filter_map(|path| path.canonicalize().ok()),
    );
    protected
}

/// The hidden directories a scan with these rules needs to enter.
///
/// A cache anchor lives behind a leading dot -- `~/.cargo`, `~/.cache` -- so a rule
/// naming one would find nothing unless the walk were told to go in, and `--caches`
/// would appear to do nothing without `--all` beside it. Reading the names off the
/// anchors keeps that working when the environment has moved a cache somewhere else
/// hidden, and adds nothing at all when no cache rule is enabled.
pub fn scanned_hidden(rules: &[Rule]) -> HashSet<String> {
    SCANNED_HIDDEN_DIRS
        .iter()
        .map(|dir| (*dir).to_string())
        .chain(
            rules
                .iter()
                .filter_map(Rule::anchor)
                .flat_map(hidden_components),
        )
        .collect()
}

/// The dotted components of a path, which are the ones the walk has to be told about.
fn hidden_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|name| name.starts_with('.'))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests;
