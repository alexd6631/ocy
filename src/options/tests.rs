use super::{OcyOptions, resolve_ignore};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

fn with_ignores(values: &[&str]) -> OcyOptions {
    OcyOptions {
        ignores: values.iter().map(|v| (*v).to_string()).collect(),
        ..Default::default()
    }
}

/// Resolve `values` against a directory holding `existing`, returning the leaf names.
///
/// Each comma-separated part is qualified with the temp directory, so the values reaching
/// [`OcyOptions::ignores_set`] look exactly like what a user would type.
fn resolved(existing: &[&str], values: &[&str]) -> eyre::Result<HashSet<PathBuf>> {
    let temp = tempfile::tempdir()?;
    for name in existing {
        fs::create_dir_all(temp.path().join(name))?;
    }

    let qualified: Vec<String> = values
        .iter()
        .map(|value| {
            value
                .split(',')
                .map(|part| temp.path().join(part.trim()).display().to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    let refs: Vec<&str> = qualified.iter().map(String::as_str).collect();

    Ok(with_ignores(&refs)
        .ignores_set()?
        .into_iter()
        .map(|path| PathBuf::from(path.file_name().unwrap()))
        .collect())
}

fn names(values: &[&str]) -> HashSet<PathBuf> {
    values.iter().map(PathBuf::from).collect()
}

#[test]
fn accepts_a_repeated_flag() -> eyre::Result<()> {
    assert_eq!(
        names(&["foo", "bar"]),
        resolved(&["foo", "bar"], &["foo", "bar"])?
    );
    Ok(())
}

#[test]
fn accepts_a_comma_separated_list() -> eyre::Result<()> {
    assert_eq!(
        names(&["foo", "bar"]),
        resolved(&["foo", "bar"], &["foo,bar"])?
    );
    Ok(())
}

/// The two forms compose, so neither has to be used exclusively.
#[test]
fn accepts_both_forms_together() -> eyre::Result<()> {
    assert_eq!(
        names(&["foo", "bar", "baz"]),
        resolved(&["foo", "bar", "baz"], &["foo,bar", "baz"])?
    );
    Ok(())
}

/// A comma is legal in a filename and the shell cannot protect one, so an existing path
/// containing a comma has to win over the split reading.
#[test]
fn a_real_path_containing_a_comma_is_not_split() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let comma_named = temp.path().join("foo,bar");
    fs::create_dir_all(&comma_named)?;

    let resolved = resolve_ignore(&comma_named.display().to_string())?;

    assert_eq!(1, resolved.len(), "split a real path: {resolved:?}");
    assert!(resolved[0].ends_with("foo,bar"), "got {resolved:?}");
    Ok(())
}

/// The split reading still applies when no such path exists.
#[test]
fn a_comma_value_that_names_nothing_is_split() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    fs::create_dir_all(temp.path().join("foo"))?;
    fs::create_dir_all(temp.path().join("bar"))?;

    let value = format!(
        "{},{}",
        temp.path().join("foo").display(),
        temp.path().join("bar").display()
    );

    assert_eq!(2, resolve_ignore(&value)?.len());
    Ok(())
}

#[test]
fn ignores_surrounding_whitespace() -> eyre::Result<()> {
    assert_eq!(
        names(&["foo", "bar"]),
        resolved(&["foo", "bar"], &[" foo , bar "])?
    );
    Ok(())
}

#[test]
fn no_flag_yields_nothing() -> eyre::Result<()> {
    assert!(with_ignores(&[]).ignores_set()?.is_empty());
    Ok(())
}

#[test]
fn an_unresolvable_path_is_an_error_not_a_panic() {
    assert!(resolve_ignore("/does/not/exist/anywhere").is_err());
}
