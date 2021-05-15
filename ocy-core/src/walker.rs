use std::{collections::HashSet, path::PathBuf, sync::Arc};

use crate::{
    filesystem::{FileInfo, FileSystem, SimpleFileKind},
    matcher::Matcher,
};
use eyre::Report;
use eyre::Result;

pub struct Walker<FS: FileSystem, N: WalkNotifier> {
    fs: FS,
    matchers: Vec<Matcher>,
    notifier: N,
    ignores: HashSet<PathBuf>,
    walk_all: bool,
}

pub trait WalkNotifier {
    fn notify_entered_directory(&self, dir: &FileInfo);
    fn notify_candidate_for_removal(&self, candidate: RemovalCandidate);
    fn notify_fail_to_scan(&self, e: &FileInfo, report: Report);
    fn notify_walk_finish(&self);
}

impl<FS: FileSystem, N: WalkNotifier> Walker<FS, N> {
    pub fn new(
        fs: FS,
        matchers: Vec<Matcher>,
        notifier: N,
        ignores: HashSet<PathBuf>,
        walk_all: bool,
    ) -> Self {
        Self {
            fs,
            matchers,
            notifier,
            ignores,
            walk_all,
        }
    }

    pub fn walk_from_path(&self, path: &FileInfo) {
        self.process_dir(&path);
        self.notifier.notify_walk_finish();
    }

    fn process_dir(&self, file: &FileInfo) {
        if self.ignores.contains(&file.path) {
            return;
        }
        match self.process_entries(file) {
            Ok(children) => {
                children.iter().for_each(|d| self.process_dir(&d));
            }
            Err(report) => self.notifier.notify_fail_to_scan(file, report),
        }
    }

    fn process_entries(&self, file: &FileInfo) -> Result<Vec<FileInfo>> {
        self.notifier.notify_entered_directory(file);
        let mut entries = self.fs.list_files(&file)?;

        for matcher in &self.matchers {
            entries = self.process_matcher(matcher, entries);
        }
        entries.retain(|f| self.is_walkable(f));
        Ok(entries)
    }

    fn process_matcher(&self, matcher: &Matcher, entries: Vec<FileInfo>) -> Vec<FileInfo> {
        if matcher.any_entry_match(&entries) {
            let (mut to_remove, remaining) = matcher.find_files_to_remove(entries);
            to_remove.retain(|p| !self.ignores.contains(&p.path));
            self.notify_removal_candidates(matcher, to_remove);
            remaining
        } else {
            entries
        }
    }

    fn notify_removal_candidates(&self, matcher: &Matcher, to_remove: Vec<FileInfo>) {
        to_remove
            .into_iter()
            .map(|f| self.removal_candidate(matcher, f))
            .for_each(|c| self.notifier.notify_candidate_for_removal(c));
    }

    fn removal_candidate(&self, matcher: &Matcher, file: FileInfo) -> RemovalCandidate {
        let size = self.fs.file_size(&file).ok();
        RemovalCandidate::new(matcher.name.clone(), file, size)
    }

    fn is_walkable(&self, file: &FileInfo) -> bool {
        file.kind == SimpleFileKind::Directory && (self.walk_all || !file.name.starts_with('.'))
    }
}

#[derive(Debug)]
pub struct RemovalCandidate {
    pub matcher_name: Arc<str>,
    pub file_info: FileInfo,
    pub file_size: Option<u64>,
}

impl RemovalCandidate {
    pub fn new(matcher_name: Arc<str>, file_info: FileInfo, file_size: Option<u64>) -> Self {
        Self {
            matcher_name,
            file_info,
            file_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashSet, path::PathBuf, str::FromStr};

    use glob::Pattern;

    use crate::{
        filesystem::{FileInfo, FileSystem},
        matcher::Matcher,
        test_utils::{MockFS, MockFSNode},
        walker::Walker,
    };

    use super::{RemovalCandidate, WalkNotifier};

    #[derive(Debug, Default)]
    struct VecWalkNotifier {
        pub to_remove: RefCell<Vec<RemovalCandidate>>,
    }

    impl WalkNotifier for &VecWalkNotifier {
        fn notify_entered_directory(&self, _dir: &FileInfo) {}

        fn notify_candidate_for_removal(&self, candidate: RemovalCandidate) {
            self.to_remove.borrow_mut().push(candidate);
        }

        fn notify_fail_to_scan(&self, _e: &FileInfo, _report: eyre::Error) {}

        fn notify_walk_finish(&self) {}
    }

    fn setup_mock_fs() -> MockFS {
        MockFS::new(MockFSNode::dir(
            "/",
            vec![MockFSNode::dir(
                "home",
                vec![MockFSNode::dir(
                    "user",
                    vec![
                        MockFSNode::dir(
                            "projectA",
                            vec![MockFSNode::file("Cargo.toml"), MockFSNode::file("target")],
                        ),
                        MockFSNode::dir("projectB", vec![MockFSNode::file("target")]),
                    ],
                )],
            )],
        ))
    }

    #[test]
    fn test() -> eyre::Result<()> {
        let fs = setup_mock_fs();
        let current_dir = setup_mock_fs().current_directory()?;
        let notifier = VecWalkNotifier::default();
        let walker = Walker::new(
            fs,
            vec![Matcher::new(
                "Cargo".into(),
                Pattern::new("Cargo.toml")?,
                Pattern::new("target")?,
            )],
            &notifier,
            HashSet::new(),
            false,
        );
        walker.walk_from_path(&current_dir);

        let to_remove = notifier.to_remove.into_inner();
        println!("{:?}", to_remove);

        assert_eq!(1, to_remove.len());
        let c = to_remove.into_iter().next().unwrap();
        assert_eq!(c.matcher_name.as_ref(), "Cargo");
        assert_eq!(c.file_info.path, PathBuf::from_str("/home/user/projectA/target").unwrap());

        Ok(())
    }
}
