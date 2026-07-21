#!/usr/bin/env bash
# Build the KaadanEngine game as an Android APK.
#
# Pipeline: cargo-ndk compiles the `kaadan_android` crate to a per-ABI cdylib
# (libkaadan_android.so), this script stages each .so under
# mobile/android/jniLibs/<abi>/, then Gradle assembles the APK (which the
# NativeActivity loads via `android.app.lib_name = kaadan_android`).
#
# Prerequisites (one-time):
#   * Android SDK + NDK installed; export ANDROID_NDK_HOME (or ANDROID_NDK_ROOT)
#   * cargo install cargo-ndk
#   * rustup target add aarch64-linux-android   (add others to ABIS below)
#   * A JDK + Gradle (or open mobile/android/ in Android Studio) to assemble.
#
# Usage: scripts/build_android.sh [debug|release]
set -euo pipefail

PROFILE="${1:-debug}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_DIR="$REPO_ROOT/mobile/android"
JNILIBS_DIR="$ANDROID_DIR/jniLibs"

# "abi:rust-target" pairs (portable to macOS bash 3.2 — no associative arrays).
# Start with arm64 only; add more here once verified (and mirror them in
# build.gradle.kts abiFilters):
#   armeabi-v7a:armv7-linux-androideabi
#   x86_64:x86_64-linux-android          (emulator)
ABIS=(
    "arm64-v8a:aarch64-linux-android"
)

CARGO_PROFILE_FLAG=""
CARGO_OUT_DIR="debug"
if [[ "$PROFILE" == "release" ]]; then
    CARGO_PROFILE_FLAG="--release"
    CARGO_OUT_DIR="release"
fi

echo ">> Building kaadan_android cdylib for Android ($PROFILE)"
for pair in "${ABIS[@]}"; do
    abi="${pair%%:*}"
    target="${pair##*:}"
    echo "   - $abi ($target)"
    cargo ndk --target "$target" --platform 24 \
        build -p kaadan_android $CARGO_PROFILE_FLAG

    mkdir -p "$JNILIBS_DIR/$abi"
    cp "$REPO_ROOT/target/$target/$CARGO_OUT_DIR/libkaadan_android.so" \
        "$JNILIBS_DIR/$abi/libkaadan_android.so"
done

echo ">> Staged native libs under $JNILIBS_DIR"

# Assemble the APK if Gradle is available; otherwise stop after staging.
if command -v gradle >/dev/null 2>&1; then
    GRADLE_TASK="assembleDebug"
    [[ "$PROFILE" == "release" ]] && GRADLE_TASK="assembleRelease"
    echo ">> gradle $GRADLE_TASK"
    (cd "$ANDROID_DIR" && gradle "$GRADLE_TASK")
    echo ">> APK written under $ANDROID_DIR/build/outputs/apk/"
    echo "   Install with: adb install -r <path-to-apk>"
else
    echo ">> Gradle not found. Native libs are staged; open mobile/android/ in"
    echo "   Android Studio, or install Gradle, then run 'gradle assembleDebug'."
fi
