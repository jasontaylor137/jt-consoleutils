//! Scaffolds `build.rs` and `rel.sh` for a new Rust CLI project that uses
//! `jt-consoleutils` build support.
//!
//! # Usage
//!
//! ```sh
//! cargo run --example scaffold_project -- \
//!   --binary-name mybinary \
//!   --install aarch64-apple-darwin:~/.local/bin \
//!   --install x86_64-unknown-linux-gnu:/var/home/jason/.local/bin \
//!   --install x86_64-pc-windows-msvc:/c/Users/jason/.local/bin \
//!   --windows-exe
//! ```
//!
//! Run from the root of the project you want to scaffold, or pass `--output-dir`
//! to specify a destination. Files will NOT be overwritten unless `--force` is given.

use std::{collections::BTreeMap, fmt::Write as FmtWrite, fs, path::PathBuf, process};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

struct Args {
   binary_name: String,
   installs: BTreeMap<String, String>,
   windows_exe: bool,
   extra_rustflags: Vec<String>,
   output_dir: PathBuf,
   force: bool
}

fn parse_args() -> Result<Args, String> {
   let raw: Vec<String> = std::env::args().skip(1).collect();
   let mut binary_name: Option<String> = None;
   let mut installs: BTreeMap<String, String> = BTreeMap::new();
   let mut windows_exe = false;
   let mut extra_rustflags: Vec<String> = Vec::new();
   let mut output_dir: Option<PathBuf> = None;
   let mut force = false;

   let mut i = 0;
   while i < raw.len() {
      match raw[i].as_str() {
         "--binary-name" => {
            i += 1;
            binary_name = Some(next_value(&raw, i, "--binary-name")?);
         }
         "--install" => {
            i += 1;
            let val = next_value(&raw, i, "--install")?;
            let (target, path) =
               val.split_once(':').ok_or_else(|| format!("--install expects TARGET:PATH, got: {val}"))?;
            installs.insert(target.to_string(), path.to_string());
         }
         "--windows-exe" => {
            windows_exe = true;
         }
         "--extra-rustflag" => {
            i += 1;
            extra_rustflags.push(next_value(&raw, i, "--extra-rustflag")?);
         }
         "--output-dir" => {
            i += 1;
            output_dir = Some(PathBuf::from(next_value(&raw, i, "--output-dir")?));
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

   let binary_name = binary_name.ok_or("--binary-name is required")?;
   let output_dir = output_dir.unwrap_or_else(|| PathBuf::from("."));

   Ok(Args { binary_name, installs, windows_exe, extra_rustflags, output_dir, force })
}

fn next_value(raw: &[String], i: usize, flag: &str) -> Result<String, String> {
   raw.get(i).cloned().ok_or_else(|| format!("{flag} requires a value"))
}

fn print_help() {
   println!(
      r#"scaffold_project — writes build.rs and rel.sh for a new Rust CLI project

USAGE:
  cargo run --example scaffold_project -- [OPTIONS]

OPTIONS:
  --binary-name <NAME>          Binary name (required, e.g. "mybinary")
  --install <TARGET:PATH>       Install mapping, repeatable
                                  e.g. aarch64-apple-darwin:~/.local/bin
  --windows-exe                 Emit .exe suffix handling for Windows targets
  --extra-rustflag <FLAG>       Extra RUSTFLAG to pass to cargo, repeatable
                                  e.g. -Zunstable-options
  --output-dir <DIR>            Directory to write files into (default: .)
  --force                       Overwrite existing files
  --help, -h                    Show this help

EXAMPLE:
  cargo run --example scaffold_project -- \
    --binary-name vr \
    --install aarch64-apple-darwin:~/.vr/bin \
    --install x86_64-unknown-linux-gnu:/var/home/jason/.vr/bin \
    --install x86_64-pc-windows-msvc:/c/Users/jason/.vr/bin \
    --windows-exe \
    --extra-rustflag -Zunstable-options \
    --extra-rustflag -Cpanic=immediate-abort"#
   );
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

fn generate_rel_sh(args: &Args) -> String {
   let mut s = String::new();

   writeln!(s, "#!/bin/bash").unwrap();
   writeln!(s, "set -euo pipefail").unwrap();
   writeln!(s).unwrap();
   writeln!(s, r#"TARGET="${{1:-$(rustc -vV | grep '^host:' | cut -d' ' -f2)}}""#).unwrap();
   writeln!(s).unwrap();

   if args.windows_exe {
      writeln!(s, "# Windows targets produce .exe binaries").unwrap();
      writeln!(s, r#"case "${{TARGET}}" in"#).unwrap();
      writeln!(s, r#"  *-windows-*) BINARY="target/${{TARGET}}/release/{}.exe" ;;"#, args.binary_name).unwrap();
      writeln!(s, r#"  *)           BINARY="target/${{TARGET}}/release/{}" ;;"#, args.binary_name).unwrap();
      writeln!(s, "esac").unwrap();
   } else {
      writeln!(s, r#"BINARY="target/${{TARGET}}/release/{}""#, args.binary_name).unwrap();
   }

   writeln!(s).unwrap();
   writeln!(s, r#"echo "Building for target: ${{TARGET}}""#).unwrap();
   writeln!(s).unwrap();

   // Base rustflags — always present
   let mut rustflags = vec!["-Zlocation-detail=none".to_string(), "-Zfmt-debug=none".to_string()];
   rustflags.extend(args.extra_rustflags.iter().cloned());
   let rustflags_str = rustflags.join(" ");

   writeln!(s, r#"RUSTFLAGS="{rustflags_str}" cargo +nightly build \"#).unwrap();
   writeln!(s, r#"  -Z build-std=std,panic_abort \"#).unwrap();
   writeln!(s, r#"  -Z build-std-features="optimize_for_size" \"#).unwrap();
   writeln!(s, r#"  --target "${{TARGET}}" --release"#).unwrap();

   if !args.installs.is_empty() {
      writeln!(s).unwrap();
      writeln!(s, "# Install to platform-specific location").unwrap();
      writeln!(s, r#"case "${{TARGET}}" in"#).unwrap();
      for (target, path) in &args.installs {
         writeln!(s, r#"  {target})"#).unwrap();
         writeln!(s, r#"    cp "${{BINARY}}" {path} ;;"#).unwrap();
      }
      writeln!(s, r#"  *)"#).unwrap();
      writeln!(s, r#"    echo "No install path configured for ${{TARGET}}, skipping install" ;;"#).unwrap();
      writeln!(s, "esac").unwrap();
   }

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

   if let Err(e) = fs::create_dir_all(&args.output_dir) {
      eprintln!("Failed to create output directory {}: {e}", args.output_dir.display());
      process::exit(1);
   }

   let build_rs_path = args.output_dir.join("build.rs");
   let rel_sh_path = args.output_dir.join("rel.sh");

   let build_rs_content = generate_build_rs();
   let rel_sh_content = generate_rel_sh(&args);

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
      eprintln!("Skipping {} (already exists, use --force to overwrite)", path.display());
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
