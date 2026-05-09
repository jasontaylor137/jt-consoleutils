//! Path-manipulation helpers that don't touch the filesystem (or only do so
//! to canonicalize). For filesystem I/O with error contexts, see
//! [`crate::fs_utils`].

use std::{
   env,
   path::{Component, Path, PathBuf}
};

/// Return the user's home directory, or `None` if the relevant environment
/// variable (`HOME` on Unix, `USERPROFILE` on Windows) is not set.
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
   #[cfg(unix)]
   return env::var("HOME").ok().map(PathBuf::from);
   #[cfg(windows)]
   return env::var("USERPROFILE").ok().map(PathBuf::from);
}

/// Normalize a path by resolving `.` and `..` segments without touching the
/// filesystem (unlike `fs::canonicalize`, which requires the path to exist
/// and resolves symlinks).
#[must_use]
pub fn normalize_path(path: &Path) -> PathBuf {
   let mut out = PathBuf::new();
   for component in path.components() {
      match component {
         Component::CurDir => {} // skip `.`
         Component::ParentDir => {
            out.pop();
         }
         other => out.push(other)
      }
   }
   out
}

/// Canonicalize a path that is known to exist, falling back to the input
/// on failure (e.g. permission errors on Windows). Resolves symlinks so
/// distinct paths to the same physical file produce the same result.
#[must_use]
pub fn canonicalize_existing(path: &Path) -> PathBuf {
   match std::fs::canonicalize(path) {
      Ok(c) => strip_unc_prefix(c),
      Err(_) => path.to_path_buf()
   }
}

/// On Windows, `fs::canonicalize` returns extended-length `\\?\C:\...` paths.
/// Strip the prefix when present so display strings and hash inputs stay
/// consistent with the absolute paths users actually type.
#[cfg(windows)]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
   const UNC: &str = r"\\?\";
   let s = path.to_string_lossy();
   if let Some(rest) = s.strip_prefix(UNC) {
      return PathBuf::from(rest.to_string());
   }
   path
}

#[cfg(not(windows))]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
   path
}

/// Return the directory containing `path`, falling back to `"."`.
///
/// Unlike `path.parent().unwrap_or(Path::new("."))`, this also normalizes
/// the empty-string parent that `Path::new("bare_file.ts").parent()`
/// returns.
#[must_use]
pub fn script_dir(path: &Path) -> &Path {
   match path.parent() {
      Some(p) if !p.as_os_str().is_empty() => p,
      _ => Path::new(".")
   }
}

/// Extract the filename from a path. Returns `None` for paths with no
/// final component (e.g. `/`) or whose filename isn't valid UTF-8.
///
/// Callers decide their own fallback — this helper stays neutral about
/// what a "default filename" should look like.
#[must_use]
pub fn script_filename(path: &Path) -> Option<&str> {
   path.file_name().and_then(|f| f.to_str())
}

/// Strip the file extension for display (e.g. `"deploy.sh"` → `"deploy"`).
/// Uses `Path::file_stem`, which strips only the last extension.
#[must_use]
pub fn strip_extension(filename: &str) -> String {
   Path::new(filename).file_stem().and_then(|s| s.to_str()).unwrap_or(filename).to_string()
}

/// Check whether `dir` is present in the process `PATH` environment variable.
///
/// Both sides are canonicalized before comparison so symlinks and `..`
/// segments don't cause false negatives. Paths that don't exist on disk fall
/// back to lexical absolute-path normalization. On Windows the comparison is
/// case-insensitive.
#[must_use]
pub fn is_dir_on_path(dir: &Path) -> bool {
   let Ok(path_var) = env::var("PATH") else {
      return false;
   };
   is_dir_in_path_var(dir, &path_var)
}

/// Check whether `dir` is present in the given `PATH` string.
///
/// Separated from [`is_dir_on_path`] so tests can pass an explicit `PATH`
/// value without mutating the process environment. See [`is_dir_on_path`]
/// for canonicalization semantics.
#[must_use]
pub fn is_dir_in_path_var(dir: &Path, path_var: &str) -> bool {
   let canon_dir = canonicalize_for_path_compare(dir);
   for entry in env::split_paths(path_var) {
      if entry.as_os_str().is_empty() {
         continue;
      }
      let canon_entry = canonicalize_for_path_compare(&entry);
      if paths_equal(&canon_dir, &canon_entry) {
         return true;
      }
   }
   false
}

