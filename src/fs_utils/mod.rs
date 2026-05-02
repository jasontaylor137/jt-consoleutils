//! Filesystem helpers with rich error context.
//!
//! Provides a unified [`FsError`](crate::fs_utils::FsError) type whose
//! contextualized variants (`Read`, `Write`, `Remove`, `Chmod`, `CreateDir`,
//! `Parse`) carry the offending path so error messages name the file that
//! failed. Higher-level helpers in this module (`read_json_file`,
//! `write_if_changed`, etc.) always produce the contextualized form.
//!
//! For dry-run-aware variants of write/remove that route through the
//! [`Output`](crate::output::Output) trait, see [`dry`](crate::fs_utils::dry).

use std::{io, path::Path};

use crate::{
   json::{self, FromJsonValue, JsonError, ToJson},
   str_utils::path_to_string
};

pub mod dry;

// ---------------------------------------------------------------------------
// FsError
// ---------------------------------------------------------------------------

/// Unified filesystem-layer error. Covers raw I/O, contextualized
/// reads/writes/removes/chmods, and JSON(C) parse failures produced by the
/// JSON file-reading helpers in this module.
///
/// The contextualized variants (`Read`, `Write`, `Remove`, `Chmod`, `Parse`)
/// carry the path so error messages name the file that failed. The bare
/// `Io` and `Json` variants exist for `?`-conversion at call sites that
/// don't have a path on hand; helpers in this module always produce the
/// contextualized form.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
   /// Raw I/O error without path context.
   #[error(transparent)]
   Io(#[from] io::Error),
   /// Failed to read the file at `path`.
   #[error("read {path}: {source}")]
   Read {
      /// Path of the file whose read failed.
      path: String,
      /// Underlying I/O error.
      #[source]
      source: io::Error
   },
   /// Failed to write the file at `path`.
   #[error("write {path}: {source}")]
   Write {
      /// Path of the file whose write failed.
      path: String,
      /// Underlying I/O error.
      #[source]
      source: io::Error
   },
   /// Failed to remove the file at `path`.
   #[error("remove {path}: {source}")]
   Remove {
      /// Path of the file whose removal failed.
      path: String,
      /// Underlying I/O error.
      #[source]
      source: io::Error
   },
   /// Failed to set permissions on the file at `path`.
   #[error("chmod {path}: {source}")]
   Chmod {
      /// Path of the file whose chmod failed.
      path: String,
      /// Underlying I/O error.
      #[source]
      source: io::Error
   },
   /// Failed to create the directory at `path`.
   #[error("create dir {path}: {source}")]
   CreateDir {
      /// Path of the directory whose creation failed.
      path: String,
      /// Underlying I/O error.
      #[source]
      source: io::Error
   },
   /// Failed to parse the file at `path` as JSON(C).
   #[error("parse {path}: {source}")]
   Parse {
      /// Path of the file whose parse failed.
      path: String,
      /// Underlying JSON error.
      #[source]
      source: JsonError
   },
   /// Raw JSON error without path context.
   #[error(transparent)]
   Json(#[from] JsonError)
}

impl FsError {
   /// Construct a [`FsError::Read`] for `path`.
   pub fn read(path: &Path, source: io::Error) -> Self {
      Self::Read { path: path_to_string(path), source }
   }

   /// Construct a [`FsError::Write`] for `path`.
   pub fn write(path: &Path, source: io::Error) -> Self {
      Self::Write { path: path_to_string(path), source }
   }

   /// Construct a [`FsError::Remove`] for `path`.
   pub fn remove(path: &Path, source: io::Error) -> Self {
      Self::Remove { path: path_to_string(path), source }
   }

   /// Construct a [`FsError::Chmod`] for `path`.
   pub fn chmod(path: &Path, source: io::Error) -> Self {
      Self::Chmod { path: path_to_string(path), source }
   }

   /// Construct a [`FsError::CreateDir`] for `path`.
   pub fn create_dir(path: &Path, source: io::Error) -> Self {
      Self::CreateDir { path: path_to_string(path), source }
   }

   /// Construct a [`FsError::Parse`] for `path`.
   pub fn parse(path: &Path, source: JsonError) -> Self {
      Self::Parse { path: path_to_string(path), source }
   }
}

// ---------------------------------------------------------------------------
// File helpers
// ---------------------------------------------------------------------------

/// Set file permissions to owner-only read/write (0o600) on Unix.
/// No-op on Windows (NTFS ACLs handle this differently).
///
/// # Errors
///
/// Returns [`FsError::Chmod`] if the permissions cannot be applied.
#[cfg(unix)]
pub fn restrict_permissions(path: &Path) -> Result<(), FsError> {
   use std::os::unix::fs::PermissionsExt;
   std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| FsError::chmod(path, e))
}

