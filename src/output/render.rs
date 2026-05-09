//! Internal rendering primitives for the typed output kinds.
//!
//! - [`RenderTheme`] — pluggable glyph + connector-word table. Use [`DEFAULT_THEME`] for the
//!   canonical Unicode look or [`ASCII_THEME`] for terminals without emoji/Unicode support.
//!   Translate by building a custom theme.
//! - [`Trailing`] — the trailing-context variants an action can carry.
//! - Render functions return `String`; the caller decides whether to emit ANSI based on its own
//!   `colors_enabled` flag and which theme to apply.

use std::fmt::Write as _;

use crate::{
   output::Output,
   terminal::colors::{BOLD, CYAN, DIM, GREEN, RED, RESET, YELLOW},
   vocab::AsNoun
};

/// Pluggable glyphs and connector words used by every render function.
///
/// Fields are `&'static str` so themes can be declared as `const` items.
/// Build a custom theme to localize `warn:`/`error:` labels, swap connector
/// words (`to`/`from`) for translations, or replace Unicode glyphs with
/// ASCII-friendly equivalents (see [`ASCII_THEME`]).
#[derive(Copy, Clone, Debug)]
pub struct RenderTheme {
   /// Leading glyph for a successful action line. Default: `✓`.
   pub success_glyph: &'static str,
   /// Leading glyph for a steady-state info line. Default: `•`.
   pub state_glyph: &'static str,
   /// Leading glyph for a standalone hint line. Default: `→`.
   pub hint_glyph: &'static str,
   /// Leading glyph for a warning line. Default: `⚠`.
   pub warn_glyph: &'static str,
   /// Leading glyph for an error line. Default: `✗`.
   pub error_glyph: &'static str,
   /// Separator drawn between subject and trailing path. Default: `→`.
   pub arrow: &'static str,
   /// Separator drawn between the line and an inline hint. Default: `—`.
   pub em_dash: &'static str,
   /// Label printed after [`warn_glyph`](Self::warn_glyph). Default: `warn:`.
   pub warn_label: &'static str,
   /// Label printed after [`error_glyph`](Self::error_glyph). Default: `error:`.
   pub error_label: &'static str,
   /// Connector word for [`Trailing::PrepTo`]. Default: `to`.
   pub prep_to: &'static str,
   /// Connector word for [`Trailing::PrepFrom`]. Default: `from`.
   pub prep_from: &'static str
}

/// The canonical theme: Unicode glyphs and English connector words.
pub const DEFAULT_THEME: RenderTheme = RenderTheme {
   success_glyph: "\u{2713}", // ✓
   state_glyph: "\u{2022}",   // •
   hint_glyph: "\u{2192}",    // →
   warn_glyph: "\u{26A0}",    // ⚠
   error_glyph: "\u{2717}",   // ✗
   arrow: "\u{2192}",         // →
   em_dash: "\u{2014}",       // —
   warn_label: "warn:",
   error_label: "error:",
   prep_to: "to",
   prep_from: "from"
};

/// An ASCII-only theme for terminals without Unicode/emoji support.
///
/// Glyphs use single ASCII characters and `->` / `--` separators so the
/// output stays readable in legacy Windows consoles, screen readers, and
/// log scrapers that mishandle multi-byte sequences.
pub const ASCII_THEME: RenderTheme = RenderTheme {
   success_glyph: "+",
   state_glyph: "*",
   hint_glyph: ">",
   warn_glyph: "!",
   error_glyph: "x",
   arrow: "->",
   em_dash: "--",
   warn_label: "warn:",
   error_label: "error:",
   prep_to: "to",
   prep_from: "from"
};

/// Trailing context attached to an [`OutputAction::action`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trailing {
   /// Arrow-prefixed path: ` → /some/path`.
   ArrowPath(String),
   /// Themed prepositional phrase using [`RenderTheme::prep_to`].
   PrepTo(String),
   /// Themed prepositional phrase using [`RenderTheme::prep_from`].
   PrepFrom(String),
   /// Caller-supplied connector word + target. Bypasses the theme — use
   /// only when the connector cannot be expressed via [`PrepTo`](Self::PrepTo)
   /// / [`PrepFrom`](Self::PrepFrom).
   PrepCustom {
      /// Connector word, e.g. `"into"` or `"as"`.
      word: &'static str,
      /// Target text rendered in dim.
      target: String
   },
   /// Pre-rendered count phrase: `"2 deps"` or `"1 environment"`.
   Count(String),
   /// Bare object — already in `subject`; render nothing.
   None
}

