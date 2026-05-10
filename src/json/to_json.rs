use super::escape::push_json_string;

/// Trait for types that can serialize themselves to a pretty JSON string.
/// This is the typed counterpart to `serialize::to_json_pretty` which operates
/// on `JsonValue`. Implementing `ToJson` allows structs to skip the intermediate
/// `JsonValue` representation when all we need is the output string.
pub trait ToJson {
   /// Serialize `self` directly to a pretty-printed JSON string.
   fn to_json_pretty(&self) -> String;
}

// ---------------------------------------------------------------------------
// StructSerializer — incremental builder for pretty JSON objects
// ---------------------------------------------------------------------------

/// Builder for serializing a struct as a pretty JSON object.
///
/// Handles 2-space indentation, comma separators, empty-object detection
/// (renders `{}` rather than `{\n\n}`), and nested object construction via
/// closures.
///
/// ```
/// use jt_consoleutils::json::StructSerializer;
///
/// let mut s = StructSerializer::new();
/// s.field_str("name", "deploy");
/// s.field_i64("count", 42);
/// s.field_bool("active", true);
/// s.field_object("source", |inner| {
///    inner.field_str("path", "deploy.ts");
/// });
/// let json = s.finish();
/// // {
/// //   "name": "deploy",
/// //   "count": 42,
/// //   "active": true,
/// //   "source": {
/// //     "path": "deploy.ts"
/// //   }
/// // }
/// ```
pub struct StructSerializer {
   out: String,
   field_count: usize,
   indent: usize
}

impl Default for StructSerializer {
   fn default() -> Self {
      Self::new()
   }
}

impl StructSerializer {
   /// Create a new top-level serializer.
   pub fn new() -> Self {
      Self::nested(1)
   }

   /// Internal: create a serializer at a given indent level. Used by
   /// `field_object` to construct a child serializer that shares output state
   /// with the parent.
   fn nested(indent: usize) -> Self {
      StructSerializer { out: String::from("{\n"), field_count: 0, indent }
   }

   // -- string fields -------------------------------------------------------

   /// Write a string field.
   pub fn field_str(&mut self, key: &str, value: &str) {
      self.write_separator_and_key(key);
      push_json_string(&mut self.out, value);
   }

   /// Write an optional string field. `None` is skipped.
   pub fn field_opt_str(&mut self, key: &str, value: &Option<String>) {
      if let Some(v) = value {
         self.field_str(key, v);
      }
   }

   // -- bool fields ---------------------------------------------------------

   /// Write a boolean field.
   pub fn field_bool(&mut self, key: &str, value: bool) {
      self.write_separator_and_key(key);
      self.out.push_str(if value { "true" } else { "false" });
   }

   /// Write an optional boolean field. `None` is skipped.
   pub fn field_opt_bool(&mut self, key: &str, value: Option<bool>) {
      if let Some(v) = value {
         self.field_bool(key, v);
      }
   }

   // -- numeric fields ------------------------------------------------------

   /// Write an `i64` numeric field.
   pub fn field_i64(&mut self, key: &str, value: i64) {
      self.write_separator_and_key(key);
      self.out.push_str(&value.to_string());
   }

   /// Write an optional `i64` numeric field. `None` is skipped.
   pub fn field_opt_i64(&mut self, key: &str, value: Option<i64>) {
      if let Some(v) = value {
         self.field_i64(key, v);
      }
   }

   /// Write an `f64` numeric field. **Lossy on non-finite input:** JSON has
   /// no native representation for `NaN`, `+∞`, or `−∞`, so this method
   /// silently emits `null` for those values, matching the lossy
   /// `JsonValue::from(f64)` conversion. Note that `serde_json` would error
   /// instead.
   ///
   /// A fallible variant is deliberately not offered — an `Err` path would
   /// pull additional float-handling code into every consumer's binary.
   /// Callers that must reject non-finite values should check with
   /// `f64::is_finite` before calling.
   pub fn field_f64(&mut self, key: &str, value: f64) {
      self.write_separator_and_key(key);
      if value.is_finite() {
         self.out.push_str(&value.to_string());
      } else {
         self.out.push_str("null");
      }
   }

