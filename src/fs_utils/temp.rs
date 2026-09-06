//! Collision-safe temporary files and atomic file replacement.

use std::{
   fs::{self, File, OpenOptions},
   io::{self, ErrorKind, Write},
   path::{Path, PathBuf},
   sync::atomic::{AtomicUsize, Ordering}
};

/// How many names to try before giving up on finding an unused one.
const NAME_ATTEMPTS: u32 = 16;

/// A uniquely-named file that is deleted when the value is dropped.
///
/// Created exclusively — an existing name is never opened or truncated — and,
/// on Unix, with mode `0600`, so the staged copy is never world-readable even
/// transiently. The file handle is closed before the constructor returns, so
/// another process (an `$EDITOR`, say) can take the file over.
///
/// Dropping removes the file; [`persist`](TempFile::persist) instead renames it
/// over a destination, which is how [`atomic_write`] replaces a file without
/// ever exposing a truncated one.
#[derive(Debug)]
pub struct TempFile {
   path: PathBuf,
   persisted: bool
}

impl TempFile {
   /// Create `<dir>/<prefix><unique><suffix>` holding `contents`.
   ///
   /// `dir` must exist. Stage files for an atomic replacement belong in the
   /// destination's own directory, since a rename is only atomic within one
   /// filesystem.
   ///
   /// The unique middle section is derived from
   /// [`RandomState`](std::hash::RandomState)'s OS-seeded keys. It exists to
   /// avoid collisions, not to be unguessable — exclusive creation is what
   /// makes this safe against a hostile directory, and a name already taken is
   /// simply retried.
   ///
   /// # Errors
   ///
   /// Returns the underlying [`io::Error`] if the file cannot be created or
   /// written. A partially written file is removed rather than left behind.
   pub fn create_in(dir: &Path, prefix: &str, suffix: &str, contents: &[u8]) -> io::Result<Self> {
      let mut collision = None;
      for _ in 0..NAME_ATTEMPTS {
         let path = dir.join(format!("{prefix}{}{suffix}", unique_token()));
         match open_exclusive(&path) {
            Ok(file) => {
               // Constructed before the write so an error mid-write unwinds
               // through `Drop` and removes the partial file. `write_and_close`
               // consumes the handle, so it is already closed by then —
               // Windows refuses to remove a file that is still open.
               let temp = Self { path, persisted: false };
               write_and_close(file, contents)?;
               return Ok(temp);
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => collision = Some(e),
            Err(e) => return Err(e)
         }
      }
      Err(collision.unwrap_or_else(|| io::Error::new(ErrorKind::AlreadyExists, "no unused temporary file name")))
   }

   /// The path of the temporary file.
   #[must_use]
   pub fn path(&self) -> &Path {
      &self.path
   }

   /// Rename the temporary file over `dest`, replacing it.
   ///
   /// Atomic on POSIX and on Windows (`MoveFileExW` with
   /// `MOVEFILE_REPLACE_EXISTING`) as long as both paths are on one
   /// filesystem: a reader sees either the old file or the new one, never a
   /// truncated write. Nothing is fsynced, so this orders writes against
   /// readers and against a dying process — not against a power loss.
   ///
   /// # Errors
   ///
   /// Returns the underlying [`io::Error`] if the rename fails — because the
   /// two paths are on different filesystems, or because Windows still has
   /// `dest` locked after the retries below. The temporary file is removed
   /// either way.
   pub fn persist(mut self, dest: &Path) -> io::Result<()> {
      rename_with_retry(&self.path, dest)?;
      self.persisted = true;
      Ok(())
   }
}

impl Drop for TempFile {
   fn drop(&mut self) {
      if !self.persisted {
         let _ = fs::remove_file(&self.path);
      }
   }
}

/// Whether an atomic rewrite carried the original file's permissions over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermsCopy {
   /// The original's mode was applied to the replacement — or there was no
   /// original file to inherit from.
   Preserved,
   /// The rewrite succeeded but the mode could not be copied, so the file may
   /// now be more permissive than it was. Worth warning the user about when
   /// the file holds credentials.
   Failed
}

