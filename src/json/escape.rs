//! Shared JSON string-escape helper used by both the [`super::serialize`] and
//! [`super::to_json`] code paths. See [`super::serialize`] for the dialect
//! the output conforms to.

/// Append a JSON-escaped, double-quoted string to `out`.
pub(crate) fn push_json_string(out: &mut String, s: &str) {
   out.push('"');
   for ch in s.chars() {
      match ch {
         '"' => out.push_str("\\\""),
         '\\' => out.push_str("\\\\"),
         '\n' => out.push_str("\\n"),
         '\r' => out.push_str("\\r"),
         '\t' => out.push_str("\\t"),
         '\u{08}' => out.push_str("\\b"),
         '\u{0C}' => out.push_str("\\f"),
         c if c < '\u{20}' => {
            // Control chars are < U+0020, so the high byte is always `00`.
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let n = c as usize;
            out.push_str("\\u00");
            out.push(HEX[n >> 4] as char);
            out.push(HEX[n & 0xf] as char);
         }
         c => out.push(c)
      }
   }
   out.push('"');
}
