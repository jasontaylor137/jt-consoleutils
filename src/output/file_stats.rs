//! Generic file-operation telemetry: per-run counters and a "--- Summary ---"
//! renderer.
//!
//! Designed for CLIs that copy/move/purge/dedupe files and want a consistent
//! end-of-run summary row. The verb (e.g. `"Copied"` vs `"Would copy"`) and
//! the noun (e.g. `"file"`, `"duplicate"`) are caller-supplied — this module
//! does not know about run-vs-dry-run mode or any binary-specific vocabulary.

use crate::{
   output::Output,
   str_utils::{format_bytes, plural},
   terminal::colors::{BOLD, GREEN, RED, RESET, YELLOW}
};

/// Whether [`FileStats::display`] renders a `bytes_freed` row (for dedupe
/// operations) or a skipped-count row (for copy/purge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowBytes {
   /// Render `<verb> <count> <noun>s, freed <bytes>, <errors>` (used by dedupe).
   Yes,
   /// Render `<verb> <count> <noun>s (<skipped>, <errors>)` (used by copy/purge).
   No
}

/// Per-run file-op counters.
#[derive(Debug, Default)]
pub struct FileStats {
   /// Files inspected during the run.
   pub files_processed: usize,
   /// Files actually copied/moved/purged/deleted.
   pub files_acted: usize,
   /// Files skipped (e.g. destination already exists).
   pub files_skipped: usize,
   /// Errors encountered.
   pub errors: usize,
   /// Bytes reclaimed (only meaningful for dedupe — see [`ShowBytes::Yes`]).
   pub bytes_freed: u64
}

impl FileStats {
   /// Add `other` into `self` field-by-field.
   pub fn merge(&mut self, other: &FileStats) {
      self.files_processed += other.files_processed;
      self.files_acted += other.files_acted;
      self.files_skipped += other.files_skipped;
      self.errors += other.errors;
      self.bytes_freed += other.bytes_freed;
   }

   /// Render a `--- Summary ---` heading followed by a single summary row.
   ///
   /// `verb` and `noun` are caller-supplied: typically `"Copied"` /
   /// `"Would copy"` paired with `"file"`, or `"Deleted"` / `"Would delete"`
   /// paired with `"duplicate"`. The noun is pluralized automatically.
   ///
   /// `show_bytes` selects between the two summary formats; see [`ShowBytes`].
   ///
   /// ANSI color codes are emitted only when `output.colors_enabled()` is
   /// true, so the same call works correctly on a TTY, in a piped log, and
   /// in tests.
   pub fn display(&self, output: &mut dyn Output, verb: &str, noun: &str, show_bytes: ShowBytes) {
      let colors = output.colors_enabled();
      output.writeln("");
      output.writeln(&heading(colors));

      let count = self.files_acted;
      let noun_suffix = plural(count);
      let errors_part = self.format_errors_part(colors);

      match show_bytes {
         ShowBytes::Yes => {
            let bytes_str = format_bytes(self.bytes_freed);
            let action = action_part(verb, count, noun, noun_suffix, colors);
            output.writeln(&format!("{action}, freed {bytes_str}, {errors_part}"));
         }
         ShowBytes::No => {
            let action = action_part(verb, count, noun, noun_suffix, colors);
            let skipped_part = self.format_skipped_part(colors);
            output.writeln(&format!("{action} ({skipped_part}, {errors_part})"));
         }
      }
   }

   fn format_errors_part(&self, colors: bool) -> String {
      let n = self.errors;
      if !colors {
         return if n > 0 { format!("{n} error{}", plural(n)) } else { "0 errors".to_string() };
      }
      if n > 0 { format!("{RED}{n} error{}{RESET}", plural(n)) } else { format!("{GREEN}0 errors{RESET}") }
   }

   fn format_skipped_part(&self, colors: bool) -> String {
      let n = self.files_skipped;
      if !colors {
         return if n > 0 { format!("{n} skipped") } else { "0 skipped".to_string() };
      }
      if n > 0 { format!("{YELLOW}{n} skipped{RESET}") } else { format!("{GREEN}0 skipped{RESET}") }
   }
}

