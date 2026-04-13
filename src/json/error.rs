/// JSON parse/conversion error with optional position information.
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
      msg: String,
   },

   /// Semantic error during value extraction or deserialization.
   #[error("{0}")]
   Value(String),

   /// I/O error (e.g. reading a JSON file from disk).
   #[error("{0}")]
   Io(#[from] std::io::Error)
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
