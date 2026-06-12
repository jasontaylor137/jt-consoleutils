//! Insertion-ordered object map backing [`JsonValue::Object`].

use super::value::JsonValue;

/// Insertion-ordered map for [`JsonValue::Object`].
///
/// Backed by a `Vec` of pairs rather than a `BTreeMap`: JSON objects handled
/// here are small config maps where linear lookup wins, source key order
/// survives parse → serialize round-trips, and the binary drops the
/// `BTreeMap<String, JsonValue>` node/balancing machinery (~6 KiB).
///
/// `insert` keeps an existing key's position and replaces its value
/// (last-wins), so duplicate keys cannot occur.
#[derive(Debug, Clone, Default)]
pub struct JsonMap(Vec<(String, JsonValue)>);

impl JsonMap {
   /// Create an empty map.
   #[must_use]
   pub fn new() -> Self {
      Self(Vec::new())
   }

   fn position(&self, key: &str) -> Option<usize> {
      self.0.iter().position(|(k, _)| k == key)
   }

   /// Get the value for `key`.
   #[must_use]
   pub fn get(&self, key: &str) -> Option<&JsonValue> {
      self.position(key).map(|i| &self.0[i].1)
   }

   /// Get a mutable reference to the value for `key`.
   pub fn get_mut(&mut self, key: &str) -> Option<&mut JsonValue> {
      self.position(key).map(|i| &mut self.0[i].1)
   }

   /// Insert `value` under `key`, returning the previous value if present.
   /// An existing key keeps its position; a new key appends at the end.
   pub fn insert(&mut self, key: String, value: JsonValue) -> Option<JsonValue> {
      match self.position(&key) {
         Some(i) => Some(std::mem::replace(&mut self.0[i].1, value)),
         None => {
            self.0.push((key, value));
            None
         }
      }
   }

   /// Remove `key`, returning its value if present. Preserves the order of
   /// the remaining entries.
   pub fn remove(&mut self, key: &str) -> Option<JsonValue> {
      self.position(key).map(|i| self.0.remove(i).1)
   }

   /// Mutable reference to the value at `key`, inserting `default` first when
   /// the key is absent (the `entry(key).or_insert(default)` shape).
   pub fn or_insert(&mut self, key: &str, default: JsonValue) -> &mut JsonValue {
      let i = match self.position(key) {
         Some(i) => i,
         None => {
            self.0.push((key.to_string(), default));
            self.0.len() - 1
         }
      };
      &mut self.0[i].1
   }

   /// Return `true` if `key` is present.
   #[must_use]
   pub fn contains_key(&self, key: &str) -> bool {
      self.position(key).is_some()
   }

   /// Number of entries.
   #[must_use]
   pub fn len(&self) -> usize {
      self.0.len()
   }

   /// Return `true` if the map has no entries.
   #[must_use]
   pub fn is_empty(&self) -> bool {
      self.0.is_empty()
   }

   /// Iterate entries in insertion order.
   pub fn iter(&self) -> std::slice::Iter<'_, (String, JsonValue)> {
      self.0.iter()
   }

   /// Iterate keys in insertion order.
   pub fn keys(&self) -> impl Iterator<Item = &String> {
      self.0.iter().map(|(k, _)| k)
   }
}

/// Key order is ignored — `{"a":1,"b":2}` equals `{"b":2,"a":1}` — matching
/// JSON object semantics (and the canonical-order equality of the `BTreeMap`
/// this replaced).
impl PartialEq for JsonMap {
   fn eq(&self, other: &Self) -> bool {
      self.0.len() == other.0.len() && self.0.iter().all(|(k, v)| other.get(k) == Some(v))
   }
}

/// Index by key; missing keys return [`JsonValue::Null`] (matching
/// `JsonValue`'s `Index<&str>` behavior).
impl std::ops::Index<&str> for JsonMap {
   type Output = JsonValue;

   fn index(&self, key: &str) -> &JsonValue {
      static NULL: JsonValue = JsonValue::Null;
      self.get(key).unwrap_or(&NULL)
   }
}

impl<'a> IntoIterator for &'a JsonMap {
   type Item = &'a (String, JsonValue);
   type IntoIter = std::slice::Iter<'a, (String, JsonValue)>;

   fn into_iter(self) -> Self::IntoIter {
      self.0.iter()
   }
}

impl IntoIterator for JsonMap {
   type Item = (String, JsonValue);
   type IntoIter = std::vec::IntoIter<(String, JsonValue)>;

   fn into_iter(self) -> Self::IntoIter {
      self.0.into_iter()
   }
}

impl FromIterator<(String, JsonValue)> for JsonMap {
   fn from_iter<I: IntoIterator<Item = (String, JsonValue)>>(iter: I) -> Self {
      let mut map = Self::new();
      for (k, v) in iter {
         map.insert(k, v);
      }
      map
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn insert_preserves_first_position_and_replaces_value() {
      let mut map = JsonMap::new();
      map.insert("a".into(), JsonValue::Bool(true));
      map.insert("b".into(), JsonValue::Bool(true));
      map.insert("a".into(), JsonValue::Bool(false));

      let keys: Vec<&String> = map.keys().collect();

      assert_eq!(keys, ["a", "b"]);
      assert_eq!(map.get("a"), Some(&JsonValue::Bool(false)));
   }

   #[test]
   fn iteration_follows_insertion_order_not_key_order() {
      let mut map = JsonMap::new();
      map.insert("z".into(), JsonValue::Null);
      map.insert("a".into(), JsonValue::Null);

      let keys: Vec<&String> = map.keys().collect();

      assert_eq!(keys, ["z", "a"]);
   }

   #[test]
   fn equality_ignores_key_order() {
      let mut ab = JsonMap::new();
      ab.insert("a".into(), JsonValue::Bool(true));
      ab.insert("b".into(), JsonValue::Null);
      let mut ba = JsonMap::new();
      ba.insert("b".into(), JsonValue::Null);
      ba.insert("a".into(), JsonValue::Bool(true));

      assert_eq!(ab, ba);
   }

   #[test]
   fn remove_returns_value_and_keeps_remaining_order() {
      let mut map = JsonMap::new();
      map.insert("a".into(), JsonValue::Bool(true));
      map.insert("b".into(), JsonValue::Null);
      map.insert("c".into(), JsonValue::Bool(false));

      let removed = map.remove("b");

      assert_eq!(removed, Some(JsonValue::Null));
      let keys: Vec<&String> = map.keys().collect();
      assert_eq!(keys, ["a", "c"]);
   }

   #[test]
   fn or_insert_returns_existing_or_inserts_default() {
      let mut map = JsonMap::new();
      map.insert("a".into(), JsonValue::Bool(true));

      assert_eq!(*map.or_insert("a", JsonValue::Null), JsonValue::Bool(true));
      assert_eq!(*map.or_insert("b", JsonValue::Null), JsonValue::Null);
      assert_eq!(map.len(), 2);
   }
}