/// No-op on Windows — NTFS uses ACLs rather than Unix permission bits.
///
/// # Errors
///
/// Always returns `Ok(())` on Windows.
#[cfg(not(unix))]
pub fn restrict_permissions(_path: &Path) -> Result<(), FsError> {
   Ok(())
}

/// Write `contents` to `path` only if the file doesn't exist or its content differs.
/// Returns `Ok(true)` if the file was written, `Ok(false)` if skipped.
///
/// # Errors
///
/// Returns [`FsError::Read`] if the existing file cannot be read,
/// or [`FsError::Write`] if the new contents cannot be written.
pub fn write_if_changed(path: &Path, contents: &str) -> Result<bool, FsError> {
   if path.is_file() {
      let existing = std::fs::read_to_string(path).map_err(|e| FsError::read(path, e))?;
      if existing == contents {
         return Ok(false);
      }
   }
   std::fs::write(path, contents).map_err(|e| FsError::write(path, e))?;
   Ok(true)
}

/// Read and parse a JSON file at `path` into `T`.
///
/// # Errors
///
/// Returns [`FsError::Read`] if the file cannot be read, or [`FsError::Parse`]
/// if it cannot be parsed as JSON or converted into `T`.
pub fn read_json_file<T: FromJsonValue>(path: &Path) -> Result<T, FsError> {
   let content = std::fs::read_to_string(path).map_err(|e| FsError::read(path, e))?;
   let value = json::parse_json(&content).map_err(|e| FsError::parse(path, e))?;
   let parsed = T::from_json_value(&value).map_err(|e| FsError::parse(path, e))?;
   Ok(parsed)
}

/// Read and parse a JSONC file (JSON with `//` / `/* */` comments and
/// trailing commas) at `path` into `T`.
///
/// # Errors
///
/// Returns [`FsError::Read`] if the file cannot be read, or [`FsError::Parse`]
/// if it cannot be parsed as JSONC or converted into `T`.
pub fn read_jsonc_file<T: FromJsonValue>(path: &Path) -> Result<T, FsError> {
   let content = std::fs::read_to_string(path).map_err(|e| FsError::read(path, e))?;
   let value = json::parse_jsonc(&content).map_err(|e| FsError::parse(path, e))?;
   let parsed = T::from_json_value(&value).map_err(|e| FsError::parse(path, e))?;
   Ok(parsed)
}

/// Serialize `value` as pretty JSON and write it to `path` with a trailing newline.
///
/// # Errors
///
/// Returns [`FsError::Write`] if the file cannot be written.
pub fn write_json_file_pretty<T: ToJson>(path: &Path, value: &T) -> Result<(), FsError> {
   let json = value.to_json_pretty();
   std::fs::write(path, format!("{json}\n")).map_err(|e| FsError::write(path, e))?;
   Ok(())
}

/// Return `<dir-of-current-exe>/<filename>` if such a file exists, otherwise
/// `None`. Useful for "config.json sitting next to the binary" defaults.
///
/// Returns `None` if the current executable path can't be resolved or the
/// candidate file doesn't exist. Callers typically fall back to a
/// CWD-relative path.
#[must_use]
pub fn exe_adjacent_path(filename: &str) -> Option<std::path::PathBuf> {
   let exe = std::env::current_exe().ok()?;
   let candidate = exe.with_file_name(filename);
   candidate.is_file().then_some(candidate)
}

/// Check whether two paths refer to the same file on disk.
///
/// Returns `false` if either path doesn't exist or can't be canonicalized.
#[must_use]
pub fn same_file(a: &Path, b: &Path) -> bool {
   match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
      (Ok(ca), Ok(cb)) => ca == cb,
      _ => false
   }
}

/// Check whether two files have identical content.
///
/// Returns `false` if either file doesn't exist or can't be read, or if their
/// sizes differ (avoids reading content when lengths don't match).
#[must_use]
pub fn same_content(a: &Path, b: &Path) -> bool {
   let (Ok(meta_a), Ok(meta_b)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
      return false;
   };
   if meta_a.len() != meta_b.len() {
      return false;
   }
   let (Ok(bytes_a), Ok(bytes_b)) = (std::fs::read(a), std::fs::read(b)) else {
      return false;
   };
   bytes_a == bytes_b
}

