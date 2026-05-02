//! Help and version printing helpers for CLI tools.
//!
//! Provides two diverging functions that print to stdout and exit:
//! - `print_help(text: &str) -> !` — colorize and print a help string, then exit 0
//! - `print_version(version_str: &str) -> !` — print a version string, then exit 0
//!
//! Both functions are intended to be called from argument parsing code when
//! `-h`/`--help` or `--version` flags are detected.

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
/// width, print it to stdout, and exit with code 0.
///
/// Lines longer than the terminal width are word-wrapped before colorizing,
/// preserving leading indentation.
///
/// This is the standard help-printing pattern shared by CLI tools in this
/// ecosystem. Call it when `-h` or `--help` is detected.
///
/// # Example
///
/// ```rust,ignore
/// if args.iter().any(|a| a == "-h" || a == "--help") {
///     jt_consoleutils::cli::help::print_help(&build_help_text());
/// }
/// ```
pub fn print_help(text: &str) -> ! {
   let width = terminal_width();
   let wrapped = wrap_help_text(text, width);
   println!("{}", colorize_text_with_width(&wrapped, Some(width)));
   std::process::exit(0);
}

/// Print `version_str` to stdout and exit with code 0.
///
/// Call this when `--version` is detected. `version_str` is typically produced
/// by `jt_consoleutils::cli::version::version_string(BUILD_DATE, GIT_HASH)`.
///
/// # Example
///
/// ```rust,ignore
/// if raw.version {
///     jt_consoleutils::cli::help::print_version(&version::version_string());
/// }
/// ```
pub fn print_version(version_str: &str) -> ! {
   println!("{version_str}");
   std::process::exit(0);
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
