use std::collections::{BTreeMap, HashMap};

use super::{error::JsonError, value::JsonValue};

/// Trait for types that can be constructed from a `JsonValue`.
pub trait FromJsonValue: Sized {
   /// Deserialize a `JsonValue` into `Self`.
   fn from_json_value(value: &JsonValue) -> Result<Self, JsonError>;
}

// ---------------------------------------------------------------------------
// Helpers for implementing FromJsonValue
// ---------------------------------------------------------------------------

/// Build a type-mismatch error: `"context.key: expected TYPE, got TYPE"`.
fn type_err(context: &str, key: &str, expected: &str, found: &str) -> JsonError {
   JsonError::value(format!("{context}.{key}: expected {expected}, got {found}"))
}

/// Expect the value to be an object; return its map and a key tracker for
/// deny_unknown_fields validation.
pub fn expect_object<'a>(
   value: &'a JsonValue,
   context: &str
) -> Result<&'a BTreeMap<String, JsonValue>, JsonError> {
   value
      .as_object()
      .ok_or_else(|| JsonError::value(format!("{context}: expected object, got {}", value.type_name())))
}

/// Check for unknown fields given a set of known camelCase key names.
pub fn deny_unknown_fields(
   map: &BTreeMap<String, JsonValue>,
   known: &[&str],
   context: &str
) -> Result<(), JsonError> {
   for key in map.keys() {
      if !known.contains(&key.as_str()) {
         return Err(JsonError::value(format!("{context}: unknown field '{key}'")));
      }
   }
   Ok(())
}

/// Extract a required string field.
pub fn require_string(
   map: &BTreeMap<String, JsonValue>,
   key: &str,
   context: &str
) -> Result<String, JsonError> {
   match map.get(key) {
      Some(JsonValue::String(s)) => Ok(s.clone()),
      Some(other) => Err(type_err(context, key, "string", other.type_name())),
      None => Err(JsonError::value(format!("{context}.{key}: required field missing")))
   }
}

/// Extract an optional string field.
pub fn optional_string(
   map: &BTreeMap<String, JsonValue>,
   key: &str,
   context: &str
) -> Result<Option<String>, JsonError> {
   match map.get(key) {
      Some(JsonValue::String(s)) => Ok(Some(s.clone())),
      Some(JsonValue::Null) | None => Ok(None),
      Some(other) => Err(type_err(context, key, "string", other.type_name()))
   }
}

/// Extract an optional bool field.
pub fn optional_bool(
   map: &BTreeMap<String, JsonValue>,
   key: &str,
   context: &str
) -> Result<Option<bool>, JsonError> {
   match map.get(key) {
      Some(JsonValue::Bool(b)) => Ok(Some(*b)),
      Some(JsonValue::Null) | None => Ok(None),
      Some(other) => Err(type_err(context, key, "boolean", other.type_name()))
   }
}

/// Extract an optional `Vec<String>` field.
pub fn optional_string_vec(
   map: &BTreeMap<String, JsonValue>,
   key: &str,
   context: &str
) -> Result<Option<Vec<String>>, JsonError> {
   match map.get(key) {
      Some(JsonValue::Array(arr)) => {
         let mut out = Vec::with_capacity(arr.len());
         for (i, v) in arr.iter().enumerate() {
            match v {
               JsonValue::String(s) => out.push(s.clone()),
               _ => {
                  return Err(JsonError::value(format!(
                     "{context}.{key}[{i}]: expected string, got {}",
                     v.type_name()
                  )));
               }
            }
         }
         Ok(Some(out))
      }
      Some(JsonValue::Null) | None => Ok(None),
      Some(other) => Err(type_err(context, key, "array", other.type_name()))
   }
}

/// Extract an optional `HashMap<String, String>` field.
pub fn optional_string_map(
   map: &BTreeMap<String, JsonValue>,
   key: &str,
   context: &str
) -> Result<Option<HashMap<String, String>>, JsonError> {
   match map.get(key) {
      Some(JsonValue::Object(obj)) => {
         let mut out = HashMap::with_capacity(obj.len());
         for (k, v) in obj {
            match v {
               JsonValue::String(s) => {
                  out.insert(k.clone(), s.clone());
               }
               _ => {
                  return Err(JsonError::value(format!("{context}.{key}.{k}: expected string, got {}", v.type_name())));
               }
            }
         }
         Ok(Some(out))
      }
      Some(JsonValue::Null) | None => Ok(None),
      Some(other) => Err(type_err(context, key, "object", other.type_name()))
   }
}

/// Extract an optional nested type that implements `FromJsonValue`.
pub fn optional_nested<T: FromJsonValue>(
   map: &BTreeMap<String, JsonValue>,
   key: &str
) -> Result<Option<T>, JsonError> {
   match map.get(key) {
      Some(JsonValue::Null) | None => Ok(None),
      Some(v) => T::from_json_value(v).map(Some)
   }
}

/// Extract an optional `HashMap<String, T>` where T implements FromJsonValue.
pub fn optional_map_of<T: FromJsonValue>(
   map: &BTreeMap<String, JsonValue>,
   key: &str,
   context: &str
) -> Result<Option<HashMap<String, T>>, JsonError> {
   match map.get(key) {
      Some(JsonValue::Object(obj)) => {
         let mut out = HashMap::with_capacity(obj.len());
         for (k, v) in obj {
            out.insert(k.clone(), T::from_json_value(v)?);
         }
         Ok(Some(out))
      }
      Some(JsonValue::Null) | None => Ok(None),
      Some(other) => Err(type_err(context, key, "object", other.type_name()))
   }
}
