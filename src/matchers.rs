use glob::Pattern;
use ocy_core::matcher::Matcher;

macro_rules! matcher {
    ($name: expr, $to_match: expr, $to_remove: expr) => {
        Matcher::with_remove_strategy(
            $name.into(),
            Pattern::new($to_match).unwrap(),
            Pattern::new($to_remove).unwrap(),
        )
    };
}

macro_rules! matcher_cmd {
    ($name: expr, $to_match: expr, $cmd: expr) => {
        Matcher::with_command_strategy(
            $name.into(),
            Pattern::new($to_match).unwrap(),
            $cmd.to_string(),
        )
    };
}

/// The built-in rule set.
///
/// Command rules are gated behind `allow_commands`. A rule such as `make clean` runs a
/// script that the scanned directory controls, so enabling it for an ordinary scan would
/// execute arbitrary code from any tree that happens to contain a `Makefile`.
pub fn standard_matchers(allow_commands: bool) -> Vec<Matcher> {
    let command_matchers = allow_commands
        .then(|| matcher_cmd!("Make", "Makefile", "make clean"))
        .into_iter();

    [
        matcher!("Cargo", "Cargo.toml", "target"),
        matcher!("Gradle", "build.gradle", "build"),
        matcher!("GradleKTS", "build.gradle.kts", "build"),
        matcher!("Maven", "pom.xml", "target"),
        matcher!("NodeJS", "package.json", "node_modules"),
        matcher!("XCode", "*.xcodeproj", "DerivedData"),
        matcher!("SBT", "build.sbt", "target"),
        matcher!("SBT", "plugins.sbt", "target"),
        matcher!("Flutter/Dart", "pubspec.yaml", "build"),
    ]
    .into_iter()
    .chain(command_matchers)
    .collect()
}

/// The width of the widest rule name, for column alignment.
pub fn widest_name(matchers: &[Matcher]) -> usize {
    matchers
        .iter()
        .map(|m| m.name.chars().count())
        .max()
        .unwrap_or(0)
}
