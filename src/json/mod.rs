//! Lightweight, zero-dependency JSON and JSONC handling.
//!
//! Provides `parse_json` and `parse_jsonc` for parsing, `to_json_pretty`
//! for serialization, and [`JsonValue`](crate::json::value::JsonValue) as the central
//! value type.
//!
//! For typed deserialization, implement `FromJsonValue` using the provided
//! helpers (`require_string`, `optional_bool`, etc.).  For typed serialization
//! without an intermediate `JsonValue`, implement `ToJson` with
//! `StructSerializer`.

mod deserialize;
mod error;
mod escape;
mod ops;
mod parser;
mod serialize;
mod to_json;
/// The [`JsonValue`] enum and convenience impls.
pub mod value;

// Re-export the public API from a single entry point.

pub use deserialize::{
   FromJsonValue, deny_unknown_fields, expect_object, optional_bool, optional_f64, optional_i64, optional_map_of,
   optional_nested, optional_string, optional_string_map, optional_string_vec, optional_vec_of, require_f64,
   require_string
};
pub use error::JsonError;
pub use ops::{json_deep_merge, json_remove_paths};
pub use parser::{parse_json, parse_jsonc};
pub use serialize::to_json_pretty;
pub use to_json::{StructSerializer, ToJson};
pub use value::JsonValue;
