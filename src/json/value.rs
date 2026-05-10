use std::collections::BTreeMap;

/// Lightweight JSON value type. Uses `BTreeMap` for objects so keys are
/// always sorted (matches `serde_json`'s pretty-print ordering).
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
   /// A JSON object with sorted keys.
   Object(BTreeMap<String, JsonValue>),
   /// A JSON array.
   Array(Vec<JsonValue>),
   /// A JSON string.
   String(String),
   /// A JSON number, stored as the original lexeme. Lazy: `as_f64()` parses
   /// on demand so consumers that never inspect numeric values don't drag
   /// `<f64 as FromStr>::from_str` into the binary.
   Number(String),
   /// A JSON boolean.
   Bool(bool),
   /// JSON `null`.
   Null
}

impl JsonValue {
   // -- constructors ----------------------------------------------------------

   /// Create an empty object.
   pub fn object() -> Self {
      JsonValue::Object(BTreeMap::new())
   }

   /// Create a string value.
   pub fn string(s: impl Into<String>) -> Self {
      JsonValue::String(s.into())
   }

   /// Build an object from key-value pairs.
   pub fn obj(pairs: &[(&str, JsonValue)]) -> Self {
      let mut map = BTreeMap::new();
      for (k, v) in pairs {
         map.insert(k.to_string(), v.clone());
      }
      JsonValue::Object(map)
   }

   // -- type checks -----------------------------------------------------------

   /// Return `true` if this is an object.
   pub fn is_object(&self) -> bool {
      matches!(self, JsonValue::Object(_))
   }

   /// Return `true` if this is an array.
   pub fn is_array(&self) -> bool {
      matches!(self, JsonValue::Array(_))
   }

   /// Return `true` if this is a string.
   pub fn is_string(&self) -> bool {
      matches!(self, JsonValue::String(_))
   }

   /// Return `true` if this is a number.
   pub fn is_number(&self) -> bool {
      matches!(self, JsonValue::Number(_))
   }

   /// Return `true` if this is a boolean.
   pub fn is_bool(&self) -> bool {
      matches!(self, JsonValue::Bool(_))
   }

   /// Return `true` if this is `null`.
   pub fn is_null(&self) -> bool {
      matches!(self, JsonValue::Null)
   }

   // -- accessors -------------------------------------------------------------

   /// Get a child value by key (objects only). Returns `None` for non-objects
   /// or missing keys.
   pub fn get(&self, key: &str) -> Option<&JsonValue> {
      match self {
         JsonValue::Object(m) => m.get(key),
         _ => None
      }
   }

   /// Return the inner object map, if this is an object.
   pub fn as_object(&self) -> Option<&BTreeMap<String, JsonValue>> {
      match self {
         JsonValue::Object(m) => Some(m),
         _ => None
      }
   }

   /// Return a mutable reference to the inner object map.
   pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<String, JsonValue>> {
      match self {
         JsonValue::Object(m) => Some(m),
         _ => None
      }
   }

   /// Return the inner string slice, if this is a string.
   pub fn as_str(&self) -> Option<&str> {
      match self {
         JsonValue::String(s) => Some(s),
         _ => None
      }
   }

