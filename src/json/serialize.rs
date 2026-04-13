use super::escape::push_json_string;
use super::value::JsonValue;

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
      JsonValue::Number(n) => write_number(out, *n),
      JsonValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
      JsonValue::Null => out.push_str("null")
   }
}

/// Write an f64 as a JSON number without pulling in the ~3 KB
/// `core::fmt::float` Display machinery.  Integers are printed without a
/// decimal point; non-finite values become `null` (JSON has no Inf/NaN);
/// other values are decomposed via integer arithmetic to avoid
/// `<f64 as Display>::fmt`.
fn write_number(out: &mut String, n: f64) {
   // Integers → no decimal point (matches serde_json)
   if n.fract() == 0.0 && n.is_finite() && n.abs() < (i64::MAX as f64) {
      out.push_str(&(n as i64).to_string());
      return;
   }
   if !n.is_finite() {
      out.push_str("null");
      return;
   }

   if n < 0.0 {
      out.push('-');
      write_positive_float(out, -n);
   } else {
      write_positive_float(out, n);
   }
}

/// Format a positive, finite f64 using integer arithmetic.
/// Produces up to 15 significant fractional digits (f64 precision limit).
fn write_positive_float(out: &mut String, n: f64) {
   let int_part = n.trunc() as u64;
   out.push_str(&int_part.to_string());
   out.push('.');

   let mut frac = n - (int_part as f64);
   // Emit up to 15 fractional digits, stopping at trailing zeros
   let mut buf = [0u8; 15];
   let mut len = 0;
   for slot in &mut buf {
      frac *= 10.0;
      let digit = frac.trunc() as u8;
      *slot = digit;
      frac -= digit as f64;
      len += 1;
      if frac.abs() < 1e-15 {
         break;
      }
   }
   // Trim trailing zeros but keep at least one digit
   while len > 1 && buf[len - 1] == 0 {
      len -= 1;
   }
   for &d in &buf[..len] {
      out.push((b'0' + d) as char);
   }
}

fn push_indent(out: &mut String, level: usize) {
   for _ in 0..level {
      out.push_str("  ");
   }
}

#[cfg(test)]
mod tests;
