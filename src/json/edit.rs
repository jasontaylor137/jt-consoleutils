//! Span-aware, comment-preserving JSONC editing. Navigates only the addressed
//! path over raw source bytes and splices the single addressed span; comments,
//! key order, and formatting outside that span are preserved byte-for-byte.
//!
//! Schema-awareness, value typing, and validation live in the caller (`sr`):
//! paths arrive pre-split into object-key segments, values arrive as
//! `JsonValue`, and the caller re-parses + validates the returned string before
//! writing it.

use super::{
   scan::{comment_len, scan_string, skip_trivia},
   to_json_pretty,
   value::JsonValue
};

/// Error from a JSONC edit operation.
#[derive(Debug, thiserror::Error)]
pub enum EditError {
   /// A path segment tried to descend through a non-object scalar/array.
   #[error("path segment '{segment}' is not an object")]
   NotAnObject {
      /// The segment that failed to descend.
      segment: String
   },

   /// The source text is not well-formed enough to navigate.
   #[error("malformed JSONC: {0}")]
   Malformed(String)
}

struct MemberSpan {
   key: String,
   key_start: usize,
   value_start: usize,
   value_end: usize
}

#[cfg(test)]
impl MemberSpan {
   fn value_start_for_test(&self) -> usize {
      self.value_start
   }
}

struct ObjectScan {
   open: usize,
   close: usize,
   members: Vec<MemberSpan>
}

/// Scan the object whose `{` is at or after `start` (the first non-trivia byte
/// from `start` must be `{`). Records each member's key + spans. Stops at the
/// matching `}`.
fn scan_object(bytes: &[u8], start: usize) -> Result<ObjectScan, EditError> {
   let open = skip_trivia(bytes, start);
   if open >= bytes.len() || bytes[open] != b'{' {
      return Err(EditError::Malformed("expected '{'".into()));
   }
   let mut members = Vec::new();
   let mut i = skip_trivia(bytes, open + 1);
   loop {
      if i >= bytes.len() {
         return Err(EditError::Malformed("unterminated object".into()));
      }
      if bytes[i] == b'}' {
         return Ok(ObjectScan { open, close: i, members });
      }
      if bytes[i] != b'"' {
         return Err(EditError::Malformed("expected object key".into()));
      }
      let key_start = i;
      let key_end = scan_string(bytes, i);
      let key = decode_key(&bytes[key_start..key_end]);
      let after_colon = {
         let c = skip_trivia(bytes, key_end);
         if c >= bytes.len() || bytes[c] != b':' {
            return Err(EditError::Malformed("expected ':'".into()));
         }
         skip_trivia(bytes, c + 1)
      };
      let value_start = after_colon;
      let value_end = scan_value(bytes, value_start)?;
      members.push(MemberSpan { key, key_start, value_start, value_end });
      // advance past optional comma
      i = skip_trivia(bytes, value_end);
      if i < bytes.len() && bytes[i] == b',' {
         i = skip_trivia(bytes, i + 1);
      }
   }
}

/// Decode a JSON string literal (incl. surrounding quotes) into its key text.
/// Handles the standard JSON escape set; sufficient for object keys. NOTE:
/// multi-byte UTF-8 keys are decoded lossily (`byte as char`) — fine for the
/// ASCII config keys this serves; see bead sr-61q2.5 follow-up note.
fn decode_key(lit: &[u8]) -> String {
   let inner = &lit[1..lit.len().saturating_sub(1)];
   let mut s = String::with_capacity(inner.len());
   let mut k = 0;
   while k < inner.len() {
      if inner[k] == b'\\' && k + 1 < inner.len() {
         match inner[k + 1] {
            b'n' => s.push('\n'),
            b't' => s.push('\t'),
            b'r' => s.push('\r'),
            b'"' => s.push('"'),
            b'\\' => s.push('\\'),
            b'/' => s.push('/'),
            other => {
               s.push('\\');
               s.push(other as char);
            }
         }
         k += 2;
      } else {
         // raw UTF-8 byte; rebuild lossily-safe via from_utf8 on a slice run
         s.push(inner[k] as char);
         k += 1;
      }
   }
   s
}

/// Return the index one past the end of the value token/structure at `start`.
/// Handles strings, objects, arrays (brace/bracket matching with comment and
/// string awareness), and bare scalars (number/true/false/null) which end at
/// the next structural byte (`,`/`}`/`]`) or trivia.
fn scan_value(bytes: &[u8], start: usize) -> Result<usize, EditError> {
   if start >= bytes.len() {
      return Err(EditError::Malformed("expected value".into()));
   }
   match bytes[start] {
      b'"' => Ok(scan_string(bytes, start)),
      b'{' | b'[' => scan_bracketed(bytes, start),
      _ => {
         // bare scalar: consume until structural terminator or trivia start
         let mut j = start;
         while j < bytes.len() {
            let c = bytes[j];
            if c == b',' || c == b'}' || c == b']' || c.is_ascii_whitespace() || comment_len(bytes, j) != 0 {
               break;
            }
            j += 1;
         }
         Ok(j)
      }
   }
}

