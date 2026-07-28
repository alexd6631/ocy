use super::{WalkNotifier, WalkOptions, Walker};
use crate::filesystem::FileSystem;
use crate::models::{FileInfo, RemovalAction, RemovalCandidate};
use crate::rule::Rule;
use crate::test_utils::{MockFs, MockFsNode};
use std::{cell::RefCell, collections::HashSet, path::PathBuf};

#[derive(Debug, Default)]
struct VecWalkNotifier {
    to_remove: RefCell<Vec<RemovalCandidate>>,
}

impl WalkNotifier for &VecWalkNotifier {
    fn notify_entered_directory(&self, _dir: &FileInfo) {}

    fn notify_candidate_for_removal(&self, candidate: RemovalCandidate) {
        self.to_remove.borrow_mut().push(candidate);
    }

    fn notify_fail_to_scan(&self, _e: &FileInfo, _report: eyre::Error) {}

    fn notify_walk_finish(&self) {}
}

/// The paths a walk proposes for deletion, sorted for stable comparison.
fn reclaimed(tree: MockFsNode, rules: Vec<Rule>, options: WalkOptions) -> Vec<String> {
    let fs = MockFs::new(tree);
    let current_dir = fs.current_directory().unwrap();
    let notifier = VecWalkNotifier::default();
    let walker = Walker::new(fs, rules, &notifier, options);

    walker.walk_from_path(&current_dir);

    let mut paths: Vec<String> = notifier
        .to_remove
        .into_inner()
        .into_iter()
        .map(|candidate| match candidate.action {
            RemovalAction::Delete { file_info, .. } => file_info.path.display().to_string(),
            RemovalAction::RunCommand { work_dir, command } => {
                format!("{command} in {}", work_dir.path.display())
            }
        })
        .collect();
    paths.sort();
    paths
}

/// Wrap `children` in the `/home/user` prefix that [`MockFs`] scans from.
fn under_home(children: Vec<MockFsNode>) -> MockFsNode {
    MockFsNode::dir(
        "/",
        vec![MockFsNode::dir(
            "home",
            vec![MockFsNode::dir("user", children)],
        )],
    )
}

fn hidden(names: &[&str]) -> WalkOptions {
    WalkOptions {
        scanned_hidden: names.iter().map(|n| (*n).to_string()).collect(),
        ..Default::default()
    }
}

fn cargo_rule() -> Vec<Rule> {
    vec![Rule::remove("Cargo", &["Cargo.toml"], &["target"]).unwrap()]
}

#[test]
fn reclaims_a_target_beside_its_marker() -> eyre::Result<()> {
    let tree = under_home(vec![
        MockFsNode::dir(
            "projectA",
            vec![
                MockFsNode::file("Cargo.toml"),
                MockFsNode::empty_dir("target"),
            ],
        ),
        // No Cargo.toml, so this target is not evidence of a Cargo project.
        MockFsNode::dir("projectB", vec![MockFsNode::empty_dir("target")]),
    ]);

    let found = reclaimed(tree, cargo_rule(), WalkOptions::default());

    assert_eq!(vec!["/home/user/projectA/target"], found);
    Ok(())
}

/// A nested target reclaims only the inner directory, not its parent.
#[test]
fn reclaims_a_nested_target() -> eyre::Result<()> {
    let tree = under_home(vec![MockFsNode::dir(
        "app",
        vec![
            MockFsNode::file("angular.json"),
            MockFsNode::dir(
                ".angular",
                vec![MockFsNode::dir("cache", vec![MockFsNode::file("blob")])],
            ),
        ],
    )]);

    let found = reclaimed(
        tree,
        vec![Rule::remove(
            "Angular",
            &["angular.json"],
            &[".angular/cache"],
        )?],
        hidden(&[".angular"]),
    );

    assert_eq!(vec!["/home/user/app/.angular/cache"], found);
    Ok(())
}

/// A venv is identified by the `pyvenv.cfg` it contains -- issue #6. No sibling-only rule
/// can express that, which is why `RemoveSelf` exists.
#[test]
fn reclaims_a_self_marked_directory() -> eyre::Result<()> {
    let tree = under_home(vec![MockFsNode::dir(
        "ml",
        vec![
            MockFsNode::file("main.py"),
            MockFsNode::dir(
                ".venv",
                vec![
                    MockFsNode::file("pyvenv.cfg"),
                    MockFsNode::dir("lib", vec![MockFsNode::file("torch")]),
                ],
            ),
        ],
    )]);

    let found = reclaimed(
        tree,
        vec![Rule::remove_self("Python venv", &["pyvenv.cfg"])?],
        hidden(&[".venv"]),
    );

    assert_eq!(vec!["/home/user/ml/.venv"], found);
    Ok(())
}

