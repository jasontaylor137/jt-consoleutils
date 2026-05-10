//! Drop-guard action-line builder and the [`OutputAction`] extension trait.
//!
//! [`ActionBuilder`] composes an action line via chainable setters and emits
//! it on Drop — callers don't have to remember a final `.emit()` call. The
//! [`OutputAction`] extension trait pulls the typed entry points (`.action`,
//! `.summary`) into method-call position on every [`Output`].

use super::Output;
use super::render::{
   self, Hint, Note, RenderTheme, count_phrase
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
   /// Pre-rendered count phrase: `"2 deps"` or `"1 environment"`.
   Count(String),
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
      let line = render::render_action(
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
