//! Core types for CLI parsing: [`CommandParser`], [`ParsedCli`], [`CliOutcome`], [`CliError`].

use crate::output::OutputMode;

/// The result of parsing CLI arguments: an [`OutputMode`] and the app's command.
#[derive(Debug)]
pub struct ParsedCli<C> {
   /// The resolved output mode (log level + dry-run flag).
   pub mode: OutputMode,
   /// The parsed command.
   pub command: C
}

/// Successful outcomes from [`parse_cli`](crate::cli::parse_cli) /
/// [`parse_cli_from`](crate::cli::parse_cli_from).
///
/// Argument parsing has three success shapes — the user gave a real command,
/// the user asked for help, or the user asked for the version. None of these
/// are errors, so they share the `Ok` arm of the result; only [`CliError`]
/// values are surfaced via `Err`.
///
/// The framework never calls [`std::process::exit`] itself — the application
/// owns its exit codes. Help and version requests are produced as plain
/// strings so the caller can print, log, embed, or otherwise route them as
/// it wishes.
#[derive(Debug)]
pub enum CliOutcome<C> {
   /// Args parsed into a runnable command.
   Parsed(ParsedCli<C>),
   /// User requested help (`--help`/`-h`/`help [<cmd>]`/no args). Print and
   /// typically exit with status `0`.
   Help(String),
   /// User requested `--version`. Print and typically exit with status `0`.
   Version(String)
}

/// Errors that can occur during CLI argument parsing.
///
/// Help and version requests are **not** modeled here — they flow through the
/// `Ok` arm as [`CliOutcome::Help`] / [`CliOutcome::Version`]. This enum only
/// carries genuine usage failures.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
   /// Invalid usage: missing required arg, unknown subcommand, etc.
   #[error("{0}")]
   Usage(String),
   /// Conflicting flags that cannot be combined.
   #[error("{0}")]
   Conflict(String)
}

impl CliError {
   /// Create a [`CliError::Usage`] from anything that converts to `String`.
   pub fn usage(msg: impl Into<String>) -> Self {
      CliError::Usage(msg.into())
   }

   /// Create a [`CliError::Conflict`] from anything that converts to `String`.
   pub fn conflict(msg: impl Into<String>) -> Self {
      CliError::Conflict(msg.into())
   }
}

/// Trait that apps implement on their `Command` enum to participate in
/// the shared CLI parsing framework.
///
/// The framework handles global flags (`-v`, `-q`, `-d`, `-t`), `--version`,
/// `--help`/`help`, and subcommand name matching. Apps provide subcommand
/// names, parsing logic, help text, and version string.
///
/// # Exit codes
///
/// The framework never calls [`std::process::exit`]. Help and version
/// requests are surfaced as [`CliOutcome::Help`] / [`CliOutcome::Version`]
/// so the application can decide how (and whether) to terminate the process.
/// This makes the parser embeddable in TUIs, tests, and tools that wrap
/// other CLIs.
pub trait CommandParser: Sized {
   /// List of recognized subcommand names.
   ///
   /// Used for subcommand matching and "unknown command" error messages.
   fn subcommands() -> &'static [&'static str];

   /// Parse a recognized subcommand.
   ///
   /// `name` is the matched subcommand name from [`subcommands()`](Self::subcommands).
   /// `args` contains everything after the subcommand name.
   fn parse(name: &str, args: &[String]) -> Result<Self, CliError>;

   /// Called when the first arg doesn't match any subcommand and isn't a flag.
   ///
   /// Return `Some(Ok(..))` to handle it as a default command (e.g. sr treats
   /// unknown first args as script paths). Return `None` to fall through to
   /// an "unknown command" error.
   fn default_command(first_arg: &str, rest: &[String]) -> Option<Result<Self, CliError>> {
      let _ = (first_arg, rest);
      None
   }

   /// Version string for `--version` output.
   fn version() -> String;

   /// Main help text for `--help` / `help` with no subcommand.
   fn help_text() -> String;

   /// Per-subcommand help text for `help <cmd> [sub...]`.
   ///
   /// `args` contains any additional tokens after the command name (e.g. for
   /// `help config show`, `cmd` is `"config"` and `args` is `&["show"]`).
   /// Return `None` to fall through to an "unknown command" message.
   fn command_help(cmd: &str, args: &[String]) -> Option<String> {
      let _ = (cmd, args);
      None
   }
}
