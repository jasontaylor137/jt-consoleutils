//! Two-tier progress bar with `\r`-based redraw, ANSI colors, optional
//! caller-supplied substatus text, threshold-delayed reveal, and
//! clear/finish/redraw operations.
//!
//! The bar tracks a single integer counter (`current` / `total`) and renders
//! as a fixed-width `[##---] n/m` line. The substatus is an arbitrary string
//! supplied by the caller (e.g. `"Hashing 12/30 (size 1024)"`); it does not
//! appear until the threshold has elapsed since the last
//! [`Progress::next`] call, so quick steps never flash a substatus.
//!
//! # Output integration
//!
//! All draw/clear/finish methods call back into a `&mut dyn Output`. The bar
//! writes to [`Output::write`] (no trailing newline) so the next render can
//! overwrite the line via `\r`. Use [`Output::writeln`] in between for
//! permanent log lines, then [`Progress::redraw`] to reprint the bar.
//!
//! # Example
//!
//! ```no_run
//! use jt_consoleutils::output::{progress::Progress, ConsoleOutput, OutputMode};
//!
//! let mut out = ConsoleOutput::new(OutputMode::default());
//! let mut bar = Progress::new("Comparing: ", 100);
//! for _ in 0..100 {
//!    bar.next(&mut out);
//!    // do work...
//!    bar.set_substatus("processing chunk 1/4", &mut out);
//! }
//! bar.finish(&mut out);
//! ```

use std::time::{Duration, Instant};

use crate::{
   output::Output,
   terminal::colors::{DIM, GREEN, RESET}
};

/// Default substatus reveal threshold. Substatus updates before this elapses
/// since the last [`Progress::next`] call are silently dropped, so quick
/// steps don't flash a substatus.
pub const DEFAULT_SUBSTATUS_THRESHOLD: Duration = Duration::from_secs(5);

/// Default rendered width of the bar (the `[####----]` portion, in chars).
pub const DEFAULT_BAR_WIDTH: usize = 30;

/// Two-tier progress bar with optional caller-supplied substatus.
pub struct Progress {
   prefix: String,
   total: usize,
   current: usize,
   substatus: String,
   substatus_start: Instant,
   substatus_shown: bool,
   threshold: Duration,
   bar_width: usize,
   last_len: usize
}

impl Progress {
   /// Create a new progress bar with the given prefix label and total step
   /// count. Uses [`DEFAULT_SUBSTATUS_THRESHOLD`] and [`DEFAULT_BAR_WIDTH`].
   pub fn new(prefix: impl Into<String>, total: usize) -> Self {
      Self::with_settings(prefix, total, DEFAULT_SUBSTATUS_THRESHOLD, DEFAULT_BAR_WIDTH)
   }

   /// Create a new progress bar with a custom substatus threshold and bar width.
   pub fn with_settings(prefix: impl Into<String>, total: usize, threshold: Duration, bar_width: usize) -> Self {
      Self {
         prefix: prefix.into(),
         total,
         current: 0,
         substatus: String::new(),
         substatus_start: Instant::now(),
         substatus_shown: false,
         threshold,
         bar_width,
         last_len: 0
      }
   }

   /// Advance the bar by one step. Resets the substatus and its threshold
   /// timer; redraws the bar.
   pub fn next(&mut self, output: &mut dyn Output) {
      self.current += 1;
      self.substatus.clear();
      self.substatus_start = Instant::now();
      self.substatus_shown = false;
      self.draw(output);
   }

   /// Update the substatus text. The text is only rendered after the
   /// threshold has elapsed since the last [`Progress::next`] call (or
   /// immediately, on subsequent calls within the same step once the
   /// threshold has been crossed).
   ///
   /// Pass an empty string to clear the substatus on the next draw.
   pub fn set_substatus(&mut self, text: impl Into<String>, output: &mut dyn Output) {
      self.substatus = text.into();
      if self.substatus_shown || self.substatus_start.elapsed() >= self.threshold {
         self.substatus_shown = true;
         self.draw(output);
      }
   }

   /// Erase the current bar from the terminal. Useful before printing a
   /// permanent log line that should appear above the bar.
   pub fn clear(&mut self, output: &mut dyn Output) {
      if self.last_len > 0 {
         output.write(&format!("\r{}\r", " ".repeat(self.last_len)));
      }
   }

   /// Re-render the bar. Pair with [`Progress::clear`] around log writes
   /// that need to appear above the bar.
   pub fn redraw(&mut self, output: &mut dyn Output) {
      self.draw(output);
   }

   /// Erase the bar permanently. After this, [`Progress::next`] /
   /// [`Progress::redraw`] will draw a fresh bar.
   pub fn finish(&mut self, output: &mut dyn Output) {
      self.clear(output);
      self.last_len = 0;
   }

   /// Current step count (`0` initially, bumped by each [`Progress::next`]).
   pub fn current(&self) -> usize {
      self.current
   }

   /// Total step count, as supplied to the constructor.
   pub fn total(&self) -> usize {
      self.total
   }

