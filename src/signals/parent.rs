//! Parent-survives-Ctrl+C handler so post-run cleanup hooks can run.
//!
//! When the terminal sends SIGINT to the foreground process group, both the
//! parent CLI and any spawned child receive it. By default the parent dies
//! before its cleanup phase runs. [`install_parent_handlers`] installs a
//! no-op handler in the parent so it survives the signal and reaches its
//! cleanup code; the child still receives SIGINT directly from the terminal
//! and exits on its own.
//!
//! On Windows the equivalent is `SetConsoleCtrlHandler(NULL, TRUE)` which
//! tells the OS to ignore Ctrl+C events for this process. The child receives
//! its own Ctrl+C event from the console.

use super::{InstallSlot, SignalInstallError, try_claim_sigint};

/// Install signal handlers that let the parent survive Ctrl+C so cleanup hooks
/// can run after the child exits.
///
/// Mutually exclusive with [`super::interrupt::install_interrupt_handler`]:
/// both target the same SIGINT slot, so calling one after the other would
/// silently overwrite the first handler. Pick one per process.
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

pub(crate) fn set_sigint_ignored(ignored: bool) {
   #[cfg(unix)]
   {
      let handler = if ignored { libc::SIG_IGN } else { libc::SIG_DFL };
      // SAFETY: install_sigint_action only touches SIGINT with a libc-provided
      // handler value (SIG_IGN / SIG_DFL); both are valid for any signal slot.
      unsafe { super::install_sigint_action(handler) };
   }

   #[cfg(windows)]
   unsafe {
      use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
      // Passing a null handler with TRUE installs the "ignore Ctrl+C" filter;
      // FALSE removes it so the default terminate-on-Ctrl+C behavior returns.
      SetConsoleCtrlHandler(None, if ignored { 1 } else { 0 });
   }
}
