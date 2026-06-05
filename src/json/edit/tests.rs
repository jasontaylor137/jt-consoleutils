use super::*;
use crate::json::parse_jsonc;

/// Helper: assert that two documents parse to equal `JsonValue` trees.
fn parses_equal(a: &str, b: &str) -> bool {
   parse_jsonc(a).unwrap() == parse_jsonc(b).unwrap()
}

#[test]
fn scan_object_lists_members_in_order() {
   let src = "{ \"a\": 1, \"b\": { \"c\": 2 } }";
   let obj = scan_object(src.as_bytes(), 0).unwrap();
   let keys: Vec<&str> = obj.members.iter().map(|m| m.key.as_str()).collect();
   assert_eq!(keys, vec!["a", "b"]);
   // value of "a" is "1"
   let a = &obj.members[0];
   assert_eq!(&src[a.key_start..a.value_end].trim_start_matches(['"', 'a']).trim_start_matches(['"', ':', ' ']), &"1");
}

#[test]
fn scan_object_value_end_covers_nested_object() {
   let src = "{ \"b\": { \"c\": 2 } }";
   let obj = scan_object(src.as_bytes(), 0).unwrap();
   let b = &obj.members[0];
   assert_eq!(&src[b.value_start_for_test()..b.value_end], "{ \"c\": 2 }");
}

#[test]
fn scan_object_handles_trailing_comma() {
   let src = "{ \"a\": 1, }";
   let obj = scan_object(src.as_bytes(), 0).unwrap();
   assert_eq!(obj.members.len(), 1);
   assert_eq!(obj.members[0].key, "a");
}

#[test]
fn scan_object_skips_comments_between_members() {
   let src = "{ \"a\": 1, // note\n \"b\": 2 }";
   let obj = scan_object(src.as_bytes(), 0).unwrap();
   let keys: Vec<&str> = obj.members.iter().map(|m| m.key.as_str()).collect();
   assert_eq!(keys, vec!["a", "b"]);
}

#[test]
fn scan_object_empty() {
   let obj = scan_object(b"{}", 0).unwrap();
   assert!(obj.members.is_empty());
   assert_eq!(obj.open, 0);
   assert_eq!(obj.close, 1);
}

#[test]
fn get_scalar_returns_raw_span_with_quotes() {
   let src = "{ \"dep\": { \"axios\": \"^1.6\" } }";
   assert_eq!(jsonc_get(src, &["dep", "axios"]).unwrap(), Some("\"^1.6\""));
}

#[test]
fn get_number_leaf_raw() {
   let src = "{ \"a\": 42 }";
   assert_eq!(jsonc_get(src, &["a"]).unwrap(), Some("42"));
}

#[test]
fn get_subtree_returns_object_span() {
   let src = "{ \"work\": { \"runtime\": \"node\" } }";
   assert_eq!(jsonc_get(src, &["work"]).unwrap(), Some("{ \"runtime\": \"node\" }"));
}

#[test]
fn get_absent_path_is_none() {
   let src = "{ \"a\": 1 }";
   assert_eq!(jsonc_get(src, &["b"]).unwrap(), None);
   // "a" is present but a scalar; a NON-last segment that is not an object is
   // NotAnObject (not None).
   assert!(matches!(jsonc_get(src, &["a", "deeper"]), Err(EditError::NotAnObject { .. })));
}

#[test]
fn get_through_scalar_errors() {
   let src = "{ \"a\": 1 }";
   // "a" is a scalar; descending into it is an error, not None.
   assert!(matches!(jsonc_get(src, &["a", "x", "y"]), Err(EditError::NotAnObject { .. })));
}

#[test]
fn unset_absent_is_none() {
   assert_eq!(jsonc_unset("{ \"a\": 1 }", &["b"]).unwrap(), None);
}

#[test]
fn unset_only_member_yields_empty_object() {
   assert_eq!(jsonc_unset("{ \"a\": 1 }", &["a"]).unwrap().unwrap(), "{}");
}

