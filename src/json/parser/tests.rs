use super::*;

// ---------------------------------------------------------------------------
// Basic values
// ---------------------------------------------------------------------------

#[test]
fn parse_null() {
   assert_eq!(parse_json("null").unwrap(), JsonValue::Null);
}

#[test]
fn parse_true() {
   assert_eq!(parse_json("true").unwrap(), JsonValue::Bool(true));
}

#[test]
fn parse_false() {
   assert_eq!(parse_json("false").unwrap(), JsonValue::Bool(false));
}

#[test]
fn parse_integer() {
   assert_eq!(parse_json("42").unwrap(), JsonValue::Number("42".into()));
}

#[test]
fn parse_negative_integer() {
   assert_eq!(parse_json("-7").unwrap(), JsonValue::Number("-7".into()));
}

#[test]
fn parse_float() {
   assert_eq!(parse_json("3.14").unwrap(), JsonValue::Number("3.14".into()));
}

#[test]
fn parse_exponent() {
   assert_eq!(parse_json("1e3").unwrap(), JsonValue::Number("1e3".into()));
}

#[test]
fn parse_simple_string() {
   assert_eq!(parse_json(r#""hello""#).unwrap(), JsonValue::String("hello".into()));
}

#[test]
fn parse_string_with_escapes() {
   let val = parse_json(r#""a\nb\tc""#).unwrap();
   assert_eq!(val.as_str().unwrap(), "a\nb\tc");
}

#[test]
fn parse_string_with_unicode_escape() {
   let val = parse_json(r#""\u0041\u0042""#).unwrap();
   assert_eq!(val.as_str().unwrap(), "AB");
}

#[test]
fn parse_string_with_surrogate_pair() {
   // U+1F600 (😀) = \uD83D\uDE00
   let val = parse_json(r#""\uD83D\uDE00""#).unwrap();
   assert_eq!(val.as_str().unwrap(), "😀");
}

#[test]
fn parse_string_with_raw_utf8() {
   let val = parse_json(r#""café""#).unwrap();
   assert_eq!(val.as_str().unwrap(), "café");
}

// ---------------------------------------------------------------------------
// Objects
// ---------------------------------------------------------------------------

#[test]
fn parse_empty_object() {
   let val = parse_json("{}").unwrap();
   assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn parse_simple_object() {
   let val = parse_json(r#"{"key": "value"}"#).unwrap();
   assert_eq!(val["key"], "value");
}

#[test]
fn parse_nested_object() {
   let val = parse_json(r#"{"a": {"b": {"c": 1}}}"#).unwrap();
   assert_eq!(val["a"]["b"]["c"], 1);
}

#[test]
fn parse_object_preserves_all_fields() {
   let val = parse_json(r#"{"x": 1, "y": 2, "z": 3}"#).unwrap();
   assert_eq!(val.as_object().unwrap().len(), 3);
}

// ---------------------------------------------------------------------------
// Arrays
// ---------------------------------------------------------------------------

#[test]
fn parse_empty_array() {
   let val = parse_json("[]").unwrap();
   assert!(val.as_array().unwrap().is_empty());
}

#[test]
fn parse_string_array() {
   let val = parse_json(r#"["a", "b", "c"]"#).unwrap();
   assert_eq!(val[0], "a");
   assert_eq!(val[1], "b");
   assert_eq!(val[2], "c");
}

#[test]
fn parse_mixed_array() {
   let val = parse_json(r#"[1, "two", true, null]"#).unwrap();
   assert_eq!(val[0], 1);
   assert_eq!(val[1], "two");
   assert_eq!(val[2], true);
   assert_eq!(val[3], JsonValue::Null);
}

// ---------------------------------------------------------------------------
// Whitespace
// ---------------------------------------------------------------------------

#[test]
fn parse_with_leading_and_trailing_whitespace() {
   let val = parse_json("  \n  42  \n  ").unwrap();
   assert_eq!(val, JsonValue::Number("42".into()));
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn error_on_trailing_content() {
   assert!(parse_json("42 extra").is_err());
}

#[test]
fn error_on_unterminated_string() {
   assert!(parse_json(r#""hello"#).is_err());
}

#[test]
fn error_on_empty_input() {
   assert!(parse_json("").is_err());
}

#[test]
fn error_has_line_and_column() {
   let err = parse_json("{\n  \"a\": }").unwrap_err();
   let msg = err.to_string();
   assert!(msg.contains("line 2"), "expected line 2 in: {msg}");
}

// ---------------------------------------------------------------------------
// JSONC features
// ---------------------------------------------------------------------------

#[test]
fn jsonc_strips_line_comments() {
   let val = parse_jsonc("{\n  // comment\n  \"key\": \"value\" // trailing\n}").unwrap();
   assert_eq!(val["key"], "value");
}

#[test]
fn jsonc_strips_block_comments() {
   let val = parse_jsonc("{\n  /* block */ \"key\": /* inline */ \"value\"\n}").unwrap();
   assert_eq!(val["key"], "value");
}

#[test]
fn jsonc_strips_trailing_commas() {
   let val = parse_jsonc(r#"{"a": 1, "b": [1, 2, 3,], "c": {"d": true,},}"#).unwrap();
   assert_eq!(val["a"], 1);
   assert_eq!(val["b"][2], 3);
   assert_eq!(val["c"]["d"], true);
}

#[test]
fn jsonc_preserves_strings_with_slashes() {
   let val = parse_jsonc(r#"{"url": "https://example.com/path", "pattern": "a//b/*c*/d",}"#).unwrap();
   assert_eq!(val["url"], "https://example.com/path");
   assert_eq!(val["pattern"], "a//b/*c*/d");
}

#[test]
fn jsonc_all_features_together() {
   let input = r#"{
  // database settings
  "host": "localhost", /* default host */
  "port": 5432,
  "tags": ["a", "b",],
}"#;
   let val = parse_jsonc(input).unwrap();
   assert_eq!(val["host"], "localhost");
   assert_eq!(val["port"], 5432);
   assert_eq!(val["tags"][1], "b");
}

#[test]
fn jsonc_plain_json_passes_through() {
   let val = parse_jsonc(r#"{"key": "value", "num": 42}"#).unwrap();
   assert_eq!(val["key"], "value");
   assert_eq!(val["num"], 42);
}
