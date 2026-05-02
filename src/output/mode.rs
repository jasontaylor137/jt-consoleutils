//! [`LogLevel`] and [`OutputMode`] — the verbosity / dry-run dial passed into
//! every [`Output`](super::Output) implementation.

/// Ordered verbosity level for CLI output.
///
/// Levels are ordered from least to most verbose:
/// `Quiet < Normal < Verbose < Trace`.
/// This allows range comparisons: `level >= LogLevel::Verbose`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
   /// Suppress all output, including normal progress messages.
   Quiet,
   /// Normal progress messages are printed; verbose output is hidden.
   #[default]
   Normal,
   /// Commands, their arguments, and verbose messages are printed.
   #[cfg(feature = "verbose")]
   Verbose,
   /// All verbose output plus trace-level diagnostics.
   #[cfg(feature = "trace")]
   Trace
}

/// Carries the standard CLI output-mode configuration.
///
/// Construct with struct literal syntax or [`Default::default`] (normal level,
/// dry-run off):
///
/// ```rust,ignore
/// // Requires the "verbose" feature to be enabled.
/// use jt_consoleutils::output::{LogLevel, OutputMode};
///
/// let mode = OutputMode { level: LogLevel::Verbose, ..OutputMode::default() };
/// assert!(mode.is_verbose());
/// assert!(!mode.is_quiet());
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct OutputMode {
   /// The verbosity level.
   pub level: LogLevel,
   /// Dry-run mode: announce operations without executing them.
   pub dry_run: bool
}

impl OutputMode {
   /// Returns `true` when verbose (or trace) output is enabled.
   #[cfg(feature = "verbose")]
   #[must_use]
   pub const fn is_verbose(self) -> bool {
      #[cfg(feature = "trace")]
      return matches!(self.level, LogLevel::Verbose | LogLevel::Trace);
      #[cfg(not(feature = "trace"))]
      return matches!(self.level, LogLevel::Verbose);
   }

   /// Returns `true` when quiet mode is active (all output suppressed).
   #[must_use]
   pub const fn is_quiet(self) -> bool {
      matches!(self.level, LogLevel::Quiet)
   }

   /// Returns `true` when trace mode is active.
   #[cfg(feature = "trace")]
   #[must_use]
   pub const fn is_trace(self) -> bool {
      matches!(self.level, LogLevel::Trace)
   }

   /// Returns `true` when dry-run mode is active.
   #[must_use]
   pub const fn is_dry_run(self) -> bool {
      self.dry_run
   }
}
