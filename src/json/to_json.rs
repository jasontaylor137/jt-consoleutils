use super::escape::push_json_string;

/// Trait for types that can serialize themselves to a pretty JSON string.
/// This is the typed counterpart to `serialize::to_json_pretty` which operates
/// on `JsonValue`. Implementing `ToJson` allows structs to skip the intermediate
/// `JsonValue` representation when all we need is the output string.
pub trait ToJson {
   /// Serialize `self` directly to a pretty-printed JSON string.
   fn to_json_pretty(&self) -> String;
}

// ---------------------------------------------------------------------------
// Helper: struct serializer that builds a pretty JSON object directly
// ---------------------------------------------------------------------------

/// Builder for serializing a struct as a pretty JSON object.
/// Handles 2-space indentation and trailing-comma-free output.
pub struct StructSerializer {
   out: String,
   field_count: usize,
   indent: usize
}

impl StructSerializer {
   /// Create a new serializer starting an object (`{`).
   pub fn new() -> Self {
      StructSerializer { out: String::from("{\n"), field_count: 0, indent: 1 }
   }

   /// Write a string field (always present).
   pub fn field_str(&mut self, key: &str, value: &str) {
      self.comma();
      self.push_indent();
      push_json_string(&mut self.out, key);
      self.out.push_str(": ");
      push_json_string(&mut self.out, value);
   }

   /// Write an optional string field (skipped when None).
   pub fn field_opt_str(&mut self, key: &str, value: &Option<String>) {
      if let Some(v) = value {
         self.field_str(key, v);
      }
   }

   /// Finish and return the JSON string.
   pub fn finish(mut self) -> String {
      self.out.push_str("\n}");
      self.out
   }

   fn comma(&mut self) {
      if self.field_count > 0 {
         self.out.push_str(",\n");
      }
      self.field_count += 1;
   }

   fn push_indent(&mut self) {
      for _ in 0..self.indent {
         self.out.push_str("  ");
      }
   }
}

