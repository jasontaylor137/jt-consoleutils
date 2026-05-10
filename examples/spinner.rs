//! Drive [`jt_consoleutils::terminal::overlay::Spinner`] directly, without
//! routing through `Shell`. Useful when the long-running work is in-process
//! (HTTP fetch, file I/O, async task) rather than a child command.
//!
//! Run with:
//!
//!   cargo run --example spinner
//!
//! Three back-to-back scenarios — a plain spinner with no viewport lines, a
//! spinner with a streaming log of recent events, and a spinner whose last
//! line is overwritten in place (carriage-return style for a progress bar).

use std::{thread, time::Duration};

use jt_consoleutils::terminal::overlay::Spinner;

fn plain_spinner() {
   let mut s = Spinner::new("waiting on slow thing", 0);
   for _ in 0..30 {
      s.tick();
      thread::sleep(Duration::from_millis(80));
   }
   s.clear();
   println!("plain spinner: done");
}

fn streaming_log() {
   let mut s = Spinner::new("processing items", 5);
   let items = [
      "loaded config.json",
      "connecting to api.example.com",
      "fetched 142 records",
      "validating schema",
      "applying transforms",
      "writing batch 1/3",
      "writing batch 2/3",
      "writing batch 3/3",
      "verifying checksums",
      "closing connections"
   ];
   for item in items {
      s.push_line(item);
      // A few ticks per line so the spinner glyph keeps animating.
      for _ in 0..6 {
         s.tick();
         thread::sleep(Duration::from_millis(80));
      }
   }
   s.clear();
   println!("streaming log: done ({} steps)", items.len());
}

fn progress_bar() {
   let mut s = Spinner::new("downloading 50 MB", 1);
   for pct in 0..=100 {
      let filled = pct / 5;
      let empty = 20 - filled;
      let bar = format!("[{}{}] {pct:>3}%", "#".repeat(filled), " ".repeat(empty));
      s.replace_last_line(bar);
      s.tick();
      thread::sleep(Duration::from_millis(40));
   }
   s.clear();
   println!("progress bar: done");
}

fn main() {
   plain_spinner();
   streaming_log();
   progress_bar();
}