/// Match a `{...}` or `[...]` run, skipping strings and comments inside.
fn scan_bracketed(bytes: &[u8], start: usize) -> Result<usize, EditError> {
   let mut depth = 0usize;
   let mut j = start;
   while j < bytes.len() {
      let c = bytes[j];
      if c == b'"' {
         j = scan_string(bytes, j);
         continue;
      }
      let clen = comment_len(bytes, j);
      if clen != 0 {
         j += clen;
         continue;
      }
      match c {
         b'{' | b'[' => depth += 1,
         b'}' | b']' => {
            depth -= 1;
            if depth == 0 {
               return Ok(j + 1);
            }
         }
         _ => {}
      }
      j += 1;
   }
   Err(EditError::Malformed("unterminated object/array".into()))
}

/// Navigate to the object that directly contains the final path segment.
/// Returns `(object_scan, final_segment)`, or `Ok(None)` if a non-final
/// segment's key is absent. Non-final segment present but non-object → Err.
fn navigate<'a>(
   bytes: &[u8],
   root_start: usize,
   path: &'a [&'a str]
) -> Result<Option<(ObjectScan, &'a str)>, EditError> {
   let (last, parents) = match path.split_last() {
      Some(x) => x,
      None => return Err(EditError::Malformed("empty path".into()))
   };
   let mut start = root_start;
   for seg in parents {
      let obj = scan_object(bytes, start)?;
      let m = match obj.members.iter().find(|m| m.key == *seg) {
         Some(m) => m,
         None => return Ok(None)
      };
      // descend: the member's value must be an object
      let vs = skip_trivia(bytes, m.value_start);
      if vs >= bytes.len() || bytes[vs] != b'{' {
         return Err(EditError::NotAnObject { segment: (*seg).to_string() });
      }
      start = vs;
   }
   let obj = scan_object(bytes, start)?;
   Ok(Some((obj, last)))
}

/// Raw value slice for `path` (borrows `src`), or `None` if any segment is
/// absent. A scalar leaf comes back WITH its quotes (`"^1.6"`); the caller
/// unquotes. Descending through a non-object scalar is an error.
pub fn jsonc_get<'a>(src: &'a str, path: &[&str]) -> Result<Option<&'a str>, EditError> {
   let bytes = src.as_bytes();
   match navigate(bytes, 0, path)? {
      None => Ok(None),
      Some((obj, last)) => match obj.members.iter().find(|m| m.key == last) {
         Some(m) => Ok(Some(&src[m.value_start..m.value_end])),
         None => Ok(None)
      }
   }
}

/// New document with `path` removed, or `None` if the path was absent.
/// Surgical: never prunes a parent left empty; a same-line trailing comment
/// after the removed value goes with the removed line, comment lines above the
/// removed key stay.
pub fn jsonc_unset(src: &str, path: &[&str]) -> Result<Option<String>, EditError> {
   let bytes = src.as_bytes();
   let (obj, last) = match navigate(bytes, 0, path)? {
      None => return Ok(None),
      Some(x) => x
   };
   let idx = match obj.members.iter().position(|m| m.key == last) {
      Some(i) => i,
      None => return Ok(None)
   };

   // Special case: the only member -> clear the object body to "{}".
   if obj.members.len() == 1 {
      let mut out = String::with_capacity(src.len());
      out.push_str(&src[..obj.open]);
      out.push_str("{}");
      out.push_str(&src[obj.close + 1..]);
      return Ok(Some(out));
   }

   let m = &obj.members[idx];
   // Start of the member's own line: backtrack whitespace (not newline) from key_start.
   let line_start = line_indent_start(bytes, m.key_start);

   // Does this member own a trailing comma (a comma after its value, before
   // the next member or `}`)?
   let after_val = skip_trivia(bytes, m.value_end);
   let owns_comma = after_val < bytes.len() && bytes[after_val] == b',';

   let mut out = String::with_capacity(src.len());
   if owns_comma {
      // Remove [line_start ..= comma], plus the trailing newline if the line is
      // now empty up to it.
      let mut end = after_val + 1;
      end = swallow_trailing_newline(bytes, end);
      out.push_str(&src[..line_start]);
      out.push_str(&src[end..]);
   } else {
      // Last member, no own trailing comma: remove the PREVIOUS member's comma
      // (single byte, located right after prev.value_end), and separately the
      // member's own line span. Trivia/comments between the prev comma and this
      // member are preserved.
      let prev = &obj.members[idx - 1];
      let prev_comma = skip_trivia(bytes, prev.value_end); // bytes[prev_comma] == ','
      debug_assert_eq!(bytes[prev_comma], b',');
      let mut end = m.value_end;
      end = swallow_trailing_newline(bytes, end);
      out.push_str(&src[..prev_comma]);
      out.push_str(&src[prev_comma + 1..line_start]);
      out.push_str(&src[end..]);
   }
   Ok(Some(out))
}

