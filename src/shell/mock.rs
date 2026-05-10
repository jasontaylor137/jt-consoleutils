//! Mock [`Shell`](super::Shell) implementation for unit tests.

use std::{cell::RefCell, collections::VecDeque};

use super::{CommandResult, Shell, ShellError, helpers::format_command};
use crate::output::{Output, OutputMode};

/// Mock shell for unit tests: records calls and returns configurable results.
///
/// Intended for **testing use**. Not gated behind `#[cfg(test)]` so that downstream
/// crates can use it in their own test suites. The type is always compiled into the
/// crate; rustc's dead-code elimination (and LTO, when enabled) can drop it from a
/// release binary that never constructs it or holds a `dyn Shell` pointing at it,
/// but exclusion is not guaranteed. Consumers that require guaranteed exclusion
/// should put their test-only construction sites behind `#[cfg(test)]` or a feature
/// flag of their own.
pub struct MockShell {
   /// Ordered log of every call made to this shell, formatted as `"program arg1 arg2"`.
   pub calls: RefCell<Vec<String>>,
   /// Value returned by `run_command` / `shell_exec` / `exec_capture`. Defaults to `true`.
   pub run_success: bool,
   /// Value returned by `command_exists`. Defaults to `true`.
   pub command_exists_result: bool,
   /// Stdout value returned by `command_output` when `command_output_ok` is `true`.
   pub command_output_value: String,
   /// When false, `command_output` returns `Err` (e.g. to simulate a tool not installed).
   pub command_output_ok: bool,
   /// Queue of results for `exec_capture` calls; pops front on each call.
   /// If empty, falls back to `CommandResult { success: run_success, stderr: "" }`.
   pub exec_capture_results: RefCell<VecDeque<CommandResult>>
}

impl Default for MockShell {
   fn default() -> Self {
      Self::new()
   }
}

impl MockShell {
   /// Create a new `MockShell` with all success flags set to `true` and empty recorded calls.
   #[must_use]
   pub const fn new() -> Self {
      Self {
         calls: RefCell::new(Vec::new()),
         run_success: true,
         command_exists_result: true,
         command_output_value: String::new(),
         command_output_ok: true,
         exec_capture_results: RefCell::new(VecDeque::new())
      }
   }

   /// Return a snapshot of all calls recorded so far.
   pub fn calls(&self) -> Vec<String> {
      self.calls.borrow().clone()
   }

   /// Push a `CommandResult` onto the back of the `exec_capture` queue.
   ///
   /// Each call to `exec_capture` pops one result from the front. Use this
   /// instead of mutating `exec_capture_results` directly so callers don't
   /// need to know the underlying container type.
   pub fn push_capture(&self, result: CommandResult) {
      self.exec_capture_results.borrow_mut().push_back(result);
   }
}

impl Shell for MockShell {
   fn run_command(
      &self,
      _label: &str,
      program: &str,
      args: &[&str],
      _output: &mut dyn Output,
      _mode: OutputMode
   ) -> Result<CommandResult, ShellError> {
      self.calls.borrow_mut().push(format_command(program, args));
      Ok(CommandResult { success: self.run_success, code: None, stderr: String::new() })
   }

   fn shell_exec(
      &self,
      script: &str,
      _output: &mut dyn Output,
      _mode: OutputMode
   ) -> Result<CommandResult, ShellError> {
      self.calls.borrow_mut().push(format!("shell_exec: {script}"));
      Ok(CommandResult { success: self.run_success, code: None, stderr: String::new() })
   }

   fn command_exists(&self, _program: &str) -> bool {
      self.command_exists_result
   }

   fn command_output(&self, program: &str, args: &[&str]) -> Result<String, ShellError> {
      let call = format_command(program, args);
      self.calls.borrow_mut().push(call.clone());
      if !self.command_output_ok {
         return Err(ShellError::Failed(format!("'{call}' failed (mocked)")));
      }
      Ok(self.command_output_value.clone())
   }

   fn exec_capture(&self, cmd: &str, _output: &mut dyn Output, _mode: OutputMode) -> Result<CommandResult, ShellError> {
      self.calls.borrow_mut().push(format!("exec_capture: {cmd}"));
      let result = self.exec_capture_results.borrow_mut().pop_front().unwrap_or_else(|| CommandResult {
         success: self.run_success,
         code: None,
         stderr: String::new()
      });
      Ok(result)
   }

   fn exec_interactive(&self, cmd: &str, _output: &mut dyn Output, _mode: OutputMode) -> Result<(), ShellError> {
      self.calls.borrow_mut().push(format!("interactive: {cmd}"));
      Ok(())
   }
}

#[cfg(test)]
mod tests {
   use super::MockShell;
   use crate::{
      output::{OutputMode, StringOutput},
      shell::{CommandResult, Shell, ShellError}
   };

   fn default_mode() -> OutputMode {
      OutputMode::default()
   }

   // -----------------------------------------------------------------------
   // Call recording
   // -----------------------------------------------------------------------

