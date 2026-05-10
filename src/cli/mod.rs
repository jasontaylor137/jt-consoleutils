//! CLI parsing framework: global flag extraction, subcommand dispatch, help/version handling.
//!
//! Apps implement `CommandParser` on their `Command` enum and call
//! `parse_cli` to get a `ParsedCli` with the resolved
//! [`OutputMode`](crate::output::OutputMode) and parsed command.

/// Help and version printing helpers for CLI entry points.
pub mod help;
mod parse;
mod types;
/// Build-info version string formatter.
pub mod version;

pub use parse::{parse_cli, parse_cli_from, to_pargs};
pub use types::{CliError, CliOutcome, CommandParser, ParsedCli};
