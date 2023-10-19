use std::{path::PathBuf, sync::Arc};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SimpleFileKind {
    File,
    Directory,
}
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub name: String,
    pub kind: SimpleFileKind,
}

impl FileInfo {
    pub(crate) fn new(path: PathBuf, name: String, kind: SimpleFileKind) -> Self {
        Self { path, name, kind }
    }
}

#[derive(Debug)]
pub enum RemovalAction {
    Delete {
        file_info: FileInfo,
        file_size: Option<u64>,
    },
    RunCommand {
        work_dir: FileInfo,
        command: Arc<str>,
    },
}

#[derive(Debug)]
pub struct RemovalCandidate {
    pub matcher_name: Arc<str>,
    pub action: RemovalAction,
}

impl RemovalCandidate {
    pub fn new(matcher_name: Arc<str>, file_info: FileInfo, file_size: Option<u64>) -> Self {
        let action = RemovalAction::Delete {
            file_info,
            file_size,
        };
        Self {
            matcher_name,
            action,
        }
    }

    pub fn new_cmd(matcher_name: Arc<str>, work_dir: FileInfo, command: Arc<str>) -> Self {
        let action = RemovalAction::RunCommand { work_dir, command };
        Self {
            matcher_name,
            action,
        }
    }

    pub fn estimate_file_size(&self) -> u64 {
        match &self.action {
            RemovalAction::Delete { file_size, .. } => file_size.unwrap_or(0),
            RemovalAction::RunCommand { .. } => 0,
        }
    }

    pub fn file_size(&self) -> Option<u64> {
        match &self.action {
            RemovalAction::Delete { file_size, .. } => *file_size,
            RemovalAction::RunCommand { .. } => None,
        }
    }
}
