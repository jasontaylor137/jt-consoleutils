//! Path-manipulation helpers that don't touch the filesystem (or only do so
//! to canonicalize). For filesystem I/O with error contexts, see
//! [`crate::fs_utils`].

#[cfg(any(unix, windows))]
use std::ffi::OsString;
use std::{
   env,
   path::{Component, Path, PathBuf}
};

/// Return the user's home directory.
///
/// Resolution order:
/// - **Unix**: `$HOME` if set and non-empty, otherwise the `pw_dir` of the current uid via
///   `getpwuid_r`. Returns `None` only if the passwd entry is missing or has no `pw_dir`.
/// - **Windows**: `%USERPROFILE%` if set and non-empty, otherwise `%HOMEDRIVE%%HOMEPATH%` if both
///   are set and non-empty. Returns `None` if no source yields a value.
///
/// Empty environment variable values are treated as unset, matching how
/// most shells resolve `$HOME` after `unset HOME`.
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
   #[cfg(unix)]
   {
      home_dir_unix(env::var_os("HOME"))
   }
   #[cfg(windows)]
   {
      home_dir_windows(env::var_os("USERPROFILE"), env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH"))
   }
}

#[cfg(unix)]
fn home_dir_unix(home_var: Option<OsString>) -> Option<PathBuf> {
   if let Some(s) = home_var.filter(|s| !s.is_empty()) {
      return Some(PathBuf::from(s));
   }
   home_dir_from_passwd()
}

#[cfg(windows)]
fn home_dir_windows(
   userprofile: Option<OsString>,
   homedrive: Option<OsString>,
   homepath: Option<OsString>
) -> Option<PathBuf> {
   if let Some(s) = userprofile.filter(|s| !s.is_empty()) {
      return Some(PathBuf::from(s));
   }
   let drive = homedrive.filter(|s| !s.is_empty())?;
   let path = homepath.filter(|s| !s.is_empty())?;
   let mut combined = drive;
   combined.push(path);
   Some(PathBuf::from(combined))
}

