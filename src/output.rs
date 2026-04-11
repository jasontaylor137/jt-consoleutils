//! The [`Output`](crate::output::Output) trait and its standard implementations.
//!
//! # Overview
//!
//! - [`LogLevel`](crate::output::LogLevel) — ordered enum representing the verbosity level.
//! - [`OutputMode`](crate::output::OutputMode) — a plain `Copy` struct that carries a [`LogLevel`](crate::output::LogLevel)
//!   and the `dry_run` flag.
//! - [`Output`](crate::output::Output) — the core trait; implement it to redirect output anywhere.
//! - [`ConsoleOutput`](crate::output::ConsoleOutput) — the production implementation; respects
//!   `quiet` / `verbose` and writes to stdout.
//! - [`StringOutput`](crate::output::StringOutput) — an in-memory implementation for use in tests;
//!   captures all output in a `String` that can be inspected with
//!   [`StringOutput::log`](crate::output::StringOutput::log).

// ---------------------------------------------------------------------------
// LogLevel
// ---------------------------------------------------------------------------

/// Ordered verbosity level for CLI output.
///
/// Levels are ordered from least to most verbose:
/// `Quiet < Normal < Verbose < Trace`.
/// This allows range comparisons: `level >= LogLevel::Verbose`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
   /// Suppress all output, including normal progress messages.
   Quiet,
   /// Normal progress messages are printed; verbose output is hidden.
   #[default]
   Normal,
   /// Commands, their arguments, and verbose messages are printed.
   #[cfg(feature = "verbose")]
   Verbose,
   /// All verbose output plus trace-level diagnostics.
   #[cfg(feature = "trace")]
   Trace
}

// ---------------------------------------------------------------------------
// OutputMode
// ---------------------------------------------------------------------------

/// Carries the standard CLI output-mode configuration.
///
/// Construct with struct literal syntax or [`Default::default`] (normal level,
/// dry-run off):
///
/// ```rust,ignore
/// // Requires the "verbose" feature to be enabled.
/// use jt_consoleutils::output::{LogLevel, OutputMode};
///
/// let mode = OutputMode { level: LogLevel::Verbose, ..OutputMode::default() };
/// assert!(mode.is_verbose());
/// assert!(!mode.is_quiet());
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct OutputMode {
   /// The verbosity level.
   pub level: LogLevel,
   /// Dry-run mode: announce operations without executing them.
   pub dry_run: bool
}

impl OutputMode {
   /// Returns `true` when verbose (or trace) output is enabled.
   #[cfg(feature = "verbose")]
   #[must_use]
   pub const fn is_verbose(self) -> bool {
      #[cfg(feature = "trace")]
      return matches!(self.level, LogLevel::Verbose | LogLevel::Trace);
      #[cfg(not(feature = "trace"))]
      return matches!(self.level, LogLevel::Verbose);
   }

   /// Returns `true` when quiet mode is active (all output suppressed).
   #[must_use]
   pub const fn is_quiet(self) -> bool {
      matches!(self.level, LogLevel::Quiet)
   }