/// Write `contents` to `path` atomically, replacing any existing file.
///
/// See [`atomic_write_keep_perms`], which this wraps, for the mechanics. Use
/// this form for files whose permissions don't need carrying over — typically
/// ones this process created itself.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] if the stage file cannot be written or
/// the rename fails.
pub fn atomic_write(path: &Path, contents: &str) -> io::Result<()> {
   atomic_write_keep_perms(path, contents).map(|_| ())
}

/// Write `contents` to `path` atomically, carrying the existing file's
/// permissions over to the replacement.
///
/// Stages a sibling temporary file in `path`'s own directory, then renames it
/// over the destination, so a concurrent reader — or a process that dies
/// mid-write — sees either the original or the finished file, never a
/// truncated one. Nothing is fsynced, so a power loss can still lose the tail
/// of a write. The caller is responsible for creating `path.parent()` if it
/// does not exist.
///
/// When `path` already exists its permission bits are copied onto the stage
/// file *before* the rename, so a rewrite never widens the mode — silently
/// opening up a credentials file would be a security regression. The stage
/// file is created `0600` on Unix, so it is never world-readable even in the
/// window before that copy. [`PermsCopy::Failed`] reports the rare case where
/// the rewrite succeeded but the mode could not be carried over.
///
/// The stage file is named `.<file_name>.tmp.<unique>`: hidden, and
/// recognizable to a caller that sweeps up files orphaned by a hard crash.
///
/// A symlinked `path` is written *through* rather than replaced: the rename
/// targets the link's destination, so a config file linked into a dotfiles
/// repo keeps its link.
///
/// The rename replaces the destination whatever its own permissions say — it
/// needs write access to the containing directory, not to the file — so a
/// read-only destination is overwritten rather than refused.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] if a symlinked `path` cannot be
/// resolved, the stage file cannot be written, or the rename fails.
pub fn atomic_write_keep_perms(path: &Path, contents: &str) -> io::Result<PermsCopy> {
   let resolved = resolve_symlink(path)?;
   let path = resolved.as_deref().unwrap_or(path);
   let dir = path.parent().unwrap_or_else(|| Path::new("."));
   let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp");
   let temp = TempFile::create_in(dir, &stage_prefix(file_name), "", contents.as_bytes())?;
   // A failed `set_permissions` is rare — a filesystem without chmod, or a
   // metadata race — and must not fail the write, only be reported.
   let perms = match fs::metadata(path) {
      Ok(meta) if fs::set_permissions(temp.path(), meta.permissions()).is_err() => PermsCopy::Failed,
      // No original file means nothing to inherit.
      _ => PermsCopy::Preserved
   };
   temp.persist(path)?;
   Ok(perms)
}

/// The stage-file name prefix for a rewrite of `file_name`: hidden, and
/// carrying a `.tmp.` marker so a consumer can recognize — and sweep up —
/// stage files orphaned by a hard crash.
fn stage_prefix(file_name: &str) -> String {
   format!(".{file_name}.tmp.")
}

/// The file a symlink points at, or `None` when `path` is not a symlink (or
/// dangles). Renaming over a symlink would replace the link itself, so an
/// atomic rewrite has to target what it points at.
fn resolve_symlink(path: &Path) -> io::Result<Option<PathBuf>> {
   if !path.is_symlink() {
      return Ok(None);
   }
   match fs::canonicalize(path) {
      Ok(target) => Ok(Some(target)),
      // A dangling link has no target to write through, so the replacement
      // takes the link's own place.
      Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
      // Anything else — on Windows, a target held open elsewhere — must not
      // quietly downgrade to replacing the link the caller wanted kept.
      Err(e) => Err(e)
   }
}

/// Create `path` only if it does not already exist, `0600` on Unix.
///
/// Windows gets no equivalent: NTFS inherits the containing directory's ACL,
/// which for a per-user directory is already owner-only. `FILE_ATTRIBUTE_-
/// TEMPORARY` is deliberately not set either — it would survive
/// [`persist`](TempFile::persist)'s rename and mark the *destination* as
/// temporary, telling the cache manager to delay flushing a file that is meant
/// to last.
fn open_exclusive(path: &Path) -> io::Result<File> {
   let mut opts = OpenOptions::new();
   opts.write(true).create_new(true);
   #[cfg(unix)]
   {
      use std::os::unix::fs::OpenOptionsExt;
      opts.mode(0o600);
   }
   opts.open(path)
}

