use super::{CommandExecutor, RealCommandExecutor};
use crate::models::{FileInfo, SimpleFileKind};

fn work_dir(temp: &tempfile::TempDir) -> FileInfo {
    FileInfo::new(
        temp.path().to_path_buf(),
        "work".to_string(),
        SimpleFileKind::Directory,
    )
}

#[test]
fn reports_success_for_a_command_that_succeeds() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    RealCommandExecutor.execute_command(&work_dir(&temp), "true")?;
    Ok(())
}

/// A clean step that fails must not be reported as a successful reclaim.
#[test]
fn reports_failure_for_a_non_zero_exit() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let result = RealCommandExecutor.execute_command(&work_dir(&temp), "false");

    assert!(result.is_err(), "non-zero exit was reported as success");
    Ok(())
}

#[test]
fn reports_failure_when_the_program_does_not_exist() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let result =
        RealCommandExecutor.execute_command(&work_dir(&temp), "ocy-no-such-program-exists");

    assert!(result.is_err());
    Ok(())
}

#[test]
fn refuses_an_empty_command() -> eyre::Result<()> {
    let temp = tempfile::tempdir()?;
    let result = RealCommandExecutor.execute_command(&work_dir(&temp), "   ");

    assert!(result.is_err(), "empty command should not panic or succeed");
    Ok(())
}
