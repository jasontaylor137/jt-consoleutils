use std::{
   io::{self, BufRead, IsTerminal},
   process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio},
   sync::mpsc,
   thread,
   time::{Duration, Instant}
};

use super::{CommandResult, ShellError, helpers::format_command};
use crate::{
   output::{Output, OutputMode},
   terminal::overlay
};

const FRAME_INTERVAL: Duration = Duration::from_millis(80);

pub(super) enum Line {
   Stdout(String),
   /// A stdout chunk terminated by `\r` rather than `\n` — overwrites the
   /// current viewport line in place instead of appending a new one.
   StdoutCr(String),
   Stderr(String)
}

impl Line {
   fn text(&self) -> &str {
      match self {
         Self::Stdout(s) | Self::StdoutCr(s) | Self::Stderr(s) => s
      }
   }
}

pub(super) struct RenderedOverlay {
   pub viewport: Vec<String>,
   pub stderr_lines: Vec<String>,
   pub elapsed: Duration
}

struct SpawnedCommand {
   child: Child,
   lines: mpsc::Receiver<Line>,
   /// Each reader is paired with a static label (`"stdout"` / `"stderr"`) so a
   /// panicked thread surfaces in [`ShellError::ReaderPanic`] with enough
   /// context to debug. Without the label, the join error is an opaque `Any`.
   readers: Vec<(&'static str, thread::JoinHandle<()>)>
}

/// Run a shell command with mode-appropriate output:
/// - **quiet**: collect output silently
/// - **verbose**: stream with `| label...` and `> ` prefixed lines
/// - **default + TTY**: animated spinner overlay with scrolling viewport
/// - **default + non-TTY**: collect silently, then emit `step_result` (the spinner has no
///   meaningful non-interactive form, so we fall back to end-of-run reporting)
///
/// # Errors
///
/// Returns a [`ShellError`] if the process cannot be spawned, waited on, or
/// exits with a non-zero status.
pub fn run_command(
   label: &str,
   program: &str,
   args: &[&str],
   output: &mut dyn Output,
   mode: OutputMode,
   viewport_size: usize
) -> Result<CommandResult, ShellError> {
   if mode.is_quiet() {
      return run_quiet(program, args);
   }
   #[cfg(feature = "verbose")]
   if mode.is_verbose() {
      return run_verbose(label, program, args, output);
   }
   if !io::stdout().is_terminal() {
      return run_non_tty(label, program, args, output, viewport_size);
   }
   run_overlay(label, program, args, output, viewport_size)
}

/// Run a command with stdout and stderr inherited from the parent process.
///
/// Use this for read-only inspection commands (e.g. `outdated`) where the
/// output should be displayed directly to the user without any spinner or
/// prefix decoration.
///
/// # Errors
///
/// Returns a [`ShellError`] if the process cannot be spawned or waited on.
pub fn run_passthrough(
   program: &str,
   args: &[&str],
   output: &mut dyn Output,
   mode: OutputMode
) -> Result<CommandResult, ShellError> {
   if mode.is_dry_run() {
      output.dry_run_shell(&format_command(program, args));
      return Ok(CommandResult { success: true, code: None, stderr: String::new() });
   }

   let status = Command::new(program)
      .args(args)
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .spawn()
      .map_err(|e| ShellError::Spawn(program.to_string(), e))?
      .wait()
      .map_err(|e| ShellError::Wait(program.to_string(), e))?;

   Ok(CommandResult { success: status.success(), code: status.code(), stderr: String::new() })
}

/// Quiet mode: collect output silently, no terminal rendering.
pub(super) fn run_quiet(program: &str, args: &[&str]) -> Result<CommandResult, ShellError> {
   let SpawnedCommand { child, lines, readers } = spawn_command_with_lines(program, args)?;
   let stderr_lines = collect_stderr_lines(lines, |_| {});
   let status = wait_and_join(program, child, readers)?;

   Ok(CommandResult { success: status.success(), code: status.code(), stderr: stderr_lines.join("\n") })
}

/// Verbose mode: stream output with `| label...` header and `> ` prefixed lines.
#[cfg(feature = "verbose")]
fn run_verbose(
   label: &str,
   program: &str,
   args: &[&str],
   output: &mut dyn Output
) -> Result<CommandResult, ShellError> {
   output.emit_verbose(format!("{label}..."));
   output.shell_command(&format_command(program, args));

   let SpawnedCommand { child, lines, readers } = spawn_command_with_lines(program, args)?;
   let stderr_lines = collect_stderr_lines(lines, |line| output.shell_line(line));
   let status = wait_and_join(program, child, readers)?;

   Ok(CommandResult { success: status.success(), code: status.code(), stderr: stderr_lines.join("\n") })
}

/// Default mode without a TTY: collect output silently, then emit a single
/// `step_result` at the end. The spinner overlay has no meaningful
/// non-interactive form (cursor moves and line erases are TTY-only), so we
/// fall back to end-of-run reporting that's safe to write into a log file.
fn run_non_tty(
   label: &str,
   program: &str,
   args: &[&str],
   output: &mut dyn Output,
   viewport_size: usize
) -> Result<CommandResult, ShellError> {
   let SpawnedCommand { child, lines, readers } = spawn_command_with_lines(program, args)?;
   let start = Instant::now();
   let mut viewport: Vec<String> = Vec::new();
   let mut stderr_lines: Vec<String> = Vec::new();

   for line in lines {
      let text = line.text().to_string();
      match line {
         Line::StdoutCr(_) => {
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
         Line::Stdout(_) => viewport.push(text)
      }
   }

   let elapsed = start.elapsed();
   let status = wait_and_join(program, child, readers)?;

   // Trim to last viewport_size lines so a long-running command doesn't dump
   // hundreds of failure lines into the log on a single failure.
   let start_idx = viewport.len().saturating_sub(viewport_size);
   let trimmed = &viewport[start_idx..];
   output.step_result(label, status.success(), elapsed.as_millis(), trimmed);

   Ok(CommandResult { success: status.success(), code: status.code(), stderr: stderr_lines.join("\n") })
}

/// Default mode: animated spinner with scrolling viewport overlay.
fn run_overlay(
   label: &str,
   program: &str,
   args: &[&str],
   output: &mut dyn Output,
   viewport_size: usize
) -> Result<CommandResult, ShellError> {
   let SpawnedCommand { child, lines, readers } = spawn_command_with_lines(program, args)?;
   let rendered = render_overlay_lines(label, &lines, viewport_size);
   let status = wait_and_join(program, child, readers)?;
   output.step_result(label, status.success(), rendered.elapsed.as_millis(), &rendered.viewport);
   Ok(CommandResult { success: status.success(), code: status.code(), stderr: rendered.stderr_lines.join("\n") })
}

/// Drive the animated spinner overlay from a pre-built line receiver.
/// Returns viewport, collected stderr lines, and elapsed time.
/// Callers are responsible for calling `output.step_result` afterward.
pub(super) fn render_overlay_lines(label: &str, lines: &mpsc::Receiver<Line>, viewport_size: usize) -> RenderedOverlay {
   let mut stderr_lines: Vec<String> = Vec::new();
   let mut viewport: Vec<String> = Vec::new();
   let start = Instant::now();

   {
      let stdout_handle = io::stdout();
      let mut out = stdout_handle.lock();
      let mut frame = 0usize;
      let mut last_rows = overlay::render_frame(&mut out, label, &[], 0, 0, viewport_size).unwrap_or(0);

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
               last_rows = overlay::render_frame(&mut out, label, &viewport, frame, last_rows, viewport_size)
                  .unwrap_or(last_rows);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
               frame += 1;
               last_rows = overlay::render_frame(&mut out, label, &viewport, frame, last_rows, viewport_size)
                  .unwrap_or(last_rows);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break
         }
      }

      let _ = overlay::clear_lines(&mut out, last_rows);
   }

