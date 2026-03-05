//! Help and version printing helpers for CLI tools.
//!
//! Provides two diverging functions that print to stdout and exit:
//! - `print_help(text: &str) -> !` — colorize and print a help string, then exit 0
//! - `print_version(version_str: &str) -> !` — print a version string, then exit 0
//!
//! Both functions are intended to be called from argument parsing code when
//! `-h`/`--help` or `--version` flags are detected.

use crate::{colorize::colorize_text_with_width, terminal::terminal_width};

/// Colorize `text` with a left-to-right rainbow spanning the current terminal
/// width, print it to stdout, and exit with code 0.
///
/// This is the standard help-printing pattern shared by CLI tools in this
/// ecosystem. Call it when `-h` or `--help` is detected.
///
/// # Example
///
/// ```rust,ignore
/// if args.iter().any(|a| a == "-h" || a == "--help") {
///     jt_consoleutils::help::print_help(&build_help_text());
/// }
/// ```
pub fn print_help(text: &str) -> ! {
   let width = terminal_width();
   println!("{}", colorize_text_with_width(text, Some(width)));
   std::process::exit(0);
}

/// Print `version_str` to stdout and exit with code 0.
///
/// Call this when `--version` is detected. `version_str` is typically produced
/// by `jt_consoleutils::version::version_string(BUILD_DATE, GIT_HASH)`.
///
/// # Example
///
/// ```rust,ignore
/// if raw.version {
///     jt_consoleutils::help::print_version(&version::version_string());
/// }
/// ```
pub fn print_version(version_str: &str) -> ! {
   println!("{version_str}");
   std::process::exit(0);
}

#[cfg(test)]
mod tests {
   // print_help and print_version call process::exit, so they cannot be tested
   // for their exit behaviour in-process. The colorize and terminal modules
   // have their own tests; there is nothing further to unit-test here.
}
