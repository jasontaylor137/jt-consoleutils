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
/// can run after the child exits. Safe to call exactly once at startup.
pub fn install_parent_handlers() {
   set_sigint_ignored(true);
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
   unsafe {
      let handler = if ignored { libc::SIG_IGN } else { libc::SIG_DFL };
      libc::signal(libc::SIGINT, handler);
   }

   #[cfg(windows)]
   unsafe {
      use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
      // Passing a null handler with TRUE installs the "ignore Ctrl+C" filter;
      // FALSE removes it so the default terminate-on-Ctrl+C behavior returns.
      SetConsoleCtrlHandler(None, if ignored { 1 } else { 0 });
   }
}

// ---------------------------------------------------------------------------
// Cooperative interrupt flag
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Install a SIGINT (Unix) / Ctrl+C (Windows) handler that flips the global
/// interrupt flag. Long-running in-process loops should poll
/// [`is_interrupted`] at safe break points and exit cleanly when set, so
/// post-loop summaries (progress finish, stats, etc.) can still run.
///
/// **Conflicts with [`install_parent_handlers`]** — both target the same
/// SIGINT slot. Pick one per process: `install_parent_handlers` for CLIs
/// that spawn a child and want the parent to survive Ctrl+C until cleanup;
/// `install_interrupt_handler` for in-process work loops that want
/// cooperative cancellation.
///
/// Safe to call exactly once at startup.
pub fn install_interrupt_handler() {
   #[cfg(unix)]
   unsafe {
      libc::signal(libc::SIGINT, handle_sigint as *const () as libc::sighandler_t);
   }

   #[cfg(windows)]
   unsafe {
      use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
      SetConsoleCtrlHandler(Some(handle_ctrl_c), 1);
   }
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
   use super::*;

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
}