   /// Write an optional `f64` numeric field. `None` is skipped.
   pub fn field_opt_f64(&mut self, key: &str, value: Option<f64>) {
      if let Some(v) = value {
         self.field_f64(key, v);
      }
   }

   // -- nested object -------------------------------------------------------

   /// Write a nested object field, populated by the supplied closure.
   ///
   /// The closure receives a child `StructSerializer` at one deeper indent
   /// level; its `finish` is called automatically when the closure returns.
   /// Empty closures emit `"{}"`.
   pub fn field_object<F: FnOnce(&mut StructSerializer)>(&mut self, key: &str, build: F) {
      self.write_separator_and_key(key);
      let mut child = StructSerializer::nested(self.indent + 1);
      build(&mut child);
      self.out.push_str(&child.finish());
   }

   /// Write an optional nested object field. `None` is skipped; otherwise the
   /// closure runs with the inner value and a child serializer.
   pub fn field_opt_object<T, F>(&mut self, key: &str, value: &Option<T>, build: F)
   where
      F: FnOnce(&mut StructSerializer, &T)
   {
      if let Some(inner) = value {
         self.field_object(key, |child| build(child, inner));
      }
   }

   // -- arrays --------------------------------------------------------------

   /// Write an array of strings. Empty arrays emit `"[]"`.
   pub fn field_array_str(&mut self, key: &str, values: &[String]) {
      self.write_separator_and_key(key);
      if values.is_empty() {
         self.out.push_str("[]");
         return;
      }
      self.out.push_str("[\n");
      let inner = self.indent + 1;
      let mut first = true;
      for v in values {
         if !first {
            self.out.push_str(",\n");
         }
         first = false;
         for _ in 0..inner {
            self.out.push_str("  ");
         }
         push_json_string(&mut self.out, v);
      }
      self.out.push('\n');
      for _ in 0..self.indent {
         self.out.push_str("  ");
      }
      self.out.push(']');
   }

   // -- finish --------------------------------------------------------------

   /// Finish the object and return the JSON string. Empty objects produce
   /// `"{}"` rather than `"{\n\n}"`.
   pub fn finish(mut self) -> String {
      if self.field_count == 0 {
         return "{}".to_string();
      }
      self.out.push('\n');
      // Closing brace is indented one level shallower than fields inside.
      for _ in 0..(self.indent - 1) {
         self.out.push_str("  ");
      }
      self.out.push('}');
      self.out
   }

   // -- internal ------------------------------------------------------------

