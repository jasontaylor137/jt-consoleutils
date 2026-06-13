#!/bin/bash
set -e

echo "==> cargo build"
cargo build

echo "==> cargo test"
cargo test -- --format terse -q

echo "==> cargo clippy"
cargo clippy

echo "==> all checks passed"
