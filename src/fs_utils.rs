use std::path::Path;

/// Check whether two paths refer to the same file on disk.
/// Returns false if either path doesn't exist or can't be canonicalized.
pub fn same_file(a: &Path, b: &Path) -> bool {
   match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
      (Ok(ca), Ok(cb)) => ca == cb,
      _ => false
   }
}

/// Check whether two files have identical content.
/// Returns false if either file doesn't exist or can't be read, or if their
/// sizes differ (avoids reading content when lengths don't match).
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

/// Remove a path that is a symlink, handling the platform difference between
/// symlinks to directories (Windows: `remove_dir`) and symlinks to files
/// (Unix: `remove_file`). Returns `Ok(false)` if `path` is not a symlink.
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
}
