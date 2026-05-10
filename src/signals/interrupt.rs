//! Cooperative interrupt flag.
//!
//! [`install_interrupt_handler`] installs a SIGINT (Unix) / Ctrl+C (Windows)
//! handler that flips a global flag. Long-running in-process loops should
//! poll [`is_interrupted`] at safe break points and exit cleanly when set,
//! so post-loop summaries (progress finish, stats, etc.) can still run.

use std::sync::atomic::{AtomicBool, Ordering};

use super::{InstallSlot, SignalInstallError, try_claim_sigint};

pub(crate) static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Install a SIGINT (Unix) / Ctrl+C (Windows) handler that flips the global
/// interrupt flag. Long-running in-process loops should poll
/// [`is_interrupted`] at safe break points and exit cleanly when set, so
/// post-loop summaries (progress finish, stats, etc.) can still run.
///
/// Mutually exclusive with [`super::parent::install_parent_handlers`] — both
/// target the same SIGINT slot. Pick one per process: `install_parent_handlers`
/// for CLIs that spawn a child and want the parent to survive Ctrl+C until
/// cleanup; `install_interrupt_handler` for in-process work loops that want
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
         super::install_sigint_action(handle_sigint as *const () as libc::sighandler_t);
      }
   }

   #[cfg(windows)]
   unsafe {
      use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
      SetConsoleCtrlHandler(Some(handle_ctrl_c), 1);
   }

   Ok(())
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
