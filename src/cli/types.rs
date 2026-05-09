//! Core types for CLI parsing: [`CommandParser`], [`ParsedCli`], [`CliError`].

use crate::output::OutputMode;

/// The result of parsing CLI arguments: an [`OutputMode`] and the app's command.
#[derive(Debug)]
pub struct ParsedCli<C> {
   /// The resolved output mode (log level + dry-run flag).
   pub mode: OutputMode,
   /// The parsed command.
   pub command: C
}

/// Errors that can occur during CLI argument parsing.
///
/// Note that two variants — [`ShowHelp`](Self::ShowHelp) and
/// [`ShowVersion`](Self::ShowVersion) — are not actually errors. They signal
/// that the user requested help or version output and the application should
/// print the carried text and exit with status `0`. The framework never calls
/// [`std::process::exit`] itself; the application owns its exit codes.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
   /// Invalid usage: missing required arg, unknown subcommand, etc.
   #[error("{0}")]
   Usage(String),
   /// Conflicting flags that cannot be combined.
   #[error("{0}")]
   Conflict(String),
   /// Display the carried help text as a successful response, rather than as an
   /// error. Returned by the framework on `--help`/`-h`/`help` and by parsers
   /// when reaching a position where printing the matching command's help is
   /// the right answer (e.g. `myprog config` with no subcommand). The caller
   /// is expected to detect this variant and print the text via
   /// [`crate::cli::help::print_help`] (then typically `exit(0)`) instead of
   /// routing through the "Error: ..." path used for [`Usage`](Self::Usage)
   /// and [`Conflict`](Self::Conflict).
   #[error("{0}")]
   ShowHelp(String),
   /// Display the carried version string and exit successfully. Returned by
   /// the framework when `--version` is detected. Caller typically prints via
   /// [`crate::cli::help::print_version`] and then `exit(0)`.
   #[error("{0}")]
   ShowVersion(String)
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

   /// Create a [`CliError::ShowHelp`] from anything that converts to `String`.
   pub fn show_help(text: impl Into<String>) -> Self {
      CliError::ShowHelp(text.into())
   }

   /// Create a [`CliError::ShowVersion`] from anything that converts to `String`.
   pub fn show_version(text: impl Into<String>) -> Self {
      CliError::ShowVersion(text.into())
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
/// requests are surfaced as [`CliError::ShowHelp`] / [`CliError::ShowVersion`]
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
