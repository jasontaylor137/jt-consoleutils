use super::{dispatch, extract_global_flags, flags_to_level, parse_cli_from};
use crate::{
   cli::types::{CliError, CommandParser},
   output::LogLevel
};

// ---------------------------------------------------------------------------
// Test command types
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum TestCmd {
   Greet { name: String },
   List,
   DefaultRun { path: String, args: Vec<String> }
}

impl CommandParser for TestCmd {
   fn subcommands() -> &'static [&'static str] {
      &["greet", "list"]
   }

   fn parse(name: &str, args: &[String]) -> Result<Self, CliError> {
      match name {
         "greet" => {
            let name = args.first().ok_or_else(|| CliError::usage("greet requires a name"))?;
            Ok(TestCmd::Greet { name: name.clone() })
         }
         "list" => Ok(TestCmd::List),
         _ => unreachable!()
      }
   }

   fn default_command(first_arg: &str, rest: &[String]) -> Option<Result<Self, CliError>> {
      Some(Ok(TestCmd::DefaultRun { path: first_arg.to_string(), args: rest.to_vec() }))
   }

   fn version() -> String {
      "test 1.0".to_string()
   }

   fn help_text() -> String {
      "Test help text".to_string()
   }

   fn command_help(cmd: &str, _args: &[String]) -> Option<String> {
      match cmd {
         "greet" => Some("Greet someone".to_string()),
         _ => None
      }
   }
}

#[derive(Debug, PartialEq)]
enum StrictCmd {
   Run
}

impl CommandParser for StrictCmd {
   fn subcommands() -> &'static [&'static str] {
      &["run"]
   }

   fn parse(name: &str, _args: &[String]) -> Result<Self, CliError> {
      match name {
         "run" => Ok(StrictCmd::Run),
         _ => unreachable!()
      }
   }

   fn version() -> String {
      "strict 1.0".to_string()
   }

   fn help_text() -> String {
      "Strict help".to_string()
   }
}

