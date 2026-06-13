use std::io::{self, Write};

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn term_width() -> usize {
   super::terminal_width()
}

/// Treat `BrokenPipe` as success — the consumer end is gone (e.g. `head` cut
/// us off) and there's no recovery, so a decorative spinner should just stop
/// drawing rather than panic. Other errors propagate.
fn ignore_broken_pipe(result: io::Result<()>) -> io::Result<()> {
   match result {
      Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
      other => other
   }
}

/// Move cursor up `n` lines and clear each line with `\r\x1b[K`, returning
/// cursor to the top.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] when stdout writes fail. A closed
/// pipe (e.g. piping to `head`) is treated as success and absorbed silently.
pub(crate) fn clear_lines(out: &mut io::StdoutLock, n: usize) -> io::Result<()> {
   ignore_broken_pipe(write_clear_lines(out, n))
}

/// Raw clear-lines writes — no broken-pipe absorption. Use inside an existing
/// `ignore_broken_pipe` closure (e.g. [`render_frame`]); otherwise prefer
/// [`clear_lines`].
fn write_clear_lines(out: &mut io::StdoutLock, n: usize) -> io::Result<()> {
   if n == 0 {
      return Ok(());
   }
   write!(out, "\x1b[{n}A")?;
   for _ in 0..n {
      write!(out, "\r\x1b[K\n")?;
   }
   write!(out, "\x1b[{n}A")
}

/// Erase the previous frame (cursor-up + per-line clear), draw the spinner header and
/// the last N viewport lines truncated to terminal width, and return the number of
/// lines written (each is guaranteed to be exactly one terminal row).
///
/// Viewport slots may contain embedded `\n` characters (e.g. a multi-line progress
/// bar stored as a single `StdoutCr` unit). Each such slot is expanded into multiple
/// visual rows; all rows count toward `prev_lines` so the next frame erases them
/// correctly.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] when stdout writes fail. A closed pipe
/// (e.g. piping to `head`) is treated as success and absorbed silently — the
/// returned row count still reflects what *would* have been drawn so the next
/// frame's erase math stays consistent.
pub(crate) fn render_frame(
   out: &mut io::StdoutLock,
   label: &str,
   viewport: &[String],
   frame: usize,
   prev_lines: usize,
   viewport_size: usize
) -> io::Result<usize> {
   let tw = term_width();

   let visual_rows: Vec<&str> = viewport.iter().flat_map(|s| s.split('\n')).collect();
   let shown_start = visual_rows.len().saturating_sub(viewport_size);
   let shown = &visual_rows[shown_start..];
   let rows = 1 + shown.len();

   ignore_broken_pipe((|| {
      write_clear_lines(out, prev_lines)?;

      let spinner = SPINNER[frame % SPINNER.len()];
      // "⠋ label..." = 1 (spinner) + 1 (space) + label + 3 ("...") = label + 5 visible columns
      let max_label = tw.saturating_sub(5).max(1);
      let display_label: String = label.chars().take(max_label).collect();
      write!(out, "\r\x1b[K{spinner} \x1b[1m{display_label}...\x1b[0m\n")?;

      for row in shown {
         let display = truncate_visible(row, tw.saturating_sub(2).max(1));
         if display.contains('\x1b') {
            write!(out, "\r\x1b[K  {display}\n")?;
         } else {
            write!(out, "\r\x1b[K  \x1b[2m{display}\x1b[0m\n")?;
         }
      }

      out.flush()
   })())?;

   Ok(rows)
}

