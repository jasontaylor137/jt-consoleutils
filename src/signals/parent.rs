//! Parent-survives-Ctrl+C handler so post-run cleanup hooks can run.
//!
//! When the terminal sends SIGINT to the foreground process group, both the
//! parent CLI and any spawned child receive it. By default the parent dies
//! before its cleanup phase runs. [`install_parent_handlers`] installs a
//! no-op handler in the parent so it survives the signal and reaches its
//! cleanup code; the child still receives SIGINT directly from the terminal
//! and exits on its own.
//!
//! A handler is used rather than SIG_IGN on purpose. SIG_IGN is *inherited*
//! across `exec(2)`, so every runtime the CLI spawns — and recursively their
//! children — would inherit it and the whole process tree would become immune
//! to Ctrl+C. A handler, by contrast, is reset to SIG_DFL in the exec'd child,
//! so spawned children still terminate on Ctrl+C as intended.
//!
//! On Windows the parent installs a console control handler *routine* (via
//! `SetConsoleCtrlHandler`) that reports Ctrl+C as handled so the parent
//! survives. It deliberately does NOT use `SetConsoleCtrlHandler(NULL, TRUE)`,
//! whose "ignore Ctrl+C" attribute "is inherited by child processes" (per the
//! Win32 docs) and would likewise make every spawned runtime immune. A handler
//! routine is a per-process function pointer and is not inherited, so each
//! child still receives — and acts on — its own Ctrl+C event from the console.

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
/// survive-Ctrl+C handler that protects post-run cleanup would otherwise
/// swallow Ctrl+C while the user is at a prompt, leaving them no way to abort.
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
      // "Ignoring" installs a real (no-op) handler rather than SIG_IGN: a
      // handler is reset to SIG_DFL across exec(2) so spawned children still
      // die on Ctrl+C, whereas SIG_IGN is inherited and would make the whole
      // process tree immune. `false` restores SIG_DFL (e.g. around prompts so
      // Ctrl+C aborts).
      let handler = if ignored { survive_sigint as *const () as libc::sighandler_t } else { libc::SIG_DFL };
      // SAFETY: survive_sigint is async-signal-safe (empty body); SIG_DFL is
      // always valid. Both are valid dispositions for the SIGINT slot.
      unsafe { super::install_sigint_action(handler) };
   }

   #[cfg(windows)]
   unsafe {
      use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
      // Add/remove a handler routine (TRUE/FALSE) rather than the inheritable
      // SetConsoleCtrlHandler(NULL, TRUE) ignore-flag, so spawned children are
      // not made immune to Ctrl+C. Removing the routine (FALSE) restores the
      // default terminate-on-Ctrl+C behavior.
      SetConsoleCtrlHandler(Some(survive_ctrl_c), if ignored { 1 } else { 0 });
   }
}

/// No-op SIGINT handler that lets the parent survive Ctrl+C so its post-run
/// cleanup can execute. Installed instead of SIG_IGN precisely because a
/// handler is reset to SIG_DFL in `exec(2)`'d children while SIG_IGN is
/// inherited. Async-signal-safe: the body is empty.
#[cfg(unix)]
extern "C" fn survive_sigint(_: libc::c_int) {}

/// Windows console handler that lets the parent survive Ctrl+C / Ctrl+Break by
/// reporting the event as handled (returns TRUE), suppressing the default
/// terminate. Installed as a handler routine rather than the inheritable
/// `SetConsoleCtrlHandler(NULL, TRUE)` ignore-flag so spawned children are not
/// made immune to Ctrl+C.
#[cfg(windows)]
unsafe extern "system" fn survive_ctrl_c(ctrl_type: u32) -> windows_sys::core::BOOL {
   use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT};
   if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT {
      1 // TRUE: handled — the parent survives; default terminate is suppressed.
   } else {
      0 // FALSE: defer other control events (close, logoff, shutdown).
   }
}
