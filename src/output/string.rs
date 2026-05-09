//! [`StringOutput`] — in-memory [`Output`] for tests.

use std::fmt::Write as _;

#[cfg(any(feature = "verbose", feature = "trace"))]
use super::with_prefix;
use super::{DEFAULT_THEME, Output, RenderTheme, format_elapsed};

/// In-memory [`Output`] implementation for use in tests.
///
/// All output is appended to an internal `String`. Call [`StringOutput::log`]
/// to retrieve the full captured output and assert on it.
///
/// `is_verbose()` and `is_trace()` both return `true` so that verbose and trace
/// messages are always captured, allowing tests to assert on their content.
///
/// ```rust
/// use jt_consoleutils::output::{Output, StringOutput};
///
/// let mut out = StringOutput::new();
/// out.writeln("hello");
/// assert_eq!(out.log(), "hello\n");
/// ```
pub struct StringOutput {
   buf: String,
   err_buf: String,
   theme: RenderTheme
}

impl StringOutput {
   /// Create a new, empty `StringOutput` using [`DEFAULT_THEME`](crate::output::DEFAULT_THEME).
   #[must_use]
   pub const fn new() -> Self {
      Self { buf: String::new(), err_buf: String::new(), theme: DEFAULT_THEME }
   }

   /// Replace the [`RenderTheme`] used to format glyphs and connector words.
   ///
   /// Builder-style: `StringOutput::new().with_theme(ASCII_THEME)`.
   #[must_use]
   pub const fn with_theme(mut self, theme: RenderTheme) -> Self {
      self.theme = theme;
      self
   }

   /// Return the full captured stdout output as a string slice.
   #[must_use]
   pub fn log(&self) -> &str {
      &self.buf
   }

   /// Return the full captured stderr output as a string slice.
   #[must_use]
   pub fn err_log(&self) -> &str {
      &self.err_buf
   }
}

impl Default for StringOutput {
   fn default() -> Self {
      Self::new()
   }
}

impl Output for StringOutput {
   fn writeln(&mut self, line: &str) {
      self.buf.push_str(line);
      self.buf.push('\n');
   }

   fn write(&mut self, text: &str) {
      self.buf.push_str(text);
   }

   fn eprintln(&mut self, line: &str) {
      self.err_buf.push_str(line);
      self.err_buf.push('\n');
   }

   fn theme(&self) -> RenderTheme {
      self.theme
   }

   #[cfg(feature = "verbose")]
   fn is_verbose(&self) -> bool {
      true
   }

   #[cfg(feature = "verbose")]
   fn emit_verbose(&mut self, msg: String) {
      self.buf.push_str(&with_prefix("| ", &msg));
   }

   #[cfg(feature = "trace")]
   fn is_trace(&self) -> bool {
      true
   }

   #[cfg(feature = "trace")]
   fn emit_trace(&mut self, msg: String) {
      self.buf.push_str(&with_prefix("· ", &msg));
   }

   #[cfg(feature = "verbose")]
   fn shell_command(&mut self, cmd: &str) {
      self.buf.push_str(&with_prefix("> ", cmd));
   }

   #[cfg(feature = "verbose")]
   fn shell_line(&mut self, line: &str) {
      self.buf.push_str(&with_prefix("> ", line));
   }

   fn step_result(&mut self, label: &str, success: bool, elapsed_ms: u128, _viewport: &[String]) {
      let symbol = if success { self.theme.success_glyph } else { self.theme.error_glyph };
      let _ = writeln!(self.buf, "{symbol} {label} ({})", format_elapsed(elapsed_ms));
   }

   fn dry_run_shell(&mut self, cmd: &str) {
      let _ = writeln!(self.buf, "[dry-run] would run: {cmd}");
   }

   fn dry_run_write(&mut self, path: &str) {
      let _ = writeln!(self.buf, "[dry-run] would write: {path}");
   }

   fn dry_run_delete(&mut self, path: &str) {
      let _ = writeln!(self.buf, "[dry-run] would delete: {path}");
   }
}

#[cfg(test)]
mod tests {
   use rstest::rstest;

   use super::*;
   use crate::output::render::OutputAction;

   #[test]
   fn string_output_captures_lines() {
      let mut out = StringOutput::new();
      out.writeln("hello");
      out.writeln("world");
      assert_eq!(out.log(), "hello\nworld\n");
   }

