#!/usr/bin/env bash
# Build KaadanEngine as an iOS static library, then (optionally) wrap it in an
# Xcode app target.
#
# Scaffold — desktop-first build. To produce a runnable .app you still need:
#   * Xcode + command-line tools
#   * the iOS Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`
#   * an app crate that builds a `staticlib` with an iOS entry point (see
#     kaadan_platform's iOS backend, currently a scaffold)
#   * an Xcode project linking that .a (use mobile/ios/Info.plist)
#
# Usage: scripts/build_ios.sh [aarch64-apple-ios | aarch64-apple-ios-sim]
set -euo pipefail

TARGET="${1:-aarch64-apple-ios-sim}"

echo ">> cargo build --release --target $TARGET"
cargo build --release --target "$TARGET"

# With an Xcode project under mobile/ios/ linking the produced .a:
#   xcodebuild -project mobile/ios/KaadanEngine.xcodeproj -scheme KaadanEngine -sdk iphonesimulator build
echo "iOS static library built for $TARGET. Link it from an Xcode project (mobile/ios/Info.plist) to produce the .app."
