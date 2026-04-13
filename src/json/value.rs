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
   /// A JSON number (stored as `f64`).
   Number(f64),
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

   /// Return the inner number as `f64`, if this is a number.
   pub fn as_f64(&self) -> Option<f64> {
      match self {
         JsonValue::Number(n) => Some(*n),
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