/// Sets executable permission bits (+x) on Unix; no-op on Windows.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if the file metadata cannot be read or the
/// permissions cannot be applied.
pub fn make_executable(path: &Path) -> std::io::Result<()> {
   #[cfg(unix)]
   {
      use std::os::unix::fs::PermissionsExt;
      let mut perms = std::fs::metadata(path)?.permissions();
      perms.set_mode(perms.mode() | 0o111);
      std::fs::set_permissions(path, perms)?;
   }
   #[cfg(not(unix))]
   {
      let _ = path;
   }
   Ok(())
}

/// Remove a path that is a symlink.
///
/// Handles the platform difference between symlinks to directories
/// (Windows: `remove_dir`) and symlinks to files (Unix: `remove_file`).
/// Returns `Ok(false)` if `path` is not a symlink.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if the symlink exists but cannot be removed.
pub fn remove_symlink_dir_like(path: &Path) -> Result<bool, std::io::Error> {
   if !path.is_symlink() {
      return Ok(false);
   }

   #[cfg(windows)]
   std::fs::remove_dir(path)?;

   #[cfg(unix)]
   std::fs::remove_file(path)?;

   Ok(true)
}

#[cfg(test)]
mod tests {
   use std::fs;

   use tempfile::TempDir;

   use super::*;

   // -------------------------------------------------------------------------
   // same_file
   // -------------------------------------------------------------------------

   #[test]
   fn same_file_returns_true_for_identical_paths() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("file.txt");
      fs::write(&path, b"hello").unwrap();

