use super::{FALLBACK_TINTS, RULE_TINTS, rule_color};
use crate::rules::standard_rules;
use std::collections::HashSet;

#[test]
fn a_name_always_gets_the_same_colour() {
    assert_eq!(rule_color("Cargo"), rule_color("Cargo"));
    assert_eq!(rule_color("Nonesuch"), rule_color("Nonesuch"));
}

/// The three Python rules share a name, so they have to read as one ecosystem.
#[test]
fn rules_sharing_a_name_share_a_colour() {
    assert_eq!(rule_color("Python"), rule_color("Python"));
    assert_ne!(rule_color("Python"), rule_color("Python venv"));
}

#[test]
fn different_ecosystems_get_different_colours() {
    assert_ne!(rule_color("Cargo"), rule_color("NodeJS"));
    assert_ne!(rule_color("Maven"), rule_color("Gradle"));
}

/// Every built-in rule should be curated rather than falling back to an arbitrary tint.
#[test]
fn every_built_in_rule_has_a_curated_tint() -> eyre::Result<()> {
    let curated: HashSet<&str> = RULE_TINTS.iter().map(|(name, _)| *name).collect();

    let uncurated: Vec<String> = standard_rules(true)?
        .iter()
        .map(|rule| rule.name.to_string())
        .filter(|name| !curated.contains(name.as_str()))
        .collect();

    assert!(uncurated.is_empty(), "no tint for {uncurated:?}");
    Ok(())
}

#[test]
fn no_curated_tint_is_listed_twice() {
    let mut names: Vec<&str> = RULE_TINTS.iter().map(|(name, _)| *name).collect();
    let before = names.len();
    names.sort_unstable();
    names.dedup();

    assert_eq!(before, names.len(), "duplicate rule name in the palette");
}

/// Two rules rendering in exactly the same colour would defeat the point.
#[test]
fn curated_tints_are_distinct() {
    let tints: HashSet<(u8, u8, u8)> = RULE_TINTS.iter().map(|(_, tint)| *tint).collect();

    assert_eq!(RULE_TINTS.len(), tints.len(), "duplicate tint");
}

/// Mid-tone only: near-black vanishes on a dark terminal, near-white on a light one.
#[test]
fn every_tint_is_legible_on_either_background() {
    let all = RULE_TINTS
        .iter()
        .map(|(_, tint)| *tint)
        .chain(FALLBACK_TINTS.iter().copied());

    for (r, g, b) in all {
        // Rec. 601 luma, the usual stand-in for perceived brightness.
        let luma = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
        assert!(
            (60.0..=210.0).contains(&luma),
            "rgb({r},{g},{b}) has luma {luma:.0}, outside the legible band"
        );
    }
}

#[test]
fn an_unknown_rule_still_gets_a_colour() {
    let fallback = rule_color("Some User Rule");

    assert!(FALLBACK_TINTS.iter().any(|(r, g, b)| {
        fallback
            == colored::Color::TrueColor {
                r: *r,
                g: *g,
                b: *b,
            }
    }));
}
