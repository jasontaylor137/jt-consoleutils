use std::path::Path;

/// Format a byte count as a human-readable string with one decimal place.
/// Examples: `"0 B"`, `"512 B"`, `"1.0 KB"`, `"3.5 MB"`, `"1.2 GB"`.
///
/// Uses integer arithmetic to avoid pulling in the ~5 KB f64 Display
/// formatting machinery from `core::fmt::float`.
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
   const KB: u64 = 1024;
   const MB: u64 = 1024 * KB;
   const GB: u64 = 1024 * MB;

   if bytes >= GB {
      let tenths = (bytes * 10 + GB / 2) / GB;
      format!("{}.{} GB", tenths / 10, tenths % 10)
   } else if bytes >= MB {
      let tenths = (bytes * 10 + MB / 2) / MB;
      format!("{}.{} MB", tenths / 10, tenths % 10)
   } else if bytes >= KB {
      let tenths = (bytes * 10 + KB / 2) / KB;
      format!("{}.{} KB", tenths / 10, tenths % 10)
   } else {
      format!("{bytes} B")
   }
}

/// Convert a `Path` to a `String` via `display()`.
#[must_use]
pub fn path_to_string(path: &Path) -> String {
   path.display().to_string()
}

/// Returns `""` when `n == 1`, otherwise `"s"`.
#[must_use]
pub const fn plural(n: usize) -> &'static str {
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

   // -------------------------------------------------------------------------
   // format_bytes
   // -------------------------------------------------------------------------

   #[test]
   fn format_bytes_bytes() {
      assert_eq!(format_bytes(0), "0 B");
      assert_eq!(format_bytes(512), "512 B");
      assert_eq!(format_bytes(1023), "1023 B");
   }

   #[test]
   fn format_bytes_kilobytes() {
      assert_eq!(format_bytes(1024), "1.0 KB");
      assert_eq!(format_bytes(1536), "1.5 KB");
   }

   #[test]
   fn format_bytes_megabytes() {
      assert_eq!(format_bytes(1_048_576), "1.0 MB");
      assert_eq!(format_bytes(1_572_864), "1.5 MB");
   }

   #[test]
   fn format_bytes_gigabytes() {
      assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
      assert_eq!(format_bytes(1_610_612_736), "1.5 GB");
   }

   // -------------------------------------------------------------------------
   // plural
   // -------------------------------------------------------------------------

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
