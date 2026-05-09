//! The [`Shell`](crate::shell::Shell) trait and its standard implementations.
//!
//! # Overview
//!
//! - [`Shell`](crate::shell::Shell) — the core trait; implement it to control how processes are
//!   spawned.
//! - [`ProcessShell`](crate::shell::ProcessShell) — the production implementation; spawns real OS
//!   processes.
//! - [`DryRunShell`](crate::shell::DryRunShell) — logs what would be executed and returns fake
//!   success; safe to use in dry-run workflows.
//! - [`MockShell`](crate::shell::MockShell) — records calls and returns configurable results;
//!   intended for unit tests.
//! - [`ScriptedShell`](crate::shell::scripted::ScriptedShell) — drives the real spinner overlay
//!   using pre-configured output scripts; intended for overlay integration tests.
//!
//! Use [`create`](crate::shell::create) to get a boxed `Shell` at runtime based on a `dry_run`
//! flag.

use std::{
   io,
   process::{Command, Stdio}
};

use crate::output::{Output, OutputMode};

mod exec;
pub mod scripted;

pub use exec::{run_command, run_passthrough};

// ---------------------------------------------------------------------------
// ShellConfig
// ---------------------------------------------------------------------------

/// Configuration for shell execution behaviour.
///
/// Build one explicitly or use `ShellConfig::default()` to get sensible
/// defaults (viewport height of 5 lines).
#[derive(Debug, Clone)]
pub struct ShellConfig {
   /// Number of output lines visible in the animated overlay viewport.
   /// Older lines scroll out of view once this limit is reached.
   pub viewport_size: usize
}

impl Default for ShellConfig {
   fn default() -> Self {
      Self { viewport_size: 5 }
   }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors that can be returned by [`Shell`] methods.
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
   /// The OS refused to spawn the process (e.g. binary not found, permission denied).
   #[error("failed to spawn '{0}': {1}")]
   Spawn(String, io::Error),
   /// The process was spawned but waiting on it failed.
   #[error("failed to wait on '{0}': {1}")]
   Wait(String, io::Error),
   /// The process exited with a non-zero status code.
   #[error("command failed: {0}")]
   Failed(String)
}

// ---------------------------------------------------------------------------
// CommandResult
// ---------------------------------------------------------------------------

/// The outcome of a completed shell command.
#[derive(Debug)]
pub struct CommandResult {
   /// `true` when the process exited with status 0.
   pub success: bool,
   /// The numeric exit code of the process, if available.
   /// `None` for dry-run, mock, and scripted shells, or on platforms where
   /// the process was terminated by a signal rather than a normal exit.
   pub code: Option<i32>,
   /// All stderr output collected from the process, joined with newlines.
   pub stderr: String
}

impl CommandResult {
   /// Return `Ok(())` if the command succeeded, or a `ShellError::Failed` whose
   /// message is `"{cmd} failed"`. Callers that want to embed app-specific advice
   /// (e.g. "run with --verbose") should use [`CommandResult::require_success_with_hint`]
   /// or wrap this in an extension trait.
   pub fn require_success(self, cmd: &str) -> Result<(), ShellError> {
      if self.success {
         Ok(())
      } else {
         Err(ShellError::Failed(format!("{cmd} failed")))
      }
   }

   /// Return `Ok(())` if the command succeeded, or a `ShellError::Failed` whose
   /// message is `"{cmd} failed — {hint}"`. Use this when the application has
   /// concrete recovery advice to surface to the user.
   pub fn require_success_with_hint(self, cmd: &str, hint: &str) -> Result<(), ShellError> {
      if self.success {
         Ok(())
      } else {
         Err(ShellError::Failed(format!("{cmd} failed — {hint}")))
      }
   }

   /// Return `Ok(())` if the command succeeded, or call `err` with the captured
   /// stderr string to produce an error value of any type.
   pub fn check<E>(self, err: impl FnOnce(String) -> E) -> Result<(), E> {
      if self.success { Ok(()) } else { Err(err(self.stderr)) }
   }
}

// ---------------------------------------------------------------------------
// Shell trait
// ---------------------------------------------------------------------------

/// Abstraction over shell execution, enabling unit tests to mock process spawning.
pub trait Shell {
   /// Run `program` with `args`, displaying progress under `label`.
   ///
   /// Output behavior is controlled by `mode`:
   /// - **quiet**: output is collected silently.
   /// - **verbose**: each line is echoed with a `> ` prefix.
   /// - **default**: an animated spinner overlay is shown.
   /// # Errors
   ///
   /// Returns a [`ShellError`] if the process cannot be spawned, waited on, or
   /// exits with a non-zero status.
   fn run_command(
      &self,
      label: &str,
      program: &str,
      args: &[&str],
      output: &mut dyn Output,
      mode: OutputMode
   ) -> Result<CommandResult, ShellError>;

