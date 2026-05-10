//! SIGINT / Ctrl+C handling.
//!
//! Two distinct, mutually exclusive APIs live in submodules — pick one per
//! process, since both target the same kernel-level SIGINT slot:
//!
//! - [`crate::signals::parent`] — install a no-op SIGINT handler so the
//!   parent CLI survives Ctrl+C and reaches its post-run cleanup code while
//!   the spawned child exits on its own. Use this for tools that wrap a
//!   child process.
//! - [`crate::signals::interrupt`] — install a handler that flips a global
//!   flag so long-running in-process loops can poll
//!   [`crate::signals::interrupt::is_interrupted`] and exit cooperatively.
//!   Use this for in-process work loops.
//!
//! Both functions share a single install latch
//! ([`crate::signals::SignalInstallError`]): whichever is called first wins,
//! and the second returns
//! [`crate::signals::SignalInstallError::AlreadyInstalled`] naming the
//! original installer.
//!
//! On Windows the parent path uses
//! `SetConsoleCtrlHandler(NULL, TRUE)` (ignore Ctrl+C); the interrupt path
//! installs a `PHANDLER_ROUTINE` that sets the flag and returns `TRUE`.
//!
//! Caveat: SIGKILL / power loss are uncatchable. Stale state left behind in
//! those cases must be cleaned up by the caller's own next-run logic.
//!
//! # Compatibility re-exports
//!
//! Both APIs are also re-exported at the [`crate::signals`] root so existing
//! consumers don't break. New code should prefer the submodule paths
//! ([`crate::signals::parent::install_parent_handlers`],
//! [`crate::signals::interrupt::install_interrupt_handler`]) to make the API
//! choice explicit at the import site.

pub mod interrupt;
pub mod parent;

pub use interrupt::{install_interrupt_handler, is_interrupted, reset_interrupt};
pub use parent::{SigintDefaultGuard, install_parent_handlers};

use std::sync::atomic::{AtomicU8, Ordering};

/// Returned when a second SIGINT handler install is attempted. The two install
/// functions in this module are mutually exclusive — both target the same
/// kernel-level SIGINT slot — so the second call cannot succeed without
/// silently overwriting the first.
#[derive(Debug, thiserror::Error)]
pub enum SignalInstallError {
   /// A SIGINT handler is already installed by `existing` and a new install
   /// would clobber it.
   #[error(
      "SIGINT handler already installed by {existing}; install_parent_handlers and install_interrupt_handler are mutually exclusive and may each be called at most once per process"
   )]
   AlreadyInstalled {
      /// Name of the function that performed the original install.
      existing: &'static str
   }
}

#[derive(Copy, Clone)]
pub(crate) enum InstallSlot {
   Parent = 1,
   Interrupt = 2
}

impl InstallSlot {
   fn name(self) -> &'static str {
      match self {
         Self::Parent => "install_parent_handlers",
         Self::Interrupt => "install_interrupt_handler"
      }
   }
}

static INSTALL_STATE: AtomicU8 = AtomicU8::new(0);

/// Atomically claim the SIGINT slot for `slot`, or report which function got
/// there first. Uses a single CAS so concurrent installers across threads
/// can't both think they won.
pub(crate) fn try_claim_sigint(slot: InstallSlot) -> Result<(), SignalInstallError> {
   match INSTALL_STATE.compare_exchange(0, slot as u8, Ordering::SeqCst, Ordering::SeqCst) {
      Ok(_) => Ok(()),
      Err(prev) => {
         let existing = match prev {
            x if x == InstallSlot::Parent as u8 => InstallSlot::Parent.name(),
            x if x == InstallSlot::Interrupt as u8 => InstallSlot::Interrupt.name(),
            _ => "unknown"
         };
         Err(SignalInstallError::AlreadyInstalled { existing })
      }
   }
}

/// Test-only escape hatch to clear the install latch so each test can exercise
/// a fresh install. Production code must not call this — clearing the latch
/// without restoring the underlying disposition leaves the SIGINT slot in a
/// stale state.
#[cfg(test)]
pub(crate) fn reset_install_state_for_tests() {
   INSTALL_STATE.store(0, Ordering::SeqCst);
}

