//! Small string and path formatting helpers used across CLI output.
//!
//! These are deliberately allocation-free where possible (`plural` returns a
//! `&'static str`) and avoid pulling in `f64`-formatting machinery (`format_bytes`
//! uses integer arithmetic).

use std::path::Path;

/// Format a byte count as a human-readable string with one decimal place,
/// using **IEC binary units** (powers of 1024) and matching suffixes:
/// `"0 B"`, `"512 B"`, `"1.0 KiB"`, `"3.5 MiB"`, `"1.2 GiB"`.
///
/// Matches the convention used by `du -h`, BSD tools, and most modern
/// Linux/Unix utilities. For decimal/SI units (powers of 1000, "KB"/"MB"/"GB"),
/// see [`format_bytes_si`].
///
/// Uses integer arithmetic to avoid pulling in the ~5 KB f64 Display
/// formatting machinery from `core::fmt::float`.
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
   const KIB: u64 = 1024;
   const MIB: u64 = 1024 * KIB;
   const GIB: u64 = 1024 * MIB;

   if bytes >= GIB {
      let tenths = (bytes * 10 + GIB / 2) / GIB;
      format!("{}.{} GiB", tenths / 10, tenths % 10)
   } else if bytes >= MIB {
      let tenths = (bytes * 10 + MIB / 2) / MIB;
      format!("{}.{} MiB", tenths / 10, tenths % 10)
   } else if bytes >= KIB {
      let tenths = (bytes * 10 + KIB / 2) / KIB;
      format!("{}.{} KiB", tenths / 10, tenths % 10)
   } else {
      format!("{bytes} B")
   }
}

/// Format a byte count as a human-readable string with one decimal place,
/// using **decimal/SI units** (powers of 1000) and matching suffixes:
/// `"0 B"`, `"512 B"`, `"1.0 KB"`, `"3.5 MB"`, `"1.2 GB"`.
///
/// Matches the convention used by disk-drive marketing, network speeds,
/// and `ls -h --si`. For binary/IEC units, see [`format_bytes`].
#[must_use]
pub fn format_bytes_si(bytes: u64) -> String {
   const KB: u64 = 1000;
   const MB: u64 = 1000 * KB;
   const GB: u64 = 1000 * MB;

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

/// Convert a `Path` to a `String` via [`Path::display`].
///
/// One-line wrapper kept deliberately. It marks every site that performs a
/// **lossy, display-only** conversion (error messages, dry-run logs, JSON
/// metadata fields) — distinct from `path.to_str().ok_or(…)` for sites that
/// must preserve byte-for-byte fidelity. Greppable: searching for
/// `path_to_string` finds all the "this is OK to mangle non-UTF-8" sites in
/// one pass; an open-coded `.display().to_string()` blends in with all other
/// `to_string` calls.
///
/// Use this at every user-facing display site; use `to_string_lossy` or
/// `to_str()` directly only when the result is fed back to the filesystem.
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
   fn format_bytes_kibibytes() {
      assert_eq!(format_bytes(1024), "1.0 KiB");
      assert_eq!(format_bytes(1536), "1.5 KiB");
   }

   #[test]
   fn format_bytes_mebibytes() {
      assert_eq!(format_bytes(1_048_576), "1.0 MiB");
      assert_eq!(format_bytes(1_572_864), "1.5 MiB");
   }

   #[test]
   fn format_bytes_gibibytes() {
      assert_eq!(format_bytes(1_073_741_824), "1.0 GiB");
      assert_eq!(format_bytes(1_610_612_736), "1.5 GiB");
   }

   // -------------------------------------------------------------------------
   // format_bytes_si
   // -------------------------------------------------------------------------

   #[test]
   fn format_bytes_si_bytes() {
      assert_eq!(format_bytes_si(0), "0 B");
      assert_eq!(format_bytes_si(512), "512 B");
      assert_eq!(format_bytes_si(999), "999 B");
   }

   #[test]
   fn format_bytes_si_kilobytes() {
      assert_eq!(format_bytes_si(1000), "1.0 KB");
      assert_eq!(format_bytes_si(1500), "1.5 KB");
   }

   #[test]
   fn format_bytes_si_megabytes() {
      assert_eq!(format_bytes_si(1_000_000), "1.0 MB");
      assert_eq!(format_bytes_si(1_500_000), "1.5 MB");
   }

   #[test]
   fn format_bytes_si_gigabytes() {
      assert_eq!(format_bytes_si(1_000_000_000), "1.0 GB");
      assert_eq!(format_bytes_si(1_500_000_000), "1.5 GB");
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