/// Optional parenthetical note: `(switched from auth.ts)`.
pub type Note = Option<String>;

/// Optional inline hint: ` — run 'sr unedit' when done`.
pub type Hint = Option<String>;

/// Render an action line.
///
/// Pattern: `<success_glyph> <Verb> <subject>[ <trailing>][ (<note>)][ <em_dash> <hint>]`
#[must_use]
pub fn render_action(
   verb: &str,
   subject: Option<&str>,
   trailing: &Trailing,
   note: &Note,
   hint: &Hint,
   colors: bool,
   theme: &RenderTheme
) -> String {
   let mut s = String::new();
   if colors {
      let _ = write!(s, "{GREEN}{}{RESET} {BOLD}{verb}{RESET}", theme.success_glyph);
   } else {
      let _ = write!(s, "{} {verb}", theme.success_glyph);
   }
   if let Some(subj) = subject {
      let _ = write!(s, " {subj}");
   }
   match trailing {
      Trailing::ArrowPath(p) => {
         if colors {
            let _ = write!(s, " {DIM}{} {p}{RESET}", theme.arrow);
         } else {
            let _ = write!(s, " {} {p}", theme.arrow);
         }
      }
      Trailing::PrepTo(target) => write_prep(&mut s, theme.prep_to, target, colors),
      Trailing::PrepFrom(target) => write_prep(&mut s, theme.prep_from, target, colors),
      Trailing::PrepCustom { word, target } => write_prep(&mut s, word, target, colors),
      Trailing::Count(text) => {
         let _ = write!(s, " {text}");
      }
      Trailing::None => {}
   }
   if let Some(n) = note {
      if colors {
         let _ = write!(s, " {DIM}({n}){RESET}");
      } else {
         let _ = write!(s, " ({n})");
      }
   }
   if let Some(h) = hint {
      if colors {
         let _ = write!(s, "{DIM} {} {h}{RESET}", theme.em_dash);
      } else {
         let _ = write!(s, " {} {h}", theme.em_dash);
      }
   }
   s
}

fn write_prep(s: &mut String, word: &str, target: &str, colors: bool) {
   if colors {
      let _ = write!(s, " {DIM}{word} {target}{RESET}");
   } else {
      let _ = write!(s, " {word} {target}");
   }
}

/// Render a failed action line.
#[must_use]
pub fn render_action_failed(label: &str, colors: bool, theme: &RenderTheme) -> String {
   if colors {
      format!("{RED}{}{RESET} {BOLD}{}{RESET} {label}", theme.error_glyph, theme.error_label)
   } else {
      format!("{} {} {label}", theme.error_glyph, theme.error_label)
   }
}

/// Render a state line: `<state_glyph> <msg>`.
#[must_use]
pub fn render_state(msg: &str, colors: bool, theme: &RenderTheme) -> String {
   if colors { format!("{CYAN}{}{RESET} {msg}", theme.state_glyph) } else { format!("{} {msg}", theme.state_glyph) }
}

/// Render a standalone hint: `<hint_glyph> <msg>` (whole line dim).
#[must_use]
pub fn render_hint(msg: &str, colors: bool, theme: &RenderTheme) -> String {
   if colors { format!("{DIM}{} {msg}{RESET}", theme.hint_glyph) } else { format!("{} {msg}", theme.hint_glyph) }
}

/// Render a warning: `<warn_glyph> <warn_label> <msg>`.
#[must_use]
pub fn render_warn(msg: &str, colors: bool, theme: &RenderTheme) -> String {
   if colors {
      format!("{YELLOW}{}{RESET} {BOLD}{}{RESET} {msg}", theme.warn_glyph, theme.warn_label)
   } else {
      format!("{} {} {msg}", theme.warn_glyph, theme.warn_label)
   }
}

