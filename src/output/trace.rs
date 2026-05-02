//! Trace-output formatting helpers.
//!
//! Diagnostic formatting gated on `feature = "trace"`. Pass the formatted
//! string into the [`trace!`](crate::trace) macro, which applies the dim style.

/// Format a labelled content block for trace output.
///
/// Produces:
/// ```text
/// --- {label} ---
/// {content, up to 300 chars}[... +N chars omitted]
/// --------------------
/// ```
/// No per-line prefixing — pass the result to `trace!` which applies the dim style.
#[must_use]
pub fn format_trace_block(label: &str, content: &str) -> String {
   const MAX: usize = 300;
   let header = format!("--- {label} ---");
   let footer = "--------------------".to_string();
   let total_chars = content.chars().count();
   let body = if total_chars > MAX {
      let kept: String = content.chars().take(MAX).collect();
      let omitted = total_chars - MAX;
      format!("{kept}[... +{omitted} chars omitted]")
   } else {
      content.to_string()
   };
   format!("{header}\n{body}\n{footer}")
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn format_trace_block_short_content() {
      let result = format_trace_block("config.json", "hello world");
      assert_eq!(result, "--- config.json ---\nhello world\n--------------------");
   }

   #[test]
   fn format_trace_block_truncates_long_content() {
      let content = "x".repeat(350);
      let result = format_trace_block("label", &content);
      assert!(result.contains("[... +50 chars omitted]"));
      assert!(result.starts_with("--- label ---"));
      assert!(result.ends_with("--------------------"));
   }

   #[test]
   fn format_trace_block_exact_limit_not_truncated() {
      let content = "y".repeat(300);
      let result = format_trace_block("x", &content);
      assert!(!result.contains("omitted"));
   }

   #[test]
   fn format_trace_block_truncates_on_char_boundary_for_multibyte() {
      // 'é' is 2 bytes; 200 of them = 400 bytes but 200 chars. With a 300-char
      // window, all 200 chars fit and nothing is truncated.
      let content = "é".repeat(200);
      let result = format_trace_block("x", &content);
      assert!(!result.contains("omitted"));

      // 400 'é' chars exceed the 300-char window; truncation happens on a
      // char boundary (not a byte boundary, which would have panicked).
      let content = "é".repeat(400);
      let result = format_trace_block("x", &content);
      assert!(result.contains("[... +100 chars omitted]"));
   }
}
