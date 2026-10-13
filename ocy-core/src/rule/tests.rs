use super::{Rule, RuleError, Target};
use crate::models::{FileInfo, SimpleFileKind};
use std::path::{Path, PathBuf};

/// The directory the fixture entries live in.
const DIR: &str = "/p";

fn entries(names: &[&str]) -> Vec<FileInfo> {
    names
        .iter()
        .map(|name| {
            FileInfo::new(
                PathBuf::from(DIR).join(name),
                (*name).to_string(),
                SimpleFileKind::Directory,
            )
        })
        .collect()
}

/// Whether the rule applies to the fixture directory holding `names`.
fn matches(rule: &Rule, names: &[&str]) -> bool {
    rule.matches(Path::new(DIR), &entries(names))
}

#[test]
fn matches_when_every_marker_is_present() -> Result<(), RuleError> {
    let rule = Rule::remove("Cargo", &["Cargo.toml"], &["target"])?;

    assert!(matches(&rule, &["Cargo.toml", "src", "target"]));
    assert!(!matches(&rule, &["src", "target"]));
    Ok(())
}

#[test]
fn requires_all_markers_not_just_one() -> Result<(), RuleError> {
    let rule = Rule::remove("Unity", &["Assets", "ProjectSettings"], &["Library"])?;

    assert!(matches(&rule, &["Assets", "ProjectSettings", "Library"]));
    assert!(!matches(&rule, &["Assets", "Library"]));
    Ok(())
}

#[test]
fn markers_may_be_globs() -> Result<(), RuleError> {
    let rule = Rule::remove("Gradle", &["build.gradle*"], &["build"])?;

    assert!(matches(&rule, &["build.gradle"]));
    assert!(matches(&rule, &["build.gradle.kts"]));
    assert!(!matches(&rule, &["pom.xml"]));
    Ok(())
}

/// A cache is identified by where it is, so an anchored rule must ignore everything a
/// look-alike directory elsewhere happens to contain.
#[test]
fn an_anchored_rule_applies_only_to_its_own_directory() -> Result<(), RuleError> {
    let rule = Rule::remove_at("Tool cache", PathBuf::from("/home/u/.cache"), &["sccache"])?;
    let entries = entries(&["sccache"]);

    assert!(rule.matches(Path::new("/home/u/.cache"), &entries));
    assert!(!rule.matches(Path::new("/home/u/project/.cache"), &entries));
    Ok(())
}

/// The anchor is what identifies the directory, so there is nothing to require inside it.
#[test]
fn an_anchored_rule_needs_no_markers() -> Result<(), RuleError> {
    let rule = Rule::remove_at("Cargo git", PathBuf::from("/home/u/.cargo/git"), &["db"])?;

    assert!(rule.matches(Path::new("/home/u/.cargo/git"), &[]));
    assert_eq!(Some(Path::new("/home/u/.cargo/git")), rule.anchor());
    Ok(())
}

/// Anchoring must not depend on the kind of target: a rule reclaiming loose files is
/// just as confined to its own directory as one reclaiming directories.
#[test]
fn an_anchored_file_rule_applies_only_to_its_own_directory() -> Result<(), RuleError> {
    let targets = vec![Target::file("archives/*.deb").expect("valid pattern")];
    let rule = Rule::remove_targets_at("APT cache", PathBuf::from("/var/cache/apt"), targets)?;

    assert!(rule.matches(Path::new("/var/cache/apt"), &[]));
    assert!(!rule.matches(Path::new("/home/u/mirror/apt"), &[]));
    assert_eq!(Some(Path::new("/var/cache/apt")), rule.anchor());
    Ok(())
}

#[test]
fn an_unanchored_rule_reports_no_anchor() -> Result<(), RuleError> {
    assert_eq!(
        None,
        Rule::remove("Cargo", &["Cargo.toml"], &["target"])?.anchor()
    );
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
    assert!(matches!(
        Rule::remove_at("Bad", PathBuf::from("/home/u/.cache"), &[]),
        Err(RuleError::NoTargets { .. })
    ));
    assert!(matches!(
        Rule::remove_targets_at("Bad", PathBuf::from("/var/cache/apt"), Vec::new()),
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
