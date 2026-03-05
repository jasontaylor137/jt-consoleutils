//! Scaffolds `build.rs` and `rel.sh` for a Rust CLI project that uses
//! `jt-consoleutils` build support.
//!
//! Everything is inferred from the target project's `Cargo.toml`:
//!
//! - Binary name: first `[[bin]]` `name`, or `[package]` `name` as fallback
//! - Windows `.exe` handling: inferred from presence of `[target.'cfg(windows)'.dependencies]`
//!
//! # Usage
//!
//! ```sh
//! cargo run --example scaffold_project -- --project-dir ../my-project
//! cargo run --example scaffold_project -- --project-dir ../my-project --force
//! ```

use std::{
   fmt::Write as FmtWrite,
   fs,
   path::{Path, PathBuf},
   process
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

struct Args {
   project_dir: PathBuf,
   force: bool
}

fn parse_args() -> Result<Args, String> {
   let raw: Vec<String> = std::env::args().skip(1).collect();
   let mut project_dir: Option<PathBuf> = None;
   let mut force = false;

   let mut i = 0;
   while i < raw.len() {
      match raw[i].as_str() {
         "--project-dir" => {
            i += 1;
            let val = raw.get(i).ok_or("--project-dir requires a value")?;
            project_dir = Some(PathBuf::from(val));
         }
         "--force" => {
            force = true;
         }
         "--help" | "-h" => {
            print_help();
            process::exit(0);
         }
         other => {
            return Err(format!("Unknown argument: {other}"));
         }
      }
      i += 1;
   }

   let project_dir = project_dir.ok_or("--project-dir is required")?;

   Ok(Args { project_dir, force })
}

fn print_help() {
   println!(
      r#"scaffold_project — writes build.rs and rel.sh for a Rust CLI project

USAGE:
  cargo run --example scaffold_project -- --project-dir <PATH> [--force]

OPTIONS:
  --project-dir <PATH>   Path to the target project (must contain Cargo.toml)
  --force                Overwrite existing files
  --help, -h             Show this help

EXAMPLE:
  cargo run --example scaffold_project -- --project-dir ../vr
  cargo run --example scaffold_project -- --project-dir ../filebydaterust --force"#
   );
}

// ---------------------------------------------------------------------------
// Cargo.toml parsing
// ---------------------------------------------------------------------------

struct ProjectConfig {
   binary_name: String,
   windows_exe: bool
}

fn parse_cargo_toml(project_dir: &Path) -> Result<ProjectConfig, String> {
   let cargo_toml_path = project_dir.join("Cargo.toml");
   let content =
      fs::read_to_string(&cargo_toml_path).map_err(|e| format!("Failed to read {}: {e}", cargo_toml_path.display()))?;

   let binary_name = parse_binary_name(&content)?;
   let windows_exe = has_windows_dependencies(&content);

   Ok(ProjectConfig { binary_name, windows_exe })
}

/// Returns the first `[[bin]]` `name` value, or `[package]` `name` as fallback.
fn parse_binary_name(toml: &str) -> Result<String, String> {
   // Look for a [[bin]] section first.
   // We scan for the pattern:    name = "..."   that follows a [[bin]] header.
   let mut in_bin_section = false;
   for line in toml.lines() {
      let trimmed = line.trim();
      if trimmed == "[[bin]]" {
         in_bin_section = true;
         continue;
      }
      // Any new section header ends the [[bin]] block
      if trimmed.starts_with('[') {
         in_bin_section = false;
      }
      if in_bin_section {
         if let Some(name) = extract_string_value(trimmed, "name") {
            return Ok(name);
         }
      }
   }

   // Fall back to [package] name
   let mut in_package_section = false;
   for line in toml.lines() {
      let trimmed = line.trim();
      if trimmed == "[package]" {
         in_package_section = true;
         continue;
      }
      if trimmed.starts_with('[') {
         in_package_section = false;
      }
      if in_package_section {
         if let Some(name) = extract_string_value(trimmed, "name") {
            return Ok(name);
         }
      }
   }

   Err("Could not determine binary name from Cargo.toml (no [[bin]] name or [package] name found)".to_string())
}

/// Returns true if the Cargo.toml contains a `[target.'cfg(windows)'.dependencies]` section.
fn has_windows_dependencies(toml: &str) -> bool {
   toml.lines().any(|line| {
      let t = line.trim();
      t.starts_with("[target.") && t.contains("cfg(windows)") && t.contains("dependencies")
   })
}

/// Extracts the string value from a line like `key = "value"`.
fn extract_string_value(line: &str, key: &str) -> Option<String> {
   let prefix = format!("{key} =");
   let line = line.trim();
   if !line.starts_with(&prefix) {
      return None;
   }
   let rest = line[prefix.len()..].trim();
   if rest.starts_with('"') && rest.ends_with('"') && rest.len() >= 2 {
      Some(rest[1..rest.len() - 1].to_string())
   } else {
      None
   }
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn generate_build_rs() -> String {
   r#"fn main() {
    jt_consoleutils::build_support::emit_build_info();
}
"#
   .to_string()
}

fn generate_rel_sh(config: &ProjectConfig) -> String {
   let mut s = String::new();

   writeln!(s, "#!/bin/bash").unwrap();
   writeln!(s, "set -euo pipefail").unwrap();
   writeln!(s).unwrap();
   writeln!(s, r#"TARGET="${{1:-$(rustc -vV | grep '^host:' | cut -d' ' -f2)}}""#).unwrap();
   writeln!(s).unwrap();

   if config.windows_exe {
      writeln!(s, "# Windows targets produce .exe binaries").unwrap();
      writeln!(s, r#"case "${{TARGET}}" in"#).unwrap();
      writeln!(s, r#"  *-windows-*) BINARY="target/${{TARGET}}/release/{}.exe" ;;"#, config.binary_name).unwrap();
      writeln!(s, r#"  *)           BINARY="target/${{TARGET}}/release/{}" ;;"#, config.binary_name).unwrap();
      writeln!(s, "esac").unwrap();
   } else {
      writeln!(s, r#"BINARY="target/${{TARGET}}/release/{}""#, config.binary_name).unwrap();
   }

   writeln!(s).unwrap();
   writeln!(s, r#"echo "Building for target: ${{TARGET}}""#).unwrap();
   writeln!(s).unwrap();

   writeln!(s, r#"RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none -Zunstable-options -Cpanic=immediate-abort" cargo +nightly build \"#).unwrap();
   writeln!(s, r#"  -Z build-std=std,panic_abort \"#).unwrap();
   writeln!(s, r#"  -Z build-std-features="optimize_for_size" \"#).unwrap();
   writeln!(s, r#"  --target "${{TARGET}}" --release"#).unwrap();
   writeln!(s).unwrap();
   writeln!(s, r#"ls -al "${{BINARY}}""#).unwrap();
   writeln!(s, r#""${{BINARY}}" -h"#).unwrap();

   s
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
   let args = match parse_args() {
      Ok(a) => a,
      Err(e) => {
         eprintln!("Error: {e}");
         eprintln!("Run with --help for usage.");
         process::exit(1);
      }
   };

   let config = match parse_cargo_toml(&args.project_dir) {
      Ok(c) => c,
      Err(e) => {
         eprintln!("Error: {e}");
         process::exit(1);
      }
   };

   println!("Inferred binary name : {}", config.binary_name);
   println!("Windows .exe support : {}", config.windows_exe);
   println!();

   let build_rs_path = args.project_dir.join("build.rs");
   let rel_sh_path = args.project_dir.join("rel.sh");

   let build_rs_content = generate_build_rs();
   let rel_sh_content = generate_rel_sh(&config);

   write_file(&build_rs_path, &build_rs_content, args.force);
   write_file(&rel_sh_path, &rel_sh_content, args.force);

   // Make rel.sh executable on Unix
   #[cfg(unix)]
   {
      use std::os::unix::fs::PermissionsExt;
      if let Ok(meta) = fs::metadata(&rel_sh_path) {
         let mut perms = meta.permissions();
         perms.set_mode(perms.mode() | 0o755);
         let _ = fs::set_permissions(&rel_sh_path, perms);
      }
   }

   println!();
   println!("Scaffolded:");
   println!("  {}", build_rs_path.display());
   println!("  {}", rel_sh_path.display());
   println!();
   println!("Next steps:");
   println!("  1. Add to your Cargo.toml:");
   println!("       [build-dependencies]");
   println!("       jt-consoleutils = {{ path = \"../jt-consoleutils\", features = [\"build-support\"] }}");
   println!("  2. Use the env vars in your application:");
   println!("       const BUILD_DATE: &str = env!(\"BUILD_DATE\");");
   println!("       const GIT_HASH:   &str = env!(\"GIT_HASH\");");
}

fn write_file(path: &PathBuf, content: &str, force: bool) {
   if path.exists() && !force {
      println!("Skipping {} (already exists, use --force to overwrite)", path.display());
      return;
   }
   match fs::write(path, content) {
      Ok(_) => println!("Wrote {}", path.display()),
      Err(e) => {
         eprintln!("Failed to write {}: {e}", path.display());
         process::exit(1);
      }
   }
}