/// Index of the first byte of the indentation run on `pos`'s line (i.e. just
/// after the preceding `\n`, or 0). Only spaces/tabs are skipped backward.
fn line_indent_start(bytes: &[u8], pos: usize) -> usize {
   let mut s = pos;
   while s > 0 && (bytes[s - 1] == b' ' || bytes[s - 1] == b'\t') {
      s -= 1;
   }
   s
}

/// The `[start, end)` byte range of the indentation run on the line that
/// contains `pos` — from just after the preceding `\n` (or 0) forward over any
/// spaces/tabs. Unlike [`line_indent_start`], this is correct even when `pos`
/// sits mid-line after non-whitespace (e.g. the `{` in `"env": {}`).
fn line_indentation(bytes: &[u8], pos: usize) -> (usize, usize) {
   let mut start = pos;
   while start > 0 && bytes[start - 1] != b'\n' {
      start -= 1;
   }
   let mut end = start;
   while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
      end += 1;
   }
   (start, end)
}

/// If `end` (after a removal) sits at a `\n`, consume that newline so no blank
/// line is left.
fn swallow_trailing_newline(bytes: &[u8], end: usize) -> usize {
   if end < bytes.len() && bytes[end] == b'\n' {
      return end + 1;
   }
   end
}

/// Detect the file's indentation unit from the first indented line (the run of
/// spaces or tabs that begins a line). Falls back to two spaces.
fn detect_indent_unit(src: &str) -> String {
   for line in src.lines() {
      let trimmed = line.trim_start_matches([' ', '\t']);
      let indent = &line[..line.len() - trimmed.len()];
      if !indent.is_empty() && !trimmed.is_empty() {
         return indent.to_string();
      }
   }
   "  ".to_string()
}

/// Render a `JsonValue` as a JSONC fragment for insertion. Scalars render to a
/// single token. Nested values are rendered by `to_json_pretty` (2-space
/// structural indent) and then re-emitted so that each structural level uses
/// `unit`, prefixed by `base` (the indentation of the line the value lands on).
/// The first line is NOT prefixed (it follows `"key": ` on the opening line).
fn render_fragment(value: &JsonValue, unit: &str, base: &str) -> String {
   let pretty = to_json_pretty(value);
   if !pretty.contains('\n') {
      return pretty; // scalar or empty {}/[]
   }
   let mut out = String::with_capacity(pretty.len());
   for (n, line) in pretty.lines().enumerate() {
      if n > 0 {
         out.push('\n');
      }
      if n == 0 {
         out.push_str(line);
         continue;
      }
      // count leading 2-space levels produced by to_json_pretty
      let trimmed = line.trim_start_matches(' ');
      let levels = (line.len() - trimmed.len()) / 2;
      out.push_str(base);
      for _ in 0..levels {
         out.push_str(unit);
      }
      out.push_str(trimmed);
   }
   out
}

/// New document with `path` set to `value`. Replaces an existing value span, or
/// appends/creates. Comments, key order, and formatting outside the touched
/// span are byte-identical. Empty/missing source starts from `{}`.
pub fn jsonc_set(src: &str, path: &[&str], value: &JsonValue) -> Result<String, EditError> {
   let unit = detect_indent_unit(src);

   // Empty/blank source has no object to navigate — synthesize from `{}`.
   if src.trim().is_empty() {
      return create_path(src, path, value, &unit);
   }

   let bytes = src.as_bytes();
   if let Some((obj, last)) = navigate(bytes, 0, path)? {
      if let Some(m) = obj.members.iter().find(|m| m.key == last) {
         // Replace existing value. Base indent = the member's line indent.
         let base = &src[line_indent_start(bytes, m.key_start)..m.key_start];
         let fragment = render_fragment(value, &unit, base);
         let mut out = String::with_capacity(src.len() + fragment.len());
         out.push_str(&src[..m.value_start]);
         out.push_str(&fragment);
         out.push_str(&src[m.value_end..]);
         return Ok(out);
      }
      // key absent in an existing object -> append
      return append_member(src, &obj, last, value, &unit);
   }
   // a non-final segment was absent -> create parent chain
   create_path(src, path, value, &unit)
}

