use std::io::{self, Write};

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn term_width() -> usize {
   super::terminal_width()
}

/// Animated spinner with a scrolling viewport, drawn directly to stdout.
///
/// Each call to [`Spinner::tick`] erases the previous frame, advances the spinner
/// glyph, and redraws a header (`spinner-glyph label...`) followed by the last
/// `viewport_size` lines pushed via [`Spinner::push_line`] /
/// [`Spinner::replace_last_line`]. Lines wider than the terminal are truncated;
/// embedded `\n` characters expand into multiple visual rows.
///
/// The spinner is drawn to the locked stdout — callers should avoid writing to
/// stdout from other code paths between [`Spinner::tick`] calls or the rendered
/// frame will be corrupted. Call [`Spinner::clear`] when done to erase the frame
/// before printing a final summary line.
///
/// ```no_run
/// use std::{thread, time::Duration};
///
/// use jt_consoleutils::terminal::overlay::Spinner;
///
/// let mut s = Spinner::new("downloading", 5);
/// for i in 0..20 {
///    s.push_line(format!("chunk {i} of 20"));
///    s.tick();
///    thread::sleep(Duration::from_millis(80));
/// }
/// s.clear();
/// println!("done");
/// ```
pub struct Spinner {
   label: String,
   viewport: Vec<String>,
   viewport_size: usize,
   frame: usize,
   last_rows: usize
}

impl Spinner {
   /// Create a new spinner with the given header `label` and a viewport that
   /// shows the most recent `viewport_size` lines. A `viewport_size` of `0`
   /// renders only the header row.
   #[must_use]
   pub fn new(label: impl Into<String>, viewport_size: usize) -> Self {
      Self { label: label.into(), viewport: Vec::new(), viewport_size, frame: 0, last_rows: 0 }
   }

   /// Replace the header label shown next to the spinner glyph. Takes effect on
   /// the next [`Self::tick`] / [`Self::render`] call.
   pub fn set_label(&mut self, label: impl Into<String>) {
      self.label = label.into();
   }

   /// Append a new line to the viewport.
   pub fn push_line(&mut self, line: impl Into<String>) {
      self.viewport.push(line.into());
   }

   /// Overwrite the most recently pushed viewport line in place — the
   /// equivalent of `\r` on a TTY. If the viewport is empty this pushes the
   /// line instead.
   pub fn replace_last_line(&mut self, line: impl Into<String>) {
      let line = line.into();
      if let Some(last) = self.viewport.last_mut() {
         *last = line;
      } else {
         self.viewport.push(line);
      }
   }

   /// Advance the spinner glyph by one frame and redraw.
   pub fn tick(&mut self) {
      self.frame = self.frame.wrapping_add(1);
      self.draw();
   }

   /// Redraw without advancing the spinner glyph. Useful after a label or
   /// viewport change when no animation tick is desired.
   pub fn render(&mut self) {
      self.draw();
   }

   /// Erase the current frame from the terminal. The spinner can be used again
   /// after this — the frame counter is preserved so animation continues from
   /// where it left off.
   pub fn clear(&mut self) {
      let stdout = io::stdout();
      let mut out = stdout.lock();
      clear_lines(&mut out, self.last_rows);
      self.last_rows = 0;
   }

   fn draw(&mut self) {
      let stdout = io::stdout();
      let mut out = stdout.lock();
      self.last_rows = render_frame(&mut out, &self.label, &self.viewport, self.frame, self.last_rows, self.viewport_size);
   }
}

/// Move cursor up `n` lines and clear each line with `\r\x1b[K`, returning cursor to the top.
pub(crate) fn clear_lines(out: &mut io::StdoutLock, n: usize) {
   if n == 0 {
      return;
   }
   write!(out, "\x1b[{n}A").unwrap();
   for _ in 0..n {
      write!(out, "\r\x1b[K\n").unwrap();
   }
   write!(out, "\x1b[{n}A").unwrap();
}

/// Erase the previous frame (cursor-up + per-line clear), draw the spinner header and
/// the last N viewport lines truncated to terminal width, and return the number of
/// lines written (each is guaranteed to be exactly one terminal row).
///
/// Viewport slots may contain embedded `\n` characters (e.g. a multi-line progress
/// bar stored as a single `StdoutCr` unit). Each such slot is expanded into multiple
/// visual rows; all rows count toward `prev_lines` so the next frame erases them
/// correctly.
pub(crate) fn render_frame(
   out: &mut io::StdoutLock,
   label: &str,
   viewport: &[String],
   frame: usize,
   prev_lines: usize,
   viewport_size: usize
) -> usize {
   let tw = term_width();

   if prev_lines > 0 {
      write!(out, "\x1b[{prev_lines}A").unwrap();
      for _ in 0..prev_lines {
         write!(out, "\r\x1b[K\n").unwrap();
      }
      write!(out, "\x1b[{prev_lines}A").unwrap();
   }

   let spinner = SPINNER[frame % SPINNER.len()];
   // "⠋ label..." = 1 (spinner) + 1 (space) + label + 3 ("...") = label + 5 visible columns
   let max_label = tw.saturating_sub(5).max(1);
   let display_label: String = label.chars().take(max_label).collect();
   write!(out, "\r\x1b[K{spinner} \x1b[1m{display_label}...\x1b[0m\n").unwrap();

   // Expand every slot into its constituent visual rows so that a single slot
   // holding "line1\nline2" renders as two terminal rows.
   let visual_rows: Vec<&str> = viewport.iter().flat_map(|s| s.split('\n')).collect();

   let shown_start = visual_rows.len().saturating_sub(viewport_size);
   let shown = &visual_rows[shown_start..];
   for row in shown {
      let display = truncate_visible(row, tw.saturating_sub(2).max(1));
      if display.contains('\x1b') {
         write!(out, "\r\x1b[K  {display}\n").unwrap();
      } else {
         write!(out, "\r\x1b[K  \x1b[2m{display}\x1b[0m\n").unwrap();
      }
   }

   out.flush().unwrap();
   1 + shown.len()
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
   use rstest::rstest;

   use super::{Spinner, truncate_visible};

   // -----------------------------------------------------------------------
   // Spinner state transitions (rendering itself is not exercised — that
   // writes to stdout and depends on a TTY)
   // -----------------------------------------------------------------------

   #[test]
   fn push_line_appends_to_viewport() {
      let mut s = Spinner::new("label", 5);
      s.push_line("first");
      s.push_line("second");
      assert_eq!(s.viewport, vec!["first".to_string(), "second".to_string()]);
   }

   #[test]
   fn replace_last_line_overwrites_when_viewport_nonempty() {
      let mut s = Spinner::new("label", 5);
      s.push_line("first");
      s.replace_last_line("second");
      assert_eq!(s.viewport, vec!["second".to_string()]);
   }

   #[test]
   fn replace_last_line_pushes_when_viewport_empty() {
      let mut s = Spinner::new("label", 5);
      s.replace_last_line("only");
      assert_eq!(s.viewport, vec!["only".to_string()]);
   }

   #[test]
   fn set_label_replaces_label() {
      let mut s = Spinner::new("before", 5);
      s.set_label("after");
      assert_eq!(s.label, "after");
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
