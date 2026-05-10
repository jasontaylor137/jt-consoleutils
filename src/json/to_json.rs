use std::collections::{BTreeMap, HashMap};

use super::escape::push_json_string;

/// Trait for types that can serialize themselves directly to a pretty JSON
/// string, without first materializing a [`super::JsonValue`].
///
/// # `ToJson` vs `From<T> for JsonValue`
///
/// - **`From<T> for JsonValue`** — use when you want a typed in-memory value to plug into a larger
///   [`super::JsonValue`] tree (e.g. `JsonValue::Array(vec![v.into(), ...])`) or to roundtrip
///   through [`super::to_json_pretty`]. Builds an enum value; no string is produced until you call
///   `to_json_pretty`.
/// - **`ToJson`** — use when the destination is *just* a JSON string (writing to a file, building a
///   config blob). Skips the `JsonValue` allocation and writes straight into the output buffer.
///
/// # Implementing for your own types
///
/// Implementors only need to provide [`ToJson::write_pretty`]. The default
/// [`ToJson::to_json_pretty`] handles the top-level `String` allocation.
/// Use [`StructSerializer::at`] inside `write_pretty` so nested objects pick
/// up the surrounding indent level:
///
/// ```
/// use jt_consoleutils::json::{StructSerializer, ToJson};
///
/// struct Build { name: String, count: i64, tags: Vec<String> }
///
/// impl ToJson for Build {
///    fn write_pretty(&self, out: &mut String, indent: usize) {
///       let mut s = StructSerializer::at(indent);
///       s.field_str("name", &self.name);
///       s.field_i64("count", self.count);
///       s.field("tags", &self.tags);
///       out.push_str(&s.finish());
///    }
/// }
///
/// let b = Build { name: "deploy".into(), count: 1, tags: vec!["beta".into()] };
/// assert_eq!(
///    b.to_json_pretty(),
///    "{\n  \"name\": \"deploy\",\n  \"count\": 1,\n  \"tags\": [\n    \"beta\"\n  ]\n}"
/// );
/// ```
///
/// Blanket impls cover the std types you'd typically reach for (`String`,
/// `&str`, `bool`, `i64`, `f64`, `Option<T>`, `Vec<T>`, `[T]`, and the two
/// string-keyed map types), so a `Vec<MyStruct>` or `Option<MyStruct>` field
/// serializes through `ToJson` without a hand-rolled wrapper.
pub trait ToJson {
   /// Append the value's pretty-printed JSON to `out`. `indent` is the indent
   /// level of the *current line position* — i.e. an object's closing `}` will
   /// be written at `indent`, and its fields one level deeper.
   fn write_pretty(&self, out: &mut String, indent: usize);

   /// Serialize `self` to a pretty-printed JSON string.
   fn to_json_pretty(&self) -> String {
      let mut out = String::new();
      self.write_pretty(&mut out, 0);
      out
   }
}

// ---------------------------------------------------------------------------
// Blanket impls for std types
// ---------------------------------------------------------------------------

impl ToJson for str {
   fn write_pretty(&self, out: &mut String, _indent: usize) {
      push_json_string(out, self);
   }
}

impl ToJson for String {
   fn write_pretty(&self, out: &mut String, indent: usize) {
      self.as_str().write_pretty(out, indent);
   }
}

impl ToJson for bool {
   fn write_pretty(&self, out: &mut String, _indent: usize) {
      out.push_str(if *self { "true" } else { "false" });
   }
}

impl ToJson for i64 {
   fn write_pretty(&self, out: &mut String, _indent: usize) {
      out.push_str(&self.to_string());
   }
}

/// **Lossy on non-finite input.** `NaN`/`±∞` serialize as JSON `null`,
/// matching [`super::JsonValue::from`]`(f64)`.
impl ToJson for f64 {
   fn write_pretty(&self, out: &mut String, _indent: usize) {
      if self.is_finite() {
         out.push_str(&self.to_string());
      } else {
         out.push_str("null");
      }
   }
}

impl<T: ToJson + ?Sized> ToJson for &T {
   fn write_pretty(&self, out: &mut String, indent: usize) {
      (*self).write_pretty(out, indent);
   }
}

impl<T: ToJson> ToJson for Option<T> {
   fn write_pretty(&self, out: &mut String, indent: usize) {
      match self {
         Some(v) => v.write_pretty(out, indent),
         None => out.push_str("null")
      }
   }
}

impl<T: ToJson> ToJson for [T] {
   fn write_pretty(&self, out: &mut String, indent: usize) {
      write_array(out, indent, self.iter());
   }
}

impl<T: ToJson> ToJson for Vec<T> {
   fn write_pretty(&self, out: &mut String, indent: usize) {
      self.as_slice().write_pretty(out, indent);
   }
}

/// `BTreeMap` already iterates in sorted-key order, matching the rest of the
/// crate's JSON output.
impl<T: ToJson> ToJson for BTreeMap<String, T> {
   fn write_pretty(&self, out: &mut String, indent: usize) {
      write_object(out, indent, self.iter().map(|(k, v)| (k.as_str(), v)));
   }
}

