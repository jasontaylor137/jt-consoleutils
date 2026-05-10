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

#[cfg(test)]
mod tests {
   use super::ShellConfig;

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
}
