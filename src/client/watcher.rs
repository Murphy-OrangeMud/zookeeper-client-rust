use tokio::sync::watch;

use crate::chroot::OwnedChroot;
use crate::error::Error;
use crate::session::{OneshotReceiver, PersistentReceiver, SessionState, WatchReceiver, WatchedEvent};

/// StateWatcher tracks session state updates.
#[derive(Clone, Debug)]
pub struct StateWatcher {
    receiver: watch::Receiver<SessionState>,
}

impl StateWatcher {
    pub(super) fn new(receiver: watch::Receiver<SessionState>) -> StateWatcher {
        StateWatcher { receiver }
    }

    /// Returns and consumes most recently state.
    pub fn state(&mut self) -> SessionState {
        let state = self.receiver.borrow_and_update();
        *state
    }

    /// Waits until state changed and returns consumed state.
    ///
    /// This method will block indefinitely after one of terminal states consumed.
    pub async fn changed(&mut self) -> SessionState {
        if self.receiver.changed().await.is_err() {
            // Terminal state must be deliveried.
            std::future::pending().await
        }
        self.state()
    }

    /// Returns but not consumes most recently state.
    pub fn peek_state(&self) -> SessionState {
        let state = self.receiver.borrow();
        *state
    }
}

/// Watcher for stat, data and child event.
#[derive(Debug)]
pub struct OneshotWatcher {
    chroot: OwnedChroot,
    receiver: OneshotReceiver,
}

impl OneshotWatcher {
    fn new(chroot: OwnedChroot, receiver: OneshotReceiver) -> Self {
        OneshotWatcher { chroot, receiver }
    }

    /// Waits for node event or session broken.
    pub async fn changed(self) -> WatchedEvent {
        let mut event = self.receiver.recv().await;
        event.drain_root_path(self.chroot.root());
        event
    }

    /// Removes this watcher.
    pub async fn remove(self) -> Result<(), Error> {
        self.receiver.remove().await
    }
}

/// Watcher for persistent and recursive watch.
#[derive(Debug)]
pub struct PersistentWatcher {
    chroot: OwnedChroot,
    receiver: PersistentReceiver,
}

impl PersistentWatcher {
    fn new(chroot: OwnedChroot, receiver: PersistentReceiver) -> Self {
        PersistentWatcher { chroot, receiver }
    }

    /// Waits for next event which could be node event or session activities.
    ///
    /// # Panics
    /// Panic after terminal session event received.
    pub async fn changed(&mut self) -> WatchedEvent {
        let mut event = self.receiver.recv().await;
        event.drain_root_path(self.chroot.root());
        event
    }

    /// Removes this watcher.
    ///
    /// # Cautions
    /// It is a best effect as ZooKeper ([ZOOKEEPER-4472][]) does not support persistent watch
    /// removing individually.
    ///
    /// [ZOOKEEPER-4472]: https://issues.apache.org/jira/browse/ZOOKEEPER-4472
    pub async fn remove(self) -> Result<(), Error> {
        self.receiver.remove().await
    }
}

impl WatchReceiver {
    pub fn into_oneshot(self, chroot: &OwnedChroot) -> OneshotWatcher {
        match self {
            WatchReceiver::None => unreachable!("expect oneshot watcher, got none watcher"),
            WatchReceiver::Oneshot(receiver) => OneshotWatcher::new(chroot.clone(), receiver),
            WatchReceiver::Persistent(_) => {
                unreachable!("expect oneshot watcher, got persistent watcher")
            },
        }
    }

    pub fn into_persistent(self, chroot: &OwnedChroot) -> PersistentWatcher {
        match self {
            WatchReceiver::None => unreachable!("expect oneshot watcher, got none watcher"),
            WatchReceiver::Oneshot(_) => {
                unreachable!("expect oneshot watcher, got oneshot watcher")
            },
            WatchReceiver::Persistent(receiver) => PersistentWatcher::new(chroot.clone(), receiver),
        }
    }
}