/// Truncate `s` to at most `max_visible` visible columns, skipping over ANSI
/// escape sequences (which contribute zero visible width) when counting.
/// Any escape sequences that were opened are left open — the caller's
/// surrounding `\x1b[0m` reset closes them.
fn truncate_visible(s: &str, max_visible: usize) -> String {
   let mut out = String::with_capacity(s.len());
   let mut visible = 0usize;
   let mut chars = s.chars().peekable();
   while let Some(ch) = chars.next() {
      if visible >= max_visible {
         break;
      }
      out.push(ch);
      if ch == '\x1b' {
         // Consume the escape sequence without counting it as visible.
         // CSI sequences: \x1b[ ... <final byte in 0x40–0x7E>
         if chars.peek() == Some(&'[') {
            out.push(chars.next().unwrap());
            for inner in chars.by_ref() {
               out.push(inner);
               if ('\x40'..='\x7e').contains(&inner) {
                  break;
               }
            }
         }
      } else {
         visible += 1;
      }
   }
   out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
   use std::io;

   use rstest::rstest;

   use super::{ignore_broken_pipe, truncate_visible};

   // -----------------------------------------------------------------------
   // ignore_broken_pipe — the pipeline-safety guarantee
   // -----------------------------------------------------------------------

   #[test]
   fn broken_pipe_is_absorbed_as_success() {
      let err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed");
      assert!(ignore_broken_pipe(Err(err)).is_ok());
   }

   #[test]
   fn other_io_errors_propagate() {
      let err = io::Error::new(io::ErrorKind::PermissionDenied, "nope");
      let result = ignore_broken_pipe(Err(err));
      assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
   }

   #[test]
   fn ok_passes_through() {
      assert!(ignore_broken_pipe(Ok(())).is_ok());
   }

   // -----------------------------------------------------------------------
   // truncate_visible
   // -----------------------------------------------------------------------

   // -----------------------------------------------------------------------
   // Plain text (no ANSI)
   // -----------------------------------------------------------------------

   #[test]
   fn plain_text_shorter_than_limit_is_unchanged() {
      assert_eq!(truncate_visible("hello", 10), "hello");
   }

   #[test]
   fn plain_text_exactly_at_limit_is_unchanged() {
      assert_eq!(truncate_visible("hello", 5), "hello");
   }

   #[test]
   fn plain_text_longer_than_limit_is_truncated() {
      assert_eq!(truncate_visible("hello world", 5), "hello");
   }

   #[test]
   fn empty_string_returns_empty() {
      assert_eq!(truncate_visible("", 10), "");
   }

   #[test]
   fn zero_limit_returns_empty() {
      assert_eq!(truncate_visible("hello", 0), "");
   }

   #[rstest]
   #[case("abcde", 1, "a")]
   #[case("abcde", 3, "abc")]
   #[case("abcde", 5, "abcde")]
   #[case("abcde", 6, "abcde")]
   fn plain_text_parametrised(#[case] input: &str, #[case] max: usize, #[case] expected: &str) {
      assert_eq!(truncate_visible(input, max), expected);
   }

   // -----------------------------------------------------------------------
   // ANSI escape sequences don't count toward visible width
   // -----------------------------------------------------------------------

   #[test]
   fn ansi_bold_does_not_count_as_visible() {
      // "\x1b[1m" is the bold CSI sequence; it has zero visible width.
      // The trailing reset is beyond the visible limit and is dropped —
      // the function documents that open sequences are left for the caller to close.
      let input = "\x1b[1mhello\x1b[0m";
      assert_eq!(truncate_visible(input, 5), "\x1b[1mhello");
   }

   #[test]
   fn ansi_colour_does_not_count_as_visible() {
      // "\x1b[32m" = green; 5 visible chars.
      // Same as above: trailing reset is dropped once the limit is reached.
      let input = "\x1b[32mhello\x1b[0m";
      assert_eq!(truncate_visible(input, 5), "\x1b[32mhello");
   }

   #[test]
   fn ansi_sequence_at_start_then_truncate_visible_chars() {
      // Bold prefix + 10 visible chars; truncate to 4.
      let input = "\x1b[1m0123456789\x1b[0m";
      let result = truncate_visible(input, 4);
      // The escape is preserved; only 4 visible chars included.
      assert_eq!(result, "\x1b[1m0123");
   }

   #[test]
   fn truncation_mid_text_after_escape_sequence() {
      // "AB\x1b[31mCD" — 4 visible chars; truncate to 3.
      let input = "AB\x1b[31mCD";
      let result = truncate_visible(input, 3);
      assert_eq!(result, "AB\x1b[31mC");
   }

   #[test]
   fn multiple_escape_sequences_all_preserved_within_limit() {
      // Two colour resets surrounding a word; fits within limit.
      let input = "\x1b[2mfoo\x1b[0m";
      assert_eq!(truncate_visible(input, 10), "\x1b[2mfoo\x1b[0m");
   }

   #[test]
   fn escape_sequence_at_exact_boundary_is_dropped() {
      // "hi\x1b[0m" — 2 visible chars then a reset; limit is 2.
      // Once "hi" is written the visible counter hits the limit, so the loop
      // exits before the escape is consumed — the trailing reset is dropped.
      let input = "hi\x1b[0m";
      let result = truncate_visible(input, 2);
      assert_eq!(result, "hi");
   }

   #[test]
   fn leading_escape_only_no_visible_chars_returns_escape() {
      // A lone CSI sequence with no following text still passes through.
      let input = "\x1b[1m";
      assert_eq!(truncate_visible(input, 5), "\x1b[1m");
   }

   #[test]
   fn lone_escape_byte_without_bracket_is_passed_through() {
      // \x1b not followed by '[' is treated as a non-CSI escape: the byte is
      // emitted but no further characters are consumed as part of a sequence.
      // It still doesn't count as visible.
      let input = "\x1babc";
      // Only 'a','b','c' are visible. \x1b is passed through but not counted.
      assert_eq!(truncate_visible(input, 2), "\x1bab");
   }
}
