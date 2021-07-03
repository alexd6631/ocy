use std::{process::Command, thread::sleep, time::Duration};

use crate::models::FileInfo;

use eyre::{Context, Result};

pub trait CommandExecutor {
    fn execute_command(&self, work_dir: &FileInfo, command: &str) -> Result<()>;
}

pub struct MockCommandExecutor;

impl CommandExecutor for MockCommandExecutor {
    fn execute_command(&self, _work_dir: &FileInfo, _command: &str) -> Result<()> {
        sleep(Duration::from_secs(2));
        Ok(())
    }
}

pub struct RealCommandExecutor;

impl CommandExecutor for RealCommandExecutor {
    fn execute_command(&self, work_dir: &FileInfo, command: &str) -> Result<()> {
        let mut iter = command.split_ascii_whitespace();
        let cmd = iter.next().unwrap();

        Command::new(cmd)
            .current_dir(&work_dir.path)
            .args(iter)
            .status()
            .context("Failed to execute command")?;
        Ok(())
    }
}