/// Render an error: `<error_glyph> <error_label> <msg>`.
#[must_use]
pub fn render_error(msg: &str, colors: bool, theme: &RenderTheme) -> String {
   if colors {
      format!("{RED}{BOLD}{}{RESET} {BOLD}{}{RESET} {msg}", theme.error_glyph, theme.error_label)
   } else {
      format!("{} {} {msg}", theme.error_glyph, theme.error_label)
   }
}

/// Render a section header: bold title.
#[must_use]
pub fn render_section(title: &str, colors: bool) -> String {
   if colors { format!("{BOLD}{title}{RESET}") } else { title.to_string() }
}

/// Render an item row: `  <name>  <dim trailing>`.
#[must_use]
pub fn render_item(name: &str, trailing: &str, colors: bool) -> String {
   if trailing.is_empty() {
      format!("  {name}")
   } else if colors {
      format!("  {name}  {DIM}{trailing}{RESET}")
   } else {
      format!("  {name}  {trailing}")
   }
}

/// Build a count phrase from `(n, singular, plural)` — `"1 dep"` / `"2 deps"`.
#[must_use]
pub fn count_phrase(n: usize, singular: &str, plural: &str) -> String {
   if n == 1 { format!("{n} {singular}") } else { format!("{n} {plural}") }
}

/// Drop-guard builder for [`OutputAction::action`].
///
/// Emits the rendered line on Drop. Chainable methods set trailing context;
/// passing none emits the bare `<success_glyph> <Verb> <subject>` form.
///
/// ```ignore
/// out.action("Edited", "deploy.ts").hint("run 'sr unedit' when done");
/// ```
pub struct ActionBuilder<'a> {
   out: &'a mut (dyn Output + 'a),
   verb: String,
   subject: Option<String>,
   trailing: Trailing,
   note: Note,
   hint: Hint,
   /// Whether the destination should render colors (resolved at builder creation).
   colors: bool,
   /// Theme captured at builder creation so Drop has no borrow conflict with `out`.
   theme: RenderTheme
}

impl<'a> ActionBuilder<'a> {
   /// Internal constructor — use [`Output::action`].
   pub(crate) fn new(
      out: &'a mut (dyn Output + 'a),
      verb: String,
      subject: Option<String>,
      colors: bool,
      theme: RenderTheme
   ) -> Self {
      Self { out, verb, subject, trailing: Trailing::None, note: None, hint: None, colors, theme }
   }

   /// Trailing path with arrow separator: ` → /some/path`.
   pub fn to_path(mut self, path: impl Into<String>) -> Self {
      self.trailing = Trailing::ArrowPath(path.into());
      self
   }

   /// Trailing prepositional phrase: ` to <what>`.
   pub fn to(mut self, what: impl Into<String>) -> Self {
      self.trailing = Trailing::PrepTo(what.into());
      self
   }

   /// Trailing prepositional phrase: ` from <what>`.
   pub fn from(mut self, what: impl Into<String>) -> Self {
      self.trailing = Trailing::PrepFrom(what.into());
      self
   }

   /// Trailing count phrase: ` <n> <noun.singular|plural>`.
   pub fn count<N: AsNoun>(mut self, n: usize, noun: N) -> Self {
      self.trailing = Trailing::Count(count_phrase(n, noun.singular(), noun.plural()));
      self
   }

   /// Inline parenthetical note: ` (<note>)`.
   pub fn note(mut self, note: impl Into<String>) -> Self {
      self.note = Some(note.into());
      self
   }

   /// Inline em-dash hint: ` — <hint>`.
   pub fn hint(mut self, hint: impl Into<String>) -> Self {
      self.hint = Some(hint.into());
      self
   }
}

/// Extension trait that adds the typed [`OutputAction::action`] method to any
/// type implementing [`Output`]. Because the method is generic over `AsVerb`,
/// it cannot live on the dyn-compatible [`Output`] trait directly — this
/// extension trait is auto-implemented for every concrete `Output` and
/// separately for `dyn Output`, pulling the typed method into method-call
/// position.
pub trait OutputAction {
   /// Begin emitting an action line. Accepts any `impl AsVerb` (typically a
   /// project-specific `Verb` enum).
   fn action<V: crate::vocab::AsVerb>(&mut self, verb: V, subject: &str) -> ActionBuilder<'_>;

