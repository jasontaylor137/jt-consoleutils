//! Terminal-facing primitives: ANSI escape constants, rainbow colorizer,
//! terminal width detection, and the spinner/viewport overlay. Output and
//! shell layers depend on this module.

/// Rainbow ANSI colorizer for terminal output.
pub mod colorize;

/// Raw ANSI escape-code constants (`RESET`, `BOLD`, `RED`, etc.).
pub mod colors;

/// Terminal width detection.
pub mod width;

/// Spinner + scrolling-viewport overlay. Used by the shell layer; also
/// exposes a [`overlay::Spinner`] type for direct use without `Shell`.
pub mod overlay;

pub use width::terminal_width;