   #[test]
   fn string_output_write_no_newline() {
      let mut out = StringOutput::new();
      out.write("a");
      out.write("b");
      assert_eq!(out.log(), "ab");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_captures_verbose() {
      let mut out = StringOutput::new();
      out.emit_verbose("debug info".to_string());
      assert_eq!(out.log(), "| debug info\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_verbose_multiline() {
      let mut out = StringOutput::new();
      out.emit_verbose("line one\nline two".to_string());
      assert_eq!(out.log(), "| line one\n| line two\n");
   }

   #[cfg(feature = "trace")]
   #[test]
   fn string_output_captures_trace() {
      let mut out = StringOutput::new();
      out.emit_trace("trace detail".to_string());
      assert_eq!(out.log(), "· trace detail\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_is_verbose_always_true() {
      assert!(StringOutput::new().is_verbose());
   }

   #[cfg(feature = "trace")]
   #[test]
   fn string_output_is_trace_always_true() {
      assert!(StringOutput::new().is_trace());
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_shell_command() {
      let mut out = StringOutput::new();
      out.shell_command("pnpm install");
      assert_eq!(out.log(), "> pnpm install\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn string_output_shell_line() {
      let mut out = StringOutput::new();
      out.shell_line("installed pnpm@9.1.0");
      assert_eq!(out.log(), "> installed pnpm@9.1.0\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn log_exec_formats_command() {
      let mut out = StringOutput::new();
      let cmd = std::process::Command::new("node");
      out.log_exec(&cmd);
      assert_eq!(out.log(), "| Exec: node\n");
   }

   #[cfg(feature = "verbose")]
   #[test]
   fn log_exec_includes_args() {
      let mut out = StringOutput::new();
      let mut cmd = std::process::Command::new("pnpm");
      cmd.arg("install");
      out.log_exec(&cmd);
      assert_eq!(out.log(), "| Exec: pnpm install\n");
   }

   #[rstest]
   #[case(true, 1200, "✓ build (1s)\n")]
   #[case(false, 300, "✗ build (300ms)\n")]
   fn string_output_step_result(#[case] success: bool, #[case] elapsed_ms: u128, #[case] expected: &str) {
      let mut out = StringOutput::new();
      out.step_result("build", success, elapsed_ms, &[]);
      assert_eq!(out.log(), expected);
   }

   #[test]
   fn string_output_dry_run_shell() {
      let mut out = StringOutput::new();
      out.dry_run_shell("rm -rf /");
      assert_eq!(out.log(), "[dry-run] would run: rm -rf /\n");
   }

   #[test]
   fn string_output_dry_run_write() {
      let mut out = StringOutput::new();
      out.dry_run_write("/some/path.json");
      assert_eq!(out.log(), "[dry-run] would write: /some/path.json\n");
   }

   #[test]
   fn string_output_dry_run_delete() {
      let mut out = StringOutput::new();
      out.dry_run_delete("/some/dir");
      assert_eq!(out.log(), "[dry-run] would delete: /some/dir\n");
   }

   #[test]
   fn string_output_eprintln_captures_to_separate_buffer() {
      let mut out = StringOutput::new();
      out.eprintln("error: something went wrong");
      assert_eq!(out.log(), "");
      assert_eq!(out.err_log(), "error: something went wrong\n");
   }

   #[test]
   fn action_with_subject_no_trailing_emits_plain_line() {
      let mut out = StringOutput::new();
      out.action("Edited", "deploy.ts");
      assert_eq!(out.log(), "✓ Edited deploy.ts\n");
   }

   #[test]
   fn action_with_arrow_path_renders_arrow() {
      let mut out = StringOutput::new();
      out.action("Installed", "deploy").to_path("~/.sr/bin/deploy");
      assert_eq!(out.log(), "✓ Installed deploy → ~/.sr/bin/deploy\n");
   }

   #[test]
   fn action_with_to_renders_prep() {
      let mut out = StringOutput::new();
      out.action("Added", "lodash@4.17.21").to("deploy.ts");
      assert_eq!(out.log(), "✓ Added lodash@4.17.21 to deploy.ts\n");
   }

   #[test]
   fn action_with_hint_appends_em_dash() {
      let mut out = StringOutput::new();
      out.action("Edited", "deploy.ts").hint("run 'sr unedit' when done");
      assert_eq!(out.log(), "✓ Edited deploy.ts \u{2014} run 'sr unedit' when done\n");
   }

   #[test]
   fn action_with_note_and_hint_combines_both() {
      let mut out = StringOutput::new();
      out.action("Edited", "deploy.ts").note("switched from auth.ts").hint("run 'sr unedit' when done");
      assert_eq!(out.log(), "✓ Edited deploy.ts (switched from auth.ts) \u{2014} run 'sr unedit' when done\n");
   }

   #[test]
   fn state_emits_bullet_glyph() {
      let mut out = StringOutput::new();
      out.state("sr is ready");
      assert_eq!(out.log(), "\u{2022} sr is ready\n");
   }

   #[test]
   fn hint_emits_arrow_glyph() {
      let mut out = StringOutput::new();
      out.hint("run 'sr config edit' to customize");
      assert_eq!(out.log(), "\u{2192} run 'sr config edit' to customize\n");
   }

   #[test]
   fn section_emits_bare_title() {
      let mut out = StringOutput::new();
      out.section("Config files");
      assert_eq!(out.log(), "Config files\n");
   }

   #[test]
   fn item_with_trailing_indents_and_separates() {
      let mut out = StringOutput::new();
      out.item("./.sr/config.jsonc", "(local)");
      assert_eq!(out.log(), "  ./.sr/config.jsonc  (local)\n");
   }

   #[test]
   fn item_no_trailing_just_indents() {
      let mut out = StringOutput::new();
      out.item("./.sr/config.jsonc", "");
      assert_eq!(out.log(), "  ./.sr/config.jsonc\n");
   }

   #[test]
   fn warn_routes_to_stderr_buffer() {
      let mut out = StringOutput::new();
      out.warn("unknown key 'foo'");
      assert_eq!(out.log(), "");
      assert_eq!(out.err_log(), "\u{26A0} warn: unknown key 'foo'\n");
   }

   #[test]
   fn error_routes_to_stderr_buffer() {
      let mut out = StringOutput::new();
      out.error("could not find script 'deploy'");
      assert_eq!(out.log(), "");
      assert_eq!(out.err_log(), "\u{2717} error: could not find script 'deploy'\n");
   }

   #[test]
   fn ascii_theme_drives_action_state_warn_error_through_string_output() {
      // Given
      let mut out = StringOutput::new().with_theme(crate::output::ASCII_THEME);

      // When
      out.action("Edited", "deploy.ts").to("auth.ts");
      out.state("ready");
      out.hint("retry");
      out.warn("careful");
      out.error("nope");

      // Then
      assert_eq!(out.log(), "+ Edited deploy.ts to auth.ts\n* ready\n> retry\n");
      assert_eq!(out.err_log(), "! warn: careful\nx error: nope\n");
   }

   #[test]
   fn custom_theme_translates_connector_words_through_string_output() {
      // Given
      const FRENCH: crate::output::RenderTheme = crate::output::RenderTheme {
         success_glyph: "\u{2713}",
         state_glyph: "\u{2022}",
         hint_glyph: "\u{2192}",
         warn_glyph: "\u{26A0}",
         error_glyph: "\u{2717}",
         arrow: "\u{2192}",
         em_dash: "\u{2014}",
         warn_label: "attention :",
         error_label: "erreur :",
         prep_to: "vers",
         prep_from: "depuis"
      };
      let mut out = StringOutput::new().with_theme(FRENCH);

      // When
      out.action("Ajouté", "lodash").to("deploy.ts");
      out.action("Retiré", "lodash").from("deploy.ts");
      out.warn("clé inconnue");

      // Then
      assert_eq!(out.log(), "✓ Ajouté lodash vers deploy.ts\n✓ Retiré lodash depuis deploy.ts\n");
      assert_eq!(out.err_log(), "⚠ attention : clé inconnue\n");
   }

   #[test]
   fn step_result_uses_theme_glyphs() {
      // Given
      let mut out = StringOutput::new().with_theme(crate::output::ASCII_THEME);

      // When
      out.step_result("build", true, 1200, &[]);
      out.step_result("test", false, 300, &[]);

      // Then
      assert_eq!(out.log(), "+ build (1s)\nx test (300ms)\n");
   }
}