/// Keys are sorted before emission so output stays stable across runs (and
/// matches the [`BTreeMap`] impl).
impl<T: ToJson> ToJson for HashMap<String, T> {
   fn write_pretty(&self, out: &mut String, indent: usize) {
      let mut keys: Vec<&String> = self.keys().collect();
      keys.sort();
      write_object(out, indent, keys.into_iter().map(|k| (k.as_str(), &self[k])));
   }
}

// ---------------------------------------------------------------------------
// Shared array/object writers
// ---------------------------------------------------------------------------

fn write_array<'a, T, I>(out: &mut String, indent: usize, items: I)
where
   T: ToJson + 'a,
   I: IntoIterator<Item = &'a T>
{
   let mut iter = items.into_iter().peekable();
   if iter.peek().is_none() {
      out.push_str("[]");
      return;
   }
   out.push_str("[\n");
   let inner = indent + 1;
   let mut first = true;
   for v in iter {
      if !first {
         out.push_str(",\n");
      }
      first = false;
      push_indent(out, inner);
      v.write_pretty(out, inner);
   }
   out.push('\n');
   push_indent(out, indent);
   out.push(']');
}

fn write_object<'a, T, I>(out: &mut String, indent: usize, entries: I)
where
   T: ToJson + 'a,
   I: IntoIterator<Item = (&'a str, &'a T)>
{
   let mut iter = entries.into_iter().peekable();
   if iter.peek().is_none() {
      out.push_str("{}");
      return;
   }
   out.push_str("{\n");
   let inner = indent + 1;
   let mut first = true;
   for (k, v) in iter {
      if !first {
         out.push_str(",\n");
      }
      first = false;
      push_indent(out, inner);
      push_json_string(out, k);
      out.push_str(": ");
      v.write_pretty(out, inner);
   }
   out.push('\n');
   push_indent(out, indent);
   out.push('}');
}

