//! Internal rendering primitives for the typed output kinds.
//!
//! - [`Trailing`] — the trailing-context variants an action can carry.
//! - Render functions return `String`; the caller decides whether to emit ANSI based on its own
//!   `colors_enabled` flag.
//!
//! All public functions accept `colors: bool` so that consumers can render
//! plain or ANSI-colored output without conditional logic at call sites.

use std::fmt::Write as _;

use crate::{
   output::Output,
   terminal::colors::{BOLD, CYAN, DIM, GREEN, RED, RESET, YELLOW},
   vocab::AsNoun
};

/// Trailing context attached to an [`OutputAction::action`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trailing {
   /// Arrow-prefixed path: ` → /some/path`.
   ArrowPath(String),
   /// Prepositional phrase: ` to deploy.ts` / ` from cache`.
   Prep {
      /// Connector word, e.g. `"to"` or `"from"`.
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
/// Pattern: `✓ <Verb> <subject>[ <trailing>][ (<note>)][ — <hint>]`
#[must_use]
pub fn render_action(
   verb: &str,
   subject: Option<&str>,
   trailing: &Trailing,
   note: &Note,
   hint: &Hint,
   colors: bool
) -> String {
   let mut s = String::new();
   if colors {
      let _ = write!(s, "{GREEN}✓{RESET} {BOLD}{verb}{RESET}");
   } else {
      let _ = write!(s, "✓ {verb}");
   }
   if let Some(subj) = subject {
      let _ = write!(s, " {subj}");
   }
   match trailing {
      Trailing::ArrowPath(p) => {
         if colors {
            let _ = write!(s, " {DIM}→ {p}{RESET}");
         } else {
            let _ = write!(s, " → {p}");
         }
      }
      Trailing::Prep { word, target } => {
         if colors {
            let _ = write!(s, " {DIM}{word} {target}{RESET}");
         } else {
            let _ = write!(s, " {word} {target}");
         }
      }
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
         let _ = write!(s, "{DIM} \u{2014} {h}{RESET}");
      } else {
         let _ = write!(s, " \u{2014} {h}");
      }
   }
   s
}

/// Render a failed action line.
#[must_use]
pub fn render_action_failed(label: &str, colors: bool) -> String {
   if colors { format!("{RED}✗{RESET} {BOLD}error:{RESET} {label}") } else { format!("✗ error: {label}") }
}

/// Render a state line: `• <msg>`.
#[must_use]
pub fn render_state(msg: &str, colors: bool) -> String {
   if colors { format!("{CYAN}\u{2022}{RESET} {msg}") } else { format!("\u{2022} {msg}") }
}

/// Render a standalone hint: `→ <msg>` (whole line dim).
#[must_use]
pub fn render_hint(msg: &str, colors: bool) -> String {
   if colors { format!("{DIM}\u{2192} {msg}{RESET}") } else { format!("\u{2192} {msg}") }
}

/// Render a warning: `⚠ warn: <msg>`.
#[must_use]
pub fn render_warn(msg: &str, colors: bool) -> String {
   if colors { format!("{YELLOW}\u{26A0}{RESET} {BOLD}warn:{RESET} {msg}") } else { format!("\u{26A0} warn: {msg}") }
}

/// Render an error: `✗ error: <msg>`.
#[must_use]
pub fn render_error(msg: &str, colors: bool) -> String {
   if colors {
      format!("{RED}{BOLD}\u{2717}{RESET} {BOLD}error:{RESET} {msg}")
   } else {
      format!("\u{2717} error: {msg}")
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
/// passing none emits the bare `✓ <Verb> <subject>` form.
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
   colors: bool
}

impl<'a> ActionBuilder<'a> {
   /// Internal constructor — use [`Output::action`].
   pub(crate) fn new(out: &'a mut (dyn Output + 'a), verb: String, subject: Option<String>, colors: bool) -> Self {
      Self { out, verb, subject, trailing: Trailing::None, note: None, hint: None, colors }
   }

   /// Trailing path with arrow separator: ` → /some/path`.
   pub fn to_path(mut self, path: impl Into<String>) -> Self {
      self.trailing = Trailing::ArrowPath(path.into());
      self
   }

   /// Trailing prepositional phrase: ` to <what>`.
   pub fn to(mut self, what: impl Into<String>) -> Self {
      self.trailing = Trailing::Prep { word: "to", target: what.into() };
      self
   }

   /// Trailing prepositional phrase: ` from <what>`.
   pub fn from(mut self, what: impl Into<String>) -> Self {
      self.trailing = Trailing::Prep { word: "from", target: what.into() };
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
}

// Two near-identical impls: one for `dyn Output` (which doesn't satisfy
// `Sized`, so the generic blanket can't cover it) and one for concrete
// `T: Output`. Bodies delegate to the same helper.
fn build_action<'a>(out: &'a mut (dyn Output + 'a), verb: &str, subject: &str) -> ActionBuilder<'a> {
   let colors = out.colors_enabled();
   let subj = if subject.is_empty() { None } else { Some(subject.to_string()) };
   ActionBuilder::new(out, verb.to_string(), subj, colors)
}

impl OutputAction for dyn Output + '_ {
   fn action<V: crate::vocab::AsVerb>(&mut self, verb: V, subject: &str) -> ActionBuilder<'_> {
      build_action(self, verb.as_verb(), subject)
   }
}

impl<T: Output> OutputAction for T {
   fn action<V: crate::vocab::AsVerb>(&mut self, verb: V, subject: &str) -> ActionBuilder<'_> {
      build_action(self, verb.as_verb(), subject)
   }
}

impl Drop for ActionBuilder<'_> {
   fn drop(&mut self) {
      let line =
         render_action(&self.verb, self.subject.as_deref(), &self.trailing, &self.note, &self.hint, self.colors);
      self.out.writeln(&line);
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn render_action_minimal_no_colors() {
      // Given / When
      let s = render_action("Edited", Some("deploy.ts"), &Trailing::None, &None, &None, false);

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
         false
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
         &Trailing::Prep { word: "to", target: "deploy.ts".to_string() },
         &None,
         &None,
         false
      );

      // Then
      assert_eq!(s, "✓ Added lodash@4.17.21 to deploy.ts");
   }

   #[test]
   fn render_action_with_count_and_prep_no_colors() {
      // Given / When
      let s = render_action("Removed", None, &Trailing::Count("2 deps".to_string()), &None, &None, false);

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
         false
      );

      // Then
      assert_eq!(s, "✓ Edited deploy.ts (switched from auth.ts) — run 'sr unedit' when done");
   }

   #[test]
   fn render_state_no_colors() {
      assert_eq!(render_state("sr is ready", false), "• sr is ready");
   }

   #[test]
   fn render_hint_no_colors() {
      assert_eq!(render_hint("run 'sr config edit'", false), "→ run 'sr config edit'");
   }

   #[test]
   fn render_warn_no_colors() {
      assert_eq!(render_warn("unknown key 'foo'", false), "⚠ warn: unknown key 'foo'");
   }

   #[test]
   fn render_error_no_colors() {
      assert_eq!(render_error("not found", false), "✗ error: not found");
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
      let s = render_action("Edited", Some("deploy.ts"), &Trailing::None, &None, &None, true);

      // Then
      assert!(s.contains("\x1b[32m\u{2713}\x1b[0m"));
      assert!(s.contains("\x1b[1mEdited\x1b[0m"));
   }
}
