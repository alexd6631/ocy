use super::{SIZE_COLUMN_WIDTH, format_file_size, format_path_truncate};
use std::path::Path;

/// Every size the tool can print has to fit the column, or it shunts the path column
/// right and the output stops lining up. This is issue #3.
#[test]
fn every_rendered_size_fits_the_size_column() {
    let widest = (0..64)
        .map(|shift| 1_u64 << shift)
        .chain([u64::MAX, 150 * 1024 * 1024, 1023 * 1024 * 1024 + 1024])
        .map(|size| format_file_size(size).chars().count())
        .max()
        .unwrap();

    assert!(
        widest <= SIZE_COLUMN_WIDTH,
        "widest rendered size is {widest} chars, column is {SIZE_COLUMN_WIDTH}"
    );
}

#[test]
fn truncation_keeps_the_tail_of_a_long_path() {
    let long = Path::new("/a").join("b".repeat(200));
    let out = format_path_truncate(Path::new("/nowhere"), &long);

    assert!(out.starts_with("..."));
    assert_eq!(83, out.chars().count());
}

/// Truncation used to slice on a byte offset, which panics mid-character.
#[test]
fn truncation_does_not_split_a_multibyte_character() {
    let long = Path::new("/a").join("é".repeat(120));
    let out = format_path_truncate(Path::new("/nowhere"), &long);

    assert!(out.starts_with("..."));
}

#[test]
fn short_paths_are_left_alone() {
    let out = format_path_truncate(Path::new("/base"), Path::new("/base/proj/target"));

    assert_eq!("proj/target", out);
}
