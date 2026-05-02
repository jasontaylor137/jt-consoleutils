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
   let body = if content.len() > MAX {
      let omitted = content.len() - MAX;
      format!("{}[... +{omitted} chars omitted]", &content[..MAX])
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
}