   /// Run an arbitrary shell script string (passed to `bash -c` / `powershell -Command`).
   /// # Errors
   ///
   /// Returns a [`ShellError`] if the shell process cannot be spawned or fails.
   fn shell_exec(&self, script: &str, output: &mut dyn Output, mode: OutputMode) -> Result<CommandResult, ShellError>;

   /// Return `true` when `program` can be found on `PATH`.
   fn command_exists(&self, program: &str) -> bool;

   /// Run `program args` and return its captured stdout as a trimmed `String`.
   /// # Errors
   ///
   /// Returns a [`ShellError`] if the process cannot be spawned or exits with a non-zero status.
   fn command_output(&self, program: &str, args: &[&str]) -> Result<String, ShellError>;

   /// Run a shell command, capturing stdout/stderr silently without display.
   /// In dry-run mode (`DryRunShell`), logs the command and returns success without executing.
   /// # Errors
   ///
   /// Returns a [`ShellError`] if the process cannot be spawned or waited on.
   fn exec_capture(&self, cmd: &str, output: &mut dyn Output, mode: OutputMode) -> Result<CommandResult, ShellError>;

   /// Run a shell command with inherited stdio (for interactive flows like `aws sso login`).
   /// In dry-run mode (`DryRunShell`), logs the command and returns success without executing.
   /// # Errors
   ///
   /// Returns a [`ShellError`] if the process cannot be spawned or waited on.
   fn exec_interactive(&self, cmd: &str, output: &mut dyn Output, mode: OutputMode) -> Result<(), ShellError>;
}

/// Returns a `DryRunShell` when `dry_run` is true, otherwise a `ProcessShell`.
///
/// Both shells are configured with `ShellConfig::default()`.
/// Use `ProcessShell` or `DryRunShell` directly if you need custom config.
#[must_use]
pub fn create(dry_run: bool) -> Box<dyn Shell> {
   let config = ShellConfig::default();
   if dry_run { Box::new(DryRunShell { config }) } else { Box::new(ProcessShell { config }) }
}

// ---------------------------------------------------------------------------
// ProcessShell
// ---------------------------------------------------------------------------

/// Production shell: delegates to the free functions in this module.
#[derive(Default)]
pub struct ProcessShell {
   /// Shell execution configuration (e.g. overlay viewport height).
   pub config: ShellConfig
}

impl Shell for ProcessShell {
   fn run_command(
      &self,
      label: &str,
      program: &str,
      args: &[&str],
      output: &mut dyn Output,
      mode: OutputMode
   ) -> Result<CommandResult, ShellError> {
      exec::run_command(label, program, args, output, mode, self.config.viewport_size)
   }

   fn shell_exec(&self, script: &str, output: &mut dyn Output, mode: OutputMode) -> Result<CommandResult, ShellError> {
      shell_exec(script, output, mode, self.config.viewport_size)
   }

   fn command_exists(&self, program: &str) -> bool {
      command_exists(program)
   }

   fn command_output(&self, program: &str, args: &[&str]) -> Result<String, ShellError> {
      command_output(program, args)
   }

   fn exec_capture(&self, cmd: &str, _output: &mut dyn Output, _mode: OutputMode) -> Result<CommandResult, ShellError> {
      #[cfg(unix)]
      let (program, flag) = ("bash", "-c");
      #[cfg(windows)]
      let (program, flag) = ("powershell", "-Command");
      exec::run_quiet(program, &[flag, cmd])
   }

   fn exec_interactive(&self, cmd: &str, _output: &mut dyn Output, _mode: OutputMode) -> Result<(), ShellError> {
      #[cfg(unix)]
      let (program, flag) = ("bash", "-c");
      #[cfg(windows)]
      let (program, flag) = ("powershell", "-Command");

      let status = Command::new(program)
         .args([flag, cmd])
         .stdin(Stdio::inherit())
         .stdout(Stdio::inherit())
         .stderr(Stdio::inherit())
         .spawn()
         .map_err(|e| ShellError::Spawn(program.to_string(), e))?
         .wait()
         .map_err(|e| ShellError::Wait(program.to_string(), e))?;

      if status.success() {
         Ok(())
      } else {
         Err(ShellError::Failed(format!("'{cmd}' exited with {}", status.code().unwrap_or(-1))))
      }
   }
}

// ---------------------------------------------------------------------------
// DryRunShell
// ---------------------------------------------------------------------------