/// Build a synthesized member text `"key": <fragment>` plus the parent-object
/// chain for any remaining segments. `inner_indent` is the indentation of the
/// line the new member sits on; `unit` extends it for nested levels.
fn synth_member(keys: &[&str], value: &JsonValue, unit: &str, inner_indent: &str) -> String {
   // Build from the leaf outward.
   let mut frag = render_fragment(value, unit, &format!("{inner_indent}{}", unit.repeat(keys.len() - 1)));
   for (depth, key) in keys.iter().enumerate().rev() {
      let level_indent = format!("{inner_indent}{}", unit.repeat(depth));
      if depth + 1 == keys.len() {
         // leaf member
         frag = format!("{level_indent}\"{key}\": {frag}");
      } else {
         // wrapping object
         frag = format!("{level_indent}\"{key}\": {{\n{frag}\n{level_indent}}}");
      }
   }
   // strip the leading indent of the first line — the caller positions it.
   frag.trim_start_matches([' ', '\t']).to_string()
}

/// Append a new member (possibly carrying a synthesized sub-chain when `keys`
/// has more than one segment) as the last member of `obj`.
fn append_chain(
   src: &str,
   obj: &ObjectScan,
   keys: &[&str],
   value: &JsonValue,
   unit: &str
) -> Result<String, EditError> {
   let bytes = src.as_bytes();
   // Determine the inner indent for members of this object.
   let inner_indent = match obj.members.first() {
      Some(m) => src[line_indent_start(bytes, m.key_start)..m.key_start].to_string(),
      None => {
         // empty object: base indent of the object's own line + one unit.
         let (s, e) = line_indentation(bytes, obj.open);
         format!("{}{unit}", &src[s..e])
      }
   };
   let member_text = synth_member(keys, value, unit, &inner_indent);

   if obj.members.is_empty() {
      // "{}" -> "{\n<indent>member\n<obj indent>}"
      let (s, e) = line_indentation(bytes, obj.open);
      let obj_line_indent = src[s..e].to_string();
      let mut out = String::with_capacity(src.len() + member_text.len() + 8);
      out.push_str(&src[..obj.open + 1]);
      out.push('\n');
      out.push_str(&inner_indent);
      out.push_str(&member_text);
      out.push('\n');
      out.push_str(&obj_line_indent);
      out.push_str(&src[obj.close..]);
      return Ok(out);
   }

   // Non-empty: insert after the last member's value, before any trailing
   // comma/whitespace/`}`. Detect whether the last member already has a
   // trailing comma.
   let last = obj.members.last().unwrap();
   let after_val = skip_trivia(bytes, last.value_end);
   let has_trailing_comma = after_val < bytes.len() && bytes[after_val] == b',';

   let mut out = String::with_capacity(src.len() + member_text.len() + inner_indent.len() + 4);
   if has_trailing_comma {
      // ...keep existing trailing comma; insert new member after it, then add a
      // trailing comma to match the file's existing style.
      out.push_str(&src[..after_val + 1]); // through the comma
      out.push('\n');
      out.push_str(&inner_indent);
      out.push_str(&member_text);
      out.push(',');
      out.push_str(&src[after_val + 1..]);
   } else {
      // Insert a comma after the last value, then the new member.
      out.push_str(&src[..last.value_end]);
      out.push(',');
      out.push('\n');
      out.push_str(&inner_indent);
      out.push_str(&member_text);
      out.push_str(&src[last.value_end..]);
   }
   Ok(out)
}

fn append_member(src: &str, obj: &ObjectScan, key: &str, value: &JsonValue, unit: &str) -> Result<String, EditError> {
   append_chain(src, obj, &[key], value, unit)
}

/// A non-final segment was absent: descend through the present prefix, then
/// append a synthesized chain for the remaining segments at that object.
fn create_path(src: &str, path: &[&str], value: &JsonValue, unit: &str) -> Result<String, EditError> {
   // Empty/blank source: synthesize a root object first.
   let trimmed = src.trim();
   if trimmed.is_empty() {
      return create_path("{}", path, value, unit);
   }
   let bytes = src.as_bytes();
   // Find the deepest existing object along the path and the remaining keys.
   let mut start = 0usize;
   let mut consumed = 0usize;
   for (i, seg) in path.iter().enumerate() {
      // The last segment is always "remaining"; stop before it.
      if i == path.len() - 1 {
         break;
      }
      let obj = scan_object(bytes, start)?;
      match obj.members.iter().find(|m| m.key == *seg) {
         Some(m) => {
            let vs = skip_trivia(bytes, m.value_start);
            if bytes[vs] != b'{' {
               return Err(EditError::NotAnObject { segment: (*seg).to_string() });
            }
            start = vs;
            consumed = i + 1;
         }
         None => break
      }
   }
   let obj = scan_object(bytes, start)?;
   append_chain(src, &obj, &path[consumed..], value, unit)
}

#[cfg(test)]
mod tests;
