//! Scripted shell for driving the spinner overlay in tests without spawning real OS processes.
//!
//! [`ScriptedShell`] and [`Script`] are intended for **testing use**. They are not gated behind
//! `#[cfg(test)]` so that downstream crates can use them in their own test suites, but they carry
//! no meaningful runtime cost in production builds (LTO eliminates unused code).
//!
//! Use [`ScriptedShell::with_config`] to customise overlay behaviour (e.g. viewport height).
#![allow(dead_code)]

use std::{cell::RefCell, collections::VecDeque, sync::mpsc, thread, time::Duration};

use super::{
   CommandResult, Shell, ShellConfig, ShellError,
   exec::{Line, render_overlay_lines}
};
use crate::output::{Output, OutputMode};

// ---------------------------------------------------------------------------
// ScriptEvent
// ---------------------------------------------------------------------------

enum ScriptEvent {
   Out(String),
   Err(String),
   Delay(u64)
}

// ---------------------------------------------------------------------------
// Script builder
// ---------------------------------------------------------------------------

/// A sequence of stdout/stderr events and delays that [`ScriptedShell`] replays
/// through the spinner overlay renderer.
///
/// Build one with the fluent builder methods ([`Script::out`], [`Script::err`],
/// [`Script::delay_ms`], etc.) and enqueue it on a [`ScriptedShell`] via
/// [`ScriptedShell::push`].
///
/// Intended for **testing use**.
pub struct Script {
   events: Vec<ScriptEvent>,
   success: bool
}

impl Default for Script {
   fn default() -> Self {
      Self::new()
   }
}

impl Script {
   /// Create a new empty `Script` that exits with success by default.
   #[must_use]
   pub const fn new() -> Self {
      Self { events: Vec::new(), success: true }
   }

   /// Write raw text to stdout (no implicit newline).
   #[must_use]
   pub fn out(mut self, text: &str) -> Self {
      self.events.push(ScriptEvent::Out(text.to_string()));
      self
   }

   /// Write raw text to stdout then sleep for `ms` milliseconds.
   #[must_use]
   pub fn out_ms(self, text: &str, ms: u64) -> Self {
      self.out(text).delay_ms(ms)
   }

   /// Write text followed by a newline to stdout.
   #[must_use]
   pub fn out_line(self, text: &str) -> Self {
      self.out(text).out("\n")
   }

   /// Write text followed by a cr to stdout.
   #[must_use]
   pub fn out_cr(mut self, text: &str) -> Self {
      self.events.push(ScriptEvent::Out(format!("{text}\r")));
      self
   }

   /// Write text followed by a newline to stdout then sleep for `ms` milliseconds.
   #[must_use]
   pub fn out_line_ms(self, text: &str, ms: u64) -> Self {
      self.out_line(text).delay_ms(ms)
   }

   /// Write text followed by a cr to stdout then sleep for `ms` milliseconds.
   #[must_use]
   pub fn out_cr_ms(self, text: &str, ms: u64) -> Self {
      self.out_cr(text).delay_ms(ms)
   }

   /// Write raw text to stderr (no implicit newline).
   #[must_use]
   pub fn err(mut self, text: &str) -> Self {
      self.events.push(ScriptEvent::Err(text.to_string()));
      self
   }

   /// Write raw text to stderr then sleep for `ms` milliseconds.
   #[must_use]
   pub fn err_ms(self, text: &str, ms: u64) -> Self {
      self.err(text).delay_ms(ms)
   }

   /// Write text followed by a newline to stderr.
   #[must_use]
   pub fn err_line(self, text: &str) -> Self {
      self.err(text).err("\n")
   }

   /// Write text followed by a newline to stderr then sleep for `ms` milliseconds.
   #[must_use]
   pub fn err_line_ms(self, text: &str, ms: u64) -> Self {
      self.err_line(text).delay_ms(ms)
   }

   /// Sleep for `ms` milliseconds before processing the next event.
   #[must_use]
   pub fn delay_ms(mut self, ms: u64) -> Self {
      self.events.push(ScriptEvent::Delay(ms));
      self
   }

   /// Mark this script as exiting with a failure code. Default is success.
   #[must_use]
   pub const fn exit_failure(mut self) -> Self {
      self.success = false;
      self
   }
}

// ---------------------------------------------------------------------------
// ScriptedShell
// ---------------------------------------------------------------------------

