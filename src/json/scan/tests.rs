use super::*;

#[test]
fn scan_string_simple() {
   // "ab" cd  -> end is index just past the closing quote (4)
   assert_eq!(scan_string(b"\"ab\" cd", 0), 4);
}

#[test]
fn scan_string_with_escaped_quote() {
   // "a\"b"  -> 6 bytes, end past closing quote
   assert_eq!(scan_string(b"\"a\\\"b\"x", 0), 6);
}

#[test]
fn scan_string_unterminated_returns_len() {
   let b = b"\"abc";
   assert_eq!(scan_string(b, 0), b.len());
}

#[test]
fn comment_len_line() {
   assert_eq!(comment_len(b"// hi\nx", 0), 5); // up to but not including \n
}

#[test]
fn comment_len_block() {
   assert_eq!(comment_len(b"/* hi */x", 0), 8);
}

#[test]
fn comment_len_block_unterminated() {
   let b = b"/* hi";
   assert_eq!(comment_len(b, 0), b.len());
}

#[test]
fn comment_len_none() {
   assert_eq!(comment_len(b"x", 0), 0);
   assert_eq!(comment_len(b"/x", 0), 0);
}

#[test]
fn skip_trivia_ws_and_comments() {
   //   \n // c\n /* b */ X
   let b = b"  \n // c\n /* b */ X";
   let i = skip_trivia(b, 0);
   assert_eq!(b[i], b'X');
}
