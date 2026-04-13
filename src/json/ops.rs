use super::value::JsonValue;

/// Deep-merge `overlay` into `base`. Objects are merged recursively;
/// all other value types in `overlay` replace the corresponding key in `base`.
pub fn json_deep_merge(base: &mut JsonValue, overlay: &JsonValue) {
   match (base, overlay) {
      (JsonValue::Object(base_map), JsonValue::Object(overlay_map)) => {
         for (key, overlay_val) in overlay_map {
            let entry = base_map.entry(key.clone()).or_insert(JsonValue::Null);
            json_deep_merge(entry, overlay_val);
         }
      }
      (base, overlay) => {
         *base = overlay.clone();
      }
   }
}

/// Remove a set of key paths from a JSON value.
/// Each path is an array of segments representing nested keys.
/// After removing a leaf, prunes any parent objects left empty.
pub fn json_remove_paths(root: &mut JsonValue, paths: &[&[&str]]) {
   for parts in paths {
      remove_path_recursive(root, parts);
   }
}

fn remove_path_recursive(val: &mut JsonValue, parts: &[&str]) -> bool {
   if parts.is_empty() {
      return false;
   }
   let map = match val.as_object_mut() {
      Some(m) => m,
      None => return false
   };
   if parts.len() == 1 {
      map.remove(parts[0]);
   } else if let Some(child) = map.get_mut(parts[0]) {
      let child_empty = remove_path_recursive(child, &parts[1..]);
      if child_empty {
         map.remove(parts[0]);
      }
   }
   map.is_empty()
}
