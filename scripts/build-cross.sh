#!/bin/sh
# macOS cross-build entry point; honors the user's default sccache wrapper.
set -eu
cd "$(dirname "$0")/.."
case "${1:-all}" in
  windows) targets="x86_64-pc-windows-gnu" ;;
  linux) targets="x86_64-unknown-linux-gnu.2.28" ;;
  all) targets="x86_64-pc-windows-gnu x86_64-unknown-linux-gnu.2.28" ;;
  *) echo "Usage: $0 [windows|linux|all]" >&2; exit 2 ;;
esac
# Avoid rustup-shim process permission failures inside sccache.
if [ -z "${RUSTC:-}" ]; then RUSTC=$(rustup which rustc); export RUSTC; fi
for target in $targets; do
  cargo zigbuild --release --locked --target "$target"
  case "$target" in
    *windows*) python3 scripts/package-cross.py windows ;;
    *linux*) python3 scripts/package-cross.py linux ;;
  esac
done
