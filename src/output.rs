//! The [`Output`](crate::output::Output) trait and its standard implementations.
//!
//! # Overview
//!
//! - [`OutputMode`](crate::output::OutputMode) — a plain `Copy` struct that carries the three
//!   common CLI flags (`verbose`, `quiet`, `dry_run`).
//! - [`Output`](crate::output::Output) — the core trait; implement it to redirect output anywhere.
//! - [`ConsoleOutput`](crate::output::ConsoleOutput) — the production implementation; respects
//!   `quiet` / `verbose` and writes to stdout.
//! - [`StringOutput`](crate::output::StringOutput) — an in-memory implementation for use in tests;
//!   captures all output in a `String` that can be inspected with
//!   [`StringOutput::log`](crate::output::StringOutput::log).

// ---------------------------------------------------------------------------
// OutputMode
// ---------------------------------------------------------------------------

/// Carries the three standard CLI output-mode flags.
///
/// Construct with struct literal syntax or [`Default::default`] (all flags
/// `false`):
///
/// ```rust
/// use jt_consoleutils::output::OutputMode;
///
/// let mode = OutputMode { verbose: true, ..OutputMode::default() };
/// assert!(mode.is_verbose());
/// assert!(!mode.is_quiet());
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct OutputMode {
   /// Enable verbose output: commands and their output are echoed.
   pub verbose: bool,
   /// Suppress all output, including normal progress messages.
   pub quiet: bool,
   /// Dry-run mode: announce operations without executing them.
   pub dry_run: bool
}

impl OutputMode {
   /// Returns `true` when verbose output is enabled.
   #[must_use]
   pub const fn is_verbose(self) -> bool {
      self.verbose
   }

   /// Returns `true` when quiet mode is active (all output suppressed).
   #[must_use]
   pub const fn is_quiet(self) -> bool {
      self.quiet
   }

   /// Returns `true` when dry-run mode is active.
   #[must_use]
   pub const fn is_dry_run(self) -> bool {
      self.dry_run
   }
}

// ---------------------------------------------------------------------------
// Output trait
// ---------------------------------------------------------------------------

/// Abstraction over console output, enabling tests to capture output in memory.
///
/// The three standard implementations are:
/// - [`ConsoleOutput`] — writes to stdout, respecting `quiet` / `verbose`.
/// - [`StringOutput`] — captures everything in a `String` for assertions.
/// - `MockShell`'s internal output — not part of this trait, but follows the same pattern.
///
/// Implement this trait to redirect output to a logger, a file, or anywhere else.
pub trait Output {
   /// Write `line` followed by a newline. Suppressed in quiet mode.
   fn writeln(&mut self, line: &str);

   /// Write `text` without a trailing newline. Suppressed in quiet mode.
   fn write(&mut self, text: &str);

   /// Emit a lazily-evaluated message, only in verbose mode.
   ///
   /// The closure is only called when the implementation decides to display it,
   /// avoiding string allocation cost in non-verbose builds.
   fn verbose(&mut self, f: Box<dyn FnOnce() -> String>);

   /// Echo a shell command about to be run (verbose mode only).
   fn shell_command(&mut self, cmd: &str);

   /// Echo a single line of output from a running shell command.
   fn shell_line(&mut self, line: &str);

   /// Render the result of a completed step: a tick/cross, label, elapsed time,
   /// and (on failure) the last few lines of output from the `viewport`.
   fn step_result(&mut self, label: &str, success: bool, elapsed_ms: u128, viewport: &[String]);

   /// Dry-run: announce a shell command that would be executed.
   fn dry_run_shell(&mut self, _cmd: &str) {}

   /// Dry-run: announce a file that would be written.
   fn dry_run_write(&mut self, _path: &str) {}

   /// Dry-run: announce a file or directory that would be deleted.
   fn dry_run_delete(&mut self, _path: &str) {}

   /// Log a message in verbose mode without any extra ceremony.
   fn log(&mut self, mode: OutputMode, msg: &str) {
      if mode.is_verbose() {
         let owned = msg.to_owned();
         self.verbose(Box::new(move || owned));
      }
   }

   /// Log a command about to be executed (verbose mode).
   fn log_exec(&mut self, mode: OutputMode, cmd: &std::process::Command) {
      if mode.is_verbose() {
         let program = cmd.get_program().to_string_lossy().into_owned();
         let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
         self.verbose(Box::new(move || {
            if args.is_empty() { format!("Exec: {program}") } else { format!("Exec: {program} {}", args.join(" ")) }
         }));
      }
   }
}

