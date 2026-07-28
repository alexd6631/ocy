use std::{ffi::OsString, path::Path};

use eyre::ContextCompat;

use crate::filesystem::{DirListing, FileSystem};
use crate::models::{FileInfo, SimpleFileKind};

pub struct MockFS {
    root: MockFSNode,
}

impl MockFS {
    pub fn new(root: MockFSNode) -> Self {
        Self { root }
    }
}

pub struct MockFSNode {
    name: OsString,
    children: Vec<MockFSNode>,
}

impl MockFSNode {
    fn file_kind(&self) -> SimpleFileKind {
        if self.children.is_empty() {
            SimpleFileKind::File
        } else {
            SimpleFileKind::Directory
        }
    }

    fn to_file_info(&self, parent: &Path) -> FileInfo {
        let mut new_path = parent.to_path_buf();
        new_path.push(&self.name);
        FileInfo::new(
            new_path,
            self.name.to_string_lossy().to_string(),
            self.file_kind(),
        )
    }
}

impl MockFSNode {
    pub fn file(name: &str) -> Self {
        MockFSNode {
            name: name.into(),
            children: Vec::new(),
        }
    }

    pub fn dir(name: &str, children: Vec<MockFSNode>) -> Self {
        MockFSNode {
            name: name.into(),
            children,
        }
    }
}

impl MockFS {
    fn node(&self, path: &Path) -> Option<&MockFSNode> {
        let mut current = &self.root;

        for c in path.iter().skip(1) {
            current = current.children.iter().find(|n| n.name == c)?;
        }
        Some(current)
    }
}

impl FileSystem for MockFS {
    fn current_directory(&self) -> eyre::Result<FileInfo> {
        Ok(FileInfo::new(
            "/home/user".into(),
            "user".to_string(),
            SimpleFileKind::Directory,
        ))
    }

    fn list_files(&self, file: &FileInfo) -> eyre::Result<DirListing> {
        let path = &file.path;
        let node = self.node(path).wrap_err("Cannot find node")?;
        let entries = node
            .children
            .iter()
            .map(|node| node.to_file_info(path))
            .collect();
        Ok(DirListing {
            entries,
            errors: Vec::new(),
        })
    }

    fn file_size(&self, _file: &FileInfo) -> eyre::Result<u64> {
        Ok(42)
    }
}
