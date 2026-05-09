//! CLI argument parsing: global flag extraction, help/version handling, subcommand dispatch.

use super::types::{CliError, CommandParser, ParsedCli};
use crate::output::{LogLevel, OutputMode};

/// Parse CLI arguments using the app's [`CommandParser`] implementation.
///
/// Handles global flags (`-v`/`--verbose`, `-q`/`--quiet`, `-d`/`--dry-run`,
/// `-t`/`--trace`), `--version`, `--help`/`-h`, and `help <cmd>` before
/// dispatching to the app's subcommand parser.
///
/// # Errors
///
/// - [`CliError::ShowHelp`] when help output was requested (no args, `-h`, `--help`, or `help
///   [<cmd>]`). The caller should print the carried text via [`crate::cli::help::print_help`] and
///   exit with status `0`.
/// - [`CliError::ShowVersion`] when `--version` was passed. Print and exit `0`.
/// - [`CliError::Conflict`] for mutually exclusive flags (`-v` + `-q`, `-q` + `-d`).
/// - [`CliError::Usage`] for unknown subcommands when [`CommandParser::default_command`] returns
///   `None`, or for any error surfaced by the app's parser.
///
/// This function never calls [`std::process::exit`] — the application is
/// responsible for choosing exit codes for each variant.
pub fn parse_cli<C: CommandParser>() -> Result<ParsedCli<C>, CliError> {
   let args: Vec<String> = std::env::args().collect();
   parse_cli_inner::<C>(&args)
}

/// Like [`parse_cli`], but parses from a caller-supplied argv slice
/// instead of `std::env::args()`. `argv` should NOT include the program
/// name — this function prepends a placeholder internally so the existing
/// `skip(1)` logic (which assumes `argv[0]` is the program name) is preserved.
///
/// Use this when the caller needs to pre-process raw argv (for example,
/// to strip an app-specific global flag) before invoking the shared parser.
///
/// # Errors
///
/// Same conditions as [`parse_cli`].
pub fn parse_cli_from<C: CommandParser>(argv: &[String]) -> Result<ParsedCli<C>, CliError> {
   let mut args: Vec<String> = Vec::with_capacity(argv.len() + 1);
   args.push(String::new()); // program-name placeholder; consumers skip index 0
   args.extend(argv.iter().cloned());
   parse_cli_inner::<C>(&args)
}

/// Shared body for [`parse_cli`] and [`parse_cli_from`]. `args` is the full
/// argv including the program name at index 0.
fn parse_cli_inner<C: CommandParser>(args: &[String]) -> Result<ParsedCli<C>, CliError> {
   if args.len() <= 1 {
      return Err(CliError::ShowHelp(C::help_text()));
   }

   handle_help::<C>(args)?;

   let (trace, verbose, quiet, dry_run, filtered) = extract_global_flags(args);

   if verbose && quiet {
      return Err(CliError::conflict("--verbose and --quiet are mutually exclusive"));
   }
   if quiet && dry_run {
      return Err(CliError::conflict("--quiet and --dry-run are mutually exclusive"));
   }

   if filtered.iter().any(|a| a == "--version") {
      return Err(CliError::ShowVersion(C::version()));
   }

   let level = flags_to_level(trace, verbose, quiet);
   let mode = OutputMode { level, dry_run };

   let command = dispatch::<C>(&filtered)?;
   Ok(ParsedCli { mode, command })
}

/// Scan args for help triggers. Returns `Err(CliError::ShowHelp(...))` when a
/// help request is detected, otherwise `Ok(())`.
fn handle_help<C: CommandParser>(args: &[String]) -> Result<(), CliError> {
   // `help` as first arg
   if args[1] == "help" {
      let cmd = args.get(2).map(|s| s.as_str());
      return Err(match cmd {
         Some(name) => {
            // Pass remaining args after the command name (e.g. `help config show` → args=["show"])
            let rest: Vec<String> = args[3..].to_vec();
            match C::command_help(name, &rest) {
               Some(text) => CliError::ShowHelp(text),
               // Unknown subcommand after help — show main help
               None => CliError::ShowHelp(C::help_text())
            }
         }
         None => CliError::ShowHelp(C::help_text())
      });
   }

   // -h / --help anywhere in args
   if args.iter().any(|a| a == "-h" || a == "--help") {
      // If a subcommand precedes -h, show its help
      if let Some(cmd) = args.iter().skip(1).find(|a| C::subcommands().contains(&a.as_str()))
         && let Some(text) = C::command_help(cmd, &[])
      {
         return Err(CliError::ShowHelp(text));
      }
      return Err(CliError::ShowHelp(C::help_text()));
   }

   Ok(())
}

/// Extract global flags from args (skipping program name at index 0).
/// Returns `(trace, verbose, quiet, dry_run, filtered_args)`.
#[allow(unused_mut)]
pub(super) fn extract_global_flags(args: &[String]) -> (bool, bool, bool, bool, Vec<String>) {
   let mut trace = false;
   let mut verbose = false;
   let mut quiet = false;
   let mut dry_run = false;
   let mut filtered: Vec<String> = Vec::new();
   let mut past_separator = false;

   for arg in args.iter().skip(1) {
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
pub(super) fn dispatch<C: CommandParser>(filtered: &[String]) -> Result<C, CliError> {
   let (first, rest) = match filtered.split_first() {
      Some((f, r)) => (f.as_str(), r),
      None => return Err(CliError::ShowHelp(C::help_text()))
   };

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
