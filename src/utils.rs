use indicatif::HumanBytes;
use std::{io::Write, path::Path};

pub fn format_file_size(size: u64) -> String {
    HumanBytes(size).to_string()
}

pub fn prompt(message: &str) -> bool {
    print!("{}", message);
    std::io::stdout().flush().unwrap();

    let mut buffer = String::new();
    let stdin = std::io::stdin();
    stdin.read_line(&mut buffer).unwrap();

    buffer.trim().to_ascii_lowercase() == "y"
}

pub fn format_path(base_path: &Path, p: &Path) -> String {
    let p = try_relativize_path(base_path, p);
    p.as_os_str().to_string_lossy().to_string()
}

pub fn format_path_truncate(base_path: &Path, p: &Path) -> String {
    let mut p = format_path(base_path, p);
    let n = p.len();
    if n > 80 {
        p.replace_range(0..n - 80, "...");
    }
    p
}

fn try_relativize_path<'a>(base_path: &'a Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(base_path).unwrap_or(path)
}
