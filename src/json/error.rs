/// JSON parse/conversion error with optional position information.
///
/// Pure JSON-layer errors — no I/O. File-reading helpers in
/// [`crate::fs_utils`] wrap this in `FsError::Parse` when they fail to
/// parse a file, keeping the I/O context (`io::Error` + path) and the JSON
/// context separate.
#[derive(Debug, thiserror::Error)]
pub enum JsonError {
   /// Syntax error at a specific line and column.
   #[error("line {line}, column {col}: {msg}")]
   Parse {
      /// 1-based line number.
      line: usize,
      /// 1-based column number.
      col: usize,
      /// Human-readable error description.
      msg: String
   },

   /// Semantic error during value extraction or deserialization.
   #[error("{0}")]
   Value(String)
}

impl JsonError {
   /// Create a [`JsonError::Parse`] with position information.
   pub fn parse(line: usize, col: usize, msg: impl Into<String>) -> Self {
      JsonError::Parse { line, col, msg: msg.into() }
   }

   /// Create a [`JsonError::Value`] error.
   pub fn value(msg: impl Into<String>) -> Self {
      JsonError::Value(msg.into())
   }
}
