use crate::{filesystem::FileSystemClean, walker::RemovalCandidate};
use eyre::Report;

pub struct Cleaner<CS, FS, N>
where
    CS: IntoIterator<Item = RemovalCandidate>,
    FS: FileSystemClean,
    N: CleanerNotifier,
{
    candidates: CS,
    fs: FS,
    notifier: N,
}

pub trait CleanerNotifier {
    fn notify_removal_started(&self, candidate: &RemovalCandidate);
    fn notify_removal_success(&self, candidate: RemovalCandidate);
    fn notify_removal_failed(&self, candidate: RemovalCandidate, report: Report);
    fn notify_removal_finish(&self);
}

impl<CS, FS, N> Cleaner<CS, FS, N>
where
    CS: IntoIterator<Item = RemovalCandidate>,
    FS: FileSystemClean,
    N: CleanerNotifier,
{
    pub fn new(candidates: CS, fs: FS, notifier: N) -> Self {
        Self {
            candidates,
            fs,
            notifier,
        }
    }

    pub fn clean(self) {
        for candidate in self.candidates.into_iter() {
            self.notifier.notify_removal_started(&candidate);
            match self.fs.remove_file(&candidate.file_info) {
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