fn sv(v: &[&str]) -> Vec<String> {
   v.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// extract_global_flags tests
// ---------------------------------------------------------------------------

#[test]
fn extract_global_flags_no_flags() {
   // Given
   let args = sv(&["app", "list"]);

   // When
   let (trace, verbose, quiet, dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(!trace);
   assert!(!verbose);
   assert!(!quiet);
   assert!(!dry_run);
   assert_eq!(filtered, sv(&["list"]));
}

#[test]
fn extract_global_flags_quiet_short() {
   // Given
   let args = sv(&["app", "-q", "list"]);

   // When
   let (trace, verbose, quiet, dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(!trace);
   assert!(!verbose);
   assert!(quiet);
   assert!(!dry_run);
   assert_eq!(filtered, sv(&["list"]));
}

#[test]
fn extract_global_flags_quiet_long() {
   // Given
   let args = sv(&["app", "--quiet", "list"]);

   // When
   let (_trace, _verbose, quiet, _dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(quiet);
   assert_eq!(filtered, sv(&["list"]));
}

#[test]
fn extract_global_flags_dry_run_short() {
   // Given
   let args = sv(&["app", "-d", "list"]);

   // When
   let (_trace, _verbose, _quiet, dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(dry_run);
   assert_eq!(filtered, sv(&["list"]));
}

#[test]
fn extract_global_flags_dry_run_long() {
   // Given
   let args = sv(&["app", "--dry-run", "list"]);

   // When
   let (_trace, _verbose, _quiet, dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(dry_run);
   assert_eq!(filtered, sv(&["list"]));
}

#[cfg(feature = "verbose")]
#[test]
fn extract_global_flags_verbose_short() {
   // Given
   let args = sv(&["app", "-v", "list"]);

   // When
   let (_trace, verbose, _quiet, _dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(verbose);
   assert_eq!(filtered, sv(&["list"]));
}

#[cfg(feature = "verbose")]
#[test]
fn extract_global_flags_verbose_long() {
   // Given
   let args = sv(&["app", "--verbose", "list"]);

   // When
   let (_trace, verbose, _quiet, _dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(verbose);
   assert_eq!(filtered, sv(&["list"]));
}

#[cfg(feature = "trace")]
#[test]
fn extract_global_flags_trace_short() {
   // Given
   let args = sv(&["app", "-t", "list"]);

   // When
   let (trace, _verbose, _quiet, _dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(trace);
   assert_eq!(filtered, sv(&["list"]));
}

#[cfg(feature = "trace")]
#[test]
fn extract_global_flags_trace_long() {
   // Given
   let args = sv(&["app", "--trace", "list"]);

   // When
   let (trace, _verbose, _quiet, _dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(trace);
   assert_eq!(filtered, sv(&["list"]));
}

#[test]
fn extract_global_flags_separator_preserves_flags_after() {
   // Given
   let args = sv(&["app", "--", "-q", "extra"]);

   // When
   let (_trace, _verbose, quiet, _dry_run, filtered) = extract_global_flags(&args);

   // Then: -q after -- is not treated as a flag
   assert!(!quiet);
   assert_eq!(filtered, sv(&["--", "-q", "extra"]));
}

#[test]
fn extract_global_flags_empty_args() {
   // Given
   let args = sv(&["app"]);

   // When
   let (trace, verbose, quiet, dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(!trace);
   assert!(!verbose);
   assert!(!quiet);
   assert!(!dry_run);
   assert!(filtered.is_empty());
}

#[cfg(feature = "verbose")]
#[test]
fn extract_global_flags_multiple_flags() {
   // Given
   let args = sv(&["app", "-v", "-d", "list"]);

   // When
   let (_trace, verbose, _quiet, dry_run, filtered) = extract_global_flags(&args);

   // Then
   assert!(verbose);
   assert!(dry_run);
   assert_eq!(filtered, sv(&["list"]));
}

// ---------------------------------------------------------------------------
// flags_to_level tests
// ---------------------------------------------------------------------------

#[test]
fn flags_to_level_all_false_is_normal() {
   // Given / When
   let level = flags_to_level(false, false, false);

   // Then
   assert_eq!(level, LogLevel::Normal);
}

#[test]
fn flags_to_level_quiet() {
   // Given / When
   let level = flags_to_level(false, false, true);

   // Then
   assert_eq!(level, LogLevel::Quiet);
}

#[cfg(feature = "verbose")]
#[test]
fn flags_to_level_verbose() {
   // Given / When
   let level = flags_to_level(false, true, false);

   // Then
   assert_eq!(level, LogLevel::Verbose);
}

#[cfg(feature = "trace")]
#[test]
fn flags_to_level_trace() {
   // Given / When
   let level = flags_to_level(true, false, false);

   // Then
   assert_eq!(level, LogLevel::Trace);
}

// ---------------------------------------------------------------------------
// dispatch tests — TestCmd (has default_command)
// ---------------------------------------------------------------------------

#[test]
fn dispatch_recognized_subcommand() {
   // Given
   let filtered = sv(&["greet", "Alice"]);

   // When
   let result = dispatch::<TestCmd>(&filtered);

   // Then
   assert_eq!(result.unwrap(), TestCmd::Greet { name: "Alice".to_string() });
}

#[test]
fn dispatch_recognized_subcommand_no_args() {
   // Given
   let filtered = sv(&["list"]);

   // When
   let result = dispatch::<TestCmd>(&filtered);

   // Then
   assert_eq!(result.unwrap(), TestCmd::List);
}

#[test]
fn dispatch_default_command() {
   // Given
   let filtered = sv(&["script.ts", "arg1"]);

   // When
   let result = dispatch::<TestCmd>(&filtered);

   // Then
   assert_eq!(result.unwrap(), TestCmd::DefaultRun { path: "script.ts".to_string(), args: sv(&["arg1"]) });
}

#[test]
fn dispatch_unknown_command_starting_with_dash_is_error() {
   // Given
   let filtered = sv(&["--bogus"]);

   // When
   let result = dispatch::<TestCmd>(&filtered);

   // Then
   let err = result.unwrap_err().to_string();
   assert!(err.contains("unknown command"), "expected 'unknown command' in: {err}");
}

// ---------------------------------------------------------------------------
// dispatch tests — StrictCmd (no default_command)
// ---------------------------------------------------------------------------

#[test]
fn dispatch_no_default_command_unknown_arg_is_error() {
   // Given
   let filtered = sv(&["script.ts"]);

   // When
   let result = dispatch::<StrictCmd>(&filtered);

   // Then
   let err = result.unwrap_err().to_string();
   assert!(err.contains("unknown command: script.ts"), "expected 'unknown command: script.ts' in: {err}");
}

// ---------------------------------------------------------------------------
// dispatch error propagation
// ---------------------------------------------------------------------------

#[test]
fn dispatch_subcommand_parser_error_propagates() {
   // Given: greet requires a name argument — omit it
   let filtered = sv(&["greet"]);

   // When
   let result = dispatch::<TestCmd>(&filtered);

   // Then
   let err = result.unwrap_err().to_string();
   assert!(err.contains("greet requires a name"), "expected 'greet requires a name' in: {err}");
}

// ---------------------------------------------------------------------------
// parse_cli_from — help / version surface as ShowHelp / ShowVersion (no exit)
// ---------------------------------------------------------------------------

#[test]
fn parse_cli_from_no_args_returns_show_help() {
   // Given
   let argv: Vec<String> = sv(&[]);

   // When
   let result = parse_cli_from::<TestCmd>(&argv);

   // Then
   match result {
      Err(CliError::ShowHelp(text)) => assert_eq!(text, "Test help text"),
      other => panic!("expected ShowHelp, got {other:?}")
   }
}

#[test]
fn parse_cli_from_help_flag_returns_show_help() {
   // Given
   let argv = sv(&["--help"]);

   // When
   let result = parse_cli_from::<TestCmd>(&argv);

   // Then
   match result {
      Err(CliError::ShowHelp(text)) => assert_eq!(text, "Test help text"),
      other => panic!("expected ShowHelp, got {other:?}")
   }
}

#[test]
fn parse_cli_from_help_short_flag_returns_show_help() {
   // Given
   let argv = sv(&["-h"]);

   // When
   let result = parse_cli_from::<TestCmd>(&argv);

   // Then
   assert!(matches!(result, Err(CliError::ShowHelp(_))));
}

#[test]
fn parse_cli_from_help_subcommand_returns_command_help() {
   // Given: `help greet` should surface the per-command help text
   let argv = sv(&["help", "greet"]);

   // When
   let result = parse_cli_from::<TestCmd>(&argv);

   // Then
   match result {
      Err(CliError::ShowHelp(text)) => assert_eq!(text, "Greet someone"),
      other => panic!("expected ShowHelp(\"Greet someone\"), got {other:?}")
   }
}

#[test]
fn parse_cli_from_help_unknown_subcommand_falls_back_to_main_help() {
   // Given: `help bogus` — TestCmd::command_help returns None for "bogus"
   let argv = sv(&["help", "bogus"]);

   // When
   let result = parse_cli_from::<TestCmd>(&argv);

   // Then
   match result {
      Err(CliError::ShowHelp(text)) => assert_eq!(text, "Test help text"),
      other => panic!("expected ShowHelp main help, got {other:?}")
   }
}

#[test]
fn parse_cli_from_subcommand_then_help_returns_command_help() {
   // Given: `greet --help` — subcommand precedes -h
   let argv = sv(&["greet", "--help"]);

   // When
   let result = parse_cli_from::<TestCmd>(&argv);

   // Then
   match result {
      Err(CliError::ShowHelp(text)) => assert_eq!(text, "Greet someone"),
      other => panic!("expected ShowHelp(\"Greet someone\"), got {other:?}")
   }
}

#[test]
fn parse_cli_from_version_returns_show_version() {
   // Given
   let argv = sv(&["--version"]);

   // When
   let result = parse_cli_from::<TestCmd>(&argv);

   // Then
   match result {
      Err(CliError::ShowVersion(text)) => assert_eq!(text, "test 1.0"),
      other => panic!("expected ShowVersion, got {other:?}")
   }
}

#[test]
fn parse_cli_from_conflicting_flags_returns_conflict_not_help() {
   // Given: -v and -q together — verify normal flag conflicts still surface
   #[cfg(feature = "verbose")]
   {
      let argv = sv(&["-v", "-q", "list"]);
      let result = parse_cli_from::<TestCmd>(&argv);
      assert!(matches!(result, Err(CliError::Conflict(_))));
   }
}
