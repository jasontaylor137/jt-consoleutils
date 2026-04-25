use std::collections::BTreeMap;

use super::*;
use crate::json::JsonValue;

#[test]
fn serialize_null() {
   assert_eq!(to_json_pretty(&JsonValue::Null), "null");
}

#[test]
fn serialize_true() {
   assert_eq!(to_json_pretty(&JsonValue::Bool(true)), "true");
}

#[test]
fn serialize_false() {
   assert_eq!(to_json_pretty(&JsonValue::Bool(false)), "false");
}

#[test]
fn serialize_integer() {
   assert_eq!(to_json_pretty(&JsonValue::Number("42".into())), "42");
}

#[test]
fn serialize_negative_integer() {
   assert_eq!(to_json_pretty(&JsonValue::Number("-7".into())), "-7");
}

#[test]
fn serialize_float() {
   assert_eq!(to_json_pretty(&JsonValue::Number("3.14".into())), "3.14");
}

#[test]
fn serialize_simple_string() {
   assert_eq!(to_json_pretty(&JsonValue::String("hello".into())), r#""hello""#);
}

#[test]
fn serialize_string_with_escapes() {
   let val = JsonValue::String("a\nb\tc".into());
   assert_eq!(to_json_pretty(&val), r#""a\nb\tc""#);
}

#[test]
fn serialize_empty_object() {
   assert_eq!(to_json_pretty(&JsonValue::Object(BTreeMap::new())), "{}");
}

#[test]
fn serialize_empty_array() {
   assert_eq!(to_json_pretty(&JsonValue::Array(vec![])), "[]");
}

#[test]
fn serialize_simple_object() {
   let mut map = BTreeMap::new();
   map.insert("key".into(), JsonValue::String("value".into()));
   let out = to_json_pretty(&JsonValue::Object(map));
   assert_eq!(out, "{\n  \"key\": \"value\"\n}");
}

#[test]
fn serialize_object_keys_sorted() {
   let mut map = BTreeMap::new();
   map.insert("b".into(), JsonValue::Number("2".into()));
   map.insert("a".into(), JsonValue::Number("1".into()));
   let out = to_json_pretty(&JsonValue::Object(map));
   assert_eq!(out, "{\n  \"a\": 1,\n  \"b\": 2\n}");
}

#[test]
fn serialize_nested_object() {
   let mut inner = BTreeMap::new();
   inner.insert("c".into(), JsonValue::Bool(true));
   let mut outer = BTreeMap::new();
   outer.insert("b".into(), JsonValue::Object(inner));
   let mut root = BTreeMap::new();
   root.insert("a".into(), JsonValue::Object(outer));
   let out = to_json_pretty(&JsonValue::Object(root));
   assert_eq!(out, "{\n  \"a\": {\n    \"b\": {\n      \"c\": true\n    }\n  }\n}");
}

#[test]
fn serialize_array_of_strings() {
   let arr = vec![JsonValue::String("a".into()), JsonValue::String("b".into())];
   let out = to_json_pretty(&JsonValue::Array(arr));
   assert_eq!(out, "[\n  \"a\",\n  \"b\"\n]");
}

#[test]
fn roundtrip_parse_and_serialize() {
   let input = r#"{
  "array": [
    1,
    2,
    3
  ],
  "key": "value",
  "nested": {
    "inner": true
  },
  "num": 42
}"#;
   let parsed = crate::json::parse_json(input).unwrap();
   let output = to_json_pretty(&parsed);
   assert_eq!(output, input);
}
