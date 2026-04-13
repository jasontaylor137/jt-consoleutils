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
            let n = c as u32;
            out.push_str(&format!("\\u{n:04x}"));
         }
         c => out.push(c)
      }
   }
   out.push('"');
}
