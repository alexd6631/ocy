use humansize::{file_size_opts as options, FileSize};
use std::io::Write;

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