/// Look up the home directory of the current uid via `getpwuid_r`.
///
/// Used as a fallback when `$HOME` is unset (CI, sudo, launchd, etc.).
#[cfg(unix)]
fn home_dir_from_passwd() -> Option<PathBuf> {
   use std::{ffi::CStr, mem::MaybeUninit, os::unix::ffi::OsStringExt, ptr};

   // SAFETY: `getuid` reads a process-wide value and has no failure modes.
   let uid = unsafe { libc::getuid() };

   // `_SC_GETPW_R_SIZE_MAX` may be -1 ("indeterminate"); use a generous
   // default in that case.
   // SAFETY: `sysconf` is safe with a known-valid name constant.
   let mut buf_size = match unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) } {
      n if n > 0 => n as usize,
      _ => 1024
   };
   const MAX_BUF: usize = 64 * 1024;

   loop {
      let mut buf = vec![0u8; buf_size];
      let mut passwd = MaybeUninit::<libc::passwd>::uninit();
      let mut result: *mut libc::passwd = ptr::null_mut();

      // SAFETY: pointers are valid for the lifetime of this call; `buf` is
      // sized correctly; `result` receives a pointer into `passwd` (or
      // null) on success.
      let rc = unsafe {
         libc::getpwuid_r(uid, passwd.as_mut_ptr(), buf.as_mut_ptr().cast::<libc::c_char>(), buf_size, &mut result)
      };

      if rc == 0 && !result.is_null() {
         // SAFETY: rc==0 with non-null result means the struct is initialized.
         let passwd = unsafe { passwd.assume_init() };
         if passwd.pw_dir.is_null() {
            return None;
         }
         // SAFETY: pw_dir points into our `buf` (still alive) and is a
         // NUL-terminated C string per POSIX.
         let dir_bytes = unsafe { CStr::from_ptr(passwd.pw_dir) }.to_bytes();
         if dir_bytes.is_empty() {
            return None;
         }
         return Some(PathBuf::from(OsString::from_vec(dir_bytes.to_vec())));
      }

      if rc == libc::ERANGE && buf_size < MAX_BUF {
         buf_size = (buf_size * 2).min(MAX_BUF);
         continue;
      }
      return None;
   }
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
pub fn parent_dir_or_dot(path: &Path) -> &Path {
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
pub fn file_name_str(path: &Path) -> Option<&str> {
   path.file_name().and_then(|f| f.to_str())
}

/// Strip the file extension for display (e.g. `"deploy.sh"` → `"deploy"`).
///
/// Thin wrapper over [`Path::file_stem`] that exists because the raw API
/// returns `Option<&OsStr>`, which most user-facing display sites would have
/// to convert anyway. This helper:
///
/// - Returns `String` (display-ready) rather than `Option<&OsStr>`.
/// - Falls back to the input `filename` when `file_stem` returns `None`
///   (e.g. paths ending in `..`) **or** when the stem isn't valid UTF-8 —
///   so callers can always render *something* without an extra match arm.
/// - Strips only the last extension (so `"archive.tar.gz"` → `"archive.tar"`).
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

   // -- parent_dir_or_dot --

   #[test]
   fn parent_dir_or_dot_returns_parent() {
      // Given
      let path = Path::new("/some/dir/script.ts");

      // When
      let dir = parent_dir_or_dot(path);

      // Then
      assert_eq!(dir, Path::new("/some/dir"));
   }

   #[test]
   fn parent_dir_or_dot_falls_back_to_dot_for_bare_filename() {
      // Given
      let path = Path::new("script.ts");

      // When
      let dir = parent_dir_or_dot(path);

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

   // -- file_name_str --

   #[test]
   fn file_name_str_returns_basename() {
      assert_eq!(file_name_str(Path::new("/a/b/deploy.sh")), Some("deploy.sh"));
   }

   #[test]
   fn file_name_str_returns_none_for_root() {
      assert_eq!(file_name_str(Path::new("/")), None);
   }

   // -- home_dir --

   #[test]
   #[cfg(unix)]
   fn home_dir_unix_uses_home_when_set_and_nonempty() {
      // Given
      let home = OsString::from("/custom/home");

      // When
      let result = home_dir_unix(Some(home));

      // Then
      assert_eq!(result, Some(PathBuf::from("/custom/home")));
   }

   #[test]
   #[cfg(unix)]
   fn home_dir_unix_falls_back_to_passwd_when_home_unset() {
      // Given — HOME is None (caller didn't pass an env value)

      // When
      let result = home_dir_unix(None);

      // Then — passwd lookup should yield a real directory in any sane test env
      assert!(result.is_some(), "passwd fallback should resolve a home dir for the current uid");
      assert!(result.unwrap().is_absolute(), "passwd home dir should be absolute");
   }

   #[test]
   #[cfg(unix)]
   fn home_dir_unix_falls_back_to_passwd_when_home_empty() {
      // Given — HOME is set but empty (matches `unset` semantics in most shells)
      let empty = OsString::new();

      // When
      let result = home_dir_unix(Some(empty));

      // Then — empty value is ignored, passwd lookup runs
      assert!(result.is_some(), "empty HOME should be treated as unset and fall through to passwd");
   }

   #[test]
   #[cfg(unix)]
   fn home_dir_from_passwd_returns_some() {
      // Given / When — direct test of the syscall path
      let result = home_dir_from_passwd();

      // Then
      assert!(result.is_some(), "current uid should have a passwd entry with pw_dir");
   }

   #[test]
   #[cfg(windows)]
   fn home_dir_windows_uses_userprofile_when_set() {
      // Given
      let profile = OsString::from(r"C:\Users\alice");

      // When
      let result = home_dir_windows(Some(profile), None, None);

      // Then
      assert_eq!(result, Some(PathBuf::from(r"C:\Users\alice")));
   }

   #[test]
   #[cfg(windows)]
   fn home_dir_windows_falls_back_to_drive_plus_path() {
      // Given — USERPROFILE unset, but HOMEDRIVE+HOMEPATH set
      let drive = OsString::from(r"C:");
      let path = OsString::from(r"\Users\bob");

      // When
      let result = home_dir_windows(None, Some(drive), Some(path));

      // Then
      assert_eq!(result, Some(PathBuf::from(r"C:\Users\bob")));
   }

   #[test]
   #[cfg(windows)]
   fn home_dir_windows_treats_empty_userprofile_as_unset() {
      // Given — empty USERPROFILE, valid HOMEDRIVE+HOMEPATH
      let drive = OsString::from(r"D:");
      let path = OsString::from(r"\home\carol");

      // When
      let result = home_dir_windows(Some(OsString::new()), Some(drive), Some(path));

      // Then
      assert_eq!(result, Some(PathBuf::from(r"D:\home\carol")));
   }

   #[test]
   #[cfg(windows)]
   fn home_dir_windows_returns_none_when_no_source_set() {
      // Given / When
      let result = home_dir_windows(None, None, None);

      // Then
      assert!(result.is_none());
   }

   #[test]
   #[cfg(windows)]
   fn home_dir_windows_returns_none_when_only_homedrive_set() {
      // Given — HOMEDRIVE without HOMEPATH is incomplete

      // When
      let result = home_dir_windows(None, Some(OsString::from(r"C:")), None);

      // Then
      assert!(result.is_none());
   }
}
