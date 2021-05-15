use std::{ffi::OsString, path::Path};

use eyre::ContextCompat;

use crate::filesystem::{FileInfo, FileSystem, SimpleFileKind};

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
    fn get_node(&self, path: &Path) -> Option<&MockFSNode> {
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

    fn list_files(&self, file: &FileInfo) -> eyre::Result<Vec<FileInfo>> {
        let path = &file.path;
        let node = self.get_node(path).wrap_err("Cannot find node")?;
        let files = node
            .children
            .iter()
            .map(|node| {
                let mut new_path = path.to_path_buf();
                new_path.push(&node.name);
                FileInfo::new(
                    new_path,
                    node.name.to_string_lossy().to_string(),
                    if node.children.is_empty() {
                        SimpleFileKind::File
                    } else {
                        SimpleFileKind::Directory
                    },
                )
            })
            .collect();
        Ok(files)
    }

    fn file_size(&self, _file: &FileInfo) -> eyre::Result<u64> {
        Ok(42)
    }
}
