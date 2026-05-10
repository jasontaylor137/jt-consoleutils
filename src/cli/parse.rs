//! CLI argument parsing: global flag extraction, help/version handling, subcommand dispatch.

use super::types::{CliError, CliOutcome, CommandParser, ParsedCli};
use crate::output::{LogLevel, OutputMode};

/// Convert a slice of `String` args into a [`pico_args::Arguments`] for
/// optional flag extraction within subcommand parsers. Allocates one
/// `OsString` per arg.
#[must_use]
pub fn to_pargs(args: &[String]) -> pico_args::Arguments {
   pico_args::Arguments::from_vec(args.iter().map(std::ffi::OsString::from).collect())
}

/// Parse CLI arguments using the app's [`CommandParser`] implementation.
///
/// Handles global flags (`-v`/`--verbose`, `-q`/`--quiet`, `-d`/`--dry-run`,
/// `-t`/`--trace`), `--version`, `--help`/`-h`, and `help <cmd>` before
/// dispatching to the app's subcommand parser.
///
/// # Returns
///
/// - `Ok(CliOutcome::Parsed(..))` — args resolved to a runnable command.
/// - `Ok(CliOutcome::Help(text))` — help was requested (no args, `-h`, `--help`, or `help
///   [<cmd>]`). The caller should print the carried text via [`crate::cli::help::print_help`] and
///   exit with status `0`.
/// - `Ok(CliOutcome::Version(text))` — `--version` was passed. Print and exit `0`.
///
/// # Errors
///
/// - [`CliError::Conflict`] for mutually exclusive flags (`-v` + `-q`, `-q` + `-d`).
/// - [`CliError::Usage`] for unknown subcommands when [`CommandParser::default_command`] returns
///   `None`, or for any error surfaced by the app's parser.
///
/// This function never calls [`std::process::exit`] — the application is
/// responsible for choosing exit codes for each outcome.
pub fn parse_cli<C: CommandParser>() -> Result<CliOutcome<C>, CliError> {
   let args: Vec<String> = std::env::args().skip(1).collect();
   parse_cli_inner::<C>(&args)
}

/// Like [`parse_cli`], but parses from a caller-supplied argv slice
/// instead of `std::env::args()`. `argv` must NOT include the program name.
///
/// Use this when the caller needs to pre-process raw argv (for example,
/// to strip an app-specific global flag) before invoking the shared parser.
///
/// # Returns / Errors
///
/// Same conditions as [`parse_cli`].
pub fn parse_cli_from<C: CommandParser>(argv: &[String]) -> Result<CliOutcome<C>, CliError> {
   parse_cli_inner::<C>(argv)
}

/// Shared body for [`parse_cli`] and [`parse_cli_from`]. `args` contains
/// only the arguments that follow the program name.
fn parse_cli_inner<C: CommandParser>(args: &[String]) -> Result<CliOutcome<C>, CliError> {
   if args.is_empty() {
      return Ok(CliOutcome::Help(C::help_text()));
   }

   if let Some(text) = detect_help::<C>(args) {
      return Ok(CliOutcome::Help(text));
   }

   let (trace, verbose, quiet, dry_run, filtered) = extract_global_flags(args);

   if verbose && quiet {
      return Err(CliError::conflict("--verbose and --quiet are mutually exclusive"));
   }
   if quiet && dry_run {
      return Err(CliError::conflict("--quiet and --dry-run are mutually exclusive"));
   }

   if filtered.iter().any(|a| a == "--version") {
      return Ok(CliOutcome::Version(C::version()));
   }

   if filtered.is_empty() {
      // Only global flags were given (e.g. `app -v`) — show help.
      return Ok(CliOutcome::Help(C::help_text()));
   }

   let level = flags_to_level(trace, verbose, quiet);
   let mode = OutputMode { level, dry_run };

   let command = dispatch::<C>(&filtered)?;
   Ok(CliOutcome::Parsed(ParsedCli { mode, command }))
}

/// Scan args for help triggers. Returns `Some(help_text)` when a help request
/// is detected, otherwise `None`.
///
/// `args` contains only the arguments after the program name. The function
/// returns `None` on an empty slice instead of panicking.
fn detect_help<C: CommandParser>(args: &[String]) -> Option<String> {
   let first = args.first()?;

   // `help` as first arg
   if first == "help" {
      return Some(match args.get(1).map(String::as_str) {
         Some(name) => {
            // Pass remaining args after the command name (e.g. `help config show` → args=["show"])
            let rest: Vec<String> = args.get(2..).map(<[String]>::to_vec).unwrap_or_default();
            // Unknown subcommand after help → fall back to main help.
            C::command_help(name, &rest).unwrap_or_else(C::help_text)
         }
         None => C::help_text()
      });
   }

   // -h / --help anywhere in args
   if args.iter().any(|a| a == "-h" || a == "--help") {
      // If a subcommand precedes -h, show its help
      if let Some(cmd) = args.iter().find(|a| C::subcommands().contains(&a.as_str()))
         && let Some(text) = C::command_help(cmd, &[])
      {
         return Some(text);
      }
      return Some(C::help_text());
   }

   None
}

/// Extract global flags from args. `args` contains only the arguments after
/// the program name. Returns `(trace, verbose, quiet, dry_run, filtered_args)`.
#[allow(unused_mut)]
pub(super) fn extract_global_flags(args: &[String]) -> (bool, bool, bool, bool, Vec<String>) {
   let mut trace = false;
   let mut verbose = false;
   let mut quiet = false;
   let mut dry_run = false;
   let mut filtered: Vec<String> = Vec::new();
   let mut past_separator = false;

   for arg in args {
      if past_separator {
         filtered.push(arg.clone());
         continue;
      }

      if arg == "--" {
         past_separator = true;
         filtered.push(arg.clone());
      } else if cfg!(feature = "trace") && (arg == "-t" || arg == "--trace") {
         trace = true;
      } else if cfg!(feature = "verbose") && (arg == "-v" || arg == "--verbose") {
         verbose = true;
      } else if arg == "-q" || arg == "--quiet" {
         quiet = true;
      } else if arg == "-d" || arg == "--dry-run" {
         dry_run = true;
      } else {
         filtered.push(arg.clone());
      }
   }

   (trace, verbose, quiet, dry_run, filtered)
}

/// Convert individual flag booleans to a [`LogLevel`].
pub(super) fn flags_to_level(trace: bool, verbose: bool, quiet: bool) -> LogLevel {
   #[cfg(feature = "trace")]
   if trace {
      return LogLevel::Trace;
   }
   #[cfg(feature = "verbose")]
   if verbose {
      return LogLevel::Verbose;
   }
   let _ = (trace, verbose);
   if quiet { LogLevel::Quiet } else { LogLevel::Normal }
}

/// Dispatch to the app's subcommand parser or default command handler.
///
/// Caller guarantees `filtered` is non-empty; `parse_cli_inner` routes the
/// empty case to [`CliOutcome::Help`] before reaching here.
pub(super) fn dispatch<C: CommandParser>(filtered: &[String]) -> Result<C, CliError> {
   let (first, rest) = filtered.split_first().expect("dispatch called with empty filtered args");
   let first = first.as_str();

   // Check for recognized subcommand
   if C::subcommands().contains(&first) {
      return C::parse(first, rest);
   }

   // Not a flag → try default command
   if !first.starts_with('-')
      && let Some(result) = C::default_command(first, rest)
   {
      return result;
   }

   Err(CliError::usage(format!("unknown command: {first}")))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
