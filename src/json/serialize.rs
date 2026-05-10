//! Pretty-printed JSON serialization for [`JsonValue`].
//!
//! Output is strict JSON (RFC 8259) — never JSONC. Objects are emitted with
//! keys in sorted order (a consequence of the `BTreeMap` backing store), one
//! key/value per line, indented with 2 spaces per nesting level. Empty objects
//! and arrays render as `{}` / `[]` on a single line.
//!
//! Strings are escaped via the standard JSON set (`\"`, `\\`, `\n`, `\r`,
//! `\t`, `\b`, `\f`, and `\u00XX` for other control characters). Non-ASCII
//! characters are emitted as raw UTF-8 rather than `\uXXXX` escapes.

use super::{escape::push_json_string, to_json::push_indent, value::JsonValue};

/// Serialize a `JsonValue` to a pretty-printed JSON string with 2-space indent.
pub fn to_json_pretty(value: &JsonValue) -> String {
   let mut out = String::new();
   write_value(&mut out, value, 0);
   out
}

fn write_value(out: &mut String, value: &JsonValue, indent: usize) {
   match value {
      JsonValue::Object(map) => {
         if map.is_empty() {
            out.push_str("{}");
            return;
         }
         out.push_str("{\n");
         let inner = indent + 1;
         let mut first = true;
         for (key, val) in map {
            if !first {
               out.push_str(",\n");
            }
            first = false;
            push_indent(out, inner);
            push_json_string(out, key);
            out.push_str(": ");
            write_value(out, val, inner);
         }
         out.push('\n');
         push_indent(out, indent);
         out.push('}');
      }
      JsonValue::Array(arr) => {
         if arr.is_empty() {
            out.push_str("[]");
            return;
         }
         out.push_str("[\n");
         let inner = indent + 1;
         let mut first = true;
         for val in arr {
            if !first {
               out.push_str(",\n");
            }
            first = false;
            push_indent(out, inner);
            write_value(out, val, inner);
         }
         out.push('\n');
         push_indent(out, indent);
         out.push(']');
      }
      JsonValue::String(s) => push_json_string(out, s),
      JsonValue::Number(s) => out.push_str(s),
      JsonValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
      JsonValue::Null => out.push_str("null")
   }
}

#[cfg(test)]
mod tests;
