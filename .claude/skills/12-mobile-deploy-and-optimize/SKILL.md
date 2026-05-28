# 12 — Mobile Deploy and Optimize

## Description
Full mobile deployment: APK building (Android/HyperOS), IPA archives (iOS), performance profiling, memory optimization, battery-aware frame pacing, app lifecycle management. The capstone skill that makes the engine shippable.

## Phase
6 — Ship It

## Prerequisites
- All previous skills (01–11) — this is the integration and deployment phase

## Complexity
High — platform-specific toolchains, profiling, optimization

## Architecture Decisions

### Android via cargo-ndk + Gradle
- `cargo-ndk` cross-compiles Rust to Android native libraries (`.so`)
- A thin Gradle wrapper packages the `.so` into an APK with manifest, resources, signing
- This is how Bevy, Makepad, and other Rust projects target Android
- HyperOS is Android-based (AOSP), so the same APK works

### iOS via Xcode project
- Rust compiles to a static library targeting `aarch64-apple-ios`
- An Xcode project wraps it with `Info.plist`, launch screen, signing
- `cargo-xcode` or manual Xcode project generation
- Simulator builds use `aarch64-apple-ios-sim` target

### Adaptive frame pacing
- Mobile devices thermal-throttle under sustained load
- Rather than always targeting 60fps and dropping to 40fps when hot, explicitly target 30fps when thermal state is elevated
- Android: `PowerManager.getThermalStatus()`
- iOS: `ProcessInfo.thermalState`
- This preserves battery and avoids janky frame times

### Memory budget
- Mobile devices have 2–6GB RAM, shared with the OS and other apps
- Practical budget: ~500MB for a game on low-end devices
- Android: `ActivityManager.getMemoryClass()` returns the per-app limit
- iOS: `os_proc_available_memory()` returns remaining memory
- The engine should track allocations and respond to low-memory warnings

## Step-by-Step Implementation

### 1. Android Build Pipeline

#### Directory Structure
```
android/
├── app/
│   ├── build.gradle.kts
│   ├── src/main/
│   │   ├── AndroidManifest.xml
│   │   ├── java/com/kaadan/engine/MainActivity.kt
│   │   └── res/
│   │       ├── values/strings.xml
│   │       └── mipmap-*/ic_launcher.png
├── build.gradle.kts
├── gradle.properties
├── settings.gradle.kts
└── gradlew
```

#### AndroidManifest.xml
```xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.kaadan.engine">

    <application
        android:label="@string/app_name"
        android:icon="@mipmap/ic_launcher"
        android:theme="@android:style/Theme.NoTitleBar.Fullscreen"
        android:hasCode="true">

        <activity
            android:name="android.app.NativeActivity"
            android:configChanges="orientation|screenSize|keyboardHidden"
            android:exported="true">

            <meta-data
                android:name="android.app.lib_name"
                android:value="kaadan_engine" />

            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
```

#### Build Script
```bash
#!/bin/bash
# build_android.sh — Build APK for Android/HyperOS

set -e

# 1. Build Rust native library for target architectures
cargo ndk --target aarch64-linux-android --target armv7-linux-androideabi \
    -o android/app/src/main/jniLibs build --release -p kaadan_app

# 2. Build APK via Gradle
cd android
./gradlew assembleRelease

echo "APK at: android/app/build/outputs/apk/release/app-release.apk"
```

### 2. iOS Build Pipeline

#### Xcode Project Structure
```
ios/
├── KaadanEngine.xcodeproj/
├── KaadanEngine/
│   ├── Info.plist
│   ├── LaunchScreen.storyboard
│   ├── AppDelegate.swift
│   └── Assets.xcassets/
└── build_ios.sh
```

#### Build Script
```bash
#!/bin/bash
# build_ios.sh — Build for iOS device or simulator

set -e

TARGET=${1:-"aarch64-apple-ios-sim"} # Default: simulator

# 1. Build Rust static library
cargo build --release --target $TARGET -p kaadan_app

# 2. Copy library to Xcode project
cp target/$TARGET/release/libkaadan_app.a ios/KaadanEngine/

# 3. Build with xcodebuild
cd ios
xcodebuild -project KaadanEngine.xcodeproj \
    -scheme KaadanEngine \
    -configuration Release \
    -destination "generic/platform=iOS Simulator" \
    build

echo "Build complete. Open ios/KaadanEngine.xcodeproj in Xcode to run."
```

### 3. App Lifecycle Management