   RenderedOverlay { viewport, stderr_lines, elapsed: start.elapsed() }
}

fn spawn_command_with_lines(program: &str, args: &[&str]) -> Result<SpawnedCommand, ShellError> {
   let mut child = Command::new(program)
      .args(args)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(|e| ShellError::Spawn(program.to_string(), e))?;

   let child_stdout = child
      .stdout
      .take()
      .ok_or_else(|| ShellError::Spawn(program.to_string(), io::Error::other("stdout is not piped")))?;
   let child_stderr = child
      .stderr
      .take()
      .ok_or_else(|| ShellError::Spawn(program.to_string(), io::Error::other("stderr is not piped")))?;

   let (tx, rx) = mpsc::channel::<Line>();
   let readers = spawn_line_readers(child_stdout, child_stderr, tx);

   Ok(SpawnedCommand { child, lines: rx, readers })
}

fn spawn_line_readers(
   stdout: ChildStdout,
   stderr: ChildStderr,
   tx: mpsc::Sender<Line>
) -> Vec<(&'static str, thread::JoinHandle<()>)> {
   let tx_stderr = tx.clone();

   let stdout_reader = thread::spawn(move || {
      use std::io::Read;
      let mut buf: Vec<u8> = Vec::new();
      let mut chunk = [0u8; 1024];
      let mut reader = io::BufReader::new(stdout);
      loop {
         let n = match reader.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => n
         };
         for &b in &chunk[..n] {
            match b {
               b'\n' => {
                  let line = String::from_utf8_lossy(&buf).into_owned();
                  buf.clear();
                  let _ = tx.send(Line::Stdout(line));
               }
               b'\r' => {
                  let segment = String::from_utf8_lossy(&buf).into_owned();
                  buf.clear();
                  let _ = tx.send(Line::StdoutCr(segment));
               }
               _ => buf.push(b)
            }
         }
      }
      // Flush any remaining text without a terminator.
      if !buf.is_empty() {
         let _ = tx.send(Line::Stdout(String::from_utf8_lossy(&buf).into_owned()));
      }
   });

   let stderr_reader = thread::spawn(move || {
      for line in io::BufReader::new(stderr).lines().map_while(Result::ok) {
         let _ = tx_stderr.send(Line::Stderr(line));
      }
   });

   vec![("stdout", stdout_reader), ("stderr", stderr_reader)]
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
   readers: Vec<(&'static str, thread::JoinHandle<()>)>
) -> Result<ExitStatus, ShellError> {
   let status = child.wait().map_err(|e| ShellError::Wait(program.to_string(), e))?;

   // Drain every reader before returning so a panic in one doesn't leak the
   // other thread, and so we report the *first* panic with full context
   // instead of a `.unwrap()`'s opaque `Any` payload.
   let mut first_panic: Option<ShellError> = None;
   for (stream, reader) in readers {
      if let Err(payload) = reader.join()
         && first_panic.is_none()
      {
         first_panic = Some(ShellError::ReaderPanic {
            program: program.to_string(),
            stream,
            payload: panic_payload_to_string(payload)
         });
      }
   }

   if let Some(err) = first_panic {
      return Err(err);
   }
   Ok(status)
}

