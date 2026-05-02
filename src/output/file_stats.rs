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
   pub fn display(&self, output: &mut dyn Output, verb: &str, noun: &str, show_bytes: ShowBytes) {
      output.writeln("");
      output.writeln(&format!("{BOLD}--- Summary ---{RESET}"));

      let count = self.files_acted;
      let noun_suffix = plural(count);
      let errors_part = self.format_errors_part();

      match show_bytes {
         ShowBytes::Yes => {
            let bytes_str = format_bytes(self.bytes_freed);
            output.writeln(&format!(
               "{GREEN}{verb} {count} {noun}{noun_suffix}{RESET}, freed {bytes_str}, {errors_part}"
            ));
         }
         ShowBytes::No => {
            let skipped_part = self.format_skipped_part();
            output.writeln(&format!(
               "{GREEN}{verb} {count} {noun}{noun_suffix}{RESET} ({skipped_part}, {errors_part})"
            ));
         }
      }
   }

   fn format_errors_part(&self) -> String {
      if self.errors > 0 {
         format!("{RED}{} error{}{RESET}", self.errors, plural(self.errors))
      } else {
         format!("{GREEN}0 errors{RESET}")
      }
   }

   fn format_skipped_part(&self) -> String {
      if self.files_skipped > 0 {
         format!("{YELLOW}{} skipped{RESET}", self.files_skipped)
      } else {
         format!("{GREEN}0 skipped{RESET}")
      }
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
         bytes_freed: 1_288_490_189 // ~1.2 GB
      };
      let mut out = StringOutput::new();
      stats.display(&mut out, "Deleted", "duplicate", ShowBytes::Yes);
      let log = out.log();
      assert!(log.contains("Deleted 12 duplicates"));
      assert!(log.contains("1.2 GB"));
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
}
