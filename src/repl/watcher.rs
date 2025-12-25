use crossbeam_channel::Sender;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

/// A simple file watcher that runs on a background thread (via notify's internal threads)
/// and sends events to a channel.
pub struct FileWatcher {
    watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Create a new file watcher that sends events to the provided channel
    pub fn new(tx: Sender<notify::Result<Event>>) -> notify::Result<Self> {
        let watcher = notify::recommended_watcher(move |res| {
            // We ignore send errors because it means the receiver was dropped
            let _ = tx.send(res);
        })?;

        Ok(Self { watcher })
    }

    /// Add a path to be watched
    pub fn watch<P: AsRef<Path>>(&mut self, path: P) -> notify::Result<()> {
        self.watcher
            .watch(path.as_ref(), RecursiveMode::NonRecursive)
    }

    /// Remove a path from being watched
    pub fn unwatch<P: AsRef<Path>>(&mut self, path: P) -> notify::Result<()> {
        self.watcher.unwatch(path.as_ref())
    }
}
