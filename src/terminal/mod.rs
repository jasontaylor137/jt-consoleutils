//! Terminal-facing primitives: ANSI escape constants, rainbow colorizer, and
//! terminal width detection. Output and shell layers depend on this module.

/// Rainbow ANSI colorizer for terminal output.
pub mod colorize;

/// Raw ANSI escape-code constants (`RESET`, `BOLD`, `RED`, etc.).
pub mod colors;

/// Terminal width detection.
pub mod width;

/// Spinner + scrolling-viewport overlay used by the shell layer.
pub(crate) mod overlay;

pub use width::terminal_width;
