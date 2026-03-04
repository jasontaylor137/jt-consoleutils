/// Returns the version string in the format "yyyy-mm-dd (hash)".
///
/// Both values are typically injected at build time via `build.rs` using
/// `env!("BUILD_DATE")` and `env!("GIT_HASH")`, then passed in here.
///
/// # Example
///
/// ```rust,ignore
/// const BUILD_DATE: &str = env!("BUILD_DATE");
/// const GIT_HASH: &str = env!("GIT_HASH");
///
/// let v = jt_consoleutils::version::version_string(BUILD_DATE, GIT_HASH);
/// ```
pub fn version_string(build_date: &str, git_hash: &str) -> String {
   format!("{} ({})", build_date, git_hash)
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn version_string_formats_correctly() {
      assert_eq!(version_string("2025-01-15", "abc1234"), "2025-01-15 (abc1234)");
   }

   #[test]
   fn version_string_with_unknown_hash() {
      assert_eq!(version_string("2025-01-15", "unknown"), "2025-01-15 (unknown)");
   }
}
