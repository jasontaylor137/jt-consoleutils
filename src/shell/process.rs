//! Production [`Shell`](super::Shell) implementation backed by real OS processes.

use super::{
   CommandResult, Shell, ShellConfig, ShellError, exec,
   helpers::{command_exists, command_output, shell_exec}
};
use crate::output::{Output, OutputMode};

/// Production shell: delegates to the free functions in this module.
#[derive(Default)]
pub struct ProcessShell {
   /// Shell execution configuration (e.g. overlay viewport height).
   pub config: ShellConfig
}

/// Stdio handling for `ProcessShell::exec_via_shell`.
enum StdioKind {
   /// Pipe stdout/stderr and collect them silently.
   Capture,
   /// Inherit all three streams so the child can prompt the user.
   Interactive
}

impl ProcessShell {
   /// Spawn `cmd` under the configured shell program, with the requested
   /// stdio handling. Shared backing for `exec_capture` and `exec_interactive`.
   fn exec_via_shell(&self, cmd: &str, kind: StdioKind) -> Result<CommandResult, ShellError> {
      let (program, flag) = self.config.effective_shell_program();
      let args = [flag.as_str(), cmd];
      match kind {
         StdioKind::Capture => exec::run_quiet(&program, &args),
         StdioKind::Interactive => exec::run_interactive(&program, &args)
      }
   }
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
      let (program, flag) = self.config.effective_shell_program();
      shell_exec(script, &program, &flag, output, mode, self.config.viewport_size)
   }

   fn command_exists(&self, program: &str) -> bool {
      command_exists(program)
   }

   fn command_output(&self, program: &str, args: &[&str]) -> Result<String, ShellError> {
      command_output(program, args)
   }

   fn exec_capture(&self, cmd: &str, _output: &mut dyn Output, _mode: OutputMode) -> Result<CommandResult, ShellError> {
      self.exec_via_shell(cmd, StdioKind::Capture)
   }

   fn exec_interactive(&self, cmd: &str, _output: &mut dyn Output, _mode: OutputMode) -> Result<(), ShellError> {
      let result = self.exec_via_shell(cmd, StdioKind::Interactive)?;
      if result.success {
         Ok(())
      } else {
         Err(ShellError::Failed(format!("'{cmd}' exited with {}", result.code.unwrap_or(-1))))
      }
   }
}

#[cfg(test)]
mod tests {
   use super::{ProcessShell, Shell, ShellConfig};
   use crate::output::{OutputMode, StringOutput};

   #[cfg(unix)]
   #[test]
   fn exec_capture_uses_configured_shell_program() {
      // Given — ProcessShell pinned to /bin/sh so we know the spawn used the override
      // (an unconfigured ProcessShell could pick up $SHELL = zsh/fish/etc).
      let shell = ProcessShell {
         config: ShellConfig { viewport_size: 5, shell_program: Some(("/bin/sh".to_string(), "-c".to_string())) }
      };
      let mut out = StringOutput::new();

      // When
      let result = shell.exec_capture("exit 0", &mut out, OutputMode::default()).unwrap();

      // Then
      assert!(result.success);
   }

   #[cfg(unix)]
   #[test]
   fn exec_capture_surfaces_spawn_error_for_bogus_shell_program() {
      // Given — a deliberately non-existent program
      let shell = ProcessShell {
         config: ShellConfig {
            viewport_size: 5,
            shell_program: Some(("/definitely/not/a/real/shell/binary".to_string(), "-c".to_string()))
         }
      };
      let mut out = StringOutput::new();

      // When / Then — spawn fails rather than silently using bash
      assert!(shell.exec_capture("exit 0", &mut out, OutputMode::default()).is_err());
   }
}