/// A [`Shell`] implementation that drives the real spinner overlay using
/// pre-configured output scripts. No OS processes are spawned.
///
/// Intended for **testing use**. Enqueue one [`Script`] per expected
/// [`Shell::run_command`] call via [`ScriptedShell::push`]; each call pops the
/// front script and replays its events through the live overlay renderer,
/// letting you write overlay integration tests without real subprocesses.
///
/// Use [`ScriptedShell::with_config`] to supply a custom [`ShellConfig`]
/// (e.g. to change the overlay viewport height).
pub struct ScriptedShell {
   scripts: RefCell<VecDeque<Script>>,
   config: ShellConfig
}

impl Default for ScriptedShell {
   fn default() -> Self {
      Self::new()
   }
}

impl ScriptedShell {
   /// Create a new `ScriptedShell` with an empty script queue and default config.
   #[must_use]
   pub fn new() -> Self {
      Self { scripts: RefCell::new(VecDeque::new()), config: ShellConfig::default() }
   }

   /// Override the shell configuration (e.g. `viewport_size`).
   #[must_use]
   pub const fn with_config(mut self, config: ShellConfig) -> Self {
      self.config = config;
      self
   }

   /// Enqueue a script to be consumed by the next `run_command` call.
   #[must_use]
   pub fn push(self, script: Script) -> Self {
      self.scripts.borrow_mut().push_back(script);
      self
   }
}

impl Shell for ScriptedShell {
   fn run_command(
      &self,
      label: &str,
      _program: &str,
      _args: &[&str],
      output: &mut dyn Output,
      _mode: OutputMode
   ) -> Result<CommandResult, ShellError> {
      let script =
         self.scripts.borrow_mut().pop_front().expect("ScriptedShell: run_command called but script queue is empty");

      let (tx, rx) = mpsc::channel::<Line>();
      let success = script.success;

      thread::spawn(move || {
         let mut stdout_buf = String::new();
         let mut stderr_buf = String::new();

         for event in script.events {
            match event {
               ScriptEvent::Out(s) => feed(&s, &mut stdout_buf, false, &tx),
               ScriptEvent::Err(s) => feed(&s, &mut stderr_buf, true, &tx),
               ScriptEvent::Delay(ms) => thread::sleep(Duration::from_millis(ms))
            }
         }

         // Flush any remaining buffered text (no trailing newline).
         if !stdout_buf.is_empty() {
            let _ = tx.send(Line::Stdout(std::mem::take(&mut stdout_buf)));
         }
         if !stderr_buf.is_empty() {
            let _ = tx.send(Line::Stderr(std::mem::take(&mut stderr_buf)));
         }
         // tx dropped here → receiver disconnects → overlay loop exits
      });

      let rendered = render_overlay_lines(label, &rx, self.config.viewport_size);
      output.step_result(label, success, rendered.elapsed.as_millis(), &rendered.viewport);

      Ok(CommandResult { success, stderr: rendered.stderr_lines.join("\n") })
   }

   fn shell_exec(
      &self,
      _script: &str,
      _output: &mut dyn Output,
      _mode: OutputMode
   ) -> Result<CommandResult, ShellError> {
      Ok(CommandResult { success: true, stderr: String::new() })
   }

   fn command_exists(&self, _program: &str) -> bool {
      true
   }

   fn command_output(&self, _program: &str, _args: &[&str]) -> Result<String, ShellError> {
      Ok(String::new())
   }

   fn exec_capture(
      &self,
      _cmd: &str,
      _output: &mut dyn Output,
      _mode: OutputMode
   ) -> Result<CommandResult, ShellError> {
      Ok(CommandResult { success: true, stderr: String::new() })
   }

   fn exec_interactive(&self, _cmd: &str, _output: &mut dyn Output, _mode: OutputMode) -> Result<(), ShellError> {
      Ok(())
   }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
   use std::sync::mpsc;

   use super::{Line, Script, ScriptedShell, feed};
   use crate::{
      output::{OutputMode, StringOutput},
      shell::{Shell, ShellConfig}
   };

   // -----------------------------------------------------------------------
   // Helpers
   // -----------------------------------------------------------------------

   fn default_mode() -> OutputMode {
      OutputMode::default()
   }

   /// Drain a channel into a Vec, classifying each Line.
   fn collect_lines(rx: mpsc::Receiver<Line>) -> Vec<Line> {
      rx.into_iter().collect()
   }

