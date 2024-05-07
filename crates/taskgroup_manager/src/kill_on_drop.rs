use std::{
    ops::{Deref, DerefMut},
    process::Child,
};

/// Returns a wrapper which kills the child when it goes out of scope.
///
/// This should be used instead of kill_gently_on_drop for holochain and lair,
/// because they don't seem to exit if given SIGTERM.
pub fn kill_on_drop(child: Child) -> KillChildOnDrop {
    KillChildOnDrop {
        child,
        gentle: false,
    }
}

/// Returns a wrapper which kills the child when it goes out of scope.
///
/// On Unix, this uses SIGTERM instead of SIGKILL to allow the child to clean things up.
pub fn kill_gently_on_drop(child: Child) -> KillChildOnDrop {
    KillChildOnDrop {
        child,
        gentle: true,
    }
}

#[derive(Debug)]
pub struct KillChildOnDrop {
    child: Child,
    gentle: bool,
}

impl Drop for KillChildOnDrop {
    fn drop(&mut self) {
        if self.gentle && cfg!(unix) {
            use nix::sys::signal::{self, Signal};
            use nix::unistd::Pid;
            // Make sure to send SIGTERM, so that the child can run any cleanup logic
            let pid = Pid::from_raw(self.id().try_into().expect("PID is smaller than i32::MAX"));
            let _ = signal::kill(pid, Signal::SIGTERM);
        } else {
            let _ = self.kill();
        }

        let _ = self.wait();
    }
}

impl Deref for KillChildOnDrop {
    type Target = Child;
    fn deref(&self) -> &Self::Target {
        &self.child
    }
}
impl DerefMut for KillChildOnDrop {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}
