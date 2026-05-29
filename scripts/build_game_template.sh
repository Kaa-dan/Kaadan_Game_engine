#!/usr/bin/env bash
# Build the game_template gameplay crate as a cdylib for hot-reload.
#
# Usage:
#   scripts/build_game_template.sh            # debug build
#   scripts/build_game_template.sh --release  # release build
#
# The produced library lands in target/<profile>/ with a platform-specific name:
#   Linux:   libgame_template.so
#   macOS:   libgame_template.dylib
#   Windows: game_template.dll
set -euo pipefail

# Resolve the workspace root (this script lives in <root>/scripts).
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo build --manifest-path "$ROOT/Cargo.toml" -p game_template "$@"

case "$(uname -s)" in
    Darwin) LIB="libgame_template.dylib" ;;
    Linux)  LIB="libgame_template.so" ;;
    *)      LIB="game_template.dll" ;;
esac

PROFILE="debug"
for arg in "$@"; do
    if [ "$arg" = "--release" ]; then PROFILE="release"; fi
done

echo "Built: $ROOT/target/$PROFILE/$LIB"
