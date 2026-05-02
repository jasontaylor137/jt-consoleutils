/// Returns the version string in the format "yyyy-mm-dd (hash)".
///
/// Both values are typically injected at build time via `build.rs` using
/// `env!("BUILD_DATE")` and `env!("GIT_HASH")`, then passed in here.
///
/// # Example
///
/// ```rust,ignore
/// // This example is marked `ignore` because `BUILD_DATE` and `GIT_HASH` are
/// // environment variables injected at compile time by `build.rs`. They are
/// // not available in the doctest environment, so the example cannot be run
/// // as a test. It is provided for illustration purposes only.
/// const BUILD_DATE: &str = env!("BUILD_DATE");
/// const GIT_HASH: &str = env!("GIT_HASH");
///
/// let v = jt_consoleutils::cli::version::version_string(BUILD_DATE, GIT_HASH);
/// ```
#[must_use]
pub fn version_string(build_date: &str, git_hash: &str) -> String {
   format!("{build_date} ({git_hash})")
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
