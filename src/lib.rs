//! Console and shell utilities for Rust CLI tools.
//!
//! `jt-consoleutils` provides a set of composable building blocks used across
//! CLI tools in the `jt-*` ecosystem. Everything is designed to be testable:
//! traits with mock/test implementations are provided for every abstraction
//! that touches I/O or process execution.
//!
//! # Modules at a glance
//!
//! | Module | What it provides |
//! |---|---|
//! | [`output`] | [`output::Output`] trait, [`output::ConsoleOutput`], [`output::StringOutput`], [`output::OutputMode`] |
//! | [`shell`] | [`shell::Shell`] trait, [`shell::ProcessShell`], [`shell::DryRunShell`], [`shell::MockShell`] |
//! | [`colorize`] | Rainbow ANSI colorizer |
//! | [`colors`] | ANSI escape-code constants |
//! | [`help`] | Help / version printing helpers |
//! | [`terminal`] | Terminal width detection |
//! | [`str_utils`] | Human-readable byte formatting and other string helpers |
//! | [`fs_utils`] | Filesystem comparison and permission helpers |
//! | [`version`] | Build-info version string formatter |
//! | [`json`] | Lightweight JSON/JSONC parser, serializer, and value type |
//! | [`cli`] | CLI parsing framework: global flags, subcommand dispatch, help/version |
//! | `build_support` *(feature `build-support`)* | `build.rs` helper that emits `BUILD_DATE` / `GIT_HASH` |
//!
//! # Quick start — output
//!
//! ```rust
//! use jt_consoleutils::output::{ConsoleOutput, Output, OutputMode};
//!
//! let mode = OutputMode::default();
//! let mut out = ConsoleOutput::new(mode);
//! out.writeln("Hello, world!");
//! ```
//!
//! # Quick start — shell
//!
//! ```rust
//! use jt_consoleutils::shell;
//! use jt_consoleutils::output::{StringOutput, OutputMode};
//!
//! let mode = OutputMode::default();
//! let sh = shell::create(/*dry_run=*/ false);
//! // sh is a Box<dyn Shell> backed by ProcessShell.
//! ```

#![warn(missing_docs)]

/// Emit a verbose-level message if the output is in verbose (or trace) mode.
///
/// The format string is only evaluated when verbose is active, so no allocation
/// occurs in normal or quiet mode. When the `verbose` feature is disabled, this
/// macro expands to nothing.
///
/// ```rust
/// use jt_consoleutils::{verbose, output::{Output, StringOutput}};
///
/// let mut out = StringOutput::new();
/// verbose!(out, "cache hit: {}", "deploy");
/// // output is captured when the "verbose" feature is enabled
/// ```
#[macro_export]
macro_rules! verbose {
   ($output:expr, $($arg:tt)*) => {{
      #[cfg(feature = "verbose")]
      if $output.is_verbose() {
         $output.emit_verbose(::std::format!($($arg)*));
      }
   }};
}

/// Emit a trace-level message if the output is in trace mode.
///
/// The format string is only evaluated when trace is active. When the `trace`
/// feature is disabled, this macro expands to nothing.
///
/// ```rust
/// use jt_consoleutils::{trace, output::{Output, StringOutput}};
///
/// let mut out = StringOutput::new();
/// trace!(out, "step deps: {}", "resolved");
/// // output is captured when the "trace" feature is enabled
/// ```
#[macro_export]
macro_rules! trace {
   ($output:expr, $($arg:tt)*) => {{
      #[cfg(feature = "trace")]
      if $output.is_trace() {
         $output.emit_trace(::std::format!($($arg)*));
      }
   }};
}

/// Rainbow ANSI colorizer for terminal output.
pub mod colorize;

/// Raw ANSI escape-code constants (`RESET`, `BOLD`, `RED`, etc.).
pub mod colors;

/// Filesystem helpers: path comparison, permission bits, symlink removal.
pub mod fs_utils;

/// Help and version printing helpers for CLI entry points.
pub mod help;

/// The [`output::Output`] trait and its standard implementations.
pub mod output;

/// The [`shell::Shell`] trait and its standard implementations.
pub mod shell;

/// String formatting helpers: byte counts, plurals, path conversion.
pub mod str_utils;

/// Terminal introspection helpers (e.g. column width).
pub mod terminal;

/// Build-info version string formatter.
pub mod version;

/// CLI parsing framework: global flags, subcommand dispatch, help/version.
pub mod cli;

/// Lightweight JSON/JSONC parser, serializer, and value type.
pub mod json;

#[cfg(feature = "build-support")]
/// Build-script helper that emits `BUILD_DATE` and `GIT_HASH` env vars.
pub mod build_support;
