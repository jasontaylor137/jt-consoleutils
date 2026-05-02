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
//! | [`terminal`] | ANSI colors, rainbow colorizer, terminal width detection |
//! | [`help`] | Help / version printing helpers |
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

/// Generate an enum + [`AsVerb`](crate::vocab::AsVerb) impl whose verb string
/// is the variant identifier (via [`stringify!`]).
///
/// ```rust
/// use jt_consoleutils::{verb_enum, vocab::AsVerb};
///
/// verb_enum! {
///    pub enum Verb {
///       Created,
///       Removed,
///    }
/// }
///
/// assert_eq!(Verb::Created.as_verb(), "Created");
/// ```
#[macro_export]
macro_rules! verb_enum {
   ($vis:vis enum $name:ident { $($variant:ident),* $(,)? }) => {
      /// Action verb vocabulary. Variant identifier IS the rendered verb.
      #[derive(Copy, Clone, Debug, PartialEq, Eq)]
      #[allow(dead_code)]
      $vis enum $name {
         $(
            #[allow(missing_docs)]
            $variant,
         )*
      }

      impl $crate::vocab::AsVerb for $name {
         fn as_verb(&self) -> &str {
            match self { $(Self::$variant => stringify!($variant)),* }
         }
      }
   };
}

/// Generate an enum + [`AsNoun`](crate::vocab::AsNoun) impl. Singular and
/// plural forms are supplied as string literals.
///
/// ```rust
/// use jt_consoleutils::{noun_enum, vocab::AsNoun};
///
/// noun_enum! {
///    pub enum Noun {
///       Dep => "dep" / "deps",
///       Tool => "tool" / "tools",
///    }
/// }
///
/// assert_eq!(Noun::Dep.singular(), "dep");
/// assert_eq!(Noun::Dep.plural(), "deps");
/// ```
#[macro_export]
macro_rules! noun_enum {
   ($vis:vis enum $name:ident {
      $($variant:ident => $singular:literal / $plural:literal),* $(,)?
   }) => {
      /// Object noun vocabulary used for count phrases.
      #[derive(Copy, Clone, Debug, PartialEq, Eq)]
      #[allow(dead_code)]
      $vis enum $name {
         $(
            #[allow(missing_docs)]
            $variant,
         )*
      }

      impl $crate::vocab::AsNoun for $name {
         fn singular(&self) -> &str {
            match self { $(Self::$variant => $singular),* }
         }
         fn plural(&self) -> &str {
            match self { $(Self::$variant => $plural),* }
         }
      }
   };
}

/// `.env` file loader (feature-gated on `dotenv`).
#[cfg(feature = "dotenv")]
pub mod dotenv;

/// Process-environment `${VAR}` expansion helper.
pub mod envvars;

/// Filesystem helpers: path comparison, permission bits, symlink removal.
pub mod fs_utils;

/// Path-manipulation helpers: home dir, normalization, script-dir/-filename, PATH presence.
pub mod paths;

/// SIGINT (Ctrl+C) handling so post-run cleanup hooks survive an interrupt.
pub mod signals;

/// The [`output::Output`] trait and its standard implementations.
pub mod output;

/// Marker traits for binary-supplied output vocabulary.
pub mod vocab;

/// The [`shell::Shell`] trait and its standard implementations.
pub mod shell;

/// String formatting helpers: byte counts, plurals, path conversion.
pub mod str_utils;

/// Terminal-facing primitives: ANSI colors, rainbow colorizer, width detection.
pub mod terminal;

/// CLI parsing framework: global flags, subcommand dispatch, help/version.
pub mod cli;

/// Lightweight JSON/JSONC parser, serializer, and value type.
pub mod json;

#[cfg(feature = "build-support")]
/// Build-script helper that emits `BUILD_DATE` and `GIT_HASH` env vars.
pub mod build_support;