   #[test]
   fn records_run_command_call() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.run_command("label", "git", &["status"], &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["git status"]);
   }

   #[test]
   fn records_multiple_calls_in_order() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.run_command("a", "echo", &["one"], &mut out, default_mode()).unwrap();
      shell.run_command("b", "echo", &["two"], &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["echo one", "echo two"]);
   }

   #[test]
   fn records_shell_exec_call() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.shell_exec("npm install", &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["shell_exec: npm install"]);
   }

   #[test]
   fn records_command_output_call() {
      let shell = MockShell::new();
      let _ = shell.command_output("node", &["--version"]);
      assert_eq!(shell.calls(), vec!["node --version"]);
   }

   #[test]
   fn records_exec_capture_call() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.exec_capture("date", &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["exec_capture: date"]);
   }

   #[test]
   fn records_exec_interactive_call() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.exec_interactive("aws sso login", &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["interactive: aws sso login"]);
   }

   // -----------------------------------------------------------------------
   // run_success flag
   // -----------------------------------------------------------------------

   #[test]
   fn run_command_returns_success_by_default() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      let result = shell.run_command("x", "true", &[], &mut out, default_mode()).unwrap();
      assert!(result.success);
   }

   #[test]
   fn run_command_returns_failure_when_configured() {
      let mut shell = MockShell::new();
      shell.run_success = false;
      let mut out = StringOutput::new();
      let result = shell.run_command("x", "false", &[], &mut out, default_mode()).unwrap();
      assert!(!result.success);
   }

   #[test]
   fn shell_exec_honours_run_success() {
      let mut shell = MockShell::new();
      shell.run_success = false;
      let mut out = StringOutput::new();
      let result = shell.shell_exec("bad script", &mut out, default_mode()).unwrap();
      assert!(!result.success);
   }

   // -----------------------------------------------------------------------
   // command_exists_result flag
   // -----------------------------------------------------------------------

   #[test]
   fn command_exists_true_by_default() {
      let shell = MockShell::new();
      assert!(shell.command_exists("anything"));
   }

   #[test]
   fn command_exists_false_when_configured() {
      let mut shell = MockShell::new();
      shell.command_exists_result = false;
      assert!(!shell.command_exists("anything"));
   }

   // -----------------------------------------------------------------------
   // command_output value and ok flag
   // -----------------------------------------------------------------------

   #[test]
   fn command_output_returns_configured_value() {
      let mut shell = MockShell::new();
      shell.command_output_value = "v18.0.0".to_string();
      let result = shell.command_output("node", &["--version"]).unwrap();
      assert_eq!(result, "v18.0.0");
   }

   #[test]
   fn command_output_returns_err_when_ok_is_false() {
      let mut shell = MockShell::new();
      shell.command_output_ok = false;
      let result = shell.command_output("node", &["--version"]);
      assert!(matches!(result, Err(ShellError::Failed(_))));
   }

   #[test]
   fn command_output_error_still_records_call() {
      let mut shell = MockShell::new();
      shell.command_output_ok = false;
      let _ = shell.command_output("node", &["--version"]);
      assert_eq!(shell.calls(), vec!["node --version"]);
   }

   // -----------------------------------------------------------------------
   // exec_capture_results queue
   // -----------------------------------------------------------------------

   #[test]
   fn exec_capture_pops_from_queue() {
      let shell = MockShell::new();
      shell.exec_capture_results.borrow_mut().push_back(CommandResult {
         success: false,
         code: None,
         stderr: "queue error".to_string()
      });
      let mut out = StringOutput::new();
      let result = shell.exec_capture("any cmd", &mut out, default_mode()).unwrap();
      assert!(!result.success);
      assert_eq!(result.stderr, "queue error");
   }

   #[test]
   fn exec_capture_falls_back_to_run_success_when_queue_empty() {
      let mut shell = MockShell::new();
      shell.run_success = false;
      let mut out = StringOutput::new();
      let result = shell.exec_capture("any cmd", &mut out, default_mode()).unwrap();
      assert!(!result.success);
   }

   #[test]
   fn exec_capture_queue_consumed_in_order() {
      let shell = MockShell::new();
      shell.exec_capture_results.borrow_mut().push_back(CommandResult {
         success: true,
         code: None,
         stderr: String::new()
      });
      shell.exec_capture_results.borrow_mut().push_back(CommandResult {
         success: false,
         code: None,
         stderr: "second".to_string()
      });
      let mut out = StringOutput::new();
      let r1 = shell.exec_capture("cmd", &mut out, default_mode()).unwrap();
      let r2 = shell.exec_capture("cmd", &mut out, default_mode()).unwrap();
      assert!(r1.success);
      assert!(!r2.success);
      assert_eq!(r2.stderr, "second");
   }

   // -----------------------------------------------------------------------
   // exec_interactive
   // -----------------------------------------------------------------------

   #[test]
   fn exec_interactive_returns_ok() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      assert!(shell.exec_interactive("interactive cmd", &mut out, default_mode()).is_ok());
   }

   // -----------------------------------------------------------------------
   // Mixed call sequence
   // -----------------------------------------------------------------------

   #[test]
   fn mixed_calls_all_recorded() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.run_command("a", "git", &["fetch"], &mut out, default_mode()).unwrap();
      shell.shell_exec("make build", &mut out, default_mode()).unwrap();
      shell.exec_capture("docker ps", &mut out, default_mode()).unwrap();
      shell.exec_interactive("ssh host", &mut out, default_mode()).unwrap();
      let calls = shell.calls();
      assert_eq!(calls[0], "git fetch");
      assert_eq!(calls[1], "shell_exec: make build");
      assert_eq!(calls[2], "exec_capture: docker ps");
      assert_eq!(calls[3], "interactive: ssh host");
   }
}
