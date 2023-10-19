use crate::{
    command::CommandExecutor,
    filesystem::FileSystemClean,
    models::{RemovalAction, RemovalCandidate},
};
use eyre::Report;
use eyre::Result;

pub struct Cleaner<CS, CE, FS, N>
where
    CS: IntoIterator<Item = RemovalCandidate>,
    FS: FileSystemClean,
    CE: CommandExecutor,
    N: CleanerNotifier,
{
    candidates: CS,
    fs: FS,
    command_executor: CE,
    notifier: N,
}

pub trait CleanerNotifier {
    fn notify_removal_started(&self, candidate: &RemovalCandidate);
    fn notify_removal_success(&self, candidate: RemovalCandidate);
    fn notify_removal_failed(&self, candidate: RemovalCandidate, report: Report);
    fn notify_removal_finish(&self);
}

impl<CS, CE, FS, N> Cleaner<CS, CE, FS, N>
where
    CS: IntoIterator<Item = RemovalCandidate>,
    FS: FileSystemClean,
    CE: CommandExecutor,
    N: CleanerNotifier,
{
    pub fn new(candidates: CS, fs: FS, command_executor: CE, notifier: N) -> Self {
        Self {
            candidates,
            fs,
            command_executor,
            notifier,
        }
    }

    pub fn clean(self) {
        for candidate in self.candidates {
            self.notifier.notify_removal_started(&candidate);
            match clean_candidate(&self.fs, &self.command_executor, &candidate) {
                Ok(_) => {
                    self.notifier.notify_removal_success(candidate);
                }
                Err(report) => {
                    self.notifier.notify_removal_failed(candidate, report);
                }
            }
        }
        self.notifier.notify_removal_finish();
    }
}

fn clean_candidate(
    fs: &impl FileSystemClean,
    command_executor: &impl CommandExecutor,
    candidate: &RemovalCandidate,
) -> Result<()> {
    match &candidate.action {
        RemovalAction::Delete { file_info, .. } => fs.remove_file(file_info),
        RemovalAction::RunCommand { work_dir, command } => {
            command_executor.execute_command(work_dir, command)
        }
    }
}
