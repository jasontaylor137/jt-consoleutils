use std::io;
use std::process::{Command, Stdio};

use crate::output::{Output, OutputMode};

mod exec;
mod overlay;
pub mod scripted;

pub use exec::{run_command, run_passthrough};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("failed to spawn '{0}': {1}")]
    Spawn(String, io::Error),
    #[error("failed to wait on '{0}': {1}")]
    Wait(String, io::Error),
    #[error("command failed: {0}")]
    Failed(String),
}

// ---------------------------------------------------------------------------
// CommandResult
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct CommandResult {
    pub success: bool,
    pub stderr: String,
}

// ---------------------------------------------------------------------------
// Shell trait
// ---------------------------------------------------------------------------

/// Abstraction over shell execution, enabling unit tests to mock process spawning.
pub trait Shell {
    fn run_command(
        &self,
        label: &str,
        program: &str,
        args: &[&str],
        output: &mut dyn Output,
        mode: OutputMode,
    ) -> Result<CommandResult, ShellError>;

    fn shell_exec(
        &self,
        script: &str,
        output: &mut dyn Output,
        mode: OutputMode,
    ) -> Result<CommandResult, ShellError>;

    fn command_exists(&self, program: &str) -> bool;

    fn command_output(&self, program: &str, args: &[&str]) -> Result<String, ShellError>;

    /// Run a shell command, capturing stdout/stderr silently without display.
    /// In dry-run mode (`DryRunShell`), logs the command and returns success without executing.
    fn exec_capture(
        &self,
        cmd: &str,
        output: &mut dyn Output,
        mode: OutputMode,
    ) -> Result<CommandResult, ShellError>;

    /// Run a shell command with inherited stdio (for interactive flows like `aws sso login`).
    /// In dry-run mode (`DryRunShell`), logs the command and returns success without executing.
    fn exec_interactive(
        &self,
        cmd: &str,
        output: &mut dyn Output,
        mode: OutputMode,
    ) -> Result<(), ShellError>;
}

/// Returns a `DryRunShell` when `dry_run` is true, otherwise a `ProcessShell`.
pub fn create(dry_run: bool) -> Box<dyn Shell> {
    if dry_run {
        Box::new(DryRunShell)
    } else {
        Box::new(ProcessShell)
    }
}

// ---------------------------------------------------------------------------
// ProcessShell
// ---------------------------------------------------------------------------

/// Production shell: delegates to the free functions in this module.
pub struct ProcessShell;

impl Shell for ProcessShell {
    fn run_command(
        &self,
        label: &str,
        program: &str,
        args: &[&str],
        output: &mut dyn Output,
        mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        exec::run_command(label, program, args, output, mode)
    }

    fn shell_exec(
        &self,
        script: &str,
        output: &mut dyn Output,
        mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        shell_exec(script, output, mode)
    }

    fn command_exists(&self, program: &str) -> bool {
        command_exists(program)
    }

    fn command_output(&self, program: &str, args: &[&str]) -> Result<String, ShellError> {
        command_output(program, args)
    }

    fn exec_capture(
        &self,
        cmd: &str,
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        #[cfg(unix)]
        let (program, flag) = ("bash", "-c");
        #[cfg(windows)]
        let (program, flag) = ("powershell", "-Command");
        exec::run_quiet(program, &[flag, cmd])
    }

    fn exec_interactive(
        &self,
        cmd: &str,
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<(), ShellError> {
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
            Err(ShellError::Failed(format!(
                "'{cmd}' exited with {}",
                status.code().unwrap_or(-1)
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// DryRunShell
// ---------------------------------------------------------------------------

/// Dry-run shell: logs what would be executed and returns fake success.
/// Probe methods (command_exists, command_output) delegate to real implementations
/// because they are read-only and safe to call.
pub struct DryRunShell;

impl Shell for DryRunShell {
    fn run_command(
        &self,
        _label: &str,
        program: &str,
        args: &[&str],
        output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        output.dry_run_shell(&format_command(program, args));
        Ok(CommandResult {
            success: true,
            stderr: String::new(),
        })
    }

    fn shell_exec(
        &self,
        script: &str,
        output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        output.dry_run_shell(script);
        Ok(CommandResult {
            success: true,
            stderr: String::new(),
        })
    }

    fn command_exists(&self, program: &str) -> bool {
        command_exists(program)
    }

    fn command_output(&self, program: &str, args: &[&str]) -> Result<String, ShellError> {
        command_output(program, args)
    }

    fn exec_capture(
        &self,
        cmd: &str,
        output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        output.dry_run_shell(cmd);
        Ok(CommandResult {
            success: true,
            stderr: String::new(),
        })
    }

    fn exec_interactive(
        &self,
        cmd: &str,
        output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<(), ShellError> {
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
    pub calls: std::cell::RefCell<Vec<String>>,
    pub run_success: bool,
    pub command_exists_result: bool,
    pub command_output_value: String,
    /// When false, `command_output` returns `Err` (e.g. to simulate a tool not installed).
    pub command_output_ok: bool,
    /// Queue of results for `exec_capture` calls; pops front on each call.
    /// If empty, falls back to `CommandResult { success: run_success, stderr: "" }`.
    pub exec_capture_results: std::cell::RefCell<std::collections::VecDeque<CommandResult>>,
}

impl MockShell {
    pub fn new() -> Self {
        Self {
            calls: std::cell::RefCell::new(Vec::new()),
            run_success: true,
            command_exists_result: true,
            command_output_value: String::new(),
            command_output_ok: true,
            exec_capture_results: std::cell::RefCell::new(std::collections::VecDeque::new()),
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Shell for MockShell {
    fn run_command(
        &self,
        _label: &str,
        program: &str,
        args: &[&str],
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        self.calls.borrow_mut().push(format_command(program, args));
        Ok(CommandResult {
            success: self.run_success,
            stderr: String::new(),
        })
    }

    fn shell_exec(
        &self,
        script: &str,
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        self.calls
            .borrow_mut()
            .push(format!("shell_exec: {script}"));
        Ok(CommandResult {
            success: self.run_success,
            stderr: String::new(),
        })
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

    fn exec_capture(
        &self,
        cmd: &str,
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        self.calls.borrow_mut().push(format!("exec_capture: {cmd}"));
        let result = self
            .exec_capture_results
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| CommandResult {
                success: self.run_success,
                stderr: String::new(),
            });
        Ok(result)
    }

    fn exec_interactive(
        &self,
        cmd: &str,
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<(), ShellError> {
        self.calls.borrow_mut().push(format!("interactive: {cmd}"));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

fn format_command(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Check if a program is on PATH.
/// Uses `which` on unix, `where.exe` on windows.
pub fn command_exists(program: &str) -> bool {
    #[cfg(unix)]
    let check = Command::new("which").arg(program).output();
    #[cfg(windows)]
    let check = Command::new("where.exe").arg(program).output();

    check.map(|o| o.status.success()).unwrap_or(false)
}

/// Run a command and return its stdout (trimmed).
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
            output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string()),
            stderr.trim(),
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Execute a script via the system shell.
/// Unix: `bash -c "script"`, Windows: `powershell -Command "script"`.
pub fn shell_exec(
    script: &str,
    output: &mut dyn Output,
    mode: OutputMode,
) -> Result<CommandResult, ShellError> {
    #[cfg(unix)]
    let (program, shell_args) = ("bash", vec!["-c", script]);
    #[cfg(windows)]
    let (program, shell_args) = ("powershell", vec!["-Command", script]);

    exec::run_command(
        &format!("Running: {script}"),
        program,
        &shell_args,
        output,
        mode,
    )
}
