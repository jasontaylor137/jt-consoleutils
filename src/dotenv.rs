//! `.env` file loader (gated on `feature = "dotenv"`).
//!
//! Wraps `dotenvy` with a `thiserror`-based [`DotenvError`](crate::dotenv::DotenvError)
//! that includes the offending file path and line number, plus an
//! "absent file → empty result" convenience semantic.

use std::{collections::HashMap, io, io::ErrorKind, path::Path};

use dotenvy::Error as DotenvyError;

/// Errors returned by the loaders. Both variants carry the source file path so
/// callers can include it when reporting.
#[derive(Debug, thiserror::Error)]
pub enum DotenvError {
   /// Parse failure inside the `.env` file (line numbers are 1-based).
   #[error("{path}:{line}: {message}")]
   Parse {
      /// Display path of the offending file.
      path: String,
      /// 1-based line number, or 0 when the parser doesn't know.
      line: usize,
      /// Parse error description.
      message: String
   },

   /// I/O error while reading the file.
   #[error("read {path}: {source}")]
   Io {
      /// Display path of the file.
      path: String,
      /// The underlying I/O error.
      #[source]
      source: io::Error
   }
}

/// Like [`load_dotenv`] but preserves source-file order. Missing file → empty Vec.
///
/// # Errors
/// Returns [`DotenvError::Parse`] for malformed lines or [`DotenvError::Io`]
/// for other I/O failures (file-not-found is *not* an error).
pub fn load_dotenv_ordered(path: &Path) -> Result<Vec<(String, String)>, DotenvError> {
   let iter = match dotenvy::from_path_iter(path) {
      Ok(iter) => iter,
      Err(DotenvyError::Io(e)) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
      Err(e) => return Err(map_dotenvy_error(e, path))
   };

   let mut entries = Vec::new();
   for item in iter {
      let (key, value) = item.map_err(|e| map_dotenvy_error(e, path))?;
      entries.push((key, value));
   }
   Ok(entries)
}

/// Load a `.env` file into a `HashMap`. Missing file → empty map.
///
/// # Errors
/// Same as [`load_dotenv_ordered`].
pub fn load_dotenv(path: &Path) -> Result<HashMap<String, String>, DotenvError> {
   Ok(load_dotenv_ordered(path)?.into_iter().collect())
}

fn map_dotenvy_error(e: DotenvyError, path: &Path) -> DotenvError {
   let path = path.display().to_string();
   match e {
      DotenvyError::Io(io_err) => DotenvError::Io { path, source: io_err },
      DotenvyError::LineParse(line, idx) => {
         DotenvError::Parse { path, line: idx + 1, message: format!("parse error near: {line}") }
      }
      DotenvyError::EnvVar(var_err) => {
         DotenvError::Parse { path, line: 0, message: format!("variable expansion failed: {var_err}") }
      }
      // `dotenvy::Error` is `#[non_exhaustive]`; required for forward-compat.
      other => DotenvError::Parse { path, line: 0, message: other.to_string() }
   }
}

#[cfg(test)]
mod tests {
   use std::fs;

   use tempfile::TempDir;

   use super::*;

   #[test]
   fn load_from_file() {
      let dir = TempDir::new().unwrap();
      let path = dir.path().join(".env");
      fs::write(&path, "TOKEN=abc123\nMODE=prod").unwrap();

      let result = load_dotenv(&path).unwrap();

      assert_eq!(result.get("TOKEN").unwrap(), "abc123");
      assert_eq!(result.get("MODE").unwrap(), "prod");
   }

   #[test]
   fn load_returns_empty_when_file_missing() {
      let dir = TempDir::new().unwrap();
      let path = dir.path().join("does-not-exist");

      let result = load_dotenv(&path).unwrap();

      assert!(result.is_empty());
   }

   #[test]
   fn load_unquotes_double_quoted_values() {
      let dir = TempDir::new().unwrap();
      let path = dir.path().join(".env");
      fs::write(&path, "FOO=\"bar baz\"").unwrap();

      let result = load_dotenv(&path).unwrap();

      assert_eq!(result.get("FOO").unwrap(), "bar baz");
   }

   #[test]
   fn load_accepts_export_prefix() {
      let dir = TempDir::new().unwrap();
      let path = dir.path().join(".env");
      fs::write(&path, "export FOO=bar").unwrap();

      let result = load_dotenv(&path).unwrap();

      assert_eq!(result.get("FOO").unwrap(), "bar");
   }

   #[test]
   fn load_dotenv_ordered_preserves_file_order() {
      let dir = TempDir::new().unwrap();
      let path = dir.path().join(".env");
      fs::write(&path, "B=2\nA=1\nC=3").unwrap();

      let result = load_dotenv_ordered(&path).unwrap();

      let keys: Vec<&str> = result.iter().map(|(k, _)| k.as_str()).collect();
      assert_eq!(keys, vec!["B", "A", "C"]);
   }
}