fn format_elapsed(ms: u128) -> String {
   if ms < 1000 { format!("{ms}ms") } else { format!("{}s", ms / 1000) }
}

fn with_prefix(prefix: &str, msg: &str) -> String {
   use std::fmt::Write as _;
   let mut out = String::new();
   for l in msg.lines() {
      let _ = writeln!(out, "{prefix}{l}");
   }
   out
}

// ---------------------------------------------------------------------------
// ConsoleOutput
// ---------------------------------------------------------------------------

/// Production [`Output`] implementation that writes to stdout.
///
/// Behavior depends on the [`OutputMode`] supplied at construction:
/// - `quiet`: all methods are silent.
/// - `verbose`: commands, their arguments, and verbose messages are printed.
/// - default: normal progress messages are printed; verbose output is hidden.
pub struct ConsoleOutput {
   mode: OutputMode
}

impl ConsoleOutput {
   /// Create a new `ConsoleOutput` driven by `mode`.
   #[must_use]
   pub const fn new(mode: OutputMode) -> Self {
      Self { mode }
   }
}

impl Output for ConsoleOutput {
   fn writeln(&mut self, line: &str) {
      if !self.mode.is_quiet() {
         println!("{line}");
      }
   }

   fn write(&mut self, text: &str) {
      if !self.mode.is_quiet() {
         use std::io::Write;
         print!("{text}");
         let _ = std::io::stdout().flush();
      }
   }

   fn verbose(&mut self, f: Box<dyn FnOnce() -> String>) {
      if self.mode.is_verbose() && !self.mode.is_quiet() {
         print!("{}", with_prefix("| ", &f()));
      }
   }

   fn shell_command(&mut self, cmd: &str) {
      if self.mode.is_verbose() && !self.mode.is_quiet() {
         println!("> {cmd}");
      }
   }

   fn shell_line(&mut self, line: &str) {
      if !self.mode.is_quiet() {
         println!("> {line}");
      }
   }

   fn step_result(&mut self, label: &str, success: bool, elapsed_ms: u128, viewport: &[String]) {
      if self.mode.is_quiet() {
         return;
      }
      let t = format_elapsed(elapsed_ms);
      if success {
         println!("\x1b[32m✓\x1b[0m {label} \x1b[2m({t})\x1b[0m");
      } else {
         println!("\x1b[31m✗\x1b[0m {label} \x1b[2m({t})\x1b[0m");
         for line in viewport {
            println!("  \x1b[31m{line}\x1b[0m");
         }
      }
   }

   fn dry_run_shell(&mut self, cmd: &str) {
      if self.mode.is_dry_run() {
         println!("[dry-run] would run: {cmd}");
      }
   }

   fn dry_run_write(&mut self, path: &str) {
      if self.mode.is_dry_run() {
         println!("[dry-run] would write: {path}");
      }
   }

   fn dry_run_delete(&mut self, path: &str) {
      if self.mode.is_dry_run() {
         println!("[dry-run] would delete: {path}");
      }
   }
}

// ---------------------------------------------------------------------------
// StringOutput — a test-helper implementation that captures output in memory.
// Intentionally pub so downstream crates can use it in their own tests.
// ---------------------------------------------------------------------------

/// In-memory [`Output`] implementation for use in tests.
///
/// All output is appended to an internal `String`. Call [`StringOutput::log`]
/// to retrieve the full captured output and assert on it.
///
/// ```rust
/// use jt_consoleutils::output::{Output, StringOutput};
///
/// let mut out = StringOutput::new();
/// out.writeln("hello");
/// assert_eq!(out.log(), "hello\n");
/// ```
pub struct StringOutput {
   buf: String
}

impl StringOutput {
   /// Create a new, empty `StringOutput`.
   #[must_use]
   pub const fn new() -> Self {
      Self { buf: String::new() }
   }

   /// Return the full captured output as a string slice.
   #[must_use]
   pub fn log(&self) -> &str {
      &self.buf
   }
}

impl Default for StringOutput {
   fn default() -> Self {
      Self::new()
   }
}

use std::fmt::Write as _;

impl Output for StringOutput {
   fn writeln(&mut self, line: &str) {
      self.buf.push_str(line);
      self.buf.push('\n');
   }

   fn write(&mut self, text: &str) {
      self.buf.push_str(text);
   }

   fn verbose(&mut self, f: Box<dyn FnOnce() -> String>) {
      self.buf.push_str(&with_prefix("| ", &f()));
   }

   fn shell_command(&mut self, cmd: &str) {
      self.buf.push_str(&with_prefix("> ", cmd));
   }

   fn shell_line(&mut self, line: &str) {
      self.buf.push_str(&with_prefix("> ", line));
   }

