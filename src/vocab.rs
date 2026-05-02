//! Marker traits for binary-supplied output vocabulary.
//!
//! Binaries that use this crate may define their own `Verb` / `Noun` enums and
//! implement [`AsVerb`](crate::vocab::AsVerb) / [`AsNoun`](crate::vocab::AsNoun) on them.
//! The output methods that take these types accept `impl AsVerb` / `impl AsNoun`,
//! so call sites read as `out.action(Verb::Edited, "x")` without `.as_str()` plumbing.
//!
//! For binaries that want a small declarative `Verb` / `Noun` enum derived
//! directly from variant identifiers, see the [`verb_enum!`](crate::verb_enum)
//! and [`noun_enum!`](crate::noun_enum) macros.

/// Convertible to a verb string for action rendering.
///
/// Pre-implemented for `&str` and `String` so call sites can pass raw strings
/// when no project-specific enum is desired.
pub trait AsVerb {
   /// Borrow self as a verb string.
   fn as_verb(&self) -> &str;
}

impl AsVerb for &str {
   fn as_verb(&self) -> &str {
      self
   }
}

impl AsVerb for String {
   fn as_verb(&self) -> &str {
      self.as_str()
   }
}

/// Convertible to singular and plural noun forms for count rendering.
pub trait AsNoun {
   /// Singular form, e.g. `"dep"`.
   fn singular(&self) -> &str;
   /// Plural form, e.g. `"deps"`.
   fn plural(&self) -> &str;
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn str_implements_as_verb() {
      // Given
      let v: &str = "Edited";

      // When
      let s = v.as_verb();

      // Then
      assert_eq!(s, "Edited");
   }

   #[test]
   fn string_implements_as_verb() {
      // Given
      let v: String = String::from("Installed");

      // When
      let s = v.as_verb();

      // Then
      assert_eq!(s, "Installed");
   }

   struct Dep;
   impl AsNoun for Dep {
      fn singular(&self) -> &str {
         "dep"
      }
      fn plural(&self) -> &str {
         "deps"
      }
   }

   #[test]
   fn as_noun_returns_singular_and_plural() {
      // Given
      let d = Dep;

      // When / Then
      assert_eq!(d.singular(), "dep");
      assert_eq!(d.plural(), "deps");
   }
}