/// Dry-run shell: logs what would be executed and returns fake success.
/// Probe methods (`command_exists`, `command_output`) delegate to real implementations
/// because they are read-only and safe to call.
#[derive(Default)]
pub struct DryRunShell {
   /// Shell execution configuration (e.g. overlay viewport height).
   pub config: ShellConfig
}

impl Shell for DryRunShell {
   fn run_command(
      &self,
      _label: &str,
      program: &str,
      args: &[&str],
      output: &mut dyn Output,
      _mode: OutputMode
   ) -> Result<CommandResult, ShellError> {
      output.dry_run_shell(&format_command(program, args));
      Ok(CommandResult { success: true, code: None, stderr: String::new() })
   }

   fn shell_exec(&self, script: &str, output: &mut dyn Output, _mode: OutputMode) -> Result<CommandResult, ShellError> {
      output.dry_run_shell(script);
      Ok(CommandResult { success: true, code: None, stderr: String::new() })
   }

   fn command_exists(&self, program: &str) -> bool {
      command_exists(program)
   }

   fn command_output(&self, program: &str, args: &[&str]) -> Result<String, ShellError> {
      command_output(program, args)
   }

   fn exec_capture(&self, cmd: &str, output: &mut dyn Output, _mode: OutputMode) -> Result<CommandResult, ShellError> {
      output.dry_run_shell(cmd);
      Ok(CommandResult { success: true, code: None, stderr: String::new() })
   }

   fn exec_interactive(&self, cmd: &str, output: &mut dyn Output, _mode: OutputMode) -> Result<(), ShellError> {
      output.dry_run_shell(cmd);
      Ok(())
   }
}

// ---------------------------------------------------------------------------
// MockShell (test only)
// ---------------------------------------------------------------------------

/// Mock shell for unit tests: records calls and returns configurable results.
///
/// Intended for **testing use**. Not gated behind `#[cfg(test)]` so that downstream
/// crates can use it in their own test suites; LTO eliminates it from production builds.
pub struct MockShell {
   /// Ordered log of every call made to this shell, formatted as `"program arg1 arg2"`.
   pub calls: std::cell::RefCell<Vec<String>>,
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
   pub exec_capture_results: std::cell::RefCell<std::collections::VecDeque<CommandResult>>
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
         calls: std::cell::RefCell::new(Vec::new()),
         run_success: true,
         command_exists_result: true,
         command_output_value: String::new(),
         command_output_ok: true,
         exec_capture_results: std::cell::RefCell::new(std::collections::VecDeque::new())
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

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

fn format_command(program: &str, args: &[&str]) -> String {
   std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" ")
}

/// Extract the program name and arguments from a `Command` as owned strings.
#[must_use]
pub fn command_parts(cmd: &Command) -> (String, Vec<String>) {
   let program = cmd.get_program().to_string_lossy().into_owned();
   let args = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
   (program, args)
}

/// Check if a program is on PATH.
///
/// Scans `PATH` directories directly instead of spawning `which`/`where.exe`,
/// avoiding a subprocess fork on every call.
#[must_use]
pub fn command_exists(program: &str) -> bool {
   let path_var = std::env::var_os("PATH").unwrap_or_default();
   let sep = if cfg!(windows) { ';' } else { ':' };
   for dir in path_var.to_string_lossy().split(sep) {
      if dir.is_empty() {
         continue;
      }
      let candidate = std::path::Path::new(dir).join(program);
      if candidate.is_file() {
         return true;
      }
      #[cfg(windows)]
      {
         for ext in &["exe", "cmd", "bat", "com"] {
            let with_ext = candidate.with_extension(ext);
            if with_ext.is_file() {
               return true;
            }
         }
      }
   }
   false
}

/// Run a command and return its stdout (trimmed).
///
/// # Errors
///
/// Returns [`ShellError::Spawn`] if the process cannot be started, or
/// [`ShellError::Failed`] if the command exits with a non-zero status.
pub fn command_output(program: &str, args: &[&str]) -> Result<String, ShellError> {
   let output = Command::new(program)
      .args(args)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .output()
      .map_err(|e| ShellError::Spawn(program.to_string(), e))?;

   if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(ShellError::Failed(format!(
         "'{program}' exited with {}: {}",
         output.status.code().map_or_else(|| "signal".to_string(), |c| c.to_string()),
         stderr.trim(),
      )));
   }

   Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Execute a script via the system shell.
