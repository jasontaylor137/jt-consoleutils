//! The [`Shell`] trait and its standard implementations.
//!
//! # Overview
//!
//! - [`Shell`] — the core trait; implement it to control how processes are spawned.
//! - [`ProcessShell`] — the production implementation; spawns real OS processes.
//! - [`DryRunShell`] — logs what would be executed and returns fake success; safe to use in dry-run
//!   workflows.
//! - [`MockShell`] — records calls and returns configurable results; intended for unit tests.
//! - [`scripted::ScriptedShell`] — drives the real spinner overlay using pre-configured output
//!   scripts; intended for overlay integration tests.
//!
//! Use [`create`] to get a boxed `Shell` at runtime based on a `dry_run` flag.

use std::io;

use crate::output::{Output, OutputMode};

mod dry;
mod exec;
mod helpers;
mod mock;
mod process;
pub mod scripted;

pub use dry::DryRunShell;
pub use exec::{run_command, run_passthrough};
pub use helpers::{command_exists, command_output, command_parts, shell_exec};
pub use mock::MockShell;
pub use process::ProcessShell;

/// Configuration for shell execution behaviour.
///
/// Build one explicitly or use `ShellConfig::default()` to get sensible
/// defaults (viewport height of 5 lines, auto-detected shell program).
#[derive(Debug, Clone)]
pub struct ShellConfig {
   /// Number of output lines visible in the animated overlay viewport.
   /// Older lines scroll out of view once this limit is reached.
   pub viewport_size: usize,
   /// Optional override for the shell program used by `shell_exec`,
   /// `exec_capture`, and `exec_interactive`: `(program, flag)`, e.g.
   /// `("zsh".into(), "-c".into())` or `("pwsh".into(), "-Command".into())`.
   ///
   /// When `None`, the program is auto-detected at call time: `$SHELL` on
   /// Unix (falling back to `bash`); `pwsh` → `powershell` → `cmd /c` on
   /// Windows.
   pub shell_program: Option<(String, String)>
}

impl Default for ShellConfig {
   fn default() -> Self {
      Self { viewport_size: 5, shell_program: None }
   }
}

impl ShellConfig {
   /// Resolve the `(program, flag)` pair to use for shell-script execution.
   ///
   /// Returns the configured override if set, otherwise auto-detects: on
   /// Unix, prefers a non-empty `$SHELL` then falls back to `bash`; on
   /// Windows, prefers `pwsh` then `powershell` then `cmd`. The flag is
   /// `-c` on Unix, `-Command` for the PowerShell variants, and `/c` for
   /// `cmd`.
   #[must_use]
   pub fn effective_shell_program(&self) -> (String, String) {
      if let Some(sp) = &self.shell_program {
         return sp.clone();
      }
      detect_shell_program()
   }
}

#[cfg(unix)]
fn detect_shell_program() -> (String, String) {
   detect_shell_program_unix(std::env::var("SHELL").ok().as_deref())
}

#[cfg(unix)]
fn detect_shell_program_unix(shell_env: Option<&str>) -> (String, String) {
   match shell_env {
      Some(s) if !s.is_empty() => (s.to_string(), "-c".to_string()),
      _ => ("bash".to_string(), "-c".to_string())
   }
}

#[cfg(windows)]
fn detect_shell_program() -> (String, String) {
   if helpers::command_exists("pwsh") {
      return ("pwsh".to_string(), "-Command".to_string());
   }
   if helpers::command_exists("powershell") {
      return ("powershell".to_string(), "-Command".to_string());
   }
   ("cmd".to_string(), "/c".to_string())
}

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
      if self.success { Ok(()) } else { Err(ShellError::Failed(format!("{cmd} failed"))) }
   }

   /// Return `Ok(())` if the command succeeded, or a `ShellError::Failed` whose
   /// message is `"{cmd} failed — {hint}"`. Use this when the application has
   /// concrete recovery advice to surface to the user.
   pub fn require_success_with_hint(self, cmd: &str, hint: &str) -> Result<(), ShellError> {
      if self.success { Ok(()) } else { Err(ShellError::Failed(format!("{cmd} failed — {hint}"))) }
   }

   /// Return `Ok(())` if the command succeeded, or call `err` with the captured
   /// stderr string to produce an error value of any type.
   pub fn check<E>(self, err: impl FnOnce(String) -> E) -> Result<(), E> {
      if self.success { Ok(()) } else { Err(err(self.stderr)) }
   }
}

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

   /// Run an arbitrary shell script string. The host shell is taken from
   /// [`ShellConfig::effective_shell_program`] — either a configured
   /// override or auto-detected from the platform.
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

#[cfg(test)]
mod tests {
   use super::ShellConfig;

   #[test]
   fn shell_config_default_viewport_size_is_five() {
      assert_eq!(ShellConfig::default().viewport_size, 5);
   }

   #[test]
   fn shell_config_default_shell_program_is_none() {
      assert!(ShellConfig::default().shell_program.is_none());
   }

   #[test]
   fn shell_config_clone_is_equal() {
      let cfg = ShellConfig { viewport_size: 10, shell_program: Some(("zsh".into(), "-c".into())) };
      let cloned = cfg.clone();
      assert_eq!(cloned.viewport_size, 10);
      assert_eq!(cloned.shell_program, Some(("zsh".into(), "-c".into())));
   }

   #[test]
   fn effective_shell_program_returns_override_when_set() {
      let cfg = ShellConfig { viewport_size: 5, shell_program: Some(("fish".into(), "-c".into())) };
      assert_eq!(cfg.effective_shell_program(), ("fish".to_string(), "-c".to_string()));
   }

   #[test]
   fn effective_shell_program_falls_back_to_detection_when_unset() {
      // Given — default config with no override
      let cfg = ShellConfig::default();
      // When
      let (program, flag) = cfg.effective_shell_program();
      // Then — non-empty values are returned (concrete values are platform-dependent)
      assert!(!program.is_empty());
      assert!(!flag.is_empty());
   }

   #[cfg(unix)]
   #[test]
   fn detect_shell_program_unix_prefers_shell_env() {
      let (program, flag) = super::detect_shell_program_unix(Some("/bin/zsh"));
      assert_eq!(program, "/bin/zsh");
      assert_eq!(flag, "-c");
   }

   #[cfg(unix)]
   #[test]
   fn detect_shell_program_unix_falls_back_to_bash_when_unset() {
      let (program, flag) = super::detect_shell_program_unix(None);
      assert_eq!(program, "bash");
      assert_eq!(flag, "-c");
   }

   #[cfg(unix)]
   #[test]
   fn detect_shell_program_unix_falls_back_to_bash_when_empty() {
      let (program, flag) = super::detect_shell_program_unix(Some(""));
      assert_eq!(program, "bash");
      assert_eq!(flag, "-c");
   }
}