   /// Get a nested string field, returning `fallback` when the key is missing
   /// or the value is not a string. Shorthand for
   /// `.get(key).and_then(|v| v.as_str()).unwrap_or(fallback)`.
   pub fn str_or<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
      self.get(key).and_then(|v| v.as_str()).unwrap_or(fallback)
   }

   /// Return the inner boolean, if this is a bool.
   pub fn as_bool(&self) -> Option<bool> {
      match self {
         JsonValue::Bool(b) => Some(*b),
         _ => None
      }
   }

   /// Return the inner number as `f64`, if this is a number. Parses lazily —
   /// callers that don't invoke this avoid pulling float parsing into the
   /// final binary.
   pub fn as_f64(&self) -> Option<f64> {
      match self {
         JsonValue::Number(s) => s.parse().ok(),
         _ => None
      }
   }

   /// Return the raw number lexeme, if this is a number. Useful for callers
   /// that want to round-trip a value without parsing it.
   pub fn as_number_str(&self) -> Option<&str> {
      match self {
         JsonValue::Number(s) => Some(s),
         _ => None
      }
   }

   /// Return the inner array, if this is an array.
   pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
      match self {
         JsonValue::Array(a) => Some(a),
         _ => None
      }
   }

   /// Type name for error messages.
   pub fn type_name(&self) -> &'static str {
      match self {
         JsonValue::Object(_) => "object",
         JsonValue::Array(_) => "array",
         JsonValue::String(_) => "string",
         JsonValue::Number(_) => "number",
         JsonValue::Bool(_) => "boolean",
         JsonValue::Null => "null"
      }
   }
}

// -- Index by &str for convenience (returns Null for missing keys) ------------

impl std::ops::Index<&str> for JsonValue {
   type Output = JsonValue;
   fn index(&self, key: &str) -> &JsonValue {
      static NULL: JsonValue = JsonValue::Null;
      self.get(key).unwrap_or(&NULL)
   }
}

impl std::ops::Index<usize> for JsonValue {
   type Output = JsonValue;
   fn index(&self, idx: usize) -> &JsonValue {
      static NULL: JsonValue = JsonValue::Null;
      match self {
         JsonValue::Array(a) => a.get(idx).unwrap_or(&NULL),
         _ => &NULL
      }
   }
}

// -- From<T> for ergonomic value construction --------------------------------
//
// Only `i64` and `f64` are provided for numbers — narrower integer types
// should `as i64`. This keeps the API surface small.

impl From<&str> for JsonValue {
   fn from(s: &str) -> Self {
      JsonValue::String(s.to_string())
   }
}

impl From<String> for JsonValue {
   fn from(s: String) -> Self {
      JsonValue::String(s)
   }
}

impl From<bool> for JsonValue {
   fn from(b: bool) -> Self {
      JsonValue::Bool(b)
   }
}

impl From<i64> for JsonValue {
   fn from(n: i64) -> Self {
      JsonValue::Number(n.to_string())
   }
}

impl From<f64> for JsonValue {
   /// **Lossy on non-finite input.** JSON has no native representation for
   /// `NaN`, `+∞`, or `−∞`, so this infallible conversion silently substitutes
   /// `JsonValue::Null` for any non-finite value. Note that this differs from
   /// `serde_json`, which errors on non-finite floats.
   ///
   /// We deliberately do not offer a fallible counterpart: an `Err` path would
   /// pull additional float-handling code into every consumer's binary. Callers
   /// that must reject non-finite values should check with `f64::is_finite`
   /// before calling.
   fn from(n: f64) -> Self {
      if n.is_finite() { JsonValue::Number(n.to_string()) } else { JsonValue::Null }
   }
}

impl From<Vec<JsonValue>> for JsonValue {
   fn from(v: Vec<JsonValue>) -> Self {
      JsonValue::Array(v)
   }
}

impl From<BTreeMap<String, JsonValue>> for JsonValue {
   fn from(m: BTreeMap<String, JsonValue>) -> Self {
      JsonValue::Object(m)
   }
}

// -- PartialEq with primitives for ergonomic assertions -----------------------

impl PartialEq<&str> for JsonValue {
   fn eq(&self, other: &&str) -> bool {
      self.as_str() == Some(*other)
   }
}

impl PartialEq<bool> for JsonValue {
   fn eq(&self, other: &bool) -> bool {
      self.as_bool() == Some(*other)
   }
}

impl PartialEq<i32> for JsonValue {
   fn eq(&self, other: &i32) -> bool {
      self.as_f64() == Some(f64::from(*other))
   }
}

impl PartialEq<i64> for JsonValue {
   fn eq(&self, other: &i64) -> bool {
      self.as_f64() == Some(*other as f64)
   }
}
