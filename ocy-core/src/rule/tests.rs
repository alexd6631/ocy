use super::{Rule, RuleError, Target};
use crate::models::{FileInfo, SimpleFileKind};
use std::path::PathBuf;

fn entries(names: &[&str]) -> Vec<FileInfo> {
    names
        .iter()
        .map(|name| {
            FileInfo::new(
                PathBuf::from("/p").join(name),
                (*name).to_string(),
                SimpleFileKind::Directory,
            )
        })
        .collect()
}

#[test]
fn matches_when_every_marker_is_present() -> Result<(), RuleError> {
    let rule = Rule::remove("Cargo", &["Cargo.toml"], &["target"])?;

    assert!(rule.matches(&entries(&["Cargo.toml", "src", "target"])));
    assert!(!rule.matches(&entries(&["src", "target"])));
    Ok(())
}

#[test]
fn requires_all_markers_not_just_one() -> Result<(), RuleError> {
    let rule = Rule::remove("Unity", &["Assets", "ProjectSettings"], &["Library"])?;

    assert!(rule.matches(&entries(&["Assets", "ProjectSettings", "Library"])));
    assert!(!rule.matches(&entries(&["Assets", "Library"])));
    Ok(())
}

#[test]
fn markers_may_be_globs() -> Result<(), RuleError> {
    let rule = Rule::remove("Gradle", &["build.gradle*"], &["build"])?;

    assert!(rule.matches(&entries(&["build.gradle"])));
    assert!(rule.matches(&entries(&["build.gradle.kts"])));
    assert!(!rule.matches(&entries(&["pom.xml"])));
    Ok(())
}

/// A rule with no markers matches every directory, which for `RemoveSelf` would propose
/// deleting the whole tree.
#[test]
fn rejects_a_rule_without_markers() {
    assert!(matches!(
        Rule::remove("Bad", &[], &["target"]),
        Err(RuleError::NoMarkers { .. })
    ));
    assert!(matches!(
        Rule::remove_self("Bad", &[]),
        Err(RuleError::NoMarkers { .. })
    ));
}

#[test]
fn rejects_a_rule_that_reclaims_nothing() {
    assert!(matches!(
        Rule::remove("Bad", &["Cargo.toml"], &[]),
        Err(RuleError::NoTargets { .. })
    ));
}

#[test]
fn rejects_an_invalid_pattern() {
    assert!(matches!(
        Rule::remove("Bad", &["["], &["target"]),
        Err(RuleError::InvalidPattern { .. })
    ));
}

#[test]
fn splits_a_nested_target_into_components() -> Result<(), glob::PatternError> {
    let target = Target::directory(".angular/cache")?;

    assert_eq!(2, target.components.len());
    assert!(target.components[0].matches(".angular"));
    assert!(target.components[1].matches("cache"));
    Ok(())
}

#[test]
fn a_single_component_target_has_one_component() -> Result<(), glob::PatternError> {
    let target = Target::directory("target")?;

    assert_eq!(1, target.components.len());
    assert_eq!(Some(SimpleFileKind::Directory), target.kind);
    Ok(())
}

#[test]
fn a_file_target_only_accepts_files() -> Result<(), glob::PatternError> {
    assert_eq!(
        Some(SimpleFileKind::File),
        Target::file("CMakeCache.txt")?.kind
    );
    Ok(())
}
