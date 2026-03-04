use terminal_size::{Width, terminal_size};

/// Returns the current terminal width in columns, or 80 if it cannot be determined.
pub fn terminal_width() -> usize {
   terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80)
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn terminal_width_returns_positive() {
      assert!(terminal_width() > 0);
   }
}