/// Rename `from` over `to`, retrying briefly while Windows says the
/// destination is locked.
///
/// Windows refuses a rename while another process holds the destination open
/// without `FILE_SHARE_DELETE` — an antivirus scanner, a search indexer, or a
/// language server that grabbed a transient handle on the file just written.
/// Those handles clear in milliseconds, so a short backoff turns the classic
/// atomic-write flake into a pause. POSIX renames over an open file happily,
/// so there is nothing to retry there.
#[cfg(windows)]
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
   /// `ERROR_ACCESS_DENIED`.
   const ACCESS_DENIED: i32 = 5;
   /// `ERROR_SHARING_VIOLATION`.
   const SHARING_VIOLATION: i32 = 32;
   const BACKOFF_MS: [u64; 3] = [20, 50, 100];

   for delay in BACKOFF_MS {
      match fs::rename(from, to) {
         Err(e) if matches!(e.raw_os_error(), Some(ACCESS_DENIED | SHARING_VIOLATION)) => {
            std::thread::sleep(std::time::Duration::from_millis(delay));
         }
         result => return result
      }
   }
   fs::rename(from, to)
}

/// Rename `from` over `to`. See the Windows arm for why that one retries.
#[cfg(not(windows))]
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
   fs::rename(from, to)
}

/// Write `contents` and close the handle, whether or not the write succeeds.
fn write_and_close(mut file: File, contents: &[u8]) -> io::Result<()> {
   file.write_all(contents)?;
   file.flush()
}

/// A short base-36 token, distinct on every call within a process and
/// unrelated to the one another process would produce.
fn unique_token() -> String {
   use std::hash::{BuildHasher, Hasher, RandomState};

   static COUNTER: AtomicUsize = AtomicUsize::new(0);
   let mut hasher = RandomState::new().build_hasher();
   hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));

   let mut n = hasher.finish();
   let mut token = String::with_capacity(13);
   loop {
      let digit = (n % 36) as u32;
      token.push(char::from_digit(digit, 36).unwrap_or('0'));
      n /= 36;
      if n == 0 {
         return token;
      }
   }
}

#[cfg(test)]
mod tests {
   use tempfile::TempDir;

   use super::*;

   #[test]
   fn create_in_writes_contents_under_prefix_and_suffix() {
      // Given
      let dir = TempDir::new().unwrap();

      // When
      let temp = TempFile::create_in(dir.path(), "stage-", ".txt", b"body").unwrap();

      // Then
      let name = temp.path().file_name().unwrap().to_str().unwrap();
      assert!(name.starts_with("stage-") && name.ends_with(".txt"), "unexpected name: {name}");
      assert_eq!(fs::read_to_string(temp.path()).unwrap(), "body");
   }

   #[test]
   fn create_in_gives_each_file_a_distinct_name() {
      // Given
      let dir = TempDir::new().unwrap();

      // When — two live temp files in one directory
      let first = TempFile::create_in(dir.path(), "t", "", b"a").unwrap();
      let second = TempFile::create_in(dir.path(), "t", "", b"b").unwrap();

      // Then
      assert_ne!(first.path(), second.path());
   }

   #[test]
   fn create_in_fails_when_directory_is_missing() {
      // Given
      let dir = TempDir::new().unwrap();

      // When
      let result = TempFile::create_in(&dir.path().join("absent"), "t", "", b"x");

      // Then — the real cause surfaces rather than a name-exhaustion error
      assert_eq!(result.unwrap_err().kind(), ErrorKind::NotFound);
   }

   #[test]
   fn dropping_a_temp_file_removes_it() {
      // Given
      let dir = TempDir::new().unwrap();
      let temp = TempFile::create_in(dir.path(), "t", "", b"x").unwrap();
      let path = temp.path().to_path_buf();

      // When
      drop(temp);

      // Then
      assert!(!path.exists());
   }

   #[test]
   fn persist_renames_over_the_destination() {
      // Given an existing destination
      let dir = TempDir::new().unwrap();
      let dest = dir.path().join("dest.txt");
      fs::write(&dest, "old").unwrap();
      let temp = TempFile::create_in(dir.path(), "t", "", b"new").unwrap();
      let staged = temp.path().to_path_buf();

      // When
      temp.persist(&dest).unwrap();

      // Then — destination replaced, nothing staged left behind
      assert_eq!(fs::read_to_string(&dest).unwrap(), "new");
      assert!(!staged.exists());
   }

