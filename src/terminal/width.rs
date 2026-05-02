use terminal_size::{Width, terminal_size};

/// Returns the current terminal width in columns, or 80 if it cannot be determined.
#[must_use]
pub fn terminal_width() -> usize {
   terminal_size().map_or(80, |(Width(w), _)| w as usize)
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn terminal_width_returns_positive() {
      assert!(terminal_width() > 0);
   }
}
