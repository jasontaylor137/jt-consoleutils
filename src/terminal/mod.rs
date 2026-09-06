//! Terminal-facing primitives: ANSI escape constants, rainbow colorizer,
//! terminal width detection, console preparation, and the spinner/viewport
//! overlay. Output and shell layers depend on this module.

/// Windows console preparation: ANSI virtual-terminal mode and UTF-8 output.
pub mod ansi;

/// Rainbow ANSI colorizer for terminal output.
pub mod colorize;

/// Raw ANSI escape-code constants (`RESET`, `BOLD`, `RED`, etc.).
pub mod colors;

/// Terminal width detection.
pub mod width;

/// Scrolling-viewport overlay rendering used by the shell layer's spinner.
pub(crate) mod overlay;

pub use ansi::enable_ansi;
pub use width::terminal_width;
