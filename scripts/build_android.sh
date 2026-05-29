#!/usr/bin/env bash
# Build KaadanEngine native libraries for Android via cargo-ndk, then (optionally)
# package them into an APK with Gradle.
#
# Scaffold — desktop-first build. To actually produce a runnable APK you still need:
#   * Android SDK + NDK installed, ANDROID_NDK_HOME set
#   * `cargo install cargo-ndk`
#   * an app crate that builds a `cdylib` exposing `android_main` (see
#     kaadan_platform's Android backend, currently a scaffold)
#
# Usage: scripts/build_android.sh [debug|release]
set -euo pipefail

PROFILE="${1:-release}"
TARGETS=("aarch64-linux-android" "armv7-linux-androideabi")

for target in "${TARGETS[@]}"; do
    echo ">> cargo ndk build ($target, $PROFILE)"
    cargo ndk --target "$target" --platform 33 build --"$PROFILE"
done

# With a Gradle project under mobile/android/ wired to the cargo-ndk output:
#   (cd mobile/android && ./gradlew assembleRelease)
echo "Native libraries built ($PROFILE). Point Gradle's jniLibs at the cargo-ndk output to assemble the APK."
