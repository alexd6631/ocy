use std::{cell::RefCell, collections::HashSet, path::PathBuf, str::FromStr};

use glob::Pattern;

use crate::{
    filesystem::FileSystem,
    matcher::Matcher,
    models::FileInfo,
    test_utils::{MockFS, MockFSNode},
    walker::Walker,
};

use super::WalkNotifier;
use crate::models::{RemovalAction, RemovalCandidate};

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
        vec![Matcher::with_remove_strategy(
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

    assert_eq!(1, to_remove.len());
    let c = to_remove.into_iter().next().unwrap();
    assert_eq!(c.matcher_name.as_ref(), "Cargo");

    match c.action {
        RemovalAction::Delete { file_info, .. } => {
            assert_eq!(
                file_info.path,
                PathBuf::from_str("/home/user/projectA/target").unwrap()
            )
        }
        RemovalAction::RunCommand { .. } => panic!("should be delete"),
    }

    Ok(())
}