      // When / Then
      assert!(same_file(&path, &path));
   }

   #[test]
   fn same_file_returns_false_for_different_files() {
      // Given
      let dir = TempDir::new().unwrap();
      let a = dir.path().join("a.txt");
      let b = dir.path().join("b.txt");
      fs::write(&a, b"hello").unwrap();
      fs::write(&b, b"hello").unwrap();

      // When / Then
      assert!(!same_file(&a, &b));
   }

   #[test]
   fn same_file_returns_false_when_path_does_not_exist() {
      // Given
      let dir = TempDir::new().unwrap();
      let missing = dir.path().join("missing.txt");
      let existing = dir.path().join("existing.txt");
      fs::write(&existing, b"hi").unwrap();

      // When / Then
      assert!(!same_file(&missing, &existing));
   }

   // -------------------------------------------------------------------------
   // same_content
   // -------------------------------------------------------------------------

   #[test]
   fn same_content_returns_true_for_identical_bytes() {
      // Given
      let dir = TempDir::new().unwrap();
      let a = dir.path().join("a.txt");
      let b = dir.path().join("b.txt");
      fs::write(&a, b"hello world").unwrap();
      fs::write(&b, b"hello world").unwrap();

      // When / Then
      assert!(same_content(&a, &b));
   }

   #[test]
   fn same_content_returns_false_for_different_bytes() {
      // Given
      let dir = TempDir::new().unwrap();
      let a = dir.path().join("a.txt");
      let b = dir.path().join("b.txt");
      fs::write(&a, b"hello").unwrap();
      fs::write(&b, b"world").unwrap();

      // When / Then
      assert!(!same_content(&a, &b));
   }

   #[test]
   fn same_content_returns_false_for_different_sizes() {
      // Given
      let dir = TempDir::new().unwrap();
      let a = dir.path().join("a.txt");
      let b = dir.path().join("b.txt");
      fs::write(&a, b"hi").unwrap();
      fs::write(&b, b"hello").unwrap();

      // When / Then
      assert!(!same_content(&a, &b));
   }

   #[test]
   fn same_content_returns_false_when_file_missing() {
      // Given
      let dir = TempDir::new().unwrap();
      let a = dir.path().join("a.txt");
      let missing = dir.path().join("missing.txt");
      fs::write(&a, b"data").unwrap();

      // When / Then
      assert!(!same_content(&a, &missing));
   }

   // -------------------------------------------------------------------------
   // make_executable
   // -------------------------------------------------------------------------

   #[test]
   fn make_executable_does_not_error_on_existing_file() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("script.sh");
      fs::write(&path, b"#!/bin/sh\n").unwrap();

      // When / Then
      assert!(make_executable(&path).is_ok());
   }

   #[cfg(unix)]
   #[test]
   fn make_executable_sets_exec_bit_on_unix() {
      // Given
      use std::os::unix::fs::PermissionsExt;
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("script.sh");
      fs::write(&path, b"#!/bin/sh\n").unwrap();

      // Ensure no exec bit initially
      let mut perms = fs::metadata(&path).unwrap().permissions();
      perms.set_mode(0o600);
      fs::set_permissions(&path, perms).unwrap();

      // When
      make_executable(&path).unwrap();

      // Then
      let mode = fs::metadata(&path).unwrap().permissions().mode();
      assert!(mode & 0o111 != 0, "exec bit should be set");
   }

   // -------------------------------------------------------------------------
   // remove_symlink_dir_like
   // -------------------------------------------------------------------------

   #[cfg(unix)]
   #[test]
   fn remove_symlink_dir_like_removes_symlink_on_unix() {
      // Given
      let dir = TempDir::new().unwrap();
      let target = dir.path().join("target.txt");
      let link = dir.path().join("link");
      fs::write(&target, b"data").unwrap();
      std::os::unix::fs::symlink(&target, &link).unwrap();
      assert!(link.is_symlink());

      // When
      let result = remove_symlink_dir_like(&link).unwrap();

      // Then
      assert!(result);
      assert!(!link.exists() && !link.is_symlink());
   }

   #[test]
   fn remove_symlink_dir_like_returns_false_for_non_symlink() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("plain.txt");
      fs::write(&path, b"data").unwrap();

      // When
      let result = remove_symlink_dir_like(&path).unwrap();

      // Then
      assert!(!result);
   }

   // -------------------------------------------------------------------------
   // FsError + write_if_changed / read_json_file / write_json_file_pretty /
   // restrict_permissions
   // -------------------------------------------------------------------------

   use crate::json::{FromJsonValue, JsonError, JsonValue, ToJson};

   /// Minimal `ToJson`/`FromJsonValue` impl for the JSON helpers' tests.
   #[derive(Debug, PartialEq)]
   struct Doc(String);

   impl ToJson for Doc {
      fn to_json_pretty(&self) -> String {
         format!("{{\n  \"k\": \"{}\"\n}}", self.0)
      }
   }

   impl FromJsonValue for Doc {
      fn from_json_value(value: &JsonValue) -> Result<Self, JsonError> {
         let map = value.as_object().ok_or_else(|| JsonError::value("expected object".to_string()))?;
         let v = map.get("k").and_then(|v| v.as_str()).ok_or_else(|| JsonError::value("missing 'k'".to_string()))?;
         Ok(Doc(v.to_string()))
      }
   }

   // -- write_if_changed --

   #[test]
   fn write_if_changed_creates_new_file() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("test.json");

      // When
      let written = write_if_changed(&path, "content").unwrap();

      // Then
      assert!(written);
      assert_eq!(fs::read_to_string(&path).unwrap(), "content");
   }

   #[test]
   fn write_if_changed_skips_identical() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("test.json");
      fs::write(&path, "content").unwrap();

      // When
      let written = write_if_changed(&path, "content").unwrap();

      // Then
      assert!(!written);
   }

   #[test]
   fn write_if_changed_overwrites_different() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("test.json");
      fs::write(&path, "old content").unwrap();

      // When
      let written = write_if_changed(&path, "new content").unwrap();

      // Then
      assert!(written);
      assert_eq!(fs::read_to_string(&path).unwrap(), "new content");
   }

   #[test]
   fn write_if_changed_returns_write_error_with_path_when_dir_missing() {
      // Given — parent directory does not exist
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("missing-subdir").join("test.json");

      // When
      let err = write_if_changed(&path, "content").unwrap_err();

      // Then — Write variant carries the path
      let FsError::Write { path: p, .. } = err else {
         panic!("expected FsError::Write, got {err:?}");
      };
      assert!(p.contains("test.json"), "expected path in error, got: {p}");
   }

   // -- read_json_file --

   #[test]
   fn read_json_file_parses_valid_json() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("data.json");
      fs::write(&path, r#"{"k":"value"}"#).unwrap();

      // When
      let doc: Doc = read_json_file(&path).unwrap();

      // Then
      assert_eq!(doc, Doc("value".to_string()));
   }

   #[test]
   fn read_json_file_returns_read_error_with_path_when_missing() {
      // Given — no file
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("missing.json");

      // When
      let err: FsError = read_json_file::<Doc>(&path).unwrap_err();

      // Then — Read variant carries the path
      let FsError::Read { path: p, .. } = err else {
         panic!("expected FsError::Read, got {err:?}");
      };
      assert!(p.contains("missing.json"), "expected path in error, got: {p}");
   }

   #[test]
   fn read_json_file_returns_parse_error_with_path_on_invalid_json() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("bad.json");
      fs::write(&path, "not valid json\n").unwrap();

      // When
      let err: FsError = read_json_file::<Doc>(&path).unwrap_err();

      // Then — Parse variant carries the path
      let FsError::Parse { path: p, .. } = err else {
         panic!("expected FsError::Parse, got {err:?}");
      };
      assert!(p.contains("bad.json"), "expected path in error, got: {p}");
   }

   // -- read_jsonc_file --

   #[test]
   fn read_jsonc_file_parses_valid_jsonc() {
      // Given — JSONC with line comment, block comment, and trailing comma
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("data.jsonc");
      fs::write(
         &path,
         r#"{
            // line comment
            /* block comment */
            "k": "value",
         }"#
      )
      .unwrap();

      // When
      let doc: Doc = read_jsonc_file(&path).unwrap();

      // Then
      assert_eq!(doc, Doc("value".to_string()));
   }

   #[test]
   fn read_jsonc_file_parses_strict_json_too() {
      // Given — JSONC parser must accept ordinary JSON
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("data.json");
      fs::write(&path, r#"{"k":"value"}"#).unwrap();

      // When
      let doc: Doc = read_jsonc_file(&path).unwrap();

      // Then
      assert_eq!(doc, Doc("value".to_string()));
   }

   #[test]
   fn read_jsonc_file_returns_read_error_with_path_when_missing() {
      // Given — no file
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("missing.jsonc");

      // When
      let err: FsError = read_jsonc_file::<Doc>(&path).unwrap_err();

      // Then — Read variant carries the path
      let FsError::Read { path: p, .. } = err else {
         panic!("expected FsError::Read, got {err:?}");
      };
      assert!(p.contains("missing.jsonc"), "expected path in error, got: {p}");
   }

   #[test]
   fn read_jsonc_file_returns_parse_error_with_path_on_invalid_jsonc() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("bad.jsonc");
      fs::write(&path, "not valid {{{\n").unwrap();

      // When
      let err: FsError = read_jsonc_file::<Doc>(&path).unwrap_err();

      // Then — Parse variant carries the path
      let FsError::Parse { path: p, .. } = err else {
         panic!("expected FsError::Parse, got {err:?}");
      };
      assert!(p.contains("bad.jsonc"), "expected path in error, got: {p}");
   }

   // -- write_json_file_pretty --

   #[test]
   fn write_json_file_pretty_writes_pretty_json_with_trailing_newline() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("out.json");
      let value = Doc("v".to_string());

      // When
      write_json_file_pretty(&path, &value).unwrap();

      // Then — file ends with newline and contains pretty output
      let content = fs::read_to_string(&path).unwrap();
      assert!(content.ends_with('\n'));
      assert_eq!(content.trim_end(), value.to_json_pretty());
   }

   #[test]
   fn write_json_file_pretty_returns_write_error_with_path_when_dir_missing() {
      // Given — parent dir does not exist
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("missing-subdir").join("out.json");
      let value = Doc("v".to_string());

      // When
      let err = write_json_file_pretty(&path, &value).unwrap_err();

      // Then
      let FsError::Write { path: p, .. } = err else {
         panic!("expected FsError::Write, got {err:?}");
      };
      assert!(p.contains("out.json"), "expected path in error, got: {p}");
   }

   // -- restrict_permissions --

   #[cfg(unix)]
   #[test]
   fn restrict_permissions_sets_owner_only() {
      use std::os::unix::fs::PermissionsExt;

      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("secret.txt");
      fs::write(&path, "token=abc").unwrap();

      // When
      restrict_permissions(&path).unwrap();

      // Then
      let perms = fs::metadata(&path).unwrap().permissions();
      assert_eq!(perms.mode() & 0o777, 0o600);
   }

   #[cfg(unix)]
   #[test]
   fn restrict_permissions_returns_chmod_error_with_path_when_missing() {
      // Given — file does not exist
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("nope.txt");

      // When
      let err = restrict_permissions(&path).unwrap_err();

      // Then
      let FsError::Chmod { path: p, .. } = err else {
         panic!("expected FsError::Chmod, got {err:?}");
      };
      assert!(p.contains("nope.txt"), "expected path in error, got: {p}");
   }

   // -- exe_adjacent_path --

   #[test]
   fn exe_adjacent_path_returns_none_when_file_missing() {
      // Given — a filename that won't exist next to the test binary
      let result = exe_adjacent_path("nonexistent-xyz-config.json");

      // Then
      assert!(result.is_none());
   }

   #[test]
   fn exe_adjacent_path_returns_path_when_file_exists() {
      // Given — drop a file next to the current test exe
      let exe = std::env::current_exe().unwrap();
      let candidate = exe.with_file_name("exe-adjacent-test-marker.tmp");
      fs::write(&candidate, b"").unwrap();

      // When
      let result = exe_adjacent_path("exe-adjacent-test-marker.tmp");

      // Then
      assert_eq!(result.as_deref(), Some(candidate.as_path()));

      // Cleanup
      let _ = fs::remove_file(&candidate);
   }
}