#[test]
fn unset_first_member_removes_own_comma() {
   let out = jsonc_unset("{\n  \"a\": 1,\n  \"b\": 2\n}", &["a"]).unwrap().unwrap();
   assert_eq!(out, "{\n  \"b\": 2\n}");
}

#[test]
fn unset_middle_member_removes_own_comma() {
   let src = "{\n  \"a\": 1,\n  \"b\": 2,\n  \"c\": 3\n}";
   let out = jsonc_unset(src, &["b"]).unwrap().unwrap();
   assert_eq!(out, "{\n  \"a\": 1,\n  \"c\": 3\n}");
}

#[test]
fn unset_last_member_removes_previous_comma_preserving_comment() {
   // The hard case: previous member's comma is non-adjacent (after a comment).
   let src = "{ \"a\": 1, // note\n \"b\": 2 }";
   let out = jsonc_unset(src, &["b"]).unwrap().unwrap();
   assert_eq!(out, "{ \"a\": 1 // note\n }");
}

#[test]
fn unset_member_with_own_trailing_comma() {
   let src = "{\n  \"a\": 1,\n  \"b\": 2,\n}";
   let out = jsonc_unset(src, &["b"]).unwrap().unwrap();
   assert_eq!(out, "{\n  \"a\": 1,\n}");
}

#[test]
fn unset_nested_leaf_no_prune() {
   let src = "{\n  \"work\": {\n    \"env\": {\n      \"K\": \"v\"\n    }\n  }\n}";
   let out = jsonc_unset(src, &["work", "env", "K"]).unwrap().unwrap();
   // env stays as an empty object — surgical, no pruning.
   assert_eq!(out, "{\n  \"work\": {\n    \"env\": {}\n  }\n}");
}

#[test]
fn unset_preserves_unrelated_comments() {
   let src = "{\n  // keep me\n  \"a\": 1,\n  \"b\": 2\n}";
   let out = jsonc_unset(src, &["b"]).unwrap().unwrap();
   assert_eq!(out, "{\n  // keep me\n  \"a\": 1\n}");
}

#[test]
fn detect_indent_two_space() {
   assert_eq!(detect_indent_unit("{\n  \"a\": 1\n}"), "  ".to_string());
}

#[test]
fn detect_indent_four_space() {
   assert_eq!(detect_indent_unit("{\n    \"a\": 1\n}"), "    ".to_string());
}

#[test]
fn detect_indent_tab() {
   assert_eq!(detect_indent_unit("{\n\t\"a\": 1\n}"), "\t".to_string());
}

#[test]
fn detect_indent_fallback_two_space() {
   assert_eq!(detect_indent_unit("{}"), "  ".to_string());
}

#[test]
fn render_scalar_fragment_is_single_token() {
   assert_eq!(render_fragment(&JsonValue::string("node"), "  ", "    "), "\"node\"".to_string());
   assert_eq!(render_fragment(&JsonValue::Bool(true), "  ", "  "), "true".to_string());
}

#[test]
fn render_nested_fragment_uses_file_unit_and_base() {
   // unit = 4 spaces, base indent of the value = 2 spaces (one level in).
   let v = JsonValue::obj(&[("x", JsonValue::Number("1".into()))]);
   let out = render_fragment(&v, "    ", "  ");
   // to_json_pretty gives `{\n  "x": 1\n}` (2-space). Remap inner level to the
   // file unit (4) prefixed by base (2): inner line is base + unit = 6 spaces.
   assert_eq!(out, "{\n      \"x\": 1\n  }");
}

#[test]
fn set_replaces_scalar_leaf_minimal_diff() {
   let src = "{\n  // keep\n  \"a\": \"old\",\n  \"b\": 2\n}";
   let out = jsonc_set(src, &["a"], &JsonValue::string("new")).unwrap();
   assert_eq!(out, "{\n  // keep\n  \"a\": \"new\",\n  \"b\": 2\n}");
}

#[test]
fn set_replaces_number_leaf() {
   let src = "{ \"a\": 1 }";
   let out = jsonc_set(src, &["a"], &JsonValue::Number("42".into())).unwrap();
   assert_eq!(out, "{ \"a\": 42 }");
}