   /// Convenience: run feed() on `input` starting with an empty buffer,
   /// collect every Line sent on the channel, and return (lines, remaining_buf).
   fn feed_all(input: &str, is_stderr: bool) -> (Vec<Line>, String) {
      let (tx, rx) = mpsc::channel::<Line>();
      let mut buf = String::new();
      feed(input, &mut buf, is_stderr, &tx);
      drop(tx);
      let lines = collect_lines(rx);
      (lines, buf)
   }

   // -----------------------------------------------------------------------
   // feed() — basic newline splitting
   // -----------------------------------------------------------------------

   #[test]
   fn feed_single_complete_stdout_line() {
      let (lines, buf) = feed_all("hello\n", false);
      assert_eq!(lines.len(), 1);
      assert!(matches!(&lines[0], Line::Stdout(s) if s == "hello"));
      assert!(buf.is_empty());
   }

   #[test]
   fn feed_multiple_newline_lines() {
      let (lines, buf) = feed_all("a\nb\nc\n", false);
      assert_eq!(lines.len(), 3);
      assert!(matches!(&lines[0], Line::Stdout(s) if s == "a"));
      assert!(matches!(&lines[1], Line::Stdout(s) if s == "b"));
      assert!(matches!(&lines[2], Line::Stdout(s) if s == "c"));
      assert!(buf.is_empty());
   }

   #[test]
   fn feed_partial_line_stays_in_buffer() {
      let (lines, buf) = feed_all("partial", false);
      assert!(lines.is_empty());
      assert_eq!(buf, "partial");
   }

   #[test]
   fn feed_partial_line_flushed_by_subsequent_newline() {
      let (tx, rx) = mpsc::channel::<Line>();
      let mut buf = String::new();
      feed("partial", &mut buf, false, &tx);
      feed(" line\n", &mut buf, false, &tx);
      drop(tx);
      let lines = collect_lines(rx);
      assert_eq!(lines.len(), 1);
      assert!(matches!(&lines[0], Line::Stdout(s) if s == "partial line"));
      assert!(buf.is_empty());
   }

   // -----------------------------------------------------------------------
   // feed() — carriage-return (StdoutCr) handling
   // -----------------------------------------------------------------------

   #[test]
   fn feed_cr_produces_stdout_cr() {
      let (lines, buf) = feed_all("progress\r", false);
      assert_eq!(lines.len(), 1);
      assert!(matches!(&lines[0], Line::StdoutCr(s) if s == "progress"));
      assert!(buf.is_empty());
   }

   #[test]
   fn feed_cr_overwrites_accumulate_correctly() {
      // Simulate a progress bar that rewrites the same line three times.
      let (lines, buf) = feed_all("10%\r50%\r100%\r", false);
      assert_eq!(lines.len(), 3);
      assert!(matches!(&lines[0], Line::StdoutCr(s) if s == "10%"));
      assert!(matches!(&lines[1], Line::StdoutCr(s) if s == "50%"));
      assert!(matches!(&lines[2], Line::StdoutCr(s) if s == "100%"));
      assert!(buf.is_empty());
   }

   #[test]
   fn feed_cr_then_newline_sends_cr_then_stdout() {
      // "10%\r\n" → StdoutCr("10%") then Stdout("")
      let (lines, buf) = feed_all("10%\r\n", false);
      assert_eq!(lines.len(), 2);
      assert!(matches!(&lines[0], Line::StdoutCr(s) if s == "10%"));
      assert!(matches!(&lines[1], Line::Stdout(s) if s.is_empty()));
      assert!(buf.is_empty());
   }

   #[test]
   fn feed_newline_in_cr_payload_is_preserved() {
      // Text between \r terminators may contain \n as payload (multi-line
      // progress-bar unit). Those \n chars must survive as literal payload
      // inside the StdoutCr line, not be treated as line terminators.
      let (lines, buf) = feed_all("line1\nline2\r", false);
      assert_eq!(lines.len(), 1);
      assert!(matches!(&lines[0], Line::StdoutCr(s) if s == "line1\nline2"));
      assert!(buf.is_empty());
   }

   // -----------------------------------------------------------------------
   // feed() — stderr
   // -----------------------------------------------------------------------

   #[test]
   fn feed_stderr_produces_stderr_lines() {
      let (lines, buf) = feed_all("error msg\n", true);
      assert_eq!(lines.len(), 1);
      assert!(matches!(&lines[0], Line::Stderr(s) if s == "error msg"));
      assert!(buf.is_empty());
   }