///
/// Unix: `bash -c "script"`, Windows: `powershell -Command "script"`.
///
/// # Errors
///
/// Returns a [`ShellError`] if the process cannot be spawned or the command
/// exits with a non-zero status.
pub fn shell_exec(
   script: &str,
   output: &mut dyn Output,
   mode: OutputMode,
   viewport_size: usize
) -> Result<CommandResult, ShellError> {
   #[cfg(unix)]
   let (program, shell_args) = ("bash", vec!["-c", script]);
   #[cfg(windows)]
   let (program, shell_args) = ("powershell", vec!["-Command", script]);

   exec::run_command(&format!("Running: {script}"), program, &shell_args, output, mode, viewport_size)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
   use rstest::rstest;

   use super::{CommandResult, DryRunShell, MockShell, Shell, ShellConfig, ShellError, format_command};
   use crate::output::{OutputMode, StringOutput};

   // -----------------------------------------------------------------------
   // Helpers
   // -----------------------------------------------------------------------

   fn default_mode() -> OutputMode {
      OutputMode::default()
   }

   fn dry_run_mode() -> OutputMode {
      OutputMode { dry_run: true, ..Default::default() }
   }

   // -----------------------------------------------------------------------
   // ShellConfig
   // -----------------------------------------------------------------------

   #[test]
   fn shell_config_default_viewport_size_is_five() {
      assert_eq!(ShellConfig::default().viewport_size, 5);
   }

   #[test]
   fn shell_config_clone_is_equal() {
      let cfg = ShellConfig { viewport_size: 10 };
      let cloned = cfg.clone();
      assert_eq!(cloned.viewport_size, 10);
   }

   // -----------------------------------------------------------------------
   // format_command (tested indirectly via MockShell call recording)
   // -----------------------------------------------------------------------

   #[rstest]
   #[case("echo", &[], "echo")]
   #[case("git", &["status"], "git status")]
   #[case("cargo", &["build", "--release"], "cargo build --release")]
   fn format_command_joins_program_and_args(#[case] program: &str, #[case] args: &[&str], #[case] expected: &str) {
      assert_eq!(format_command(program, args), expected);
   }

   // -----------------------------------------------------------------------
   // DryRunShell — run_command
   // -----------------------------------------------------------------------

   #[test]
   fn dry_run_shell_run_command_emits_dry_run_line() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      let result = shell.run_command("build", "cargo", &["build"], &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: cargo build"));
      assert!(result.success);
      assert!(result.stderr.is_empty());
   }

   #[test]
   fn dry_run_shell_run_command_no_args_emits_program_only() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      shell.run_command("check", "whoami", &[], &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: whoami"));
   }

   // -----------------------------------------------------------------------
   // DryRunShell — shell_exec
   // -----------------------------------------------------------------------

   #[test]
   fn dry_run_shell_shell_exec_emits_script() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      let result = shell.shell_exec("echo hello && echo world", &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: echo hello && echo world"));
      assert!(result.success);
      assert!(result.stderr.is_empty());
   }

   // -----------------------------------------------------------------------
   // DryRunShell — exec_capture
   // -----------------------------------------------------------------------

   #[test]
   fn dry_run_shell_exec_capture_emits_command() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      let result = shell.exec_capture("ls -la", &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: ls -la"));
      assert!(result.success);
      assert!(result.stderr.is_empty());
   }

   // -----------------------------------------------------------------------
   // DryRunShell — exec_interactive
   // -----------------------------------------------------------------------

   #[test]
   fn dry_run_shell_exec_interactive_emits_command() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      shell.exec_interactive("aws sso login", &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: aws sso login"));
   }

   // -----------------------------------------------------------------------
   // DryRunShell — probe methods delegate to real OS
   // -----------------------------------------------------------------------

   #[test]
   fn dry_run_shell_command_exists_delegates_to_real_os() {
      let shell = DryRunShell::default();
      // "sh" is universally available on Unix; this just checks it doesn't panic.
      let _ = shell.command_exists("sh");
   }

   #[test]
   fn dry_run_shell_command_output_delegates_to_real_os() {
      let shell = DryRunShell::default();
      // A benign read-only command available everywhere.
      let result = shell.command_output("echo", &["hello"]);
      assert!(result.is_ok());
      assert_eq!(result.unwrap(), "hello");
   }

   // -----------------------------------------------------------------------
   // DryRunShell — mode independence
   // -----------------------------------------------------------------------

   #[test]
   fn dry_run_shell_run_command_emits_regardless_of_mode_flag() {
      // DryRunShell ignores the OutputMode — it always dry-runs.
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      shell.run_command("x", "true", &[], &mut out, dry_run_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: true"));
   }

   // -----------------------------------------------------------------------
   // MockShell — call recording
   // -----------------------------------------------------------------------

   #[test]
   fn mock_shell_records_run_command_call() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.run_command("label", "git", &["status"], &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["git status"]);
   }

   #[test]
   fn mock_shell_records_multiple_calls_in_order() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.run_command("a", "echo", &["one"], &mut out, default_mode()).unwrap();
      shell.run_command("b", "echo", &["two"], &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["echo one", "echo two"]);
   }

   #[test]
   fn mock_shell_records_shell_exec_call() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.shell_exec("npm install", &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["shell_exec: npm install"]);
   }

   #[test]
   fn mock_shell_records_command_output_call() {
      let shell = MockShell::new();
      let _ = shell.command_output("node", &["--version"]);
      assert_eq!(shell.calls(), vec!["node --version"]);
   }

   #[test]
   fn mock_shell_records_exec_capture_call() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.exec_capture("date", &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["exec_capture: date"]);
   }

   #[test]
   fn mock_shell_records_exec_interactive_call() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      shell.exec_interactive("aws sso login", &mut out, default_mode()).unwrap();
      assert_eq!(shell.calls(), vec!["interactive: aws sso login"]);
   }

   // -----------------------------------------------------------------------
   // MockShell — run_success flag
   // -----------------------------------------------------------------------

   #[test]
   fn mock_shell_run_command_returns_success_by_default() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      let result = shell.run_command("x", "true", &[], &mut out, default_mode()).unwrap();
      assert!(result.success);
   }

   #[test]
   fn mock_shell_run_command_returns_failure_when_configured() {
      let mut shell = MockShell::new();
      shell.run_success = false;
      let mut out = StringOutput::new();
      let result = shell.run_command("x", "false", &[], &mut out, default_mode()).unwrap();
      assert!(!result.success);
   }

   #[test]
   fn mock_shell_shell_exec_honours_run_success() {
      let mut shell = MockShell::new();
      shell.run_success = false;
      let mut out = StringOutput::new();
      let result = shell.shell_exec("bad script", &mut out, default_mode()).unwrap();
      assert!(!result.success);
   }

   // -----------------------------------------------------------------------
   // MockShell — command_exists_result flag
   // -----------------------------------------------------------------------

   #[test]
   fn mock_shell_command_exists_true_by_default() {
      let shell = MockShell::new();
      assert!(shell.command_exists("anything"));
   }

   #[test]
   fn mock_shell_command_exists_false_when_configured() {
      let mut shell = MockShell::new();
      shell.command_exists_result = false;
      assert!(!shell.command_exists("anything"));
   }

   // -----------------------------------------------------------------------
   // MockShell — command_output value and ok flag
   // -----------------------------------------------------------------------

   #[test]
   fn mock_shell_command_output_returns_configured_value() {
      let mut shell = MockShell::new();
      shell.command_output_value = "v18.0.0".to_string();
      let result = shell.command_output("node", &["--version"]).unwrap();
      assert_eq!(result, "v18.0.0");
   }

   #[test]
   fn mock_shell_command_output_returns_err_when_ok_is_false() {
      let mut shell = MockShell::new();
      shell.command_output_ok = false;
      let result = shell.command_output("node", &["--version"]);
      assert!(matches!(result, Err(ShellError::Failed(_))));
   }

   #[test]
   fn mock_shell_command_output_error_still_records_call() {
      let mut shell = MockShell::new();
      shell.command_output_ok = false;
      let _ = shell.command_output("node", &["--version"]);
      assert_eq!(shell.calls(), vec!["node --version"]);
   }

   // -----------------------------------------------------------------------
   // MockShell — exec_capture_results queue
   // -----------------------------------------------------------------------

   #[test]
   fn mock_shell_exec_capture_pops_from_queue() {
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
   fn mock_shell_exec_capture_falls_back_to_run_success_when_queue_empty() {
      let mut shell = MockShell::new();
      shell.run_success = false;
      let mut out = StringOutput::new();
      let result = shell.exec_capture("any cmd", &mut out, default_mode()).unwrap();
      assert!(!result.success);
   }

   #[test]
   fn mock_shell_exec_capture_queue_consumed_in_order() {
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
   // MockShell — exec_interactive
   // -----------------------------------------------------------------------

   #[test]
   fn mock_shell_exec_interactive_returns_ok() {
      let shell = MockShell::new();
      let mut out = StringOutput::new();
      assert!(shell.exec_interactive("interactive cmd", &mut out, default_mode()).is_ok());
   }

   // -----------------------------------------------------------------------
   // MockShell — mixed call sequence
   // -----------------------------------------------------------------------

   #[test]
   fn mock_shell_mixed_calls_all_recorded() {
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