/// Resolve `path` for PATH-membership comparison: prefer real-path
/// canonicalization (resolves symlinks) so two PATH entries pointing at the
/// same physical directory compare equal. Fall back to lexical absolute-path
/// normalization for paths that don't exist on disk.
fn canonicalize_for_path_compare(path: &Path) -> PathBuf {
   if let Ok(canon) = std::fs::canonicalize(path) {
      return strip_unc_prefix(canon);
   }
   std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(windows)]
fn paths_equal(a: &Path, b: &Path) -> bool {
   a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

#[cfg(unix)]
fn paths_equal(a: &Path, b: &Path) -> bool {
   a == b
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
   use tempfile::TempDir;

   use super::*;

   // -- script_dir --

   #[test]
   fn script_dir_returns_parent() {
      // Given
      let path = Path::new("/some/dir/script.ts");

      // When
      let dir = script_dir(path);

      // Then
      assert_eq!(dir, Path::new("/some/dir"));
   }

   #[test]
   fn script_dir_falls_back_to_dot_for_bare_filename() {
      // Given
      let path = Path::new("script.ts");

      // When
      let dir = script_dir(path);

      // Then
      assert_eq!(dir, Path::new("."));
   }

   // -- is_dir_in_path_var / is_dir_on_path --

   #[test]
   fn is_dir_in_path_var_returns_true_when_dir_is_in_path() {
      // Given
      let tmp = TempDir::new().unwrap();
      let dir_str = tmp.path().to_string_lossy().into_owned();
      let sep = if cfg!(windows) { ";" } else { ":" };
      let path_var = format!("{dir_str}{sep}/usr/bin");

      // When — pass the PATH value directly; no env mutation needed
      let result = is_dir_in_path_var(tmp.path(), &path_var);

      // Then
      assert!(result);
   }

   #[test]
   #[cfg(unix)]
   fn is_dir_in_path_var_resolves_symlinks() {
      // Given — a real directory and a symlink pointing at it; PATH contains
      // only the symlink, but we query for the real dir.
      let real = TempDir::new().unwrap();
      let link_parent = TempDir::new().unwrap();
      let link = link_parent.path().join("link");
      std::os::unix::fs::symlink(real.path(), &link).unwrap();
      let path_var = link.to_string_lossy().into_owned();

      // When
      let result = is_dir_in_path_var(real.path(), &path_var);

      // Then — canonicalization resolves the symlink, so they compare equal.
      assert!(result, "expected symlinked PATH entry to match its real-path target");
   }

   #[test]
   fn is_dir_in_path_var_returns_false_when_dir_not_in_path() {
      // Given
      let tmp = TempDir::new().unwrap();
      let dummy = TempDir::new().unwrap();
      let dummy_str = dummy.path().to_string_lossy().into_owned();

      // When — PATH contains only the dummy dir, not tmp; no env mutation needed
      let result = is_dir_in_path_var(tmp.path(), &dummy_str);

      // Then
      assert!(!result);
   }

   // -- strip_extension --

   #[test]
   fn strip_extension_removes_sh() {
      assert_eq!(strip_extension("deploy.sh"), "deploy");
   }

   #[test]
   fn strip_extension_strips_last_extension_only() {
      assert_eq!(strip_extension("archive.tar.gz"), "archive.tar");
   }

   #[test]
   fn strip_extension_no_extension_returns_input() {
      assert_eq!(strip_extension("deploy"), "deploy");
   }

   // -- normalize_path --

   #[test]
   fn normalize_path_resolves_dot_and_dotdot() {
      // Given / When
      let result = normalize_path(Path::new("/a/b/./c/../d"));

      // Then
      assert_eq!(result, PathBuf::from("/a/b/d"));
   }

   #[test]
   fn normalize_path_does_not_touch_filesystem() {
      // Given — path that doesn't exist
      let path = Path::new("/nonexistent/path/./that/../never/existed");

      // When
      let result = normalize_path(path);

      // Then — succeeds without I/O, resolves segments
      assert_eq!(result, PathBuf::from("/nonexistent/path/never/existed"));
   }

   // -- script_filename --

   #[test]
   fn script_filename_returns_basename() {
      assert_eq!(script_filename(Path::new("/a/b/deploy.sh")), Some("deploy.sh"));
   }

   #[test]
   fn script_filename_returns_none_for_root() {
      assert_eq!(script_filename(Path::new("/")), None);
   }
}
