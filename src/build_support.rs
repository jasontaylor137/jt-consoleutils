//! `build.rs` helper that emits `BUILD_DATE` and `GIT_HASH` for downstream
//! `env!()` use.
//!
//! Gated behind the `build-support` feature. Computes the build date with
//! `std::time` (no chrono/time dependency) and the git short hash with a
//! `git rev-parse` subprocess (falling back to `"unknown"` if git is
//! unavailable, the directory isn't a repo, or the command fails). Pair with
//! [`crate::cli::version::version_string`] to render a `--version` line.

use std::{
   process::Command,
   time::{SystemTime, UNIX_EPOCH}
};

/// Emits `BUILD_DATE` and `GIT_HASH` as `cargo:rustc-env` variables for use
/// in downstream `build.rs` files.
///
/// `BUILD_DATE` is computed from the system clock in `yyyy-mm-dd` format using
/// only `std` (no external crates). `GIT_HASH` is the short commit hash from
/// `git rev-parse --short HEAD`, or `"unknown"` if git is unavailable.
///
/// # Usage
///
/// In your project's `Cargo.toml`:
///
/// ```toml
/// [build-dependencies]
/// jt-consoleutils = { path = "../jt-consoleutils", features = ["build-support"] }
/// ```
///
/// In your project's `build.rs`:
///
/// ```rust,ignore
/// fn main() {
///     jt_consoleutils::build_support::emit_build_info();
/// }
/// ```
///
/// Then in your application code:
///
/// ```rust,ignore
/// const BUILD_DATE: &str = env!("BUILD_DATE");
/// const GIT_HASH: &str = env!("GIT_HASH");
/// ```
pub fn emit_build_info() {
   let build_date = compute_build_date();
   let git_hash = compute_git_hash();
   println!("cargo:rustc-env=BUILD_DATE={}", build_date);
   println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}

fn compute_build_date() -> String {
   let now = SystemTime::now();
   let days_since_epoch = now.duration_since(UNIX_EPOCH).unwrap().as_secs() / 86400;

   let mut y = 1970u64;
   let mut days_left = days_since_epoch;

   loop {
      let year_days = days_in_year(y);
      if days_left < year_days {
         break;
      }
      days_left -= year_days;
      y += 1;
   }

   let month_days = [31u64, 28 + if is_leap(y) { 1 } else { 0 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

   let mut m = 1u64;
   for md in month_days.iter() {
      if days_left < *md {
         break;
      }
      days_left -= *md;
      m += 1;
   }

   let d = days_left + 1;
   format!("{:04}-{:02}-{:02}", y, m, d)
}

fn compute_git_hash() -> String {
   let Ok(output) = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output() else {
      return "unknown".to_string();
   };
   if !output.status.success() {
      return "unknown".to_string();
   }
   let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
   if hash.is_empty() { "unknown".to_string() } else { hash }
}

fn is_leap(y: u64) -> bool {
   y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}

fn days_in_year(y: u64) -> u64 {
   if is_leap(y) { 366 } else { 365 }
}