   /// Begin emitting a subject-less summary line — typically followed by
   /// `.count(n, noun)` to render `<success_glyph> <Verb> <n> <nouns>`. Use
   /// when the line describes an aggregate rather than a specific named
   /// subject.
   fn summary<V: crate::vocab::AsVerb>(&mut self, verb: V) -> ActionBuilder<'_>;
}

// Two near-identical impls: one for `dyn Output` (which doesn't satisfy
// `Sized`, so the generic blanket can't cover it) and one for concrete
// `T: Output`. Bodies delegate to the same helper.
fn build_action<'a>(out: &'a mut (dyn Output + 'a), verb: &str, subject: &str) -> ActionBuilder<'a> {
   let colors = out.colors_enabled();
   let theme = out.theme();
   let subj = if subject.is_empty() { None } else { Some(subject.to_string()) };
   ActionBuilder::new(out, verb.to_string(), subj, colors, theme)
}

fn build_summary<'a>(out: &'a mut (dyn Output + 'a), verb: &str) -> ActionBuilder<'a> {
   let colors = out.colors_enabled();
   let theme = out.theme();
   ActionBuilder::new(out, verb.to_string(), None, colors, theme)
}

impl OutputAction for dyn Output + '_ {
   fn action<V: crate::vocab::AsVerb>(&mut self, verb: V, subject: &str) -> ActionBuilder<'_> {
      build_action(self, verb.as_verb(), subject)
   }

   fn summary<V: crate::vocab::AsVerb>(&mut self, verb: V) -> ActionBuilder<'_> {
      build_summary(self, verb.as_verb())
   }
}

impl<T: Output> OutputAction for T {
   fn action<V: crate::vocab::AsVerb>(&mut self, verb: V, subject: &str) -> ActionBuilder<'_> {
      build_action(self, verb.as_verb(), subject)
   }

   fn summary<V: crate::vocab::AsVerb>(&mut self, verb: V) -> ActionBuilder<'_> {
      build_summary(self, verb.as_verb())
   }
}

