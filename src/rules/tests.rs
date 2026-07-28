use super::{SCANNED_HIDDEN_DIRS, standard_rules};
use ocy_core::walker::VCS_DIRS;

#[test]
fn the_built_in_rule_set_is_valid() -> eyre::Result<()> {
    assert!(!standard_rules(false)?.is_empty());
    Ok(())
}

#[test]
fn command_rules_are_opt_in() -> eyre::Result<()> {
    let without = standard_rules(false)?.len();
    let with = standard_rules(true)?.len();

    assert_eq!(
        without + 1,
        with,
        "--allow-commands should add the Make rule"
    );
    Ok(())
}

#[test]
fn no_command_rule_is_present_by_default() -> eyre::Result<()> {
    let names: Vec<String> = standard_rules(false)?
        .iter()
        .map(|rule| rule.name.to_string())
        .collect();

    assert!(!names.contains(&"Make".to_string()), "got {names:?}");
    Ok(())
}

/// Descending into version-control metadata is never useful, so the two lists must not
/// contradict each other.
#[test]
fn no_scanned_hidden_directory_is_version_control_metadata() {
    for hidden in SCANNED_HIDDEN_DIRS {
        assert!(
            !VCS_DIRS.contains(hidden),
            "{hidden} is both scanned and skipped"
        );
    }
}

#[test]
fn every_scanned_hidden_directory_is_actually_hidden() {
    for hidden in SCANNED_HIDDEN_DIRS {
        assert!(hidden.starts_with('.'), "{hidden} needs no allow-listing");
    }
}