   fn step_result(&mut self, label: &str, success: bool, elapsed_ms: u128, _viewport: &[String]) {
      let symbol = if success { '✓' } else { '✗' };
      let _ = writeln!(self.buf, "{symbol} {label} ({})", format_elapsed(elapsed_ms));
   }

   fn dry_run_shell(&mut self, cmd: &str) {
      let _ = writeln!(self.buf, "[dry-run] would run: {cmd}");
   }

   fn dry_run_write(&mut self, path: &str) {
      let _ = writeln!(self.buf, "[dry-run] would write: {path}");
   }

   fn dry_run_delete(&mut self, path: &str) {
      let _ = writeln!(self.buf, "[dry-run] would delete: {path}");
   }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
   use rstest::rstest;

   use super::*;

   fn verbose_mode() -> OutputMode {
      OutputMode { verbose: true, ..Default::default() }
   }

   #[test]
   fn string_output_captures_lines() {
      let mut out = StringOutput::new();
      out.writeln("hello");
      out.writeln("world");
      assert_eq!(out.log(), "hello\nworld\n");
   }

   #[test]
   fn string_output_write_no_newline() {
      let mut out = StringOutput::new();
      out.write("a");
      out.write("b");
      assert_eq!(out.log(), "ab");
   }

   #[test]
   fn string_output_captures_verbose() {
      let mut out = StringOutput::new();
      out.verbose(Box::new(|| "debug info".to_string()));
      assert_eq!(out.log(), "| debug info\n");
   }

   #[test]
   fn string_output_verbose_multiline() {
      let mut out = StringOutput::new();
      out.verbose(Box::new(|| "line one\nline two".to_string()));
      assert_eq!(out.log(), "| line one\n| line two\n");
   }

   #[test]
   fn string_output_shell_command() {
      let mut out = StringOutput::new();
      out.shell_command("pnpm install");
      assert_eq!(out.log(), "> pnpm install\n");
   }

   #[test]
   fn string_output_shell_line() {
      let mut out = StringOutput::new();
      out.shell_line("installed pnpm@9.1.0");
      assert_eq!(out.log(), "> installed pnpm@9.1.0\n");
   }

   #[test]
   fn log_helper_delegates_to_verbose() {
      // Given
      let mut out = StringOutput::new();
      let mode = verbose_mode();

      // When
      Output::log(&mut out, mode, "setting up cache");

      // Then
      assert_eq!(out.log(), "| setting up cache\n");
   }

   #[test]
   fn log_helper_silent_when_not_verbose() {
      // Given
      let mut out = StringOutput::new();
      let mode = OutputMode::default();

      // When
      Output::log(&mut out, mode, "setting up cache");

      // Then
      assert_eq!(out.log(), "");
   }

   #[test]
   fn log_exec_formats_command() {
      // Given
      let mut out = StringOutput::new();
      let mode = verbose_mode();
      let cmd = std::process::Command::new("node");

      // When
      Output::log_exec(&mut out, mode, &cmd);

      // Then
      assert_eq!(out.log(), "| Exec: node\n");
   }

   #[test]
   fn log_exec_includes_args() {
      // Given
      let mut out = StringOutput::new();
      let mode = verbose_mode();
      let mut cmd = std::process::Command::new("pnpm");
      cmd.arg("install");

      // When
      Output::log_exec(&mut out, mode, &cmd);

      // Then
      assert_eq!(out.log(), "| Exec: pnpm install\n");
   }

   #[rstest]
   #[case(true, 1200, "✓ build (1s)\n")]
   #[case(false, 300, "✗ build (300ms)\n")]
   fn string_output_step_result(#[case] success: bool, #[case] elapsed_ms: u128, #[case] expected: &str) {
      // Given
      let mut out = StringOutput::new();

      // When
      out.step_result("build", success, elapsed_ms, &[]);

      // Then
      assert_eq!(out.log(), expected);
   }

   #[test]
   fn string_output_dry_run_shell() {
      let mut out = StringOutput::new();
      out.dry_run_shell("rm -rf /");
      assert_eq!(out.log(), "[dry-run] would run: rm -rf /\n");
   }

   #[test]
   fn string_output_dry_run_write() {
      let mut out = StringOutput::new();
      out.dry_run_write("/some/path.json");
      assert_eq!(out.log(), "[dry-run] would write: /some/path.json\n");
   }

   #[test]
   fn string_output_dry_run_delete() {
      let mut out = StringOutput::new();
      out.dry_run_delete("/some/dir");
      assert_eq!(out.log(), "[dry-run] would delete: /some/dir\n");
   }
}
