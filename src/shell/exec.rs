use std::io::{self, BufRead};
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::output::{Output, OutputMode};

use super::overlay;
use super::{CommandResult, ShellError};

const FRAME_INTERVAL: Duration = Duration::from_millis(80);

pub(super) enum Line {
    Stdout(String),
    /// A stdout chunk terminated by `\r` rather than `\n` — overwrites the
    /// current viewport line in place instead of appending a new one.
    StdoutCr(String),
    Stderr(String),
}

impl Line {
    fn text(&self) -> &str {
        match self {
            Line::Stdout(s) | Line::StdoutCr(s) | Line::Stderr(s) => s,
        }
    }
}

pub(super) struct RenderedOverlay {
    pub viewport: Vec<String>,
    pub stderr_lines: Vec<String>,
    pub elapsed: Duration,
}

struct SpawnedCommand {
    child: Child,
    lines: mpsc::Receiver<Line>,
    readers: Vec<thread::JoinHandle<()>>,
}

/// Run a shell command with mode-appropriate output:
/// - **quiet**: collect output silently
/// - **verbose**: stream with `| label...` and `> ` prefixed lines
/// - **default**: animated spinner overlay with scrolling viewport
pub fn run_command(
    label: &str,
    program: &str,
    args: &[&str],
    output: &mut dyn Output,
    mode: OutputMode,
) -> Result<CommandResult, ShellError> {
    if mode.is_quiet() {
        return run_quiet(program, args);
    }
    if mode.is_verbose() {
        return run_verbose(label, program, args, output, mode);
    }
    run_overlay(label, program, args, output)
}

/// Run a command with stdout and stderr inherited from the parent process.
///
/// Use this for read-only inspection commands (e.g. `outdated`) where the
/// output should be displayed directly to the user without any spinner or
/// prefix decoration.
pub fn run_passthrough(
    program: &str,
    args: &[&str],
    output: &mut dyn Output,
    mode: OutputMode,
) -> Result<CommandResult, ShellError> {
    if mode.is_dry_run() {
        output.dry_run_shell(&super::format_command(program, args));
        return Ok(CommandResult {
            success: true,
            stderr: String::new(),
        });
    }

    let status = Command::new(program)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| ShellError::Spawn(program.to_string(), e))?
        .wait()
        .map_err(|e| ShellError::Wait(program.to_string(), e))?;

    Ok(CommandResult {
        success: status.success(),
        stderr: String::new(),
    })
}

/// Quiet mode: collect output silently, no terminal rendering.
pub(super) fn run_quiet(program: &str, args: &[&str]) -> Result<CommandResult, ShellError> {
    let SpawnedCommand {
        child,
        lines,
        readers,
    } = spawn_command_with_lines(program, args)?;
    let stderr_lines = collect_stderr_lines(lines, |_| {});
    let status = wait_and_join(program, child, readers)?;

    Ok(CommandResult {
        success: status.success(),
        stderr: stderr_lines.join("\n"),
    })
}

/// Verbose mode: stream output with `| label...` header and `> ` prefixed lines.
fn run_verbose(
    label: &str,
    program: &str,
    args: &[&str],
    output: &mut dyn Output,
    mode: OutputMode,
) -> Result<CommandResult, ShellError> {
    output.log(mode, &format!("{label}..."));
    output.shell_command(&super::format_command(program, args));

    let SpawnedCommand {
        child,
        lines,
        readers,
    } = spawn_command_with_lines(program, args)?;
    let stderr_lines = collect_stderr_lines(lines, |line| output.shell_line(line));
    let status = wait_and_join(program, child, readers)?;

    Ok(CommandResult {
        success: status.success(),
        stderr: stderr_lines.join("\n"),
    })
}

/// Default mode: animated spinner with scrolling viewport overlay.
fn run_overlay(
    label: &str,
    program: &str,
    args: &[&str],
    output: &mut dyn Output,
) -> Result<CommandResult, ShellError> {
    let SpawnedCommand {
        child,
        lines,
        readers,
    } = spawn_command_with_lines(program, args)?;
    let rendered = render_overlay_lines(label, lines);
    let status = wait_and_join(program, child, readers)?;
    output.step_result(
        label,
        status.success(),
        rendered.elapsed.as_millis(),
        &rendered.viewport,
    );
    Ok(CommandResult {
        success: status.success(),
        stderr: rendered.stderr_lines.join("\n"),
    })
}

