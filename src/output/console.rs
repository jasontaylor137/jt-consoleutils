//! [`ConsoleOutput`] — production [`Output`] writing to stdout/stderr.

#[cfg(feature = "verbose")]
use super::with_prefix;
#[cfg(feature = "trace")]
use super::with_trace_prefix;
use super::{DEFAULT_THEME, Output, OutputMode, RenderTheme, format_elapsed};

/// Production [`Output`] implementation that writes to stdout.
///
/// Behavior depends on the [`OutputMode`] supplied at construction:
/// - `quiet`: all methods are silent (errors still emit to stderr).
/// - `verbose`: commands, their arguments, and verbose messages are printed.
/// - default: normal progress messages are printed; verbose output is hidden.
///
/// Color rendering is determined once at construction:
/// `colors_enabled = is_terminal(stdout) && NO_COLOR is unset`.
pub struct ConsoleOutput {
   mode: OutputMode,
   colors_enabled: bool,
   theme: RenderTheme
}

impl ConsoleOutput {
   /// Create a new `ConsoleOutput` driven by `mode`.
   ///
   /// Auto-detects whether ANSI color sequences should be emitted. Uses
   /// [`DEFAULT_THEME`](crate::output::DEFAULT_THEME); call
   /// [`with_theme`](Self::with_theme) to override.
   #[must_use]
   pub fn new(mode: OutputMode) -> Self {
      let colors_enabled = Self::detect_colors();
      Self { mode, colors_enabled, theme: DEFAULT_THEME }
   }

   /// Create a `ConsoleOutput` with explicit color setting (useful for tests).
   #[must_use]
   pub const fn with_colors(mode: OutputMode, colors_enabled: bool) -> Self {
      Self { mode, colors_enabled, theme: DEFAULT_THEME }
   }

   /// Replace the [`RenderTheme`] used to format glyphs and connector words.
   ///
   /// Builder-style: `ConsoleOutput::new(mode).with_theme(ASCII_THEME)`.
   #[must_use]
   pub const fn with_theme(mut self, theme: RenderTheme) -> Self {
      self.theme = theme;
      self
   }

   fn detect_colors() -> bool {
      use std::io::IsTerminal;
      if std::env::var_os("NO_COLOR").is_some() {
         return false;
      }
      std::io::stdout().is_terminal()
   }
}

impl Output for ConsoleOutput {
   fn writeln(&mut self, line: &str) {
      if !self.mode.is_quiet() {
         println!("{line}");
      }
   }

   fn write(&mut self, text: &str) {
      if !self.mode.is_quiet() {
         use std::io::Write;
         print!("{text}");
         let _ = std::io::stdout().flush();
      }
   }

   fn eprintln(&mut self, line: &str) {
      eprintln!("{line}");
   }

   fn colors_enabled(&self) -> bool {
      self.colors_enabled
   }

   fn theme(&self) -> RenderTheme {
      self.theme
   }

   fn warn(&mut self, msg: &str) {
      if self.mode.is_quiet() {
         return;
      }
      let line = super::render::render_warn(msg, self.colors_enabled, &self.theme);
      eprintln!("{line}");
   }

   #[cfg(feature = "verbose")]
   fn is_verbose(&self) -> bool {
      self.mode.is_verbose()
   }

   #[cfg(feature = "verbose")]
   fn emit_verbose(&mut self, msg: String) {
      print!("{}", with_prefix("| ", &msg));
   }

   #[cfg(feature = "trace")]
   fn is_trace(&self) -> bool {
      self.mode.is_trace()
   }

   #[cfg(feature = "trace")]
   fn emit_trace(&mut self, msg: String) {
      print!("{}", with_trace_prefix(&msg));
   }

   #[cfg(feature = "verbose")]
   fn shell_command(&mut self, cmd: &str) {
      if self.mode.is_verbose() && !self.mode.is_quiet() {
         println!("> {cmd}");
      }
   }

   #[cfg(feature = "verbose")]
   fn shell_line(&mut self, line: &str) {
      if !self.mode.is_quiet() {
         println!("> {line}");
      }
   }

   fn step_result(&mut self, label: &str, success: bool, elapsed_ms: u128, viewport: &[String]) {
      if self.mode.is_quiet() {
         return;
      }
      let t = format_elapsed(elapsed_ms);
      let glyph = if success { self.theme.success_glyph } else { self.theme.error_glyph };
      let (header, viewport_line): (String, fn(&str) -> String) = match (success, self.colors_enabled) {
         (true, true) => (format!("\x1b[32m{glyph}\x1b[0m {label} \x1b[2m({t})\x1b[0m"), |l| format!("  {l}")),
         (true, false) => (format!("{glyph} {label} ({t})"), |l| format!("  {l}")),
         (false, true) => {
            (format!("\x1b[31m{glyph}\x1b[0m {label} \x1b[2m({t})\x1b[0m"), |l| format!("  \x1b[31m{l}\x1b[0m"))
         }
         (false, false) => (format!("{glyph} {label} ({t})"), |l| format!("  {l}"))
      };
      println!("{header}");
      if !success {
         for line in viewport {
            println!("{}", viewport_line(line));
         }
      }
   }

   fn dry_run_shell(&mut self, cmd: &str) {
      if self.mode.is_dry_run() {
         println!("[dry-run] would run: {cmd}");
      }
   }

   fn dry_run_write(&mut self, path: &str) {
      if self.mode.is_dry_run() {
         println!("[dry-run] would write: {path}");
      }
   }

   fn dry_run_delete(&mut self, path: &str) {
      if self.mode.is_dry_run() {
         println!("[dry-run] would delete: {path}");
      }
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn console_output_with_colors_disabled() {
      let out = ConsoleOutput::with_colors(OutputMode::default(), false);
      assert!(!out.colors_enabled());
   }

   #[test]
   fn console_output_with_colors_enabled() {
      let out = ConsoleOutput::with_colors(OutputMode::default(), true);
      assert!(out.colors_enabled());
   }
}