```rust
// crates/kaadan_platform/src/lifecycle.rs

/// Lifecycle state machine for mobile apps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLifecycleState {
    /// App is in the foreground and receiving events
    Active,
    /// App is transitioning to background (save state NOW)
    Suspending,
    /// App is in the background (no rendering, minimal CPU)
    Suspended,
    /// App is returning to foreground (restore resources)
    Resuming,
}

/// Lifecycle handler — systems should respond to state changes.
pub struct LifecycleManager {
    state: AppLifecycleState,
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self { state: AppLifecycleState::Active }
    }

    pub fn state(&self) -> AppLifecycleState {
        self.state
    }

    pub fn on_suspend(&mut self) {
        tracing::info!("App suspending — saving state");
        self.state = AppLifecycleState::Suspending;
        // Renderer must drop the surface
        // Audio should pause
        // Save game state to persistent storage
        self.state = AppLifecycleState::Suspended;
    }

    pub fn on_resume(&mut self) {
        tracing::info!("App resuming");
        self.state = AppLifecycleState::Resuming;
        // Renderer must recreate the surface
        // Audio can resume
        // Restore game state
        self.state = AppLifecycleState::Active;
    }

    pub fn on_low_memory(&self) {
        tracing::warn!("Low memory warning — clearing caches");
        // AssetServer should drop unused cached assets
        // Texture cache should be trimmed
        // Any non-essential allocations should be freed
    }
}
```

### 4. Adaptive Frame Pacing

```rust
// crates/kaadan_app/src/frame_pacer.rs
use std::time::{Duration, Instant};

pub struct FramePacer {
    target_fps: u32,
    target_frame_time: Duration,
    last_frame: Instant,
    /// Accumulated frame times for averaging
    frame_times: Vec<Duration>,
    thermal_state: ThermalState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    Nominal,    // Full speed
    Fair,       // Slightly warm
    Serious,    // Throttle to 30fps
    Critical,   // Minimum work
}

impl FramePacer {
    pub fn new(target_fps: u32) -> Self {
        Self {
            target_fps,
            target_frame_time: Duration::from_secs_f64(1.0 / target_fps as f64),
            last_frame: Instant::now(),
            frame_times: Vec::with_capacity(120),
            thermal_state: ThermalState::Nominal,
        }
    }

    /// Call at the end of each frame. Sleeps if ahead of schedule.
    pub fn pace(&mut self) {
        let elapsed = self.last_frame.elapsed();
        self.frame_times.push(elapsed);
        if self.frame_times.len() > 120 {
            self.frame_times.remove(0);
        }

        // Adaptive: reduce target fps when hot
        let effective_target = match self.thermal_state {
            ThermalState::Nominal | ThermalState::Fair => self.target_frame_time,
            ThermalState::Serious => Duration::from_millis(33), // ~30fps
            ThermalState::Critical => Duration::from_millis(50), // ~20fps
        };

        if elapsed < effective_target {
            std::thread::sleep(effective_target - elapsed);
        }

        self.last_frame = Instant::now();
    }

    pub fn set_thermal_state(&mut self, state: ThermalState) {
        if state != self.thermal_state {
            tracing::info!("Thermal state changed: {:?} → {:?}", self.thermal_state, state);
            self.thermal_state = state;
        }
    }

    pub fn avg_frame_time_ms(&self) -> f32 {
        if self.frame_times.is_empty() { return 0.0; }
        let sum: Duration = self.frame_times.iter().sum();
        sum.as_secs_f32() / self.frame_times.len() as f32 * 1000.0
    }

    pub fn current_fps(&self) -> f32 {
        let avg_ms = self.avg_frame_time_ms();
        if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 }
    }
}
```

### 5. Performance Profiling

```rust
// crates/kaadan_core/src/profiling.rs

/// Lightweight frame profiler.
/// In dev builds, integrates with puffin for visual profiling.
/// In release builds, compiles to no-ops.

#[cfg(feature = "profiling")]
pub use puffin;

/// Profile a scope. No-op in release builds.
#[cfg(feature = "profiling")]
macro_rules! profile_scope {
    ($name:expr) => {
        puffin::profile_scope!($name);
    };
}

#[cfg(not(feature = "profiling"))]
macro_rules! profile_scope {
    ($name:expr) => {};
}

/// Frame statistics.
pub struct FrameStats {
    pub frame_time_ms: f32,
    pub draw_calls: u32,
    pub triangles: u32,
    pub texture_binds: u32,
    pub gpu_memory_mb: f32,
    pub cpu_memory_mb: f32,
}

impl FrameStats {
    pub fn new() -> Self {
        Self {
            frame_time_ms: 0.0,
            draw_calls: 0,
            triangles: 0,
            texture_binds: 0,
            gpu_memory_mb: 0.0,
            cpu_memory_mb: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.draw_calls = 0;
        self.triangles = 0;
        self.texture_binds = 0;
    }
}
```

### 6. Memory Optimization Checklist

```
Mobile Memory Budget Guidelines:
- Total app memory: < 500MB on low-end, < 1GB on high-end
- Texture memory: largest consumer. Use compressed formats (ASTC on mobile)
- Mesh data: use indexed meshes, 16-bit indices where possible
- Audio: stream long tracks, don't load entire BGM into memory
- Asset unloading: drop assets when changing scenes

Specific techniques:
1. Texture compression: ASTC 4x4 (Android/iOS) — 4x smaller than RGBA8
2. Mipmap generation: reduces bandwidth for distant textures
3. Texture streaming: load low-res first, swap in high-res
4. Object pooling: reuse entity allocations for bullets, particles
5. String interning: avoid allocating duplicate strings
6. Arena allocators: for per-frame temp data
```

