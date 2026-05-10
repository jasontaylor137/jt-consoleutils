//! SIGINT handling so post-run cleanup hooks survive Ctrl+C.
//!
//! When the terminal sends SIGINT to the foreground process group, both the
//! parent CLI and any spawned child receive it. By default the parent dies
//! before its cleanup phase runs.
//! [`install_parent_handlers`](crate::signals::install_parent_handlers) installs a no-op handler in
//! the parent so it survives the signal and reaches its cleanup code; the child still receives
//! SIGINT directly from the terminal and exits on its own.
//!
//! On Windows the equivalent is `SetConsoleCtrlHandler(NULL, TRUE)` which tells
//! the OS to ignore Ctrl+C events for this process. The child receives its own
//! Ctrl+C event from the console.
//!
//! Caveat: SIGKILL / power loss are uncatchable. Stale state left behind in
//! those cases must be cleaned up by the caller's own next-run logic.

/// Install signal handlers that let the parent survive Ctrl+C so cleanup hooks
/// can run after the child exits.
///
/// Mutually exclusive with [`install_interrupt_handler`]: both target the same
/// SIGINT slot, so calling one after the other would silently overwrite the
/// first handler. Pick one per process.
///
/// # Errors
///
/// Returns [`SignalInstallError::AlreadyInstalled`] if a SIGINT handler from
/// this module has already been installed (either function, calling either
/// one a second time).
pub fn install_parent_handlers() -> Result<(), SignalInstallError> {
   try_claim_sigint(InstallSlot::Parent)?;
   set_sigint_ignored(true);
   Ok(())
}

/// RAII guard that restores the default SIGINT behavior for its lifetime, so
/// Ctrl+C aborts the process. Use this around interactive prompts: the global
/// SIG_IGN that protects post-run cleanup would otherwise swallow Ctrl+C while
/// the user is at a prompt, leaving them no way to abort.
pub struct SigintDefaultGuard {
   _private: ()
}

impl SigintDefaultGuard {
   /// Restore default SIGINT (terminate) behavior; reverts on drop.
   #[must_use]
   pub fn new() -> Self {
      set_sigint_ignored(false);
      Self { _private: () }
   }
}

impl Default for SigintDefaultGuard {
   fn default() -> Self {
      Self::new()
   }
}

impl Drop for SigintDefaultGuard {
   fn drop(&mut self) {
      set_sigint_ignored(true);
   }
}

fn set_sigint_ignored(ignored: bool) {
   #[cfg(unix)]
   {
      let handler = if ignored { libc::SIG_IGN } else { libc::SIG_DFL };
      // SAFETY: install_sigint_action only touches SIGINT with a libc-provided
      // handler value (SIG_IGN / SIG_DFL); both are valid for any signal slot.
      unsafe { install_sigint_action(handler) };
   }

   #[cfg(windows)]
   unsafe {
      use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
      // Passing a null handler with TRUE installs the "ignore Ctrl+C" filter;
      // FALSE removes it so the default terminate-on-Ctrl+C behavior returns.
      SetConsoleCtrlHandler(None, if ignored { 1 } else { 0 });
   }
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
unsafe fn install_sigint_action(handler: libc::sighandler_t) {
   unsafe {
      let mut action: libc::sigaction = std::mem::zeroed();
      action.sa_sigaction = handler;
      action.sa_flags = libc::SA_RESTART;
      libc::sigemptyset(&mut action.sa_mask);
      libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
   }
}

// ---------------------------------------------------------------------------
// Cooperative interrupt flag
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Install a SIGINT (Unix) / Ctrl+C (Windows) handler that flips the global
/// interrupt flag. Long-running in-process loops should poll
/// [`is_interrupted`] at safe break points and exit cleanly when set, so
/// post-loop summaries (progress finish, stats, etc.) can still run.
///
/// Mutually exclusive with [`install_parent_handlers`] — both target the same
/// SIGINT slot. Pick one per process: `install_parent_handlers` for CLIs that
/// spawn a child and want the parent to survive Ctrl+C until cleanup;
/// `install_interrupt_handler` for in-process work loops that want
/// cooperative cancellation.
///
/// # Errors
///
/// Returns [`SignalInstallError::AlreadyInstalled`] if a SIGINT handler from
/// this module has already been installed (either function, calling either
/// one a second time).
pub fn install_interrupt_handler() -> Result<(), SignalInstallError> {
   try_claim_sigint(InstallSlot::Interrupt)?;

   #[cfg(unix)]
   {
      // SAFETY: handle_sigint is async-signal-safe — it only does an atomic
      // store on a static AtomicBool. Cast to sighandler_t per the libc ABI.
      unsafe {
         install_sigint_action(handle_sigint as *const () as libc::sighandler_t);
      }
   }

   #[cfg(windows)]
   unsafe {
      use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
      SetConsoleCtrlHandler(Some(handle_ctrl_c), 1);
   }

   Ok(())
}

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
enum InstallSlot {
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
fn try_claim_sigint(slot: InstallSlot) -> Result<(), SignalInstallError> {
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
fn reset_install_state_for_tests() {
   INSTALL_STATE.store(0, Ordering::SeqCst);
}

/// Returns `true` if a SIGINT / Ctrl+C has been received since the last
/// [`reset_interrupt`] call (or since process start).
#[must_use]
pub fn is_interrupted() -> bool {
   INTERRUPTED.load(Ordering::Relaxed)
}

/// Clear the interrupt flag. Useful between independent work phases when
/// you want each phase to honor its own Ctrl+C without one phase's signal
/// triggering the next.
pub fn reset_interrupt() {
   INTERRUPTED.store(false, Ordering::Relaxed);
}

#[cfg(unix)]
extern "C" fn handle_sigint(_: libc::c_int) {
   INTERRUPTED.store(true, Ordering::Relaxed);
}

#[cfg(windows)]
unsafe extern "system" fn handle_ctrl_c(ctrl_type: u32) -> windows_sys::core::BOOL {
   use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT};
   if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT {
      INTERRUPTED.store(true, Ordering::Relaxed);
      1 // TRUE: handled, don't propagate
   } else {
      0 // FALSE: not handled
   }
}

#[cfg(test)]
mod tests {
   use std::sync::Mutex;

   use super::*;

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