/// Drive the animated spinner overlay from a pre-built line receiver.
/// Returns viewport, collected stderr lines, and elapsed time.
/// Callers are responsible for calling `output.step_result` afterward.
pub(super) fn render_overlay_lines(label: &str, lines: mpsc::Receiver<Line>) -> RenderedOverlay {
    let mut stderr_lines: Vec<String> = Vec::new();
    let mut viewport: Vec<String> = Vec::new();
    let start = Instant::now();

    {
        let stdout_handle = io::stdout();
        let mut out = stdout_handle.lock();
        let mut frame = 0usize;
        let mut last_rows = overlay::render_frame(&mut out, label, &[], 0, 0);

        loop {
            match lines.recv_timeout(FRAME_INTERVAL) {
                Ok(line) => {
                    let text = line.text().to_string();
                    match line {
                        Line::StdoutCr(_) => {
                            // Overwrite the last viewport entry in place (progress-bar style).
                            if let Some(last) = viewport.last_mut() {
                                *last = text;
                            } else {
                                viewport.push(text);
                            }
                        }
                        Line::Stderr(s) => {
                            stderr_lines.push(s.clone());
                            viewport.push(s);
                        }
                        Line::Stdout(_) => {
                            viewport.push(text);
                        }
                    }
                    frame += 1;
                    last_rows = overlay::render_frame(&mut out, label, &viewport, frame, last_rows);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    frame += 1;
                    last_rows = overlay::render_frame(&mut out, label, &viewport, frame, last_rows);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        overlay::clear_lines(&mut out, last_rows);
    }

    RenderedOverlay {
        viewport,
        stderr_lines,
        elapsed: start.elapsed(),
    }
}

fn spawn_command_with_lines(program: &str, args: &[&str]) -> Result<SpawnedCommand, ShellError> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ShellError::Spawn(program.to_string(), e))?;

    let child_stdout = child.stdout.take().ok_or_else(|| {
        ShellError::Spawn(program.to_string(), io::Error::other("stdout is not piped"))
    })?;
    let child_stderr = child.stderr.take().ok_or_else(|| {
        ShellError::Spawn(program.to_string(), io::Error::other("stderr is not piped"))
    })?;

    let (tx, rx) = mpsc::channel::<Line>();
    let readers = spawn_line_readers(child_stdout, child_stderr, tx);

    Ok(SpawnedCommand {
        child,
        lines: rx,
        readers,
    })
}

fn spawn_line_readers(
    stdout: ChildStdout,
    stderr: ChildStderr,
    tx: mpsc::Sender<Line>,
) -> Vec<thread::JoinHandle<()>> {
    let tx_stderr = tx.clone();

    let stdout_reader = thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let mut raw = String::new();
        let mut reader = io::BufReader::new(stdout);
        let mut byte = [0u8; 1];
        while reader.read(&mut byte).unwrap_or(0) > 0 {
            let ch = byte[0] as char;
            if ch == '\n' {
                let line = std::mem::take(&mut buf);
                // Drain any \r-terminated segment that was pending in raw
                raw.clear();
                let _ = tx.send(Line::Stdout(line));
            } else if ch == '\r' {
                let segment = std::mem::take(&mut buf);
                raw.clear();
                let _ = tx.send(Line::StdoutCr(segment));
            } else {
                buf.push(ch);
                raw.push(ch);
            }
        }
        // Flush any remaining text without a terminator.
        if !buf.is_empty() {
            let _ = tx.send(Line::Stdout(buf));
        }
    });

    let stderr_reader = thread::spawn(move || {
        for line in io::BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = tx_stderr.send(Line::Stderr(line));
        }
    });

    vec![stdout_reader, stderr_reader]
}

fn collect_stderr_lines(lines: mpsc::Receiver<Line>, mut on_line: impl FnMut(&str)) -> Vec<String> {
    let mut stderr_lines = Vec::new();

    for line in lines {
        let text = line.text().to_string();
        on_line(&text);
        if let Line::Stderr(stderr) = line {
            stderr_lines.push(stderr);
        }
    }

    stderr_lines
}

fn wait_and_join(
    program: &str,
    mut child: Child,
    readers: Vec<thread::JoinHandle<()>>,
) -> Result<ExitStatus, ShellError> {
    let status = child
        .wait()
        .map_err(|e| ShellError::Wait(program.to_string(), e))?;

    for reader in readers {
        reader.join().unwrap();
    }

    Ok(status)
}
