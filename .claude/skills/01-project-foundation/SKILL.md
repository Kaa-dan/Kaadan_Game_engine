# 01 — Project Foundation

## Description
Set up the Cargo workspace, crate structure, tooling, and CI configuration for KaadanEngine — a mobile-first Rust game engine targeting Android, iOS, and HyperOS.

## Phase
1 — Scaffolding & Core Types

## Prerequisites
None — this is the starting point.

## Architecture

KaadanEngine is a modular workspace with these member crates:

```
kaadan_engine/          ← workspace root
├── Cargo.toml          ← workspace manifest
├── crates/
│   ├── kaadan_math/       ← Vec2/3/4, Mat4, Transform, Color, Handle<T>
│   ├── kaadan_core/       ← tracing, error types, engine-wide utilities
│   ├── kaadan_platform/   ← Platform trait, winit/Android/iOS backends
│   ├── kaadan_renderer/   ← wgpu renderer, shaders, pipelines
│   ├── kaadan_ecs/        ← ECS world, systems, queries
│   ├── kaadan_input/      ← Input mapping, touch gestures
│   ├── kaadan_audio/      ← Audio playback via rodio
│   ├── kaadan_assets/     ← Asset server, loaders, caching
│   ├── kaadan_scene/      ← Scene graph, serialization
│   ├── kaadan_ui/         ← Retained-mode UI widgets
│   ├── kaadan_physics/    ← Rapier integration
│   └── kaadan_app/        ← App struct, plugin system, game loop orchestration
├── examples/              ← Runnable demo programs
├── assets/                ← Test assets (textures, sounds, models)
├── .cargo/config.toml     ← Target aliases, linker config
├── rustfmt.toml
├── clippy.toml
└── README.md
```

### Crate Dependency Graph (internal)

```
kaadan_math ← kaadan_core
     ↑              ↑
kaadan_platform ← kaadan_renderer
     ↑              ↑
kaadan_ecs     kaadan_input / kaadan_audio
     ↑              ↑
kaadan_assets ← kaadan_scene / kaadan_ui
     ↑
kaadan_physics
     ↑
kaadan_app (depends on all, re-exports public API)
```

## Step-by-Step Implementation

### 1. Create the Workspace Root `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "crates/kaadan_math",
    "crates/kaadan_core",
    "crates/kaadan_platform",
    "crates/kaadan_renderer",
    "crates/kaadan_ecs",
    "crates/kaadan_input",
    "crates/kaadan_audio",
    "crates/kaadan_assets",
    "crates/kaadan_scene",
    "crates/kaadan_ui",
    "crates/kaadan_physics",
    "crates/kaadan_app",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.80"
license = "MIT OR Apache-2.0"
repository = "https://github.com/kaadan/KaadanEngine"

[workspace.dependencies]
# Math
glam = "0.29"
bytemuck = { version = "1.19", features = ["derive"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Platform
winit = "0.30"
raw-window-handle = "0.6"

# Rendering
wgpu = "23"
pollster = "0.4"

# ECS
hecs = "0.10"
rayon = "1.10"

# Audio
rodio = "0.19"

# Input
gilrs = "0.11"

# Assets
tokio = { version = "1", features = ["rt", "fs", "sync"] }
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
notify = "7"

# Serialization
serde = { version = "1", features = ["derive"] }
ron = "0.8"

# Physics
rapier2d = "0.22"
parry2d = "0.17"

# 3D
gltf = "1.4"

# Text
fontdue = "0.9"

# Error handling
thiserror = "2"

# Android
android-activity = { version = "0.6", features = ["native-activity"] }
ndk = "0.9"
jni = "0.21"

# Profiling
puffin = "0.19"
```

### 2. Create Each Member Crate

For each crate, run `cargo init --lib crates/<name>` and set up minimal `Cargo.toml`:

```toml
# Example: crates/kaadan_math/Cargo.toml
[package]
name = "kaadan_math"
version.workspace = true
edition.workspace = true

[dependencies]
glam = { workspace = true }
bytemuck = { workspace = true }
```

Each crate's `lib.rs` should start with a module doc comment and re-export nothing until implemented:

```rust
//! KaadanEngine math primitives — wraps glam with engine-specific types.
```

### 3. Configure `.cargo/config.toml`

```toml
# Target aliases for mobile cross-compilation
[target.aarch64-linux-android]
linker = "aarch64-linux-android35-clang"

[target.armv7-linux-androideabi]
linker = "armv7a-linux-androideabi35-clang"

[target.aarch64-apple-ios]
# Uses default Xcode toolchain

[target.aarch64-apple-ios-sim]
# Uses default Xcode toolchain

# Aliases
[alias]
android-arm64 = "build --target aarch64-linux-android"
android-arm32 = "build --target armv7-linux-androideabi"
ios = "build --target aarch64-apple-ios"
ios-sim = "build --target aarch64-apple-ios-sim"
```

### 4. Configure `rustfmt.toml`

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
use_try_shorthand = true
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

### 5. Configure `clippy.toml`

```toml
# Clippy configuration
cognitive-complexity-threshold = 30
too-many-arguments-threshold = 8
type-complexity-threshold = 300
```

Also add to the workspace `Cargo.toml`:

```toml
[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
# Allow common game engine patterns
cast_precision_loss = "allow"
cast_possible_truncation = "allow"
module_name_repetitions = "allow"
must_use_candidate = "allow"
```

### 6. Set Up `cargo-deny` (`deny.toml`)

```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "Unicode-3.0"]

[bans]
multiple-versions = "warn"
```

### 7. Create `README.md`

Document the engine architecture, crate graph, build instructions for desktop/Android/iOS, and link to each skill for the learning path.

## Deliverables Checklist

- [ ] `Cargo.toml` workspace root with all 12 member crates listed
- [ ] All 12 sub-crate directories exist under `crates/` with valid `Cargo.toml` and `lib.rs`
- [ ] `.cargo/config.toml` with Android/iOS target aliases and linker config
- [ ] `rustfmt.toml` with project conventions
- [ ] `clippy.toml` with game-engine-friendly thresholds
- [ ] `deny.toml` for license and advisory checks
- [ ] `README.md` describing engine architecture and crate dependency graph
- [ ] `cargo build` succeeds across the workspace
- [ ] `cargo test` passes (even if tests are empty)
- [ ] `cargo clippy -- -D warnings` reports zero warnings

## Common Pitfalls

1. **Workspace resolver must be "2"** — Rust 2021 edition requires resolver v2 for correct feature unification across workspace members.

2. **Don't add all dependencies to every crate** — each sub-crate should only depend on what it actually uses. Use `workspace = true` inheritance to keep versions consistent.

3. **Android NDK path** — `cargo-ndk` requires `ANDROID_NDK_HOME` to be set. On macOS: `~/Library/Android/sdk/ndk/<version>`. Don't hardcode paths in config.

4. **iOS requires Xcode** — The `aarch64-apple-ios` target needs Xcode command-line tools. Run `xcode-select --install` if missing.

5. **Don't over-scope crates** — It's tempting to put everything in one crate. The modular structure pays off when you need to compile only `kaadan_math` for tests or when mobile builds exclude desktop-only crates.

6. **Feature flags for platform backends** — Use Cargo features (not `#[cfg(target_os)]` everywhere) so platform backends can be opt-in for cleaner dependency trees.

## References

- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [cargo-deny](https://embarkstudios.github.io/cargo-deny/)
- [Bevy's crate structure](https://github.com/bevyengine/bevy/tree/main/crates) (inspiration for modular engine layout)
- [Rust Android cross-compilation](https://mozilla.github.io/cargo-ndk/)