fn push_indent(out: &mut String, level: usize) {
   for _ in 0..level {
      out.push_str("  ");
   }
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
   /// Indent level of this object's closing brace. Fields are written at
   /// `indent + 1`. Top-level objects start at 0, matching
   /// [`crate::json::to_json_pretty`].
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
      Self::at(0)
   }

   /// Create a serializer whose closing brace lives at `indent`. Use this
   /// inside [`ToJson::write_pretty`] so nested values pick up the surrounding
   /// indent level.
   pub fn at(indent: usize) -> Self {
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
      let mut child = StructSerializer::at(self.indent + 1);
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

   // -- generic ToJson fields ----------------------------------------------

   /// Write any [`ToJson`] value as a field. Use this for nested user types,
   /// `Vec<T>`, `Option<T>`, etc.
   pub fn field<T: ToJson + ?Sized>(&mut self, key: &str, value: &T) {
      self.write_separator_and_key(key);
      value.write_pretty(&mut self.out, self.indent + 1);
   }

   // -- arrays --------------------------------------------------------------

   /// Write an array of strings. Empty arrays emit `"[]"`.
   pub fn field_array_str(&mut self, key: &str, values: &[String]) {
      self.field(key, values);
   }

   /// Write an array of any [`ToJson`] values. Empty arrays emit `"[]"`.
   pub fn field_array<T: ToJson>(&mut self, key: &str, values: &[T]) {
      self.field(key, values);
   }

   // -- finish --------------------------------------------------------------

   /// Finish the object and return the JSON string. Empty objects produce
   /// `"{}"` rather than `"{\n\n}"`.
   pub fn finish(mut self) -> String {
      if self.field_count == 0 {
         return "{}".to_string();
      }
      self.out.push('\n');
      for _ in 0..self.indent {
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
      for _ in 0..(self.indent + 1) {
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
   fn matches_to_json_pretty_for_equivalent_data() {
      use std::collections::BTreeMap;

      use crate::json::{JsonValue, to_json_pretty};

      // Build the same shape via both paths. Use keys in alphabetical order
      // so that BTreeMap (sorted) and StructSerializer (insertion order) emit
      // the same key sequence; this test isolates indentation, not ordering.
      let mut s = StructSerializer::new();
      s.field_str("a", "x");
      s.field_array_str("b", &["one".to_string(), "two".to_string()]);
      s.field_object("c", |c| {
         c.field_str("d", "v");
         c.field_array_str("e", &["x".to_string()]);
         c.field_object("f", |f| {
            f.field_i64("g", 1);
         });
      });
      let from_struct = s.finish();

      let mut deepest = BTreeMap::new();
      deepest.insert("g".to_string(), JsonValue::Number("1".to_string()));
      let mut middle = BTreeMap::new();
      middle.insert("d".to_string(), JsonValue::String("v".to_string()));
      middle.insert("e".to_string(), JsonValue::Array(vec![JsonValue::String("x".to_string())]));
      middle.insert("f".to_string(), JsonValue::Object(deepest));
      let mut top = BTreeMap::new();
      top.insert("a".to_string(), JsonValue::String("x".to_string()));
      top.insert(
         "b".to_string(),
         JsonValue::Array(vec![JsonValue::String("one".to_string()), JsonValue::String("two".to_string())])
      );
      top.insert("c".to_string(), JsonValue::Object(middle));
      let from_value = to_json_pretty(&JsonValue::Object(top));

      assert_eq!(from_struct, from_value);
   }

   #[test]
   fn keys_with_special_chars_are_escaped() {
      let mut s = StructSerializer::new();
      s.field_str("a\"b", "v");
      assert_eq!(s.finish(), "{\n  \"a\\\"b\": \"v\"\n}");
   }

   // -- blanket ToJson impls --------------------------------------------------

   #[test]
   fn to_json_for_str_quotes_and_escapes() {
      assert_eq!("hi".to_json_pretty(), "\"hi\"");
      assert_eq!("a\"b".to_json_pretty(), "\"a\\\"b\"");
   }

   #[test]
   fn to_json_for_string_matches_str() {
      assert_eq!(String::from("hi").to_json_pretty(), "\"hi\"");
   }

   #[test]
   fn to_json_for_bool() {
      assert_eq!(true.to_json_pretty(), "true");
      assert_eq!(false.to_json_pretty(), "false");
   }

   #[test]
   fn to_json_for_i64() {
      assert_eq!(42_i64.to_json_pretty(), "42");
      assert_eq!((-7_i64).to_json_pretty(), "-7");
   }

   #[test]
   fn to_json_for_f64_finite() {
      assert_eq!(0.5_f64.to_json_pretty(), "0.5");
   }

   #[test]
   fn to_json_for_f64_non_finite_is_null() {
      assert_eq!(f64::NAN.to_json_pretty(), "null");
      assert_eq!(f64::INFINITY.to_json_pretty(), "null");
   }

   #[test]
   fn to_json_for_option_some_delegates() {
      assert_eq!(Some(42_i64).to_json_pretty(), "42");
   }

   #[test]
   fn to_json_for_option_none_is_null() {
      let v: Option<i64> = None;
      assert_eq!(v.to_json_pretty(), "null");
   }

   #[test]
   fn to_json_for_empty_vec_is_brackets() {
      let v: Vec<i64> = vec![];
      assert_eq!(v.to_json_pretty(), "[]");
   }

   #[test]
   fn to_json_for_vec_of_strings() {
      let v = vec!["a".to_string(), "b".to_string()];
      assert_eq!(v.to_json_pretty(), "[\n  \"a\",\n  \"b\"\n]");
   }

   #[test]
   fn to_json_for_vec_of_user_type_recurses() {
      struct Item(i64);
      impl ToJson for Item {
         fn write_pretty(&self, out: &mut String, indent: usize) {
            let mut s = StructSerializer::at(indent);
            s.field_i64("n", self.0);
            out.push_str(&s.finish());
         }
      }
      let v = vec![Item(1), Item(2)];
      assert_eq!(v.to_json_pretty(), "[\n  {\n    \"n\": 1\n  },\n  {\n    \"n\": 2\n  }\n]");
   }

   #[test]
   fn to_json_for_btreemap_emits_sorted_object() {
      let mut m: BTreeMap<String, i64> = BTreeMap::new();
      m.insert("b".into(), 2);
      m.insert("a".into(), 1);
      assert_eq!(m.to_json_pretty(), "{\n  \"a\": 1,\n  \"b\": 2\n}");
   }

   #[test]
   fn to_json_for_hashmap_sorts_keys() {
      let mut m: HashMap<String, i64> = HashMap::new();
      m.insert("b".into(), 2);
      m.insert("a".into(), 1);
      assert_eq!(m.to_json_pretty(), "{\n  \"a\": 1,\n  \"b\": 2\n}");
   }

   #[test]
   fn to_json_for_empty_map_is_braces() {
      let m: BTreeMap<String, i64> = BTreeMap::new();
      assert_eq!(m.to_json_pretty(), "{}");
   }

   #[test]
   fn struct_serializer_field_generic_handles_vec() {
      let mut s = StructSerializer::new();
      s.field_str("name", "x");
      s.field("tags", &vec!["a".to_string(), "b".to_string()]);
      assert_eq!(s.finish(), "{\n  \"name\": \"x\",\n  \"tags\": [\n    \"a\",\n    \"b\"\n  ]\n}");
   }

   #[test]
   fn struct_serializer_field_generic_handles_option_some() {
      let mut s = StructSerializer::new();
      s.field("count", &Some(7_i64));
      assert_eq!(s.finish(), "{\n  \"count\": 7\n}");
   }

   #[test]
   fn struct_serializer_field_array_with_user_type() {
      struct Item(i64);
      impl ToJson for Item {
         fn write_pretty(&self, out: &mut String, indent: usize) {
            let mut s = StructSerializer::at(indent);
            s.field_i64("n", self.0);
            out.push_str(&s.finish());
         }
      }
      let mut s = StructSerializer::new();
      s.field_array("items", &[Item(1), Item(2)]);
      assert_eq!(s.finish(), "{\n  \"items\": [\n    {\n      \"n\": 1\n    },\n    {\n      \"n\": 2\n    }\n  ]\n}");
   }
}
