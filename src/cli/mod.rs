//! CLI parsing framework: global flag extraction, subcommand dispatch, help/version handling.
//!
//! Apps implement [`CommandParser`] on their `Command` enum and call
//! [`parse_cli`] to get a [`ParsedCli`] with the resolved [`OutputMode`](crate::output::OutputMode)
//! and parsed command.

mod helpers;
mod parse;
mod types;

pub use helpers::to_pargs;
pub use parse::parse_cli;
pub use types::{CliError, CommandParser, ParsedCli};
