//! Drop-guard action-line builder and the [`OutputAction`] extension trait.
//!
//! [`ActionBuilder`] composes an action line via chainable setters and emits
//! it on Drop — callers don't have to remember a final `.emit()` call. The
//! [`OutputAction`] extension trait pulls the typed entry points (`.action`,
//! `.summary`) into method-call position on every [`Output`].

use super::{
   Output,
   render::{self, Hint, Note, RenderTheme, count_phrase}
};
use crate::vocab::AsNoun;

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
   /// Bare object — already in `subject`; render nothing.
   None
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
   /// Optional count phrase (`"2 deps"`) rendered between the verb and any
   /// trailing preposition. Held separately from `trailing` so a count and a
   /// `to`/`from` target compose (`Removed 2 deps from x`) instead of one
   /// clobbering the other.
   count: Option<String>,
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
      Self { out, verb, subject, count: None, trailing: Trailing::None, note: None, hint: None, colors, theme }
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

   /// Count phrase rendered after the verb: ` <n> <noun.singular|plural>`.
   /// Composes with a trailing `to`/`from` target.
   pub fn count<N: AsNoun>(mut self, n: usize, noun: N) -> Self {
      self.count = Some(count_phrase(n, noun.singular(), noun.plural()));
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

/// Extension trait that adds the rendered line kinds to any type implementing
/// [`Output`]. The core [`Output`] trait holds only the raw sinks and render
/// config; everything that renders a themed line — `action`/`summary` plus the
/// `state`/`hint`/`section`/`item`/`warn`/`error` kinds — lives here, built on
/// top of `writeln`/`eprintln`/`colors_enabled`/`theme`. Auto-implemented for
/// every concrete `Output` and separately for `dyn Output`, so the methods are
/// callable in method position without bloating the [`Output`] vtable.
pub trait OutputAction {
   /// Begin emitting an action line. Accepts any `impl AsVerb` (typically a
   /// project-specific `Verb` enum).
   fn action<V: crate::vocab::AsVerb>(&mut self, verb: V, subject: &str) -> ActionBuilder<'_>;

   /// Begin emitting a subject-less summary line — typically followed by
   /// `.count(n, noun)` to render `<success_glyph> <Verb> <n> <nouns>`. Use
   /// when the line describes an aggregate rather than a specific named
   /// subject.
   fn summary<V: crate::vocab::AsVerb>(&mut self, verb: V) -> ActionBuilder<'_>;

   /// Emit a steady-state info line: `• <msg>`.
   fn state(&mut self, msg: &str);

   /// Emit a standalone hint line: `→ <msg>` (whole line dim).
   fn hint(&mut self, msg: &str);

   /// Emit a section header: bold title.
   fn section(&mut self, title: &str);

   /// Emit an item row under a section: 2-space indent, name, dim trailing.
   fn item(&mut self, name: &str, trailing: &str);

   /// Emit a non-fatal warning to **stderr**: `⚠ warn: <msg>`. Suppressed in
   /// quiet mode (see [`Output::is_quiet`]); errors are not.
   fn warn(&mut self, msg: &str);

   /// Emit a fatal-error summary to **stderr**: `✗ error: <msg>`. Never
   /// suppressed by quiet mode — errors always flow.
   fn error(&mut self, msg: &str);
}

// Two impls are required: one for `dyn Output` (not `Sized`, so the generic
// blanket can't cover it) and one for concrete `T: Output`. A `?Sized` blanket
// would collapse them but can't coerce `&mut T` to `&mut dyn Output` without
// `T: Sized`. The macro below keeps the bodies textually identical so an edit
// to one form can't drift from the other.
macro_rules! impl_output_action {
   ($($head:tt)*) => {
      $($head)* {
         fn action<V: crate::vocab::AsVerb>(&mut self, verb: V, subject: &str) -> ActionBuilder<'_> {
            let colors = self.colors_enabled();
            let theme = self.theme();
            let subj = if subject.is_empty() { None } else { Some(subject.to_string()) };
            ActionBuilder::new(self, verb.as_verb().to_string(), subj, colors, theme)
         }

         fn summary<V: crate::vocab::AsVerb>(&mut self, verb: V) -> ActionBuilder<'_> {
            let colors = self.colors_enabled();
            let theme = self.theme();
            ActionBuilder::new(self, verb.as_verb().to_string(), None, colors, theme)
         }

         fn state(&mut self, msg: &str) {
            let line = render::render_state(msg, self.colors_enabled(), &self.theme());
            self.writeln(&line);
         }

         fn hint(&mut self, msg: &str) {
            let line = render::render_hint(msg, self.colors_enabled(), &self.theme());
            self.writeln(&line);
         }

         fn section(&mut self, title: &str) {
            let line = render::render_section(title, self.colors_enabled());
            self.writeln(&line);
         }

         fn item(&mut self, name: &str, trailing: &str) {
            let line = render::render_item(name, trailing, self.colors_enabled());
            self.writeln(&line);
         }

         fn warn(&mut self, msg: &str) {
            if self.is_quiet() {
               return;
            }
            let line = render::render_warn(msg, self.colors_enabled(), &self.theme());
            self.eprintln(&line);
         }

         fn error(&mut self, msg: &str) {
            let line = render::render_error(msg, self.colors_enabled(), &self.theme());
            self.eprintln(&line);
         }
      }
   };
}

impl_output_action!(impl OutputAction for dyn Output + '_);
impl_output_action!(impl<T: Output> OutputAction for T);

impl Drop for ActionBuilder<'_> {
   fn drop(&mut self) {
      let line = render::render_action(
         &self.verb,
         self.subject.as_deref(),
         self.count.as_deref(),
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
   use crate::{output::StringOutput, vocab::AsNoun};

   struct Deps;
   impl AsNoun for Deps {
      fn singular(&self) -> &str {
         "dep"
      }
      fn plural(&self) -> &str {
         "deps"
      }
   }

   #[test]
   fn count_and_from_compose_in_a_summary_line() {
      // Given / When — a summary that both counts an aggregate and names the
      // source it was removed from.
      let mut out = StringOutput::new();
      out.summary("Removed").count(2, Deps).from("script.hs");

      // Then — the count is NOT clobbered by the `from` preposition.
      assert_eq!(out.log().trim_end(), "✓ Removed 2 deps from script.hs");
   }
}
