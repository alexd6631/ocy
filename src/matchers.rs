use glob::Pattern;
use ocy_core::matcher::Matcher;

pub fn standard_matchers() -> Vec<Matcher> {
    vec![
        Matcher::new(
            "Cargo".into(),
            Pattern::new("Cargo.toml").unwrap(),
            Pattern::new("target").unwrap(),
        ),
        Matcher::new(
            "Gradle".into(),
            Pattern::new("build.gradle").unwrap(),
            Pattern::new("build").unwrap(),
        ),
        Matcher::new(
            "Maven".into(),
            Pattern::new("pom.xml").unwrap(),
            Pattern::new("target").unwrap(),
        ),
        Matcher::new(
            "NodeJS".into(),
            Pattern::new("*").unwrap(),
            Pattern::new("node_modules").unwrap(),
        ),
        Matcher::new(
            "XCode".into(),
            Pattern::new("*").unwrap(),
            Pattern::new("DerivedData").unwrap(),
        ),
        Matcher::new(
            "SBT".into(),
            Pattern::new("build.sbt").unwrap(),
            Pattern::new("target").unwrap(),
        ),
        Matcher::new(
            "SBT".into(),
            Pattern::new("plugins.sbt").unwrap(),
            Pattern::new("target").unwrap(),
        ),
    ]
}
