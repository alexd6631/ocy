use eyre::{Context, Result};
use indicatif::HumanBytes;
use std::{io::Write, path::Path};

/// Width of the size column.
///
/// [`HumanBytes`] renders at most four mantissa digits plus two decimals and a
/// three-character unit, so `1023.99 PiB` is the widest possible output.
pub const SIZE_COLUMN_WIDTH: usize = 11;

pub fn format_opt_file_size(size: Option<u64>) -> String {
    if let Some(size) = size {
        format_file_size(size)
    } else {
        "-".to_string()
    }
}

pub fn format_file_size_and_more(size: u64, has_more: bool) -> String {
    let size = format_file_size(size);
    if has_more { format!("{size}+") } else { size }
}

pub fn format_file_size(size: u64) -> String {
    HumanBytes(size).to_string()
}

pub fn prompt(message: &str) -> Result<bool> {
    print!("{message}");
    std::io::stdout()
        .flush()
        .context("cannot write to stdout")?;

    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .context("cannot read from stdin")?;

    Ok(buffer.trim().eq_ignore_ascii_case("y"))
}

pub fn format_path(base_path: &Path, p: &Path) -> String {
    let p = try_relativize_path(base_path, p);
    p.as_os_str().to_string_lossy().to_string()
}

pub fn format_path_truncate(base_path: &Path, p: &Path) -> String {
    let mut p = format_path(base_path, p);
    let n = p.chars().count();
    if n > 80 {
        let cut = p
            .char_indices()
            .nth(n - 80)
            .map(|(i, _)| i)
            .unwrap_or(p.len());
        p.replace_range(0..cut, "...");
    }
    p
}

fn try_relativize_path<'a>(base_path: &'a Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(base_path).unwrap_or(path)
}

#[cfg(test)]
mod tests;
