use colored::Color;

/// Tints for the rule-name column, leaning on each ecosystem's own colour where that does
/// not collide with a neighbour.
///
/// Mid-tone values throughout: a very dark tint disappears on a dark terminal and a very
/// pale one disappears on a light terminal, and `ocy` cannot know which it is on.
const RULE_TINTS: &[(&str, (u8, u8, u8))] = &[
    ("Cargo", (222, 113, 49)),
    ("Python", (75, 139, 190)),
    ("Python venv", (240, 200, 80)),
    ("NodeJS", (104, 176, 99)),
    ("Angular", (214, 64, 120)),
    ("Gradle", (72, 180, 170)),
    ("Maven", (205, 60, 60)),
    ("SBT", (110, 120, 220)),
    (".NET", (150, 110, 230)),
    ("CMake", (140, 150, 170)),
    ("Zig", (170, 200, 60)),
    ("SwiftPM", (240, 120, 90)),
    ("XCode", (230, 145, 120)),
    ("Elixir", (175, 120, 205)),
    ("Cabal", (190, 130, 160)),
    ("Stack", (165, 140, 195)),
    ("Flutter/Dart", (90, 190, 230)),
    ("Unity", (195, 195, 195)),
    ("Terraform", (150, 90, 225)),
    ("Composer", (125, 160, 110)),
    ("Git worktree", (225, 155, 60)),
    ("Make", (200, 175, 120)),
];

/// Tints used for rules with no curated entry.
///
/// Kept visually distinct from one another so that user-defined rules still separate at a
/// glance, even though which tint a name lands on is arbitrary.
const FALLBACK_TINTS: &[(u8, u8, u8)] = &[
    (120, 190, 200),
    (200, 140, 90),
    (150, 175, 220),
    (185, 165, 105),
    (170, 145, 215),
    (110, 185, 150),
];

/// The colour for a rule's name.
///
/// Keyed on the name rather than the rule, so the several rules sharing a name -- the
/// three Python ones, both .NET ones -- read as one ecosystem in the output.
pub fn rule_color(name: &str) -> Color {
    let (r, g, b) = RULE_TINTS
        .iter()
        .find(|(rule, _)| *rule == name)
        .map(|(_, tint)| *tint)
        .unwrap_or_else(|| FALLBACK_TINTS[fallback_index(name)]);

    Color::TrueColor { r, g, b }
}

/// A stable index into [`FALLBACK_TINTS`], so a given name always gets the same tint.
fn fallback_index(name: &str) -> usize {
    // FNV-1a, for a stable hash across runs. `DefaultHasher` is explicitly not guaranteed
    // to be, and a colour that changed between runs would be worse than no colour.
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });

    (hash % FALLBACK_TINTS.len() as u64) as usize
}

#[cfg(test)]
mod tests;
