//! Low-level JSONC byte primitives shared by the comment-stripping parser
//! pre-pass and the span-aware editor. All take `&[u8]` and a start index,
//! return a new index/length, and never panic on truncated input (they clamp
//! to `bytes.len()`).

/// Given `bytes[i] == b'"'`, return the index one past the closing quote,
/// honoring `\`-escapes. On an unterminated literal, returns `bytes.len()`.
pub(crate) fn scan_string(bytes: &[u8], i: usize) -> usize {
   debug_assert_eq!(bytes[i], b'"');
   let mut j = i + 1;
   while j < bytes.len() {
      match bytes[j] {
         b'\\' => j += 2,
         b'"' => return j + 1,
         _ => j += 1
      }
   }
   bytes.len()
}

/// If a comment begins at `bytes[i]`, return its byte length (covering `//` up
/// to but excluding the newline, or `/* */` including the closing `*/`).
/// Returns 0 if no comment starts here. Unterminated block comment → to end.
pub(crate) fn comment_len(bytes: &[u8], i: usize) -> usize {
   if i + 1 >= bytes.len() || bytes[i] != b'/' {
      return 0;
   }
   match bytes[i + 1] {
      b'/' => {
         let mut j = i + 2;
         while j < bytes.len() && bytes[j] != b'\n' {
            j += 1;
         }
         j - i
      }
      b'*' => {
         let mut j = i + 2;
         while j + 1 < bytes.len() {
            if bytes[j] == b'*' && bytes[j + 1] == b'/' {
               return (j + 2) - i;
            }
            j += 1;
         }
         bytes.len() - i
      }
      _ => 0
   }
}

/// Advance past any run of ASCII whitespace and `//` / `/* */` comments.
pub(crate) fn skip_trivia(bytes: &[u8], mut i: usize) -> usize {
   loop {
      while i < bytes.len() && bytes[i].is_ascii_whitespace() {
         i += 1;
      }
      let c = comment_len(bytes, i);
      if c == 0 {
         return i;
      }
      i += c;
   }
}

#[cfg(test)]
mod tests;
