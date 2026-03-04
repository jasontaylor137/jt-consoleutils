//! Scripted shell for driving the spinner overlay in tests without spawning real OS processes.
//!
//! [`ScriptedShell`] and [`Script`] are intended for **testing use**. They are not gated behind
//! `#[cfg(test)]` so that downstream crates can use them in their own test suites, but they carry
//! no meaningful runtime cost in production builds (LTO eliminates unused code).
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::output::{Output, OutputMode};

use super::exec::{Line, render_overlay_lines};
use super::{CommandResult, Shell, ShellError};

// ---------------------------------------------------------------------------
// ScriptEvent
// ---------------------------------------------------------------------------

enum ScriptEvent {
    Out(String),
    Err(String),
    Delay(u64),
}

// ---------------------------------------------------------------------------
// Script builder
// ---------------------------------------------------------------------------

/// A sequence of stdout/stderr events and delays that [`ScriptedShell`] replays
/// through the spinner overlay renderer.
///
/// Build one with the fluent builder methods ([`Script::out`], [`Script::err`],
/// [`Script::delay_ms`], etc.) and enqueue it on a [`ScriptedShell`] via
/// [`ScriptedShell::push`].
///
/// Intended for **testing use**.
pub struct Script {
    events: Vec<ScriptEvent>,
    success: bool,
}

impl Script {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            success: true,
        }
    }

    /// Write raw text to stdout (no implicit newline).
    pub fn out(mut self, text: &str) -> Self {
        self.events.push(ScriptEvent::Out(text.to_string()));
        self
    }

    /// Write raw text to stdout then sleep for `ms` milliseconds.
    pub fn out_ms(self, text: &str, ms: u64) -> Self {
        self.out(text).delay_ms(ms)
    }

    /// Write text followed by a newline to stdout.
    pub fn out_line(self, text: &str) -> Self {
        self.out(text).out("\n")
    }

    /// Write text followed by a cr to stdout.
    pub fn out_cr(mut self, text: &str) -> Self {
        self.events.push(ScriptEvent::Out(format!("{text}\r")));
        self
    }

    /// Write text followed by a newline to stdout then sleep for `ms` milliseconds.
    pub fn out_line_ms(self, text: &str, ms: u64) -> Self {
        self.out_line(text).delay_ms(ms)
    }

    /// Write text followed by a cr to stdout then sleep for `ms` milliseconds.
    pub fn out_cr_ms(self, text: &str, ms: u64) -> Self {
        self.out_cr(text).delay_ms(ms)
    }

    /// Write raw text to stderr (no implicit newline).
    pub fn err(mut self, text: &str) -> Self {
        self.events.push(ScriptEvent::Err(text.to_string()));
        self
    }

    /// Write raw text to stderr then sleep for `ms` milliseconds.
    pub fn err_ms(self, text: &str, ms: u64) -> Self {
        self.err(text).delay_ms(ms)
    }

    /// Write text followed by a newline to stderr.
    pub fn err_line(self, text: &str) -> Self {
        self.err(text).err("\n")
    }

    /// Write text followed by a newline to stderr then sleep for `ms` milliseconds.
    pub fn err_line_ms(self, text: &str, ms: u64) -> Self {
        self.err_line(text).delay_ms(ms)
    }

    /// Sleep for `ms` milliseconds before processing the next event.
    pub fn delay_ms(mut self, ms: u64) -> Self {
        self.events.push(ScriptEvent::Delay(ms));
        self
    }

    /// Mark this script as exiting with a failure code. Default is success.
    pub fn exit_failure(mut self) -> Self {
        self.success = false;
        self
    }
}

// ---------------------------------------------------------------------------
// ScriptedShell
// ---------------------------------------------------------------------------

/// A [`Shell`] implementation that drives the real spinner overlay using
/// pre-configured output scripts. No OS processes are spawned.
///
/// Intended for **testing use**. Enqueue one [`Script`] per expected
/// [`Shell::run_command`] call via [`ScriptedShell::push`]; each call pops the
/// front script and replays its events through the live overlay renderer,
/// letting you write overlay integration tests without real subprocesses.
pub struct ScriptedShell {
    scripts: RefCell<VecDeque<Script>>,
}

impl ScriptedShell {
    pub fn new() -> Self {
        Self {
            scripts: RefCell::new(VecDeque::new()),
        }
    }

    /// Enqueue a script to be consumed by the next `run_command` call.
    pub fn push(self, script: Script) -> Self {
        self.scripts.borrow_mut().push_back(script);
        self
    }
}

