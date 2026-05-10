//! Helpers to colorize text with a left-to-right rainbow.
//!
//! Provides `colorize_text_with_width(text, Option<usize>) -> String`.
//!
//! If `rainbow_width` is `None` the rainbow spans the widest line in `text`.
//! The output contains newline characters and ANSI escapes; print it directly.
//!
//! The functions emit 24-bit ANSI foreground escapes (`\x1b[38;2;R;G;Bm`).

use std::fmt::Write as _;

use super::colors::RESET;

/// Convert HSV to RGB. Hue is in degrees (any real value — wrapped into
/// `[0, 360)`); saturation and value are in `[0, 1]`. Returns RGB bytes in
/// `[0, 255]`. Useful for emitting 24-bit ANSI foreground escapes
/// (`\x1b[38;2;R;G;Bm`) — see [`colorize_text_with_width`] for an example.
#[must_use]
#[allow(clippy::many_single_char_names, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn hsv_to_rgb(h_deg: f32, s: f32, v: f32) -> (u8, u8, u8) {
   let h_prime = h_deg.rem_euclid(360.0) / 60.0;
   let c = v * s;
   let x = c * (1.0 - (h_prime.rem_euclid(2.0) - 1.0).abs());
   let (r1, g1, b1) = match h_prime as u32 {
      0 => (c, x, 0.0),
      1 => (x, c, 0.0),
      2 => (0.0, c, x),
      3 => (0.0, x, c),
      4 => (x, 0.0, c),
      _ => (c, 0.0, x)
   };
   let m = v - c;
   let to_byte = |chan: f32| ((chan + m) * 255.0).round().clamp(0.0, 255.0) as u8;
   (to_byte(r1), to_byte(g1), to_byte(b1))
}

/// Colorize the provided `text` with a left-to-right rainbow.
///
/// If `rainbow_width` is `None` the rainbow will span the full width of the
/// widest line. If a width is provided the gradient is computed across that
/// width and repeats when lines are longer.
///
/// The returned `String` contains newlines and ANSI escapes; print it directly.
#[must_use]
pub fn colorize_text_with_width(text: &str, rainbow_width: Option<usize>) -> String {
   let max_chars = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
   if max_chars == 0 {
      return text.to_string();
   }
   let rainbow_w = rainbow_width.unwrap_or(max_chars).max(1);

   // Per-column RGB palette (12 bytes/entry, no heap-allocated escapes).
   #[allow(clippy::cast_precision_loss)]
   let palette: Vec<(u8, u8, u8)> =
      (0..rainbow_w).map(|col| hsv_to_rgb((col as f32) / (rainbow_w as f32) * 360.0, 0.5, 0.99)).collect();

   // ANSI 24-bit escape is up to 19 bytes; reserve generously to avoid regrowth.
   let mut out = String::with_capacity(text.len() * 20 + RESET.len());
   for line in text.lines() {
      for (col, ch) in line.chars().enumerate() {
         let (r, g, b) = palette[col % rainbow_w];
         let _ = write!(out, "\x1b[38;2;{r};{g};{b}m");
         out.push(ch);
      }
      out.push('\n');
   }
   out.push_str(RESET);
   out
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn empty_text_returns_unchanged() {
      assert_eq!(colorize_text_with_width("", Some(80)), "");
   }

   #[test]
   fn single_line_contains_ansi_escape() {
      let output = colorize_text_with_width("hello", Some(80));
      assert!(output.contains("\x1b[38;2;"));
   }

   #[test]
   fn output_ends_with_reset() {
      let output = colorize_text_with_width("hello", Some(80));
      assert!(output.ends_with("\x1b[0m"));
   }

   #[test]
   fn explicit_width_repeats_gradient() {
      // A line much longer than rainbow_width should not panic.
      let long_line = "a".repeat(200);
      let output = colorize_text_with_width(&long_line, Some(10));
      assert!(output.contains("\x1b[38;2;"));
      assert!(output.ends_with("\x1b[0m"));
   }

   #[test]
   fn none_width_uses_max_line_width() {
      let text = "hello world";
      let explicit = colorize_text_with_width(text, Some(text.len()));
      let implicit = colorize_text_with_width(text, None);
      assert_eq!(explicit, implicit);
   }
}
