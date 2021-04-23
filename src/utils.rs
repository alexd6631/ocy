use humansize::{file_size_opts as options, FileSize};
use std::{io::Write, path::Path};

pub fn format_file_size(size: u64) -> String {
    size.file_size(options::CONVENTIONAL).unwrap()
}

pub fn prompt(message: &str) -> bool {
    print!("{}", message);
    std::io::stdout().flush().unwrap();

    let mut buffer = String::new();
    let stdin = std::io::stdin();
    stdin.read_line(&mut buffer).unwrap();

    buffer.trim().to_ascii_lowercase() == "y"
}

pub fn format_path(p: &Path) -> String {
    p.as_os_str().to_string_lossy().to_string()
}

pub fn format_path_truncate(p: &Path) -> String {
    let mut p = format_path(p);
    let n = p.len();
    if n > 80 {
        p.replace_range(0 .. n - 80, "...");
    }
    p
}