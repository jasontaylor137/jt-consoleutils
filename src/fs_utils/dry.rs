//! Dry-run-aware file write and remove helpers.
//!
//! These compose [`Output::dry_run_write`] / [`Output::dry_run_delete`] with
//! `std::fs::write` / `remove_file` and the idempotent
//! [`write_if_changed`]: in dry-run mode they log intent and return without
//! touching the filesystem; in normal mode they perform the operation and
//! (with the `verbose` feature) emit a verbose message naming the file. They
//! are the file-op counterpart of [`DryRunShell`].
//!
//! [`Output::dry_run_write`]: crate::output::Output::dry_run_write
//! [`Output::dry_run_delete`]: crate::output::Output::dry_run_delete
//! [`write_if_changed`]: super::write_if_changed
//! [`DryRunShell`]: crate::shell::DryRunShell

use std::path::Path;

use super::{FsError, write_if_changed};
use crate::{
   output::{Output, OutputMode},
   str_utils::path_to_string
};

/// Write `contents` to `path`, or log a dry-run message if in dry-run mode.
///
/// In dry-run mode: calls `output.dry_run_write` and returns `Ok(())` without
/// touching the filesystem.
/// In normal mode: writes the file and (with the `verbose` feature) emits a
/// verbose message naming the file.
///
/// # Errors
///
/// Returns [`FsError::Write`] if the write fails in normal mode.
pub fn dry_write(path: &Path, contents: &str, output: &mut dyn Output, mode: OutputMode) -> Result<(), FsError> {
   if mode.is_dry_run() {
      output.dry_run_write(&path_to_string(path));
      return Ok(());
   }
   std::fs::write(path, contents).map_err(|e| FsError::write(path, e))?;
   crate::verbose!(output, "Wrote {}", file_label(path));
   Ok(())
}

/// Idempotent variant of [`dry_write`]: skips the write when `path` already
/// contains `contents`. Used for generated config files (e.g. `package.json`,
/// `pyproject.toml`) where rewriting an unchanged file would needlessly bump
/// mtime and trigger downstream tools.
///
/// Returns `Ok(true)` when the file was actually written, `Ok(false)` when
/// skipped (either dry-run or unchanged content).
///
/// # Errors
///
/// Returns [`FsError::Read`] if the existing file cannot be read or
/// [`FsError::Write`] if the new contents cannot be written.
pub fn dry_write_if_changed(
   path: &Path,
   contents: &str,
   output: &mut dyn Output,
   mode: OutputMode
) -> Result<bool, FsError> {
   if mode.is_dry_run() {
      output.dry_run_write(&path_to_string(path));
      return Ok(false);
   }
   let changed = write_if_changed(path, contents)?;
   if changed {
      crate::verbose!(output, "Wrote {}", file_label(path));
   }
   Ok(changed)
}

/// Remove the file at `path`, or log a dry-run message if in dry-run mode.
///
/// In dry-run mode: calls `output.dry_run_delete` and returns `Ok(())` without
/// touching the filesystem.
/// In normal mode: removes the file and (with the `verbose` feature) emits a
/// verbose message naming the file.
///
/// Callers are responsible for checking existence before calling — in normal
/// mode this propagates `NotFound`; in dry-run mode it would log a misleading
/// "delete" for a nonexistent file.
///
/// # Errors
///
/// Returns [`FsError::Remove`] if the removal fails in normal mode.
pub fn dry_remove_file(path: &Path, output: &mut dyn Output, mode: OutputMode) -> Result<(), FsError> {
   if mode.is_dry_run() {
      output.dry_run_delete(&path_to_string(path));
      return Ok(());
   }
   std::fs::remove_file(path).map_err(|e| FsError::remove(path, e))?;
   crate::verbose!(output, "Removed {}", file_label(path));
   Ok(())
}

