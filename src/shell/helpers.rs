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
///
/// On Windows, candidate extensions come from `%PATHEXT%` (lowercased, leading
/// dot stripped). Falls back to `exe;cmd;bat;com` when `PATHEXT` is unset or
/// empty, matching the legacy default.
#[must_use]
pub fn command_exists(program: &str) -> bool {
   let path_var = std::env::var_os("PATH").unwrap_or_default();
   let sep = if cfg!(windows) { ';' } else { ':' };
   #[cfg(windows)]
   let pathext = parse_pathext(&std::env::var("PATHEXT").unwrap_or_default());
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
         for ext in &pathext {
            if candidate.with_extension(ext).is_file() {
               return true;
            }
         }
      }
   }
   false
}

/// Normalize a `%PATHEXT%`-style value: split on `;`, trim, lowercase, strip
/// leading `.`, drop empties. Falls back to the legacy default when the result
/// is empty so callers always get at least the historical extension set.
#[cfg(windows)]
fn parse_pathext(raw: &str) -> Vec<String> {
   let parsed: Vec<String> = raw
      .split(';')
      .filter_map(|ext| {
         let trimmed = ext.trim().trim_start_matches('.');
         if trimmed.is_empty() { None } else { Some(trimmed.to_ascii_lowercase()) }
      })
      .collect();
   if parsed.is_empty() {
      vec!["exe".to_string(), "cmd".to_string(), "bat".to_string(), "com".to_string()]
   } else {
      parsed
   }
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

/// Execute a script via the system shell, with an explicit `(program, flag)`
/// pair (typically resolved via
/// [`ShellConfig::effective_shell_program`](super::ShellConfig::effective_shell_program)).
///
/// # Errors
///
/// Returns a [`ShellError`] if the process cannot be spawned or the command
/// exits with a non-zero status.
pub fn shell_exec(
   script: &str,
   program: &str,
   flag: &str,
   output: &mut dyn Output,
   mode: OutputMode,
   viewport_size: usize
) -> Result<CommandResult, ShellError> {
   exec::run_command(&format!("Running: {script}"), program, &[flag, script], output, mode, viewport_size)
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

   #[cfg(windows)]
   mod pathext {
      use super::super::parse_pathext;

      #[test]
      fn parses_typical_value_lowercased_without_dots() {
         let ext = parse_pathext(".COM;.EXE;.BAT;.CMD;.PS1");
         assert_eq!(ext, vec!["com", "exe", "bat", "cmd", "ps1"]);
      }

      #[test]
      fn ignores_empty_segments_and_whitespace() {
         let ext = parse_pathext(";.exe; ; .ps1 ;");
         assert_eq!(ext, vec!["exe", "ps1"]);
      }

      #[test]
      fn falls_back_to_legacy_default_when_empty() {
         assert_eq!(parse_pathext(""), vec!["exe", "cmd", "bat", "com"]);
         assert_eq!(parse_pathext(";;;"), vec!["exe", "cmd", "bat", "com"]);
      }
   }
}
