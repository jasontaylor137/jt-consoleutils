//! Free functions used by the shell module and re-exported through `shell::`.

use std::process::{Command, Stdio};

use super::{CommandResult, ShellError, exec};
use crate::output::{Output, OutputMode};

/// Join `program` and `args` into a single space-separated string.
///
/// Used internally for dry-run logging and mock call recording. Visible to the
/// rest of the crate so sibling modules (e.g. `exec`) can format spawn lines.
pub(crate) fn format_command(program: &str, args: &[&str]) -> String {
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

#[cfg(test)]
mod tests {
   use rstest::rstest;

   use super::format_command;

   #[rstest]
   #[case("echo", &[], "echo")]
   #[case("git", &["status"], "git status")]
   #[case("cargo", &["build", "--release"], "cargo build --release")]
   fn format_command_joins_program_and_args(#[case] program: &str, #[case] args: &[&str], #[case] expected: &str) {
      assert_eq!(format_command(program, args), expected);
   }
}