   fn write_separator_and_key(&mut self, key: &str) {
      if self.field_count > 0 {
         self.out.push_str(",\n");
      }
      self.field_count += 1;
      for _ in 0..self.indent {
         self.out.push_str("  ");
      }
      push_json_string(&mut self.out, key);
      self.out.push_str(": ");
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn empty_object_renders_as_braces() {
      let s = StructSerializer::new();
      assert_eq!(s.finish(), "{}");
   }

   #[test]
   fn single_string_field() {
      let mut s = StructSerializer::new();
      s.field_str("name", "deploy");
      assert_eq!(s.finish(), "{\n  \"name\": \"deploy\"\n}");
   }

   #[test]
   fn multiple_fields_with_comma_separators() {
      let mut s = StructSerializer::new();
      s.field_str("name", "deploy");
      s.field_i64("count", 42);
      s.field_bool("active", true);
      assert_eq!(s.finish(), "{\n  \"name\": \"deploy\",\n  \"count\": 42,\n  \"active\": true\n}");
   }

   #[test]
   fn opt_fields_skip_none() {
      let mut s = StructSerializer::new();
      s.field_opt_str("name", &None);
      s.field_opt_bool("active", None);
      s.field_opt_i64("count", None);
      assert_eq!(s.finish(), "{}");
   }

   #[test]
   fn opt_fields_emit_some() {
      let mut s = StructSerializer::new();
      s.field_opt_str("name", &Some("x".to_string()));
      s.field_opt_i64("n", Some(7));
      assert_eq!(s.finish(), "{\n  \"name\": \"x\",\n  \"n\": 7\n}");
   }

   #[test]
   fn f64_finite_renders_normally() {
      let mut s = StructSerializer::new();
      s.field_f64("ratio", 0.5);
      assert_eq!(s.finish(), "{\n  \"ratio\": 0.5\n}");
   }

   #[test]
   fn f64_non_finite_renders_as_null() {
      let mut s = StructSerializer::new();
      s.field_f64("a", f64::NAN);
      s.field_f64("b", f64::INFINITY);
      assert_eq!(s.finish(), "{\n  \"a\": null,\n  \"b\": null\n}");
   }

   #[test]
   fn nested_object_indents_correctly() {
      let mut s = StructSerializer::new();
      s.field_str("name", "x");
      s.field_object("inner", |child| {
         child.field_str("k", "v");
         child.field_i64("n", 1);
      });
      assert_eq!(s.finish(), "{\n  \"name\": \"x\",\n  \"inner\": {\n    \"k\": \"v\",\n    \"n\": 1\n  }\n}");
   }

   #[test]
   fn empty_nested_object_renders_as_braces() {
      let mut s = StructSerializer::new();
      s.field_object("inner", |_| {});
      assert_eq!(s.finish(), "{\n  \"inner\": {}\n}");
   }

   #[test]
   fn deeply_nested_objects_compose() {
      let mut s = StructSerializer::new();
      s.field_object("a", |a| {
         a.field_object("b", |b| {
            b.field_str("c", "deep");
         });
      });
      assert_eq!(s.finish(), "{\n  \"a\": {\n    \"b\": {\n      \"c\": \"deep\"\n    }\n  }\n}");
   }

   #[test]
   fn opt_object_skips_none() {
      let mut s = StructSerializer::new();
      let payload: Option<&str> = None;
      s.field_opt_object("inner", &payload, |child, p| child.field_str("p", p));
      assert_eq!(s.finish(), "{}");
   }

   #[test]
   fn opt_object_emits_some() {
      let mut s = StructSerializer::new();
      let payload = Some("x".to_string());
      s.field_opt_object("inner", &payload, |child, p| child.field_str("p", p));
      assert_eq!(s.finish(), "{\n  \"inner\": {\n    \"p\": \"x\"\n  }\n}");
   }

   #[test]
   fn empty_string_array_renders_as_brackets() {
      let mut s = StructSerializer::new();
      s.field_array_str("tags", &[]);
      assert_eq!(s.finish(), "{\n  \"tags\": []\n}");
   }

   #[test]
   fn populated_string_array() {
      let mut s = StructSerializer::new();
      s.field_array_str("tags", &["a".to_string(), "b".to_string()]);
      assert_eq!(s.finish(), "{\n  \"tags\": [\n    \"a\",\n    \"b\"\n  ]\n}");
   }

   #[test]
   fn string_array_inside_nested_object() {
      let mut s = StructSerializer::new();
      s.field_object("inner", |child| {
         child.field_array_str("tags", &["x".to_string()]);
      });
      assert_eq!(s.finish(), "{\n  \"inner\": {\n    \"tags\": [\n      \"x\"\n    ]\n  }\n}");
   }

   #[test]
   fn keys_with_special_chars_are_escaped() {
      let mut s = StructSerializer::new();
      s.field_str("a\"b", "v");
      assert_eq!(s.finish(), "{\n  \"a\\\"b\": \"v\"\n}");
   }
}
