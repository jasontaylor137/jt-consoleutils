#!/bin/bash
set -euo pipefail

if [ $# -eq 0 ]; then
  echo "scaffold.sh — generate build.rs and rel.sh for a Rust CLI project"
  echo ""
  echo "USAGE:"
  echo "  ./scaffold.sh <project-dir>"
  echo "  ./scaffold.sh <project-dir> --force"
  echo ""
  echo "EXAMPLES:"
  echo "  ./scaffold.sh ../vr"
  echo "  ./scaffold.sh ../filebydaterust --force"
  echo ""
  echo "Everything is inferred from the target project's Cargo.toml:"
  echo "  - Binary name  : first [[bin]] name, or [package] name as fallback"
  echo "  - Windows .exe : detected from [target.'cfg(windows)'.dependencies]"
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cargo run --manifest-path "${SCRIPT_DIR}/Cargo.toml" --example scaffold_project -- --project-dir "$@"
