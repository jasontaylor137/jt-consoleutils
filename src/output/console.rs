//! [`ConsoleOutput`] — production [`Output`] writing to stdout/stderr.

use super::{DEFAULT_THEME, Output, OutputMode, RenderTheme, format_elapsed};
#[cfg(any(feature = "verbose", feature = "trace"))]
use super::{Dim, with_prefix};

/// Production [`Output`] implementation that writes to stdout.
///
/// Behavior depends on the [`OutputMode`] supplied at construction:
/// - `quiet`: all methods are silent (errors still emit to stderr).
/// - `verbose`: commands, their arguments, and verbose messages are printed.
/// - default: normal progress messages are printed; verbose output is hidden.
///
/// Color rendering is determined once at construction following the
/// [bixense CLICOLOR spec](https://bixense.com/clicolors/) and the
/// [NO_COLOR](https://no-color.org/) convention. Precedence (highest first):
///
/// 1. `NO_COLOR` set to any value (including empty) → colors off.
/// 2. `CLICOLOR_FORCE` or `FORCE_COLOR` set to a non-zero, non-empty value → colors on.
/// 3. `CLICOLOR=0` → colors off.
/// 4. Otherwise: on iff stdout is a TTY.
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
      detect_colors_with(
         |name| std::env::var_os(name).map(|v| v.to_string_lossy().into_owned()),
         || std::io::stdout().is_terminal()
      )
   }
}

/// Pure color-detection logic. `env` returns the value of an env var (or `None`
/// if unset); `is_tty` reports whether stdout is a terminal. See
/// [`ConsoleOutput`] for precedence rules.
fn detect_colors_with(env: impl Fn(&str) -> Option<String>, is_tty: impl FnOnce() -> bool) -> bool {
   if env("NO_COLOR").is_some() {
      return false;
   }
   if is_force(env("CLICOLOR_FORCE").as_deref()) || is_force(env("FORCE_COLOR").as_deref()) {
      return true;
   }
   if env("CLICOLOR").as_deref() == Some("0") {
      return false;
   }
   is_tty()
}

/// Whether a `CLICOLOR_FORCE`/`FORCE_COLOR` value should force colors on.
/// Empty string and `"0"` mean "not forced"; any other value forces.
fn is_force(val: Option<&str>) -> bool {
   matches!(val, Some(v) if !v.is_empty() && v != "0")
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
      print!("{}", with_prefix("| ", &msg, Dim::No));
   }

   #[cfg(feature = "trace")]
   fn is_trace(&self) -> bool {
      self.mode.is_trace()
   }

   #[cfg(feature = "trace")]
   fn emit_trace(&mut self, msg: String) {
      print!("{}", with_prefix("· ", &msg, Dim::Yes));
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

   fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
      move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| (*v).to_string())
   }

   #[test]
   fn no_color_disables_even_with_force_and_tty() {
      assert!(!detect_colors_with(env_from(&[("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")]), || true));
   }

   #[test]
   fn no_color_empty_value_still_disables() {
      assert!(!detect_colors_with(env_from(&[("NO_COLOR", "")]), || true));
   }

   #[test]
   fn clicolor_force_enables_on_non_tty() {
      assert!(detect_colors_with(env_from(&[("CLICOLOR_FORCE", "1")]), || false));
   }

   #[test]
   fn force_color_enables_on_non_tty() {
      assert!(detect_colors_with(env_from(&[("FORCE_COLOR", "1")]), || false));
   }

   #[test]
   fn clicolor_force_zero_does_not_force() {
      assert!(!detect_colors_with(env_from(&[("CLICOLOR_FORCE", "0")]), || false));
   }

   #[test]
   fn force_color_empty_does_not_force() {
      assert!(!detect_colors_with(env_from(&[("FORCE_COLOR", "")]), || false));
   }

   #[test]
   fn clicolor_zero_disables_on_tty() {
      assert!(!detect_colors_with(env_from(&[("CLICOLOR", "0")]), || true));
   }

   #[test]
   fn clicolor_one_falls_through_to_tty() {
      assert!(detect_colors_with(env_from(&[("CLICOLOR", "1")]), || true));
      assert!(!detect_colors_with(env_from(&[("CLICOLOR", "1")]), || false));
   }

   #[test]
   fn no_env_falls_through_to_tty() {
      assert!(detect_colors_with(env_from(&[]), || true));
      assert!(!detect_colors_with(env_from(&[]), || false));
   }

   #[test]
   fn force_beats_clicolor_zero() {
      assert!(detect_colors_with(env_from(&[("CLICOLOR", "0"), ("FORCE_COLOR", "1")]), || false));
   }
}