/// Running ocy from inside a venv must not offer to delete the directory being scanned.
#[test]
fn never_reclaims_the_scan_root_itself() -> eyre::Result<()> {
    let tree = under_home(vec![
        MockFsNode::file("pyvenv.cfg"),
        MockFsNode::dir("lib", vec![MockFsNode::file("torch")]),
    ]);

    let found = reclaimed(
        tree,
        vec![Rule::remove_self("Python venv", &["pyvenv.cfg"])?],
        WalkOptions::default(),
    );

    assert!(
        found.is_empty(),
        "proposed deleting the scan root: {found:?}"
    );
    Ok(())
}

#[test]
fn skips_hidden_directories_that_are_not_allow_listed() -> eyre::Result<()> {
    let tree = || {
        under_home(vec![MockFsNode::dir(
            ".secret",
            vec![MockFsNode::dir(
                "proj",
                vec![
                    MockFsNode::file("Cargo.toml"),
                    MockFsNode::empty_dir("target"),
                ],
            )],
        )])
    };

    assert!(reclaimed(tree(), cargo_rule(), WalkOptions::default()).is_empty());

    let with_all = WalkOptions {
        walk_all: true,
        ..Default::default()
    };
    assert_eq!(
        vec!["/home/user/.secret/proj/target"],
        reclaimed(tree(), cargo_rule(), with_all)
    );
    Ok(())
}

/// A worktree under the conventional `.worktrees` directory is real build output and must
/// be found without needing the blanket `--all`.
#[test]
fn finds_artifacts_in_an_allow_listed_hidden_directory() -> eyre::Result<()> {
    let tree = under_home(vec![MockFsNode::dir(
        ".worktrees",
        vec![MockFsNode::dir(
            "feature",
            vec![
                MockFsNode::file("Cargo.toml"),
                MockFsNode::empty_dir("target"),
            ],
        )],
    )]);

    let found = reclaimed(tree, cargo_rule(), hidden(&[".worktrees"]));

    assert_eq!(vec!["/home/user/.worktrees/feature/target"], found);
    Ok(())
}

/// `.git` holds no build output and is skipped even under `walk_all`.
#[test]
fn never_descends_into_version_control_metadata() -> eyre::Result<()> {
    let tree = under_home(vec![MockFsNode::dir(
        ".git",
        vec![MockFsNode::dir(
            "modules",
            vec![
                MockFsNode::file("Cargo.toml"),
                MockFsNode::empty_dir("target"),
            ],
        )],
    )]);

    let options = WalkOptions {
        walk_all: true,
        ..Default::default()
    };
    let found = reclaimed(tree, cargo_rule(), options);

    assert!(found.is_empty(), "walked into .git: {found:?}");
    Ok(())
}

#[test]
fn honours_max_depth() -> eyre::Result<()> {
    let tree = || {
        under_home(vec![MockFsNode::dir(
            "a",
            vec![MockFsNode::dir(
                "b",
                vec![
                    MockFsNode::file("Cargo.toml"),
                    MockFsNode::empty_dir("target"),
                ],
            )],
        )])
    };

    let shallow = WalkOptions {
        max_depth: Some(1),
        ..Default::default()
    };
    assert!(reclaimed(tree(), cargo_rule(), shallow).is_empty());

    let deep = WalkOptions {
        max_depth: Some(2),
        ..Default::default()
    };
    assert_eq!(
        vec!["/home/user/a/b/target"],
        reclaimed(tree(), cargo_rule(), deep)
    );
    Ok(())
}

#[test]
fn honours_ignores() -> eyre::Result<()> {
    let tree = under_home(vec![MockFsNode::dir(
        "projectA",
        vec![
            MockFsNode::file("Cargo.toml"),
            MockFsNode::empty_dir("target"),
        ],
    )]);

    let options = WalkOptions {
        ignores: HashSet::from([PathBuf::from("/home/user/projectA/target")]),
        ..Default::default()
    };
    let found = reclaimed(tree, cargo_rule(), options);

    assert!(found.is_empty(), "reclaimed an ignored path: {found:?}");
    Ok(())
}

#[test]
fn reports_a_command_rule_against_its_directory() -> eyre::Result<()> {
    let tree = under_home(vec![MockFsNode::dir(
        "c-proj",
        vec![MockFsNode::file("Makefile")],
    )]);

    let found = reclaimed(
        tree,
        vec![Rule::run("Make", &["Makefile"], "make clean")?],
        WalkOptions::default(),
    );

    assert_eq!(vec!["make clean in /home/user/c-proj"], found);
    Ok(())
}

