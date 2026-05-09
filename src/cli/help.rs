//! Help and version printing helpers for CLI tools.
//!
//! Provides two functions that print to stdout (without exiting):
//! - `print_help(text: &str)` — word-wrap, colorize, and print a help string
//! - `print_version(version_str: &str)` — print a version string
//!
//! Both are intended to be called from `main()` when handling
//! [`CliError::ShowHelp`](crate::cli::CliError::ShowHelp) or
//! [`CliError::ShowVersion`](crate::cli::CliError::ShowVersion) returned by
//! [`parse_cli`](crate::cli::parse_cli). The application is responsible for
//! calling [`std::process::exit`] (typically with status `0`) afterwards.

use crate::terminal::{colorize::colorize_text_with_width, terminal_width};

/// Word-wrap each line of `text` to fit within `width` columns.
///
/// Lines shorter than `width` pass through unchanged. Lines longer than `width`
/// are broken at word boundaries, with continuation lines preserving the
/// original leading indentation. A single word longer than the available width
/// is placed on its own line without breaking.
#[must_use]
pub fn wrap_help_text(text: &str, width: usize) -> String {
   text
      .lines()
      .map(|line| {
         if line.len() <= width {
            return line.to_string();
         }
         let indent_len = line.len() - line.trim_start().len();
         let indent = &line[..indent_len];
         let max_content = width.saturating_sub(indent_len);
         if max_content == 0 {
            return line.to_string();
         }
         let mut lines: Vec<String> = Vec::new();
         let mut current = String::from(indent);
         let mut content_len = 0usize;
         for word in line[indent_len..].split_whitespace() {
            if content_len == 0 {
               current.push_str(word);
               content_len = word.len();
            } else if content_len + 1 + word.len() <= max_content {
               current.push(' ');
               current.push_str(word);
               content_len += 1 + word.len();
            } else {
               lines.push(current);
               current = String::from(indent);
               current.push_str(word);
               content_len = word.len();
            }
         }
         if content_len > 0 {
            lines.push(current);
         }
         lines.join("\n")
      })
      .collect::<Vec<_>>()
      .join("\n")
}

/// Colorize `text` with a left-to-right rainbow spanning the current terminal
/// width and print it to stdout.
///
/// Lines longer than the terminal width are word-wrapped before colorizing,
/// preserving leading indentation. This function does **not** call
/// [`std::process::exit`] — callers handling
/// [`CliError::ShowHelp`](crate::cli::CliError::ShowHelp) typically exit
/// with status `0` afterwards.
///
/// # Example
///
/// ```rust,ignore
/// match parse_cli::<Cmd>() {
///     Ok(parsed) => run(parsed),
///     Err(CliError::ShowHelp(text)) => {
///         jt_consoleutils::cli::help::print_help(&text);
///         std::process::exit(0);
///     }
///     Err(e) => { eprintln!("Error: {e}"); std::process::exit(1); }
/// }
/// ```
pub fn print_help(text: &str) {
   let width = terminal_width();
   let wrapped = wrap_help_text(text, width);
   println!("{}", colorize_text_with_width(&wrapped, Some(width)));
}

/// Print `version_str` to stdout.
///
/// Intended for handling [`CliError::ShowVersion`](crate::cli::CliError::ShowVersion).
/// `version_str` is typically produced by
/// [`crate::cli::version::version_string`]. This function does **not** call
/// [`std::process::exit`] — the caller decides what to do next.
pub fn print_version(version_str: &str) {
   println!("{version_str}");
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn short_lines_pass_through() {
      let text = "hello\nworld";
      assert_eq!(wrap_help_text(text, 80), "hello\nworld");
   }

   #[test]
   fn long_line_wraps_at_word_boundary() {
      let text = "the quick brown fox jumps over the lazy dog";
      let wrapped = wrap_help_text(text, 20);
      for line in wrapped.lines() {
         assert!(line.len() <= 20, "line too long: {:?} ({})", line, line.len());
      }
      assert!(wrapped.lines().count() > 1);
   }

   #[test]
   fn preserves_indentation_on_wrap() {
      let text = "    the quick brown fox jumps over the lazy dog";
      let wrapped = wrap_help_text(text, 25);
      for line in wrapped.lines() {
         assert!(line.starts_with("    "), "missing indent: {:?}", line);
      }
   }

   #[test]
   fn blank_lines_preserved() {
      let text = "hello\n\nworld";
      assert_eq!(wrap_help_text(text, 80), "hello\n\nworld");
   }

   #[test]
   fn single_word_longer_than_width() {
      let text = "superlongwordthatcannotbreak";
      let wrapped = wrap_help_text(text, 10);
      assert_eq!(wrapped, "superlongwordthatcannotbreak");
   }
}
