use std::path::Path;

/// Convert a `Path` to a `String` via `display()`.
pub fn path_to_string(path: &Path) -> String {
   path.display().to_string()
}

/// Returns `""` when `n == 1`, otherwise `"s"`.
pub fn plural(n: usize) -> &'static str {
   if n == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn path_to_string_converts_path() {
      // Given
      let path = Path::new("/some/path/file.ts");

      // When
      let s = path_to_string(path);

      // Then
      assert_eq!(s, "/some/path/file.ts");
   }

   #[test]
   fn plural_singular() {
      assert_eq!(plural(1), "");
   }

   #[test]
   fn plural_plural() {
      assert_eq!(plural(2), "s");
   }

   #[test]
   fn plural_zero() {
      assert_eq!(plural(0), "s");
   }
}