   #[test]
   fn feed_stderr_cr_treated_as_newline() {
      // For stderr, \r is treated the same as \n (produces Line::Stderr).
      let (lines, _buf) = feed_all("err\r", true);
      assert_eq!(lines.len(), 1);
      assert!(matches!(&lines[0], Line::Stderr(_)));
   }

   #[test]
   fn feed_stderr_partial_stays_in_buffer() {
      let (lines, buf) = feed_all("partial err", true);
      assert!(lines.is_empty());
      assert_eq!(buf, "partial err");
   }

   // -----------------------------------------------------------------------
   // feed() — empty and edge cases
   // -----------------------------------------------------------------------

   #[test]
   fn feed_empty_input_produces_nothing() {
      let (lines, buf) = feed_all("", false);
      assert!(lines.is_empty());
      assert!(buf.is_empty());
   }

   #[test]
   fn feed_only_newline_produces_empty_stdout_line() {
      let (lines, buf) = feed_all("\n", false);
      assert_eq!(lines.len(), 1);
      assert!(matches!(&lines[0], Line::Stdout(s) if s.is_empty()));
      assert!(buf.is_empty());
   }

   #[test]
   fn feed_only_cr_produces_empty_stdout_cr_line() {
      let (lines, buf) = feed_all("\r", false);
      assert_eq!(lines.len(), 1);
      assert!(matches!(&lines[0], Line::StdoutCr(s) if s.is_empty()));
      assert!(buf.is_empty());
   }

   // -----------------------------------------------------------------------
   // ScriptedShell — run_command result
   // -----------------------------------------------------------------------

   #[test]
   fn scripted_shell_success_result() {
      let shell = ScriptedShell::new().push(Script::new().out_line("step done"));
      let mut out = StringOutput::new();
      let result = shell.run_command("build", "unused", &[], &mut out, default_mode()).unwrap();
      assert!(result.success);
   }

   #[test]
   fn scripted_shell_failure_result() {
      let shell = ScriptedShell::new().push(Script::new().err_line("something broke").exit_failure());
      let mut out = StringOutput::new();
      let result = shell.run_command("deploy", "unused", &[], &mut out, default_mode()).unwrap();
      assert!(!result.success);
   }

   #[test]
   fn scripted_shell_stderr_captured_in_result() {
      let shell = ScriptedShell::new().push(Script::new().err_line("warn: low disk").exit_failure());
      let mut out = StringOutput::new();
      let result = shell.run_command("check", "unused", &[], &mut out, default_mode()).unwrap();
      assert_eq!(result.stderr, "warn: low disk");
   }

   #[test]
   fn scripted_shell_multiple_stderr_lines_joined() {
      let shell =
         ScriptedShell::new().push(Script::new().err_line("error: line 1").err_line("error: line 2").exit_failure());
      let mut out = StringOutput::new();
      let result = shell.run_command("test", "unused", &[], &mut out, default_mode()).unwrap();
      assert_eq!(result.stderr, "error: line 1\nerror: line 2");
   }

   #[test]
   fn scripted_shell_step_result_written_to_output() {
      let shell = ScriptedShell::new().push(Script::new().out_line("ok"));
      let mut out = StringOutput::new();
      shell.run_command("mytask", "unused", &[], &mut out, default_mode()).unwrap();
      // StringOutput::step_result writes "✓ label (elapsed)"
      assert!(out.log().contains("mytask"));
      assert!(out.log().starts_with('✓'));
   }

   #[test]
   fn scripted_shell_failure_step_result_uses_cross() {
      let shell = ScriptedShell::new().push(Script::new().err_line("bad").exit_failure());
      let mut out = StringOutput::new();
      shell.run_command("mytask", "unused", &[], &mut out, default_mode()).unwrap();
      assert!(out.log().starts_with('✗'));
   }

   #[test]
   fn scripted_shell_multiple_scripts_consumed_in_order() {
      let shell = ScriptedShell::new()
         .push(Script::new().out_line("first"))
         .push(Script::new().out_line("second").exit_failure());
      let mut out = StringOutput::new();

      let r1 = shell.run_command("step1", "unused", &[], &mut out, default_mode()).unwrap();
      let r2 = shell.run_command("step2", "unused", &[], &mut out, default_mode()).unwrap();

      assert!(r1.success);
      assert!(!r2.success);
   }

