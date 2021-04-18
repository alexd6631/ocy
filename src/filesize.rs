use humansize::{file_size_opts as options, FileSize};

pub fn format_file_size(size: u64) -> String {
    size.file_size(options::CONVENTIONAL).unwrap()
}