#[test]
fn set_replaces_nested_with_json_value_indented_to_file_unit() {
   let src = "{\n    \"perm\": null\n}"; // 4-space file
   let v = JsonValue::Array(vec![JsonValue::string("--allow-net")]);
   let out = jsonc_set(src, &["perm"], &v).unwrap();
   assert_eq!(out, "{\n    \"perm\": [\n        \"--allow-net\"\n    ]\n}");
}

#[test]
fn set_appends_into_nonempty_object_last() {
   let src = "{\n  \"a\": 1\n}";
   let out = jsonc_set(src, &["b"], &JsonValue::Number("2".into())).unwrap();
   assert_eq!(out, "{\n  \"a\": 1,\n  \"b\": 2\n}");
}

#[test]
fn set_appends_after_existing_trailing_comma() {
   let src = "{\n  \"a\": 1,\n}";
   let out = jsonc_set(src, &["b"], &JsonValue::Number("2".into())).unwrap();
   assert_eq!(out, "{\n  \"a\": 1,\n  \"b\": 2,\n}");
}

#[test]
fn set_appends_into_empty_object() {
   let src = "{\n  \"env\": {}\n}";
   let out = jsonc_set(src, &["env", "K"], &JsonValue::string("v")).unwrap();
   assert_eq!(out, "{\n  \"env\": {\n    \"K\": \"v\"\n  }\n}");
}

#[test]
fn set_creates_parent_chain_from_nothing() {
   let src = "{\n  \"defaultNewRuntime\": \"node\"\n}";
   let out = jsonc_set(src, &["presets", "work", "env", "K"], &JsonValue::string("v")).unwrap();
   assert_eq!(
      out,
      "{\n  \"defaultNewRuntime\": \"node\",\n  \"presets\": {\n    \"work\": {\n      \"env\": {\n        \"K\": \"v\"\n      }\n    }\n  }\n}"
   );
}

#[test]
fn set_into_empty_file_creates_object() {
   assert_eq!(jsonc_set("", &["a"], &JsonValue::Number("1".into())).unwrap(), "{\n  \"a\": 1\n}");
}

#[test]
fn set_into_blank_braces() {
   assert_eq!(jsonc_set("{}", &["a"], &JsonValue::Number("1".into())).unwrap(), "{\n  \"a\": 1\n}");
}

#[test]
fn roundtrip_set_then_get() {
   let src = "{\n  \"a\": 1,\n  \"work\": {\n    \"runtime\": \"node\"\n  }\n}";
   let out = jsonc_set(src, &["work", "runtimeVersion"], &JsonValue::string("22")).unwrap();
   assert_eq!(jsonc_get(&out, &["work", "runtimeVersion"]).unwrap(), Some("\"22\""));
   // unrelated value untouched
   assert_eq!(jsonc_get(&out, &["a"]).unwrap(), Some("1"));
}

#[test]
fn roundtrip_set_unset_is_identity_modulo_formatting() {
   let src = "{\n  \"a\": 1,\n  \"b\": 2\n}";
   let added = jsonc_set(src, &["c"], &JsonValue::Number("3".into())).unwrap();
   let removed = jsonc_unset(&added, &["c"]).unwrap().unwrap();
   assert!(parses_equal(src, &removed));
}

#[test]
fn roundtrip_comments_survive_mutation() {
   let src = "{\n  // leading\n  \"a\": 1, // trailing\n  \"b\": 2\n}";
   let out = jsonc_set(src, &["a"], &JsonValue::Number("9".into())).unwrap();
   assert!(out.contains("// leading"));
   assert!(out.contains("// trailing"));
   assert_eq!(jsonc_get(&out, &["a"]).unwrap(), Some("9"));
}

#[test]
fn roundtrip_dotted_terminal_key_unsplit() {
   // The caller passes terminal-map keys whole; dots in the key are fine.
   let src = "{\n  \"dep\": {}\n}";
   let out = jsonc_set(src, &["dep", "ruamel.yaml"], &JsonValue::string("^0.18")).unwrap();
   assert_eq!(jsonc_get(&out, &["dep", "ruamel.yaml"]).unwrap(), Some("\"^0.18\""));
}
