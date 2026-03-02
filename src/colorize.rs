//! Helpers to colorize text with a left-to-right rainbow.
//!
//! Provides:
//! - `colorize_text_with_width(text, Option<usize>) -> String`
//!
//! The functions emit 24-bit ANSI foreground escapes (`\x1b[38;2;R;G;Bm`).
//! If `rainbow_width` is `None` the rainbow spans the widest line in `text`.
//! The output contains newline characters and is ready to print.

use crate::colors::RESET;

/// Convert HSV (h in degrees 0..360, s and v in 0..1) to RGB bytes (0..255).
fn hsv_to_rgb(h_deg: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = (h_deg % 360.0 + 360.0) % 360.0;
    let c = v * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let (r1, g1, b1) = if (0.0..1.0).contains(&h_prime) {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = v - c;
    let r = ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (r, g, b)
}

/// Create an ANSI 24-bit foreground escape for the given RGB bytes.
fn ansi_rgb_escape(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{};{};{}m", r, g, b)
}

/// Colorize the provided `text` with a left-to-right rainbow. If `rainbow_width`
/// is `None` the rainbow will span the full width of the widest line. If a
/// width is provided the gradient is computed across that width and repeats
/// when lines are longer.
///
/// The returned String contains newlines and ANSI escapes; print it directly.
pub fn colorize_text_with_width(text: &str, rainbow_width: Option<usize>) -> String {
    // Split into lines and characters so we can index columns.
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let char_lines: Vec<Vec<char>> = lines.iter().map(|l| l.chars().collect()).collect();

    let max_width = char_lines.iter().map(|l| l.len()).max().unwrap_or(0);
    if max_width == 0 {
        // Nothing to color; return the original text unchanged.
        return text.to_string();
    }

    // Determine rainbow width: span the text if None.
    let rainbow_w = rainbow_width.unwrap_or(max_width).max(1);

    // Build a palette of per-column colors across rainbow_w.
    let mut col_colors: Vec<String> = Vec::with_capacity(rainbow_w);
    for col in 0..rainbow_w {
        let t = (col as f32) / (rainbow_w.max(1) as f32); // 0..1
        // Map to hue (degrees). Phase can be tweaked if desired.
        let hue_deg = (t * 360.0) % 360.0;
        // Use fairly high saturation/value for vivid output; these can be tuned.
        let (r, g, b) = hsv_to_rgb(hue_deg, 0.5_f32.min(1.0), 0.99_f32.min(1.0));
        col_colors.push(ansi_rgb_escape(r, g, b));
    }

    let mut result = String::new();
    for line in &char_lines {
        let mut col = 0usize;
        while col < line.len() {
            result.push_str(&col_colors[col % rainbow_w]);
            result.push(line[col]);
            col += 1;
        }
        result.push('\n');
    }

    result.push_str(RESET);
    result
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