   /// Returns `true` when trace mode is active.
   #[cfg(feature = "trace")]
   #[must_use]
   pub const fn is_trace(self) -> bool {
      matches!(self.level, LogLevel::Trace)
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
///
/// Use the [`verbose!`](crate::verbose) and [`trace!`](crate::trace) macros to
/// emit level-gated messages — they check [`is_verbose`](Output::is_verbose) /
/// [`is_trace`](Output::is_trace) before formatting the string, so no allocation
/// occurs when the level is inactive.
///
/// Implement this trait to redirect output to a logger, a file, or anywhere else.
pub trait Output {
   /// Write `line` followed by a newline. Suppressed in quiet mode.
   fn writeln(&mut self, line: &str);

   /// Write `text` without a trailing newline. Suppressed in quiet mode.
   fn write(&mut self, text: &str);

   /// Returns `true` when verbose (or trace) output is active.
   ///
   /// Used by the [`verbose!`](crate::verbose) macro to guard message formatting.
   /// Always returns `false` when the `verbose` feature is disabled.
   #[cfg(feature = "verbose")]
   fn is_verbose(&self) -> bool;

   /// Emit a pre-formatted message in verbose mode.
   ///
   /// Call via the [`verbose!`](crate::verbose) macro, which guards this with
   /// [`is_verbose`](Output::is_verbose) so the string is never allocated when inactive.
   #[cfg(feature = "verbose")]
   fn emit_verbose(&mut self, msg: String);

   /// Returns `true` when trace output is active. Default: `false`.
   #[cfg(feature = "trace")]
   fn is_trace(&self) -> bool {
      false
   }

   /// Emit a pre-formatted message in trace mode. Default: no-op.
   ///
   /// Call via the [`trace!`](crate::trace) macro.
   #[cfg(feature = "trace")]
   fn emit_trace(&mut self, _msg: String) {}

   /// Echo a shell command about to be run (verbose mode only).
   #[cfg(feature = "verbose")]
   fn shell_command(&mut self, cmd: &str);

   /// Echo a single line of output from a running shell command.
   #[cfg(feature = "verbose")]
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

   /// Log a command about to be executed (verbose mode only).
   ///
   /// No-op when the `verbose` feature is disabled.
   #[cfg(feature = "verbose")]
   fn log_exec(&mut self, cmd: &std::process::Command) {
      if self.is_verbose() {
         let program = cmd.get_program().to_string_lossy().into_owned();
         let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
         let msg =
            if args.is_empty() { format!("Exec: {program}") } else { format!("Exec: {program} {}", args.join(" ")) };
         self.emit_verbose(msg);
      }
   }
}

fn format_elapsed(ms: u128) -> String {
   if ms < 1000 { format!("{ms}ms") } else { format!("{}s", ms / 1000) }
}

#[cfg(any(feature = "verbose", feature = "trace"))]
fn with_prefix(prefix: &str, msg: &str) -> String {
   use std::fmt::Write as _;
   let mut out = String::new();
   for l in msg.lines() {
      let _ = writeln!(out, "{prefix}{l}");
   }
   out
}

#[cfg(feature = "trace")]
fn with_trace_prefix(msg: &str) -> String {
   use std::fmt::Write as _;
   let mut out = String::new();
   for l in msg.lines() {
      let _ = writeln!(out, "[2m· {l}[0m");
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

   #[cfg(feature = "verbose")]
   fn is_verbose(&self) -> bool {
      self.mode.is_verbose()
   }

   #[cfg(feature = "verbose")]
   fn emit_verbose(&mut self, msg: String) {
      print!("{}", with_prefix("| ", &msg));
   }

   #[cfg(feature = "trace")]
   fn is_trace(&self) -> bool {
      self.mode.is_trace()
   }

   #[cfg(feature = "trace")]
   fn emit_trace(&mut self, msg: String) {
      print!("{}", with_trace_prefix(&msg));
   }

   #[cfg(feature = "verbose")]
   fn shell_command(&mut self, cmd: &str) {
      if self.mode.is_verbose() && !self.mode.is_quiet() {
         println!("> {cmd}");
      }
   }

   #[cfg(feature = "verbose")]
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
/// `is_verbose()` and `is_trace()` both return `true` so that verbose and trace
/// messages are always captured, allowing tests to assert on their content.
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

   #[cfg(feature = "verbose")]
   fn is_verbose(&self) -> bool {
      true
   }

   #[cfg(feature = "verbose")]
   fn emit_verbose(&mut self, msg: String) {
      self.buf.push_str(&with_prefix("| ", &msg));
   }

   #[cfg(feature = "trace")]
   fn is_trace(&self) -> bool {
      true
   }

   #[cfg(feature = "trace")]
   fn emit_trace(&mut self, msg: String) {
      self.buf.push_str(&with_prefix("· ", &msg));
   }

   #[cfg(feature = "verbose")]
   fn shell_command(&mut self, cmd: &str) {
      self.buf.push_str(&with_prefix("> ", cmd));
   }

   #[cfg(feature = "verbose")]
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

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_captures_verbose() {
      let mut out = StringOutput::new();
      out.emit_verbose("debug info".to_string());
      assert_eq!(out.log(), "| debug info\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_verbose_multiline() {
      let mut out = StringOutput::new();
      out.emit_verbose("line one\nline two".to_string());
      assert_eq!(out.log(), "| line one\n| line two\n");
   }

   #[cfg(feature = "trace")]
   #[test]
   fn string_output_captures_trace() {
      let mut out = StringOutput::new();
      out.emit_trace("trace detail".to_string());
      assert_eq!(out.log(), "· trace detail\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_is_verbose_always_true() {
      assert!(StringOutput::new().is_verbose());
   }

   #[cfg(feature = "trace")]
   #[test]
   fn string_output_is_trace_always_true() {
      assert!(StringOutput::new().is_trace());
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_shell_command() {
      let mut out = StringOutput::new();
      out.shell_command("pnpm install");
      assert_eq!(out.log(), "> pnpm install\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_shell_line() {
      let mut out = StringOutput::new();
      out.shell_line("installed pnpm@9.1.0");
      assert_eq!(out.log(), "> installed pnpm@9.1.0\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn log_exec_formats_command() {
      // Given
      let mut out = StringOutput::new();
      let cmd = std::process::Command::new("node");

      // When
      out.log_exec(&cmd);

      // Then
      assert_eq!(out.log(), "| Exec: node\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn log_exec_includes_args() {
      // Given
      let mut out = StringOutput::new();
      let mut cmd = std::process::Command::new("pnpm");
      cmd.arg("install");

      // When
      out.log_exec(&cmd);

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
