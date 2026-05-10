//! JSON and JSONC parsing into [`JsonValue`].
//!
//! [`parse_json`] accepts strict JSON per RFC 8259 with one extension: raw
//! UTF-8 bytes are accepted inside strings (in addition to `\uXXXX` escapes).
//!
//! [`parse_jsonc`] accepts a small JSONC superset on top of strict JSON:
//!
//! - **Line comments** — `// ...` to end of line.
//! - **Block comments** — `/* ... */`, non-nesting.
//! - **Trailing commas** — a single trailing `,` is allowed before `}` or `]`. (Multiple
//!   consecutive trailing commas are not recognized.)
//!
//! Comment markers inside string literals are preserved verbatim. JSONC is
//! handled by stripping these features in a pre-pass and then running the
//! strict-JSON parser.
//!
//! Features intentionally **not** supported (parsers will error):
//!
//! - Single-quoted strings.
//! - Unquoted object keys.
//! - Hex, octal, leading-`+`, or `Infinity` / `NaN` numbers.
//! - Multi-line string continuations.
//! - JSON5 escape sequences beyond the standard JSON set (`\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`,
//!   `\t`, `\uXXXX`).

use std::collections::BTreeMap;

use super::{error::JsonError, value::JsonValue};

/// Parse a JSON string into a `JsonValue`.
pub fn parse_json(input: &str) -> Result<JsonValue, JsonError> {
   let mut p = Parser::new(input);
   let value = p.parse_value()?;
   p.skip_ws();
   if p.pos < p.bytes.len() {
      return Err(p.err("unexpected trailing content"));
   }
   Ok(value)
}

/// Parse a JSONC string (with `//`, `/* */` comments and trailing commas)
/// into a `JsonValue`.
pub fn parse_jsonc(input: &str) -> Result<JsonValue, JsonError> {
   let stripped = strip_jsonc(input);
   parse_json(&stripped)
}

// ---------------------------------------------------------------------------
// JSONC comment/trailing-comma stripping (ported from src/jsonc/mod.rs)
// ---------------------------------------------------------------------------

pub(crate) fn strip_jsonc(input: &str) -> String {
   let mut out = String::with_capacity(input.len());
   let bytes = input.as_bytes();
   let len = bytes.len();
   let mut i = 0;

   while i < len {
      let b = bytes[i];

      // strings: copy verbatim, including escapes
      if b == b'"' {
         out.push('"');
         i += 1;
         while i < len {
            let c = bytes[i];
            out.push(c as char);
            i += 1;
            if c == b'\\' && i < len {
               out.push(bytes[i] as char);
               i += 1;
            } else if c == b'"' {
               break;
            }
         }
         continue;
      }

      // line comment
      if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
         i += 2;
         while i < len && bytes[i] != b'\n' {
            i += 1;
         }
         continue;
      }

      // block comment
      if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
         i += 2;
         while i + 1 < len {
            if bytes[i] == b'*' && bytes[i + 1] == b'/' {
               i += 2;
               break;
            }
            i += 1;
         }
         continue;
      }

      // trailing comma
      if b == b',' {
         let mut j = i + 1;
         while j < len && bytes[j].is_ascii_whitespace() {
            j += 1;
         }
         if j < len && (bytes[j] == b'}' || bytes[j] == b']') {
            i += 1;
            continue;
         }
      }

      out.push(b as char);
      i += 1;
   }

   out
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
   bytes: &'a [u8],
   pos: usize,
   /// Line number (1-based).
   line: usize,
   /// Column number (1-based).
   col: usize
}

impl<'a> Parser<'a> {
   fn new(input: &'a str) -> Self {
      Parser { bytes: input.as_bytes(), pos: 0, line: 1, col: 1 }
   }

   fn err(&self, msg: impl Into<String>) -> JsonError {
      JsonError::parse(self.line, self.col, msg)
   }

   fn peek(&self) -> Option<u8> {
      self.bytes.get(self.pos).copied()
   }

   fn advance(&mut self) {
      if self.pos < self.bytes.len() {
         if self.bytes[self.pos] == b'\n' {
            self.line += 1;
            self.col = 1;
         } else {
            self.col += 1;
         }
         self.pos += 1;
      }
   }

   fn next_byte(&mut self) -> Option<u8> {
      let b = self.peek()?;
      self.advance();
      Some(b)
   }

   fn expect(&mut self, expected: u8) -> Result<(), JsonError> {
      match self.next_byte() {
         Some(b) if b == expected => Ok(()),
         Some(b) => Err(self.err(format!("expected '{}', found '{}'", expected as char, b as char))),
         None => Err(self.err(format!("expected '{}', found end of input", expected as char)))
      }
   }

   fn skip_ws(&mut self) {
      while let Some(b) = self.peek() {
         if b.is_ascii_whitespace() {
            self.advance();
         } else {
            break;
         }
      }
   }

   // -- value parsing --------------------------------------------------------

   fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
      self.skip_ws();
      match self.peek() {
         Some(b'"') => self.parse_string().map(JsonValue::String),
         Some(b'{') => self.parse_object(),
         Some(b'[') => self.parse_array(),
         Some(b't') => self.parse_literal(b"true", JsonValue::Bool(true)),
         Some(b'f') => self.parse_literal(b"false", JsonValue::Bool(false)),
         Some(b'n') => self.parse_literal(b"null", JsonValue::Null),
         Some(b) if b == b'-' || b.is_ascii_digit() => self.parse_number(),
         Some(b) => Err(self.err(format!("unexpected character '{}'", b as char))),
         None => Err(self.err("unexpected end of input"))
      }
   }

   fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
      self.advance(); // skip '{'
      self.skip_ws();

      let mut map = BTreeMap::new();

      if self.peek() == Some(b'}') {
         self.advance();
         return Ok(JsonValue::Object(map));
      }

      loop {
         self.skip_ws();
         let key = self.parse_string()?;
         self.skip_ws();
         self.expect(b':')?;
         let val = self.parse_value()?;
         map.insert(key, val);

         self.skip_ws();
         match self.peek() {
            Some(b',') => {
               self.advance();
            }
            Some(b'}') => {
               self.advance();
               return Ok(JsonValue::Object(map));
            }
            Some(b) => return Err(self.err(format!("expected ',' or '}}', found '{}'", b as char))),
            None => return Err(self.err("unexpected end of input in object"))
         }
      }
   }

   fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
      self.advance(); // skip '['
      self.skip_ws();

      let mut arr = Vec::new();

      if self.peek() == Some(b']') {
         self.advance();
         return Ok(JsonValue::Array(arr));
      }

      loop {
         let val = self.parse_value()?;
         arr.push(val);

         self.skip_ws();
         match self.peek() {
            Some(b',') => {
               self.advance();
            }
            Some(b']') => {
               self.advance();
               return Ok(JsonValue::Array(arr));
            }
            Some(b) => return Err(self.err(format!("expected ',' or ']', found '{}'", b as char))),
            None => return Err(self.err("unexpected end of input in array"))
         }
      }
   }

   fn parse_string(&mut self) -> Result<String, JsonError> {
      self.skip_ws();
      self.expect(b'"')?;

      let mut s = String::new();
      loop {
         match self.next_byte() {
            None => return Err(self.err("unterminated string")),
            Some(b'"') => return Ok(s),
            Some(b'\\') => {
               let esc = self.next_byte().ok_or_else(|| self.err("unterminated escape"))?;
               match esc {
                  b'"' => s.push('"'),
                  b'\\' => s.push('\\'),
                  b'/' => s.push('/'),
                  b'b' => s.push('\u{08}'),
                  b'f' => s.push('\u{0C}'),
                  b'n' => s.push('\n'),
                  b'r' => s.push('\r'),
                  b't' => s.push('\t'),
                  b'u' => {
                     let cp = self.parse_hex4()?;
                     // Handle UTF-16 surrogate pairs
                     if (0xD800..=0xDBFF).contains(&cp) {
                        self.expect(b'\\')?;
                        self.expect(b'u')?;
                        let low = self.parse_hex4()?;
                        if !(0xDC00..=0xDFFF).contains(&low) {
                           return Err(self.err("invalid UTF-16 surrogate pair"));
                        }
                        let combined = 0x10000 + ((cp as u32 - 0xD800) << 10) + (low as u32 - 0xDC00);
                        s.push(char::from_u32(combined).ok_or_else(|| self.err("invalid code point"))?);
                     } else {
                        s.push(char::from_u32(cp as u32).ok_or_else(|| self.err("invalid code point"))?);
                     }
                  }
                  _ => return Err(self.err(format!("invalid escape '\\{}'", esc as char)))
               }
            }
            Some(b) => {
               // Accept raw UTF-8 bytes
               if b < 0x80 {
                  s.push(b as char);
               } else {
                  // Rewind and decode UTF-8 from the source
                  self.pos -= 1;
                  self.col -= 1;
                  let remaining = &self.bytes[self.pos..];
                  let rest = std::str::from_utf8(remaining).map_err(|_| self.err("invalid UTF-8 in string"))?;
                  let ch = rest.chars().next().unwrap();
                  s.push(ch);
                  let char_len = ch.len_utf8();
                  for _ in 0..char_len {
                     self.advance();
                  }
               }
            }
         }
      }
   }

   fn parse_hex4(&mut self) -> Result<u16, JsonError> {
      let mut val: u16 = 0;
      for _ in 0..4 {
         let b = self.next_byte().ok_or_else(|| self.err("incomplete \\u escape"))?;
         let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => 10 + b - b'a',
            b'A'..=b'F' => 10 + b - b'A',
            _ => return Err(self.err(format!("invalid hex digit '{}'", b as char)))
         };
         val = val * 16 + digit as u16;
      }
      Ok(val)
   }

   fn parse_number(&mut self) -> Result<JsonValue, JsonError> {
      let start = self.pos;

      // optional minus
      if self.peek() == Some(b'-') {
         self.advance();
      }

      // integer part
      match self.peek() {
         Some(b'0') => {
            self.advance();
         }
         Some(b) if b.is_ascii_digit() => {
            while let Some(b) = self.peek() {
               if b.is_ascii_digit() {
                  self.advance();
               } else {
                  break;
               }
            }
         }
         _ => return Err(self.err("invalid number"))
      }

      // fractional part
      if self.peek() == Some(b'.') {
         self.advance();
         if !matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
            return Err(self.err("expected digit after decimal point"));
         }
         while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
               self.advance();
            } else {
               break;
            }
         }
      }

      // exponent
      if matches!(self.peek(), Some(b'e') | Some(b'E')) {
         self.advance();
         if matches!(self.peek(), Some(b'+') | Some(b'-')) {
            self.advance();
         }
         if !matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
            return Err(self.err("expected digit in exponent"));
         }
         while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
               self.advance();
            } else {
               break;
            }
         }
      }

      let num_str = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap();
      Ok(JsonValue::Number(num_str.to_string()))
   }

   fn parse_literal(&mut self, expected: &[u8], value: JsonValue) -> Result<JsonValue, JsonError> {
      for &b in expected {
         match self.next_byte() {
            Some(got) if got == b => {}
            _ => return Err(self.err(format!("expected '{}'", std::str::from_utf8(expected).unwrap())))
         }
      }
      Ok(value)
   }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
