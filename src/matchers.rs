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

pub fn standard_matchers() -> Vec<Matcher> {
    vec![
        matcher!("Cargo", "Cargo.toml", "target"),
        matcher!("Gradle", "build.gradle", "build"),
        matcher!("GradleKTS", "build.gradle.kts", "build"),
        matcher!("Maven", "pom.xml", "target"),
        matcher!("NodeJS", "*", "node_modules"),
        matcher!("XCode", "*", "DerivedData"),
        matcher!("SBT", "build.sbt", "target"),
        matcher!("SBT", "plugins.sbt", "target"),
        matcher_cmd!("Make", "Makefile", "make clean"),
    ]
}
