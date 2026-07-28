use std::{process::Command, thread::sleep, time::Duration};

use crate::models::FileInfo;

use eyre::{Context, Result, eyre};

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
        let mut parts = command.split_ascii_whitespace();
        let program = parts
            .next()
            .ok_or_else(|| eyre!("refusing to run an empty command"))?;

        let status = Command::new(program)
            .current_dir(&work_dir.path)
            .args(parts)
            .status()
            .with_context(|| format!("failed to spawn `{command}`"))?;

        if status.success() {
            Ok(())
        } else {
            Err(eyre!("`{command}` failed: {status}"))
        }
    }
}

#[cfg(test)]
mod tests;
