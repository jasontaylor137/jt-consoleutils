//! Production [`Shell`](super::Shell) implementation backed by real OS processes.

use std::process::{Command, Stdio};

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