/// Best-effort downcast of a [`std::thread::JoinHandle::join`] error payload to
/// a printable string. Most panics are `panic!("text")` (`&'static str`) or
/// `panic!("{}", x)` (`String`); other payload types fall back to a marker so
/// callers always get *something* useful in the error message.
fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
   if let Some(s) = payload.downcast_ref::<&'static str>() {
      (*s).to_string()
   } else if let Some(s) = payload.downcast_ref::<String>() {
      s.clone()
   } else {
      "<non-string panic payload>".to_string()
   }
}

#[cfg(test)]
mod tests {
   use super::*;
   use crate::output::StringOutput;

   /// Under `cargo test`, stdout is not a terminal, so `run_command` in default
   /// mode dispatches to `run_non_tty`. This exercises that path end-to-end:
   /// real subprocess, real line accumulation, real step_result emission.
   #[test]
   #[cfg(unix)]
   fn run_command_non_tty_emits_step_result_for_success() {
      let mut out = StringOutput::new();
      let result =
         run_command("echo step", "echo", &["hello"], &mut out, OutputMode::default(), 5).expect("spawn echo");
      assert!(result.success);
      assert!(out.log().contains("✓ echo step"), "expected step_result in log, got: {}", out.log());
   }

   #[test]
   #[cfg(unix)]
   fn run_command_non_tty_reports_failure_with_step_result() {
      let mut out = StringOutput::new();
      let result = run_command("fail step", "false", &[], &mut out, OutputMode::default(), 5).expect("spawn false");
      assert!(!result.success);
      assert!(out.log().contains("✗ fail step"), "expected failure step_result, got: {}", out.log());
   }

   /// Regression: the stdout reader used to read one byte at a time and cast
   /// `byte as char`, which decoded UTF-8 bytes ≥ 0x80 as Latin-1 and silently
   /// mangled any non-ASCII subprocess output.
   #[test]
   #[cfg(unix)]
   fn stdout_reader_preserves_non_ascii_bytes() {
      let SpawnedCommand { mut child, lines, readers } =
         spawn_command_with_lines("printf", &["café\nnaïve\r"]).expect("spawn printf");
      let mut texts: Vec<String> = Vec::new();
      for line in lines {
         texts.push(match line {
            Line::Stdout(s) | Line::StdoutCr(s) | Line::Stderr(s) => s
         });
      }
      child.wait().expect("wait printf");
      for (_stream, r) in readers {
         r.join().expect("join reader");
      }
      assert!(texts.iter().any(|t| t == "café"), "expected café line, got: {texts:?}");
      assert!(texts.iter().any(|t| t == "naïve"), "expected naïve segment, got: {texts:?}");
   }

   /// `panic!("...")` payloads are `&'static str`. Verify we recover the
   /// message instead of the opaque `Any`.
   #[test]
   fn panic_payload_to_string_recovers_static_str() {
      let handle = thread::spawn(|| panic!("static str payload"));
      let payload = handle.join().expect_err("thread panicked, join must Err");
      assert_eq!(panic_payload_to_string(payload), "static str payload");
   }

   /// `panic!("{}", x)` payloads are `String`. Verify we recover that too.
   #[test]
   fn panic_payload_to_string_recovers_owned_string() {
      let handle = thread::spawn(|| panic!("{}", "owned-string payload".to_string()));
      let payload = handle.join().expect_err("thread panicked, join must Err");
      assert_eq!(panic_payload_to_string(payload), "owned-string payload");
   }

   /// Anything else falls back to a marker rather than losing all context.
   #[test]
   fn panic_payload_to_string_marker_for_unknown_type() {
      let handle = thread::spawn(|| std::panic::panic_any(42u32));
      let payload = handle.join().expect_err("thread panicked, join must Err");
      assert_eq!(panic_payload_to_string(payload), "<non-string panic payload>");
   }
}