impl Shell for ScriptedShell {
    fn run_command(
        &self,
        label: &str,
        _program: &str,
        _args: &[&str],
        output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        let script = self
            .scripts
            .borrow_mut()
            .pop_front()
            .expect("ScriptedShell: run_command called but script queue is empty");

        let (tx, rx) = mpsc::channel::<Line>();
        let success = script.success;

        thread::spawn(move || {
            let mut stdout_buf = String::new();
            let mut stderr_buf = String::new();

            for event in script.events {
                match event {
                    ScriptEvent::Out(s) => feed(&s, &mut stdout_buf, false, &tx),
                    ScriptEvent::Err(s) => feed(&s, &mut stderr_buf, true, &tx),
                    ScriptEvent::Delay(ms) => thread::sleep(Duration::from_millis(ms)),
                }
            }

            // Flush any remaining buffered text (no trailing newline).
            if !stdout_buf.is_empty() {
                let _ = tx.send(Line::Stdout(std::mem::take(&mut stdout_buf)));
            }
            if !stderr_buf.is_empty() {
                let _ = tx.send(Line::Stderr(std::mem::take(&mut stderr_buf)));
            }
            // tx dropped here → receiver disconnects → overlay loop exits
        });

        let rendered = render_overlay_lines(label, rx);
        output.step_result(
            label,
            success,
            rendered.elapsed.as_millis(),
            &rendered.viewport,
        );

        Ok(CommandResult {
            success,
            stderr: rendered.stderr_lines.join("\n"),
        })
    }

    fn shell_exec(
        &self,
        _script: &str,
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        Ok(CommandResult {
            success: true,
            stderr: String::new(),
        })
    }

    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn command_output(&self, _program: &str, _args: &[&str]) -> Result<String, ShellError> {
        Ok(String::new())
    }

    fn exec_capture(
        &self,
        _cmd: &str,
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<CommandResult, ShellError> {
        Ok(CommandResult {
            success: true,
            stderr: String::new(),
        })
    }

    fn exec_interactive(
        &self,
        _cmd: &str,
        _output: &mut dyn Output,
        _mode: OutputMode,
    ) -> Result<(), ShellError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Append `s` to `buf`, sending one `Line` per `\r` or `\n` terminator encountered.
///
/// The terminator character determines the `Line` variant:
/// - `\r` → `Line::StdoutCr`: overwrites the last viewport slot in place.
/// - `\n` → `Line::Stdout` / `Line::Stderr`: appends a new viewport slot.
///
/// Crucially, `\r` takes precedence over any `\n` characters embedded within
/// the same chunk. The input is therefore split on `\r` first; within each
/// `\r`-terminated segment the `\n` characters are **payload** (they represent
/// sub-rows of a multi-line progress-bar unit) and are preserved verbatim in
/// the emitted string. Only within segments that are ultimately `\n`-terminated
/// (i.e. no `\r` follows) are embedded `\n` characters treated as line breaks.
///
/// Any text after the final terminator remains buffered for the next call.
fn feed(s: &str, buf: &mut String, is_stderr: bool, tx: &mpsc::Sender<Line>) {
    // Split on \r first to identify CR-terminated chunks.
    let mut segments = s.split('\r').peekable();

    while let Some(seg) = segments.next() {
        let is_last = segments.peek().is_none();

        if is_last {
            // Tail after the last \r (or the whole string if no \r present).
            // Within this tail, \n characters are genuine line terminators.
            if is_stderr {
                for ch in seg.chars() {
                    if ch == '\n' {
                        let line = std::mem::take(buf);
                        let _ = tx.send(Line::Stderr(line));
                    } else {
                        buf.push(ch);
                    }
                }
            } else {
                for ch in seg.chars() {
                    if ch == '\n' {
                        let line = std::mem::take(buf);
                        let _ = tx.send(Line::Stdout(line));
                    } else {
                        buf.push(ch);
                    }
                }
            }
        } else {
            // This segment is followed by a \r, so the whole accumulated
            // content (buf + seg, with any \n kept as payload) becomes a
            // StdoutCr line.  Stderr never uses \r.
            buf.push_str(seg);
            let line = std::mem::take(buf);
            if is_stderr {
                // Treat \r as \n for stderr (shouldn't normally occur).
                let _ = tx.send(Line::Stderr(line));
            } else {
                let _ = tx.send(Line::StdoutCr(line));
            }
        }
    }
}
