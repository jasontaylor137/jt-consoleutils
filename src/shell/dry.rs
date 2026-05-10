//! Dry-run [`Shell`](super::Shell) implementation that logs commands without executing them.

use super::{
   CommandResult, Shell, ShellConfig, ShellError,
   helpers::{command_exists, command_output, format_command}
};
use crate::output::{Output, OutputMode};

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

#[cfg(test)]
mod tests {
   use super::DryRunShell;
   use crate::{
      output::{OutputMode, StringOutput},
      shell::Shell
   };

   fn default_mode() -> OutputMode {
      OutputMode::default()
   }

   fn dry_run_mode() -> OutputMode {
      OutputMode { dry_run: true, ..Default::default() }
   }

   #[test]
   fn run_command_emits_dry_run_line() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      let result = shell.run_command("build", "cargo", &["build"], &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: cargo build"));
      assert!(result.success);
      assert!(result.stderr.is_empty());
   }

   #[test]
   fn run_command_no_args_emits_program_only() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      shell.run_command("check", "whoami", &[], &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: whoami"));
   }

   #[test]
   fn shell_exec_emits_script() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      let result = shell.shell_exec("echo hello && echo world", &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: echo hello && echo world"));
      assert!(result.success);
      assert!(result.stderr.is_empty());
   }

   #[test]
   fn exec_capture_emits_command() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      let result = shell.exec_capture("ls -la", &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: ls -la"));
      assert!(result.success);
      assert!(result.stderr.is_empty());
   }

   #[test]
   fn exec_interactive_emits_command() {
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      shell.exec_interactive("aws sso login", &mut out, default_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: aws sso login"));
   }

   #[test]
   fn command_exists_delegates_to_real_os() {
      let shell = DryRunShell::default();
      // "sh" is universally available on Unix; this just checks it doesn't panic.
      let _ = shell.command_exists("sh");
   }

   #[test]
   fn command_output_delegates_to_real_os() {
      let shell = DryRunShell::default();
      // A benign read-only command available everywhere.
      let result = shell.command_output("echo", &["hello"]);
      assert!(result.is_ok());
      assert_eq!(result.unwrap(), "hello");
   }

   #[test]
   fn run_command_emits_regardless_of_mode_flag() {
      // DryRunShell ignores the OutputMode — it always dry-runs.
      let shell = DryRunShell::default();
      let mut out = StringOutput::new();
      shell.run_command("x", "true", &[], &mut out, dry_run_mode()).unwrap();
      assert!(out.log().contains("[dry-run] would run: true"));
   }
}
