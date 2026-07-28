use std::{ffi::OsString, path::Path};

use eyre::ContextCompat;

use crate::filesystem::{DirListing, FileSystem};
use crate::models::{FileInfo, SimpleFileKind};

pub struct MockFs {
    root: MockFsNode,
}

impl MockFs {
    pub fn new(root: MockFsNode) -> Self {
        Self { root }
    }
}

pub struct MockFsNode {
    name: OsString,
    /// Held explicitly rather than inferred from `children`, so that an empty directory
    /// is still a directory. Rules that require a target to be a directory depend on it.
    kind: SimpleFileKind,
    children: Vec<MockFsNode>,
}

impl MockFsNode {
    fn to_file_info(&self, parent: &Path) -> FileInfo {
        let mut new_path = parent.to_path_buf();
        new_path.push(&self.name);
        FileInfo::new(new_path, self.name.to_string_lossy().to_string(), self.kind)
    }

    pub fn file(name: &str) -> Self {
        MockFsNode {
            name: name.into(),
            kind: SimpleFileKind::File,
            children: Vec::new(),
        }
    }

    pub fn dir(name: &str, children: Vec<MockFsNode>) -> Self {
        MockFsNode {
            name: name.into(),
            kind: SimpleFileKind::Directory,
            children,
        }
    }

    /// A directory with no entries, such as a build output directory in a fixture.
    pub fn empty_dir(name: &str) -> Self {
        Self::dir(name, Vec::new())
    }
}

impl MockFs {
    fn node(&self, path: &Path) -> Option<&MockFsNode> {
        let mut current = &self.root;

        for c in path.iter().skip(1) {
            current = current.children.iter().find(|n| n.name == c)?;
        }
        Some(current)
    }
}

impl FileSystem for MockFs {
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