impl Drop for ActionBuilder<'_> {
   fn drop(&mut self) {
      let line = render_action(
         &self.verb,
         self.subject.as_deref(),
         &self.trailing,
         &self.note,
         &self.hint,
         self.colors,
         &self.theme
      );
      self.out.writeln(&line);
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn render_action_minimal_no_colors() {
      // Given / When
      let s = render_action("Edited", Some("deploy.ts"), &Trailing::None, &None, &None, false, &DEFAULT_THEME);

      // Then
      assert_eq!(s, "✓ Edited deploy.ts");
   }

   #[test]
   fn render_action_with_arrow_path_no_colors() {
      // Given / When
      let s = render_action(
         "Installed",
         Some("deploy"),
         &Trailing::ArrowPath("~/.sr/bin/deploy".to_string()),
         &None,
         &None,
         false,
         &DEFAULT_THEME
      );

      // Then
      assert_eq!(s, "✓ Installed deploy → ~/.sr/bin/deploy");
   }

   #[test]
   fn render_action_with_prep_no_colors() {
      // Given / When
      let s = render_action(
         "Added",
         Some("lodash@4.17.21"),
         &Trailing::PrepTo("deploy.ts".to_string()),
         &None,
         &None,
         false,
         &DEFAULT_THEME
      );

      // Then
      assert_eq!(s, "✓ Added lodash@4.17.21 to deploy.ts");
   }

   #[test]
   fn render_action_with_count_and_prep_no_colors() {
      // Given / When
      let s =
         render_action("Removed", None, &Trailing::Count("2 deps".to_string()), &None, &None, false, &DEFAULT_THEME);

      // Then
      assert_eq!(s, "✓ Removed 2 deps");
   }

   #[test]
   fn render_action_with_note_and_hint_no_colors() {
      // Given / When
      let s = render_action(
         "Edited",
         Some("deploy.ts"),
         &Trailing::None,
         &Some("switched from auth.ts".to_string()),
         &Some("run 'sr unedit' when done".to_string()),
         false,
         &DEFAULT_THEME
      );

      // Then
      assert_eq!(s, "✓ Edited deploy.ts (switched from auth.ts) — run 'sr unedit' when done");
   }

   #[test]
   fn render_state_no_colors() {
      assert_eq!(render_state("sr is ready", false, &DEFAULT_THEME), "• sr is ready");
   }

   #[test]
   fn render_hint_no_colors() {
      assert_eq!(render_hint("run 'sr config edit'", false, &DEFAULT_THEME), "→ run 'sr config edit'");
   }

   #[test]
   fn render_warn_no_colors() {
      assert_eq!(render_warn("unknown key 'foo'", false, &DEFAULT_THEME), "⚠ warn: unknown key 'foo'");
   }

   #[test]
   fn render_error_no_colors() {
      assert_eq!(render_error("not found", false, &DEFAULT_THEME), "✗ error: not found");
   }

   #[test]
   fn render_section_no_colors() {
      assert_eq!(render_section("Config files", false), "Config files");
   }

   #[test]
   fn render_item_with_trailing_no_colors() {
      assert_eq!(render_item("./.sr/config.jsonc", "(local)", false), "  ./.sr/config.jsonc  (local)");
   }

   #[test]
   fn render_item_no_trailing_no_colors() {
      assert_eq!(render_item("./.sr/config.jsonc", "", false), "  ./.sr/config.jsonc");
   }

   #[test]
   fn count_phrase_singular() {
      assert_eq!(count_phrase(1, "dep", "deps"), "1 dep");
   }

   #[test]
   fn count_phrase_plural() {
      assert_eq!(count_phrase(2, "dep", "deps"), "2 deps");
      assert_eq!(count_phrase(0, "dep", "deps"), "0 deps");
   }

   #[test]
   fn render_action_with_colors_includes_ansi() {
      // Given / When
      let s = render_action("Edited", Some("deploy.ts"), &Trailing::None, &None, &None, true, &DEFAULT_THEME);

      // Then
      assert!(s.contains("\x1b[32m\u{2713}\x1b[0m"));
      assert!(s.contains("\x1b[1mEdited\x1b[0m"));
   }

   #[test]
   fn ascii_theme_avoids_unicode_glyphs() {
      // Given / When
      let action = render_action(
         "Edited",
         Some("deploy.ts"),
         &Trailing::PrepTo("auth.ts".to_string()),
         &None,
         &Some("then redeploy".to_string()),
         false,
         &ASCII_THEME
      );

      // Then
      assert_eq!(action, "+ Edited deploy.ts to auth.ts -- then redeploy");
      assert!(action.is_ascii());
   }

   #[test]
   fn ascii_theme_state_warn_error() {
      assert_eq!(render_state("ready", false, &ASCII_THEME), "* ready");
      assert_eq!(render_hint("retry", false, &ASCII_THEME), "> retry");
      assert_eq!(render_warn("careful", false, &ASCII_THEME), "! warn: careful");
      assert_eq!(render_error("nope", false, &ASCII_THEME), "x error: nope");
   }

   #[test]
   fn custom_theme_translates_connector_words() {
      // Given
      const FRENCH: RenderTheme = RenderTheme {
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

      // When
      let prep_to = render_action(
         "Ajouté",
         Some("lodash"),
         &Trailing::PrepTo("deploy.ts".to_string()),
         &None,
         &None,
         false,
         &FRENCH
      );
      let prep_from = render_action(
         "Retiré",
         Some("lodash"),
         &Trailing::PrepFrom("deploy.ts".to_string()),
         &None,
         &None,
         false,
         &FRENCH
      );

      // Then
      assert_eq!(prep_to, "✓ Ajouté lodash vers deploy.ts");
      assert_eq!(prep_from, "✓ Retiré lodash depuis deploy.ts");
      assert_eq!(render_warn("clé inconnue", false, &FRENCH), "⚠ attention : clé inconnue");
      assert_eq!(render_error("introuvable", false, &FRENCH), "✗ erreur : introuvable");
   }

   #[test]
   fn prep_custom_uses_caller_supplied_word() {
      // Given / When
      let s = render_action(
         "Compiled",
         Some("main.rs"),
         &Trailing::PrepCustom { word: "into", target: "main.o".to_string() },
         &None,
         &None,
         false,
         &DEFAULT_THEME
      );

      // Then
      assert_eq!(s, "✓ Compiled main.rs into main.o");
   }
}