   #[test]
   fn scripted_shell_empty_queue_panics() {
      let shell = ScriptedShell::new();
      let mut out = StringOutput::new();
      let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
         shell.run_command("oops", "unused", &[], &mut out, default_mode()).unwrap();
      }));
      assert!(result.is_err());
   }

   // -----------------------------------------------------------------------
   // ScriptedShell — other Shell trait methods
   // -----------------------------------------------------------------------

   #[test]
   fn scripted_shell_shell_exec_returns_success() {
      let shell = ScriptedShell::new();
      let mut out = StringOutput::new();
      let result = shell.shell_exec("echo hi", &mut out, default_mode()).unwrap();
      assert!(result.success);
   }

   #[test]
   fn scripted_shell_command_exists_returns_true() {
      let shell = ScriptedShell::new();
      assert!(shell.command_exists("anything"));
   }

   #[test]
   fn scripted_shell_command_output_returns_empty_string() {
      let shell = ScriptedShell::new();
      let result = shell.command_output("anything", &["--version"]).unwrap();
      assert_eq!(result, "");
   }

   #[test]
   fn scripted_shell_exec_capture_returns_success() {
      let shell = ScriptedShell::new();
      let mut out = StringOutput::new();
      let result = shell.exec_capture("echo hi", &mut out, default_mode()).unwrap();
      assert!(result.success);
   }

   #[test]
   fn scripted_shell_exec_interactive_returns_ok() {
      let shell = ScriptedShell::new();
      let mut out = StringOutput::new();
      assert!(shell.exec_interactive("echo hi", &mut out, default_mode()).is_ok());
   }

   // -----------------------------------------------------------------------
   // ScriptedShell — custom viewport_size via with_config
   // -----------------------------------------------------------------------

   #[test]
   fn scripted_shell_with_config_accepts_custom_viewport() {
      let config = ShellConfig { viewport_size: 2 };
      let shell = ScriptedShell::new()
         .with_config(config)
         .push(Script::new().out_line("line 1").out_line("line 2").out_line("line 3"));
      let mut out = StringOutput::new();
      // Should not panic; simply verify the call completes successfully.
      let result = shell.run_command("task", "unused", &[], &mut out, default_mode()).unwrap();
      assert!(result.success);
   }
}

/// Append `s` to `buf`, sending one `Line` per `\r` or `\n` terminator encountered.
///
/// The terminator character determines the `Line` variant:
/// - `\r` → `Line::StdoutCr`: overwrites the last viewport slot in place.
/// - `\n` → `Line::Stdout` / `Line::Stderr`: appends a new viewport slot.
///
/// Crucially, `\r` takes precedence over any `\n` characters embedded within
/// the same chunk. The input is therefore split on `\r` first; within each
/// `\r`-terminated segment the `\n` characters are **payload** (they represent
/// sub-rows of a multi-line progress-bar unit) and are preserved verbatim in
/// the emitted string. Only within segments that are ultimately `\n`-terminated
/// (i.e. no `\r` follows) are embedded `\n` characters treated as line breaks.
///
/// Any text after the final terminator remains buffered for the next call.
fn feed(s: &str, buf: &mut String, is_stderr: bool, tx: &mpsc::Sender<Line>) {
   // Split on \r first to identify CR-terminated chunks.
   let mut segments = s.split('\r').peekable();

   while let Some(seg) = segments.next() {
      let is_last = segments.peek().is_none();

      if is_last {
         // Tail after the last \r (or the whole string if no \r present).
         // Within this tail, \n characters are genuine line terminators.
         if is_stderr {
            for ch in seg.chars() {
               if ch == '\n' {
                  let line = std::mem::take(buf);
                  let _ = tx.send(Line::Stderr(line));
               } else {
                  buf.push(ch);
               }
            }
         } else {
            for ch in seg.chars() {
               if ch == '\n' {
                  let line = std::mem::take(buf);
                  let _ = tx.send(Line::Stdout(line));
               } else {
                  buf.push(ch);
               }
            }
         }
      } else {
         // This segment is followed by a \r, so the whole accumulated
         // content (buf + seg, with any \n kept as payload) becomes a
         // StdoutCr line.  Stderr never uses \r.
         buf.push_str(seg);
         let line = std::mem::take(buf);
         if is_stderr {
            // Treat \r as \n for stderr (shouldn't normally occur).
            let _ = tx.send(Line::Stderr(line));
         } else {
            let _ = tx.send(Line::StdoutCr(line));
         }
      }
   }
}