/// Atomically swap SIGINT's disposition via `sigaction(2)`.
///
/// Replaces the older `signal(2)` API: `signal()` is a single call that mixes
/// "look up old" with "install new", which races against signals delivered or
/// installs done on other threads between those two operations. `sigaction()`
/// performs the swap atomically inside the kernel and is the POSIX-blessed
/// replacement.
///
/// `SA_RESTART` keeps interrupted slow syscalls (`read`, `write`, …) from
/// returning `EINTR`, matching the implicit behavior of `signal()` on most
/// modern systems and what callers of this crate expect.
///
/// # Safety
///
/// `handler` must be a valid value for `sa_sigaction` — i.e. `SIG_IGN`,
/// `SIG_DFL`, or a function pointer cast to `sighandler_t` whose function is
/// safe to call from a signal context (async-signal-safe).
#[cfg(unix)]
pub(crate) unsafe fn install_sigint_action(handler: libc::sighandler_t) {
   unsafe {
      let mut action: libc::sigaction = std::mem::zeroed();
      action.sa_sigaction = handler;
      action.sa_flags = libc::SA_RESTART;
      libc::sigemptyset(&mut action.sa_mask);
      libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
   }
}

#[cfg(test)]
mod tests {
   use std::sync::Mutex;

   use super::*;
   use crate::signals::{interrupt::INTERRUPTED, parent::set_sigint_ignored};

   /// Tests that mutate INSTALL_STATE or the actual SIGINT slot must serialize
   /// — cargo test runs them in parallel and the slot is process-global.
   /// Tests that only touch INTERRUPTED don't need this.
   static INSTALL_TEST_LOCK: Mutex<()> = Mutex::new(());

   #[test]
   fn is_interrupted_starts_false() {
      // Note: shares global state with other tests. Reset first.
      reset_interrupt();
      assert!(!is_interrupted());
   }

   #[test]
   fn reset_clears_flag() {
      // Manually set the flag to simulate a signal having fired.
      INTERRUPTED.store(true, Ordering::Relaxed);
      assert!(is_interrupted());
      reset_interrupt();
      assert!(!is_interrupted());
   }

   /// End-to-end check that the sigaction-installed handler actually fires:
   /// install, raise SIGINT into our own process, observe the flag flip.
   /// Runs unix-only; restores SIG_IGN on exit so the test harness isn't
   /// killed by a stray signal from a sibling test.
   #[cfg(unix)]
   #[test]
   fn sigaction_handler_flips_interrupted_flag() {
      let _guard = INSTALL_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
      reset_interrupt();
      reset_install_state_for_tests();
      install_interrupt_handler().expect("first install in fresh test should succeed");

      // SAFETY: raise() with a valid signal number is always defined.
      let rc = unsafe { libc::raise(libc::SIGINT) };
      assert_eq!(rc, 0, "raise(SIGINT) failed");

      // The handler is synchronous on the raising thread, so the store has
      // already happened by the time raise() returns.
      assert!(is_interrupted(), "handler did not flip the flag");

      // Leave the slot in a benign state for any later tests.
      reset_interrupt();
      set_sigint_ignored(true);
      reset_install_state_for_tests();
   }

   /// Bundled into one test so the global INSTALL_STATE / SIGINT slot are
   /// touched by exactly one parallel-test thread at a time. (The other tests
   /// in this module share the same state but only touch INTERRUPTED, not
   /// INSTALL_STATE — see `reset_install_state_for_tests` / unix-only
   /// `sigaction_handler_flips_interrupted_flag` for the careful path.)
   #[test]
   fn install_latch_rejects_redundant_and_cross_function_calls() {
      let _guard = INSTALL_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
      reset_install_state_for_tests();

      // First install wins.
      install_parent_handlers().expect("first install should succeed");

      // Same function called twice → AlreadyInstalled, naming itself.
      let err = install_parent_handlers().expect_err("second parent install should fail");
      let SignalInstallError::AlreadyInstalled { existing } = err;
      assert_eq!(existing, "install_parent_handlers");

      // Cross-function call → AlreadyInstalled, naming the original installer.
      let err = install_interrupt_handler().expect_err("interrupt install should be rejected");
      let SignalInstallError::AlreadyInstalled { existing } = err;
      assert_eq!(existing, "install_parent_handlers");

      reset_install_state_for_tests();
   }
}