   #[cfg(unix)]
   #[test]
   fn create_in_restricts_the_staged_file_to_the_owner() {
      use std::os::unix::fs::PermissionsExt;

      // Given
      let dir = TempDir::new().unwrap();

      // When
      let temp = TempFile::create_in(dir.path(), "t", "", b"secret").unwrap();

      // Then — never world-readable, even before a permission copy
      let mode = fs::metadata(temp.path()).unwrap().permissions().mode() & 0o777;
      assert_eq!(mode, 0o600);
   }

   #[test]
   fn atomic_write_replaces_existing_contents() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("data.txt");
      fs::write(&path, "old").unwrap();

      // When
      atomic_write(&path, "new").unwrap();

      // Then
      assert_eq!(fs::read_to_string(&path).unwrap(), "new");
   }

   #[test]
   fn atomic_write_creates_a_missing_file_and_leaves_no_stage_file() {
      // Given
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("fresh.txt");

      // When
      let perms = atomic_write_keep_perms(&path, "body").unwrap();

      // Then — nothing to inherit from counts as preserved
      assert_eq!(perms, PermsCopy::Preserved);
      assert_eq!(fs::read_to_string(&path).unwrap(), "body");
      assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1, "a stage file was left behind");
   }

   #[test]
   fn stage_file_name_is_hidden_and_marked_for_orphan_sweeps() {
      // Given a rewrite of a named file
      let dir = TempDir::new().unwrap();

      // When the stage file is created the way `atomic_write_keep_perms` does
      let temp = TempFile::create_in(dir.path(), &stage_prefix("package.json"), "", b"{}").unwrap();

      // Then its name stays sweepable: a crash between staging and rename
      // leaves a file a consumer can still recognize as junk.
      let name = temp.path().file_name().unwrap().to_str().unwrap();
      assert!(name.starts_with('.'), "stage files must be hidden, got {name}");
      assert!(name.contains(".tmp."), "stage files must carry the .tmp. marker, got {name}");
   }

   #[test]
   fn atomic_write_fails_when_the_parent_directory_is_missing() {
      // Given
      let dir = TempDir::new().unwrap();

      // When
      let result = atomic_write(&dir.path().join("absent").join("data.txt"), "body");

      // Then
      assert_eq!(result.unwrap_err().kind(), ErrorKind::NotFound);
   }

   #[cfg(unix)]
   #[test]
   fn atomic_write_keep_perms_does_not_widen_the_mode() {
      use std::os::unix::fs::PermissionsExt;

      // Given a credentials file restricted to its owner
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("secret.env");
      fs::write(&path, "TOKEN=old").unwrap();
      fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

      // When
      let perms = atomic_write_keep_perms(&path, "TOKEN=new").unwrap();

      // Then
      assert_eq!(perms, PermsCopy::Preserved);
      assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
   }

   #[cfg(unix)]
   #[test]
   fn atomic_write_replaces_a_dangling_symlink() {
      // Given a link to a file that no longer exists
      let dir = TempDir::new().unwrap();
      let link = dir.path().join("settings.json");
      std::os::unix::fs::symlink(dir.path().join("gone.json"), &link).unwrap();

      // When
      atomic_write(&link, "new").unwrap();

      // Then — with nothing to write through to, the link's own place is taken
      assert!(!link.is_symlink());
      assert_eq!(fs::read_to_string(&link).unwrap(), "new");
   }

   #[cfg(unix)]
   #[test]
   fn atomic_write_writes_through_a_symlink_instead_of_replacing_it() {
      // Given a link standing in for a file kept in a dotfiles repo
      let dir = TempDir::new().unwrap();
      let target = dir.path().join("real.json");
      let link = dir.path().join("settings.json");
      fs::write(&target, "old").unwrap();
      std::os::unix::fs::symlink(&target, &link).unwrap();

      // When
      atomic_write(&link, "new").unwrap();

      // Then — the link survives and its target holds the new contents
      assert!(link.is_symlink(), "the symlink was replaced by a regular file");
      assert_eq!(fs::read_to_string(&target).unwrap(), "new");
   }
}