fn heading(colors: bool) -> String {
   if colors { format!("{BOLD}--- Summary ---{RESET}") } else { "--- Summary ---".to_string() }
}

fn action_part(verb: &str, count: usize, noun: &str, noun_suffix: &str, colors: bool) -> String {
   if colors {
      format!("{GREEN}{verb} {count} {noun}{noun_suffix}{RESET}")
   } else {
      format!("{verb} {count} {noun}{noun_suffix}")
   }
}

#[cfg(test)]
mod tests {
   use super::*;
   use crate::output::StringOutput;

   #[test]
   fn merge_combines_stats() {
      let mut a = FileStats { files_processed: 5, files_acted: 3, files_skipped: 2, errors: 1, bytes_freed: 1000 };
      let b = FileStats { files_processed: 10, files_acted: 7, files_skipped: 3, errors: 0, bytes_freed: 2000 };
      a.merge(&b);
      assert_eq!(a.files_processed, 15);
      assert_eq!(a.files_acted, 10);
      assert_eq!(a.files_skipped, 5);
      assert_eq!(a.errors, 1);
      assert_eq!(a.bytes_freed, 3000);
   }

   #[test]
   fn display_real_run_shows_action_verb() {
      let stats = FileStats { files_processed: 8, files_acted: 5, files_skipped: 3, errors: 0, bytes_freed: 0 };
      let mut out = StringOutput::new();
      stats.display(&mut out, "Copied", "file", ShowBytes::No);
      let log = out.log();
      assert!(log.contains("Summary"));
      assert!(log.contains("Copied 5 files"));
      assert!(log.contains("3 skipped"));
      assert!(log.contains("0 errors"));
   }

   #[test]
   fn display_dry_run_uses_caller_supplied_verb() {
      let stats = FileStats { files_processed: 8, files_acted: 5, files_skipped: 3, errors: 0, bytes_freed: 0 };
      let mut out = StringOutput::new();
      stats.display(&mut out, "Would copy", "file", ShowBytes::No);
      assert!(out.log().contains("Would copy 5 files"));
   }

   #[test]
   fn display_show_bytes_yes_renders_bytes_row() {
      let stats = FileStats {
         files_processed: 100,
         files_acted: 12,
         files_skipped: 0,
         errors: 0,
         bytes_freed: 1_288_490_189 // 1.2 GiB
      };
      let mut out = StringOutput::new();
      stats.display(&mut out, "Deleted", "duplicate", ShowBytes::Yes);
      let log = out.log();
      assert!(log.contains("Deleted 12 duplicates"));
      assert!(log.contains("1.2 GiB"));
   }

   #[test]
   fn display_singular_noun_drops_trailing_s() {
      let stats = FileStats { files_processed: 1, files_acted: 1, files_skipped: 0, errors: 0, bytes_freed: 0 };
      let mut out = StringOutput::new();
      stats.display(&mut out, "Copied", "file", ShowBytes::No);
      assert!(out.log().contains("Copied 1 file"));
      assert!(!out.log().contains("files"));
   }

   #[test]
   fn display_errors_highlighted() {
      let stats = FileStats { files_processed: 5, files_acted: 3, files_skipped: 0, errors: 2, bytes_freed: 0 };
      let mut out = StringOutput::new();
      stats.display(&mut out, "Copied", "file", ShowBytes::No);
      assert!(out.log().contains("2 errors"));
   }

   #[test]
   fn display_emits_no_ansi_when_colors_disabled() {
      // StringOutput::colors_enabled() is false by default — the summary should
      // be plain text with no escape sequences.
      let stats = FileStats { files_processed: 5, files_acted: 3, files_skipped: 1, errors: 1, bytes_freed: 2048 };
      let mut out = StringOutput::new();
      stats.display(&mut out, "Copied", "file", ShowBytes::Yes);
      assert!(!out.log().contains('\x1b'), "expected no ANSI escapes, got: {:?}", out.log());
   }
}
