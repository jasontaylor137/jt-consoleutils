//! SIGINT handling so post-run cleanup hooks survive Ctrl+C.
//!
//! When the terminal sends SIGINT to the foreground process group, both the
//! parent CLI and any spawned child receive it. By default the parent dies
//! before its cleanup phase runs. [`install_parent_handlers`] installs a no-op
//! handler in the parent so it survives the signal and reaches its cleanup
//! code; the child still receives SIGINT directly from the terminal and exits
//! on its own.
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
