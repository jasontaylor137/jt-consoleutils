//! Windows console preparation.
//!
//! By default the Windows console neither interprets the ANSI escape sequences
//! this crate emits nor decodes UTF-8 byte streams — it prints the escapes
//! literally and renders non-ASCII text through the legacy OEM codepage.
//! [`enable_ansi`] fixes both. On every other platform it compiles to nothing,
//! so callers invoke it unconditionally at startup.

/// Prepare the console for ANSI-colored, UTF-8 output.
///
/// On Windows this enables virtual-terminal processing on stdout — so escape
/// sequences render in conhost, `cmd.exe`, and PowerShell instead of printing
/// literally (Windows Terminal enables it on its own) — and sets the console
/// output codepage to UTF-8 (65001), so this process and the children it
/// spawns can write raw UTF-8 bytes without mojibake.
///
/// Both steps are best-effort: with no console attached (output redirected to
/// a file or a pipe) the calls fail harmlessly and are ignored. Console mode
/// and codepage are shared state inherited by child processes; like `chcp
/// 65001`, this sets them once and leaves them.
///
/// On every other platform this is a no-op.
///
/// Call it once, as early in `main` as possible, before emitting any output.
///
/// ```rust,no_run
/// jt_consoleutils::terminal::enable_ansi();
/// ```
pub fn enable_ansi() {
   #[cfg(windows)]
   {
      enable_virtual_terminal_processing();
      enable_utf8_console_output();
   }
}

/// Enable ANSI virtual terminal processing on the stdout console handle.
#[cfg(windows)]
fn enable_virtual_terminal_processing() {
   use windows_sys::Win32::System::Console::{
      ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle, STD_OUTPUT_HANDLE, SetConsoleMode
   };
   // SAFETY: GetStdHandle returns a borrowed handle that needs no release, and
   // both console calls tolerate an invalid handle by returning failure, which
   // is what happens when stdout is redirected rather than a console.
   unsafe {
      let handle = GetStdHandle(STD_OUTPUT_HANDLE);
      let mut mode = 0u32;
      if GetConsoleMode(handle, &mut mode) != 0 {
         let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
      }
   }
}

/// Set the console output codepage to UTF-8 (65001) so byte-stream output
/// renders correctly. Without this, conhost decodes raw UTF-8 written by this
/// process or by a spawned child using the legacy console codepage (CP1252 /
/// CP437), turning characters like `▀`, `╭`, `→`, `✓` into mojibake. Programs
/// that write through `WriteConsoleW` instead are unaffected either way.
#[cfg(windows)]
fn enable_utf8_console_output() {
   // 65001 == CP_UTF8, which lives in Win32_Globalization — not a feature worth
   // enabling for one constant.
   // SAFETY: a plain integer-in, BOOL-out call; it fails harmlessly when no
   // console is attached.
   unsafe {
      let _ = windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
   }
}