#[cfg(feature = "verbose")]
fn file_label(path: &Path) -> &str {
   path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
   use std::fs;

   use tempfile::tempdir;

   use super::*;
   #[cfg(feature = "verbose")]
   use crate::output::LogLevel;
   use crate::output::{OutputMode, StringOutput};

   // -----------------------------------------------------------------------
   // dry_write
   // -----------------------------------------------------------------------

   #[test]
   fn dry_write_in_dry_run_mode_does_not_write_file() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("output.txt");
      let mut output = StringOutput::new();
      let mode = OutputMode { dry_run: true, ..Default::default() };

      // When
      dry_write(&path, "hello", &mut output, mode).unwrap();

      // Then — file was not created
      assert!(!path.exists());
      // And dry-run message was emitted
      assert!(output.log().contains("[dry-run]"));
   }

   #[test]
   fn dry_write_in_dry_run_mode_calls_dry_run_write_on_output() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("output.txt");
      let mut output = StringOutput::new();
      let mode = OutputMode { dry_run: true, ..Default::default() };

      // When
      dry_write(&path, "hello", &mut output, mode).unwrap();

      // Then — output contains the path
      let log = output.log();
      assert!(log.contains(path.to_str().unwrap()), "expected path in output, got: {log}");
   }

   #[test]
   fn dry_write_in_normal_mode_writes_file_contents() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("output.txt");
      let mut output = StringOutput::new();
      let mode = OutputMode::default();

      // When
      dry_write(&path, "hello world", &mut output, mode).unwrap();

      // Then — file was written with correct contents
      let contents = fs::read_to_string(&path).unwrap();
      assert_eq!(contents, "hello world");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn dry_write_in_normal_mode_emits_verbose_message_with_filename() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("my-label.txt");
      let mut output = StringOutput::new();
      let mode = OutputMode { level: LogLevel::Verbose, ..Default::default() };

      // When
      dry_write(&path, "content", &mut output, mode).unwrap();

      // Then — verbose message contains the filename
      assert!(output.log().contains("my-label.txt"), "expected filename in output, got: {}", output.log());
   }

   #[test]
   fn dry_write_returns_fs_error_with_path_context_on_failure() {
      // Given — write to a path whose parent directory does not exist
      let dir = tempdir().unwrap();
      let path = dir.path().join("missing-subdir").join("output.txt");
      let mut output = StringOutput::new();

      // When
      let err = dry_write(&path, "hello", &mut output, OutputMode::default()).unwrap_err();

      // Then — FsError::Write carries the path
      let FsError::Write { path: path_str, .. } = err else {
         panic!("expected FsError::Write, got {err:?}");
      };
      assert!(path_str.contains("output.txt"), "expected path in error, got: {path_str}");
   }

   // -----------------------------------------------------------------------
   // dry_write_if_changed
   // -----------------------------------------------------------------------

   #[test]
   fn dry_write_if_changed_in_dry_run_returns_false_and_skips_write() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("output.txt");
      let mut output = StringOutput::new();
      let mode = OutputMode { dry_run: true, ..Default::default() };

      // When
      let changed = dry_write_if_changed(&path, "hello", &mut output, mode).unwrap();

      // Then
      assert!(!changed);
      assert!(!path.exists());
      assert!(output.log().contains("[dry-run]"));
   }

   #[test]
   fn dry_write_if_changed_writes_when_file_missing() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("output.txt");
      let mut output = StringOutput::new();

      // When
      let changed = dry_write_if_changed(&path, "hello", &mut output, OutputMode::default()).unwrap();

      // Then
      assert!(changed);
      assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
   }

   #[test]
   fn dry_write_if_changed_writes_when_content_differs() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("output.txt");
      fs::write(&path, "old").unwrap();
      let mut output = StringOutput::new();

      // When
      let changed = dry_write_if_changed(&path, "new", &mut output, OutputMode::default()).unwrap();

      // Then
      assert!(changed);
      assert_eq!(fs::read_to_string(&path).unwrap(), "new");
   }

   #[test]
   fn dry_write_if_changed_skips_when_content_matches() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("output.txt");
      fs::write(&path, "same").unwrap();
      let mtime_before = fs::metadata(&path).unwrap().modified().unwrap();
      let mut output = StringOutput::new();

      // When
      let changed = dry_write_if_changed(&path, "same", &mut output, OutputMode::default()).unwrap();

      // Then — reports unchanged and does not bump mtime
      assert!(!changed);
      let mtime_after = fs::metadata(&path).unwrap().modified().unwrap();
      assert_eq!(mtime_before, mtime_after);
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn dry_write_if_changed_emits_verbose_message_only_when_changed() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("config.json");
      let mut output = StringOutput::new();
      let mode = OutputMode { level: LogLevel::Verbose, ..Default::default() };

      // When — first write changes; second write is a no-op
      dry_write_if_changed(&path, "{}", &mut output, mode).unwrap();
      let after_first = output.log().to_string();
      dry_write_if_changed(&path, "{}", &mut output, mode).unwrap();
      let after_second = output.log().to_string();

      // Then — first write logged the filename; second write added nothing
      assert!(after_first.contains("config.json"), "expected filename in log, got: {after_first}");
      assert_eq!(after_first, after_second, "second (unchanged) write should not log");
   }

   // -----------------------------------------------------------------------
   // dry_remove_file
   // -----------------------------------------------------------------------

   #[test]
   fn dry_remove_file_in_dry_run_mode_does_not_remove_file() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("to-remove.txt");
      fs::write(&path, "data").unwrap();
      let mut output = StringOutput::new();
      let mode = OutputMode { dry_run: true, ..Default::default() };

      // When
      dry_remove_file(&path, &mut output, mode).unwrap();

      // Then — file still exists
      assert!(path.exists());
      // And dry-run message was emitted
      assert!(output.log().contains("[dry-run]"));
   }

   #[test]
   fn dry_remove_file_in_dry_run_mode_calls_dry_run_delete_on_output() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("to-remove.txt");
      fs::write(&path, "data").unwrap();
      let mut output = StringOutput::new();
      let mode = OutputMode { dry_run: true, ..Default::default() };

      // When
      dry_remove_file(&path, &mut output, mode).unwrap();

      // Then — output contains the path
      let log = output.log();
      assert!(log.contains(path.to_str().unwrap()), "expected path in output, got: {log}");
   }

   #[test]
   fn dry_remove_file_in_normal_mode_removes_file() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("to-remove.txt");
      fs::write(&path, "data").unwrap();
      let mut output = StringOutput::new();

      // When
      dry_remove_file(&path, &mut output, OutputMode::default()).unwrap();

      // Then — file was removed
      assert!(!path.exists());
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn dry_remove_file_in_normal_mode_emits_verbose_message_with_filename() {
      // Given
      let dir = tempdir().unwrap();
      let path = dir.path().join("my-label.txt");
      fs::write(&path, "data").unwrap();
      let mut output = StringOutput::new();
      let mode = OutputMode { level: LogLevel::Verbose, ..Default::default() };

      // When
      dry_remove_file(&path, &mut output, mode).unwrap();

      // Then — verbose message contains the filename
      assert!(output.log().contains("my-label.txt"), "expected filename in output, got: {}", output.log());
   }

   #[test]
   fn dry_remove_file_in_normal_mode_nonexistent_file_returns_error_with_path() {
      // Given — path that does not exist
      let dir = tempdir().unwrap();
      let path = dir.path().join("nonexistent.txt");
      let mut output = StringOutput::new();

      // When
      let err = dry_remove_file(&path, &mut output, OutputMode::default()).unwrap_err();

      // Then — FsError::Remove carries the path
      let FsError::Remove { path: path_str, .. } = err else {
         panic!("expected FsError::Remove, got {err:?}");
      };
      assert!(path_str.contains("nonexistent.txt"), "expected path in error, got: {path_str}");
   }
}
