//! Raw ANSI escape-code constants for terminal text styling.
//!
//! These are the building blocks used throughout the crate whenever colored or
//! styled output is written directly. Import the constants you need and embed
//! them in format strings:
//!
//! ```rust
//! use jt_consoleutils::terminal::colors::{GREEN, RESET};
//! println!("{GREEN}success{RESET}");
//! ```

/// Resets all active ANSI text attributes (color, bold, dim, etc.).
pub const RESET: &str = "\x1b[0m";

/// Bold / increased intensity text.
pub const BOLD: &str = "\x1b[1m";

/// Dim / decreased intensity text.
pub const DIM: &str = "\x1b[2m";

/// Red foreground color.
pub const RED: &str = "\x1b[31m";

/// Green foreground color.
pub const GREEN: &str = "\x1b[32m";

/// Yellow foreground color.
pub const YELLOW: &str = "\x1b[33m";

/// Cyan foreground color.
pub const CYAN: &str = "\x1b[36m";