   fn draw(&mut self, output: &mut dyn Output) {
      let filled = (self.current * self.bar_width).checked_div(self.total).unwrap_or(0);
      let empty = self.bar_width - filled;
      let counter = format!("{}/{}", self.current, self.total);

      // Track visual width (excludes ANSI escape codes).
      let mut visual_len = self.prefix.len() + 1 + self.bar_width + 2 + counter.len(); // prefix[###---] n/m

      let mut line =
         format!("{}[{GREEN}{}{RESET}{DIM}{}{RESET}] {counter}", self.prefix, "#".repeat(filled), "-".repeat(empty));

      if self.substatus_shown && !self.substatus.is_empty() {
         visual_len += self.substatus.len();
         line.push_str(&format!("{DIM}{}{RESET}", self.substatus));
      }

      // Pad with spaces to overwrite any leftover characters from the previous render.
      if visual_len < self.last_len {
         line.push_str(&" ".repeat(self.last_len - visual_len));
      }

      output.write(&format!("\r{line}"));
      self.last_len = visual_len;
   }
}

#[cfg(test)]
mod tests {
   use super::*;
   use crate::output::StringOutput;

   // -- bar rendering --

   #[test]
   fn bar_renders_on_next() {
      // Given
      let mut out = StringOutput::new();
      let mut progress = Progress::new("", 10);

      // When
      progress.next(&mut out);

      // Then — counter and at least some filled cells visible
      assert!(out.log().contains("] 1/10"));
      assert!(out.log().contains("###"));
   }

   #[test]
   fn bar_fills_proportionally() {
      // Given
      let mut out = StringOutput::new();
      let mut progress = Progress::new("", 2);

      // When
      progress.next(&mut out);
      progress.next(&mut out);

      // Then
      assert!(out.log().contains("] 2/2"));
   }

   #[test]
   fn current_and_total_track_state() {
      // Given
      let mut out = StringOutput::new();
      let mut progress = Progress::new("", 3);

      // When
      assert_eq!(progress.current(), 0);
      assert_eq!(progress.total(), 3);
      progress.next(&mut out);

      // Then
      assert_eq!(progress.current(), 1);
   }

   // -- substatus --

   #[test]
   fn substatus_not_shown_before_threshold() {
      // Given
      let mut out = StringOutput::new();
      let mut progress = Progress::new("", 10);
      progress.next(&mut out);

      // When — set substatus immediately, before threshold elapses
      progress.set_substatus("Hashing 1/5", &mut out);
      progress.set_substatus("Hashing 2/5", &mut out);

      // Then
      assert!(!progress.substatus_shown);
      assert!(!out.log().contains("Hashing"));
   }

   #[test]
   fn substatus_shown_after_threshold() {
      // Given
      let mut out = StringOutput::new();
      let mut progress = Progress::new("", 10);
      progress.next(&mut out);
      progress.expire_threshold_for_test();

      // When
      progress.set_substatus("Hashing 1/5 (size 1024)", &mut out);

      // Then
      assert!(progress.substatus_shown);
      assert!(out.log().contains("Hashing 1/5 (size 1024)"));
   }

   #[test]
   fn next_resets_substatus_and_timer() {
      // Given — substatus shown after threshold
      let mut out = StringOutput::new();
      let mut progress = Progress::new("", 10);
      progress.next(&mut out);
      progress.expire_threshold_for_test();
      progress.set_substatus("phase A", &mut out);
      assert!(progress.substatus_shown);

      // When — next() resets state
      progress.next(&mut out);

      // Then — substatus hidden again, requires another threshold
      assert!(!progress.substatus_shown);
   }

   #[test]
   fn substatus_threshold_configurable() {
      // Given — zero threshold ⇒ substatus shows immediately
      let mut out = StringOutput::new();
      let mut progress = Progress::with_settings("", 5, Duration::from_secs(0), DEFAULT_BAR_WIDTH);
      progress.next(&mut out);

      // When
      progress.set_substatus("immediate", &mut out);

      // Then
      assert!(progress.substatus_shown);
      assert!(out.log().contains("immediate"));
   }

   // -- clear / finish --

   #[test]
   fn clear_erases_bar() {
      // Given
      let mut out = StringOutput::new();
      let mut progress = Progress::new("", 5);
      progress.next(&mut out);
      let before = out.log().len();

      // When
      progress.clear(&mut out);

      // Then — clear writes \r + spaces + \r
      assert!(out.log().len() > before);
      assert!(out.log().ends_with('\r'));
   }

   #[test]
   fn finish_clears_bar_and_zeroes_state() {
      // Given
      let mut out = StringOutput::new();
      let mut progress = Progress::new("", 5);
      progress.next(&mut out);

      // When
      progress.finish(&mut out);

      // Then
      assert!(out.log().ends_with('\r'));
      assert_eq!(progress.last_len, 0);
   }

   // -- bar width --

   #[test]
   fn bar_width_configurable() {
      // Given
      let mut out = StringOutput::new();
      let mut progress = Progress::with_settings("p:", 4, DEFAULT_SUBSTATUS_THRESHOLD, 8);

      // When
      progress.next(&mut out);

      // Then — 1/4 of an 8-wide bar = 2 filled, 6 empty (ANSI codes between)
      let log = out.log();
      assert!(log.contains("##"));
      assert!(log.contains("------"));
      assert!(log.contains("] 1/4"));
   }

   /// Test-only helper to skip past the substatus threshold.
   impl Progress {
      fn expire_threshold_for_test(&mut self) {
         self.substatus_start = Instant::now() - self.threshold - Duration::from_secs(1);
      }
   }
}