/// Rules are applied in order, so a nested target can be claimed before the parent that
/// encloses it. Reporting both would double-count the bytes and race the two deletions.
#[test]
fn never_claims_a_path_inside_another_candidate() -> eyre::Result<()> {
    let tree = under_home(vec![MockFsNode::dir(
        "app",
        vec![
            MockFsNode::file("angular.json"),
            MockFsNode::dir(".angular", vec![MockFsNode::empty_dir("cache")]),
        ],
    )]);

    let found = reclaimed(
        tree,
        vec![
            Rule::remove("Angular cache", &["angular.json"], &[".angular/cache"])?,
            Rule::remove("Angular", &["angular.json"], &[".angular"])?,
        ],
        WalkOptions::default(),
    );

    assert_eq!(vec!["/home/user/app/.angular/cache"], found);
    Ok(())
}

/// Walk a real temporary tree, rather than [`MockFs`], and return the reclaimed paths
/// relative to the root.
fn reclaimed_on_disk(
    root: &std::path::Path,
    rules: Vec<Rule>,
    options: WalkOptions,
) -> Vec<String> {
    use crate::filesystem::RealFileSystem;
    use crate::models::SimpleFileKind;

    let start = FileInfo::new(root.to_path_buf(), String::new(), SimpleFileKind::Directory);
    let notifier = VecWalkNotifier::default();
    let walker = Walker::new(RealFileSystem, rules, &notifier, options);

    walker.walk_from_path(&start);

    let mut paths: Vec<String> = notifier
        .to_remove
        .into_inner()
        .into_iter()
        .filter_map(|candidate| match candidate.action {
            RemovalAction::Delete { file_info, .. } => Some(
                file_info
                    .path
                    .strip_prefix(root)
                    .unwrap_or(&file_info.path)
                    .display()
                    .to_string(),
            ),
            RemovalAction::RunCommand { .. } => None,
        })
        .collect();
    paths.sort();
    paths
}

/// Build a repository whose linked worktree sits under a hidden directory that is not
/// allow-listed, so only the git record can lead the walk to it.
fn repo_with_hidden_worktree(root: &std::path::Path) -> std::io::Result<()> {
    let repo = root.join("repo");
    let worktree = repo.join(".claude").join("worktrees").join("feat");
    std::fs::create_dir_all(repo.join("target"))?;
    std::fs::create_dir_all(worktree.join("target"))?;
    std::fs::write(repo.join("Cargo.toml"), "")?;
    std::fs::write(worktree.join("Cargo.toml"), "")?;
    std::fs::write(worktree.join(".git"), "gitdir: elsewhere\n")?;

    let record = repo.join(".git").join("worktrees").join("feat");
    std::fs::create_dir_all(&record)?;
    std::fs::write(
        record.join("gitdir"),
        format!("{}\n", worktree.join(".git").display()),
    )
}

/// Where a worktree lives is local convention, and it is routinely hidden. The walk has
/// to reach it via the git record rather than by guessing the directory name.
#[test]
fn follows_a_linked_worktree_into_an_unlisted_hidden_directory() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    repo_with_hidden_worktree(temp.path())?;

    let rules = vec![
        Rule::remove("Cargo", &["Cargo.toml"], &["target"])?,
        Rule::prune_stale_worktrees("Git worktree", &[".git"])?,
    ];
    let found = reclaimed_on_disk(temp.path(), rules, WalkOptions::default());

    assert_eq!(
        vec!["repo/.claude/worktrees/feat/target", "repo/target"],
        found
    );
    Ok(())
}

/// A worktree reachable both by descent and by its git record must be reported once, or
/// its bytes are counted twice in the total.
#[test]
fn a_visible_worktree_is_not_scanned_twice() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let repo = temp.path().join("repo");
    let worktree = repo.join("visible");
    std::fs::create_dir_all(worktree.join("target"))?;
    std::fs::write(repo.join("Cargo.toml"), "")?;
    std::fs::write(worktree.join("Cargo.toml"), "")?;
    std::fs::write(worktree.join(".git"), "gitdir: elsewhere\n")?;

    let record = repo.join(".git").join("worktrees").join("visible");
    std::fs::create_dir_all(&record)?;
    std::fs::write(
        record.join("gitdir"),
        format!("{}\n", worktree.join(".git").display()),
    )?;

    let rules = vec![
        Rule::remove("Cargo", &["Cargo.toml"], &["target"])?,
        Rule::prune_stale_worktrees("Git worktree", &[".git"])?,
    ];
    let found = reclaimed_on_disk(temp.path(), rules, WalkOptions::default());

    assert_eq!(vec!["repo/visible/target"], found);
    Ok(())
}
