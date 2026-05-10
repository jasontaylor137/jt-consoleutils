//! The [`Output`](crate::output::Output) trait and its standard implementations.
//!
//! # Overview
//!
//! - [`LogLevel`](crate::output::LogLevel) — ordered enum representing the verbosity level.
//! - [`OutputMode`](crate::output::OutputMode) — a plain `Copy` struct that carries a `LogLevel`
//!   and the `dry_run` flag.
//! - [`Output`](crate::output::Output) — the core trait; implement it to redirect output anywhere.
//! - [`ConsoleOutput`](crate::output::ConsoleOutput) — the production implementation; respects
//!   `quiet` / `verbose` and writes to stdout.
//! - [`StringOutput`](crate::output::StringOutput) — an in-memory implementation for use in tests;
//!   captures all output in a `String` that can be inspected with
//!   [`StringOutput::log`](crate::output::StringOutput::log).

pub mod action;
mod console;
#[cfg(feature = "file-stats")]
pub mod file_stats;
mod mode;
pub mod progress;
pub mod render;
mod string;

#[cfg(feature = "trace")]
pub mod trace;

pub use console::ConsoleOutput;
pub use mode::{LogLevel, OutputMode};
pub use render::{ASCII_THEME, DEFAULT_THEME, RenderTheme};
pub use string::StringOutput;

/// Abstraction over console output, enabling tests to capture output in memory.
///
/// The two standard implementations are:
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

   /// Write `line` followed by a newline to **stderr**.
   /// Not suppressed by quiet mode (errors and warnings always flow).
   fn eprintln(&mut self, line: &str) {
      // Default: route to writeln so existing impls keep working.
      self.writeln(line);
   }

   /// Returns `true` if this output should emit ANSI color sequences.
   /// Default: `false` (plain output).
   fn colors_enabled(&self) -> bool {
      false
   }

   /// Returns the [`RenderTheme`] this output renders with.
   ///
   /// Default: [`DEFAULT_THEME`] (Unicode glyphs, English connector words).
   /// Override to swap glyphs (e.g. [`ASCII_THEME`] for terminals without
   /// Unicode support) or translate `warn:`/`error:` labels and connector
   /// words like `to`/`from`.
   fn theme(&self) -> RenderTheme {
      DEFAULT_THEME
   }

   /// Emit a steady-state info line: `• <msg>`.
   fn state(&mut self, msg: &str) {
      let line = render::render_state(msg, self.colors_enabled(), &self.theme());
      self.writeln(&line);
   }

   /// Emit a standalone hint line: `→ <msg>` (whole line dim).
   fn hint(&mut self, msg: &str) {
      let line = render::render_hint(msg, self.colors_enabled(), &self.theme());
      self.writeln(&line);
   }

   /// Emit a section header: bold title.
   fn section(&mut self, title: &str) {
      let line = render::render_section(title, self.colors_enabled());
      self.writeln(&line);
   }

   /// Emit an item row under a section: 2-space indent, name, dim trailing.
   fn item(&mut self, name: &str, trailing: &str) {
      let line = render::render_item(name, trailing, self.colors_enabled());
      self.writeln(&line);
   }

   /// Emit a non-fatal warning to **stderr**: `⚠ warn: <msg>`.
   fn warn(&mut self, msg: &str) {
      let line = render::render_warn(msg, self.colors_enabled(), &self.theme());
      self.eprintln(&line);
   }

   /// Emit a fatal-error summary to **stderr**: `✗ error: <msg>`.
   /// **Not suppressed by `--quiet`** — errors always flow.
   fn error(&mut self, msg: &str) {
      let line = render::render_error(msg, self.colors_enabled(), &self.theme());
      self.eprintln(&line);
   }

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
#[derive(Copy, Clone)]
enum Dim {
   Yes,
   No
}

#[cfg(any(feature = "verbose", feature = "trace"))]
fn with_prefix(prefix: &str, msg: &str, dim: Dim) -> String {
   use std::fmt::Write as _;
   let mut out = String::new();
   for l in msg.lines() {
      match dim {
         Dim::Yes => {
            let _ = writeln!(out, "\x1b[2m{prefix}{l}\x1b[0m");
         }
         Dim::No => {
            let _ = writeln!(out, "{prefix}{l}");
         }
      }
   }
   out
}