### 7. Android Signing

```bash
# Generate a release keystore (one-time)
keytool -genkey -v -keystore kaadan-release.keystore \
    -alias kaadan -keyalg RSA -keysize 2048 -validity 10000

# In android/app/build.gradle.kts:
android {
    signingConfigs {
        create("release") {
            storeFile = file("../kaadan-release.keystore")
            storePassword = System.getenv("KEYSTORE_PASSWORD")
            keyAlias = "kaadan"
            keyPassword = System.getenv("KEY_PASSWORD")
        }
    }
    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("release")
            isMinifyEnabled = false // No Java code to minify
        }
    }
}
```

### 8. Performance Report Template

```markdown
# Performance Report — KaadanEngine Demo

## Test Devices
| Device | OS | SoC | RAM | GPU |
|--------|-----|-----|-----|-----|
| Pixel 7 | Android 14 | Tensor G2 | 8GB | Mali-G710 |
| Xiaomi 14 | HyperOS 1.0 | Snapdragon 8 Gen 3 | 12GB | Adreno 750 |
| iPhone 13 | iOS 17 | A15 | 4GB | Apple GPU |

## Metrics (1000 sprites + physics)
| Metric | Pixel 7 | Xiaomi 14 | iPhone 13 | Target |
|--------|---------|-----------|-----------|--------|
| FPS | | | | ≥30 |
| Frame time (ms) | | | | ≤33 |
| Draw calls | | | | ≤50 |
| CPU memory (MB) | | | | ≤200 |
| GPU memory (MB) | | | | ≤100 |
| Battery drain (%/hr) | | | | ≤15 |

## Optimization Targets
1. Sprite batching: X draw calls → Y after batching
2. Texture atlas: X texture binds → Y after atlasing
3. Asset loading: X ms cold start → Y ms with async
4. Physics: X ms per step for Y bodies
```

## Deliverables Checklist

- [ ] Android build pipeline: `cargo-ndk` → Gradle → signed APK
- [ ] iOS build pipeline: cargo → static lib → Xcode → IPA
- [ ] Build scripts: `build_android.sh`, `build_ios.sh`
- [ ] `AndroidManifest.xml` with NativeActivity configuration
- [ ] Xcode project with `Info.plist` and launch screen
- [ ] Lifecycle handling: pause/resume, surface loss/recreate, low-memory
- [ ] Adaptive frame pacing based on thermal state
- [ ] `FramePacer` with configurable target FPS
- [ ] `FrameStats` tracking draw calls, memory, frame time
- [ ] Profiling macros (puffin in dev, no-op in release)
- [ ] Memory optimization documentation
- [ ] APK runs on Android device at 30+ fps
- [ ] iOS builds for Simulator
- [ ] Performance report with measured metrics

## Common Pitfalls

1. **Android NDK version mismatch** — `cargo-ndk` requires a specific NDK version. Check compatibility. NDK r26+ works with recent `android-activity` versions.

2. **iOS signing** — You need an Apple Developer account ($99/year) for device deployment. Simulator doesn't require signing.

3. **Gradle version conflicts** — Android Gradle Plugin (AGP) versions are tightly coupled with Gradle versions. Use the versions specified by Google's compatibility matrix.

4. **Native library stripping** — Release builds should strip debug symbols from `.so` files. Add `strip = true` to `[profile.release]` in `Cargo.toml`.

5. **Battery testing requires real devices** — Emulators/simulators don't model battery drain. Always test battery impact on physical hardware.

6. **Thermal throttling is real** — A benchmark that runs great for 10 seconds may drop to 20fps after 5 minutes when the device heats up. Test sustained performance.

7. **Android back button** — Handle the back button (`KeyCode::Back`). Not handling it makes the app feel broken. Usually: back from current screen or show exit confirmation.

8. **HyperOS specifics** — HyperOS (Xiaomi) is AOSP-based but has aggressive battery optimization. Users may need to whitelist the app. Test on Xiaomi devices specifically.

## Cargo Profile for Release

```toml
# Cargo.toml — workspace root
[profile.release]
opt-level = 3       # Maximum optimization
lto = "thin"        # Link-time optimization (thin for faster compile)
codegen-units = 1   # Single codegen unit for maximum optimization
strip = true        # Strip debug symbols from binaries
panic = "abort"     # Smaller binary, no unwinding on mobile
```

## References

- [cargo-ndk docs](https://github.com/nickelc/cargo-ndk)
- [Android NativeActivity](https://developer.android.com/ndk/guides/concepts#naa)
- [Rust on iOS](https://mozilla.github.io/Firefox-browser-architecture/experiments/2017-09-21-rust-on-ios.html)
- [Android thermal API](https://developer.android.com/reference/android/os/PowerManager#getThermalStatus())
- [iOS thermal state](https://developer.apple.com/documentation/foundation/processinfo/1617047-thermalstate)
- [puffin profiler](https://github.com/EmbarkStudios/puffin)
- [Mobile GPU optimization](https://developer.arm.com/documentation/102662/0100)
