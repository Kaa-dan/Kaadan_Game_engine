# KaadanEngine — Conventions

Project-wide conventions referenced by the [roadmap](plan/ROADMAP.md). Keep this short and current; it is the source of truth for the cross-cutting rules every crate follows.

## Platform scope

- **Editor** (`kaadan_editor`) is a **desktop application**: Linux, macOS, Windows.
- **Games** built with the engine ship to **Android + iOS**, and run on desktop during development (play mode / test builds).
- **HarmonyOS / HyperOS are not targets.**
- **Never break the desktop build.** Mobile-only code lives behind `cfg(target_os = "...")` or a feature flag until its roadmap phase lands.

## Crate naming

- All workspace crates are prefixed `kaadan_` (e.g. `kaadan_renderer`).
- Internal crates are declared once in the root `[workspace.dependencies]` and referenced as `kaadan_x.workspace = true`.
- External crate versions are pinned once in `[workspace.dependencies]`; member crates use `dep.workspace = true`. Do not pin a version in a member `Cargo.toml`.

## Feature-flag policy

- `default` features keep the **desktop** build working with no extra flags.
- Optional / heavy / platform-specific functionality is feature-gated. Current flags:
  - `kaadan_math` → `serde` (cfg-gated `Serialize`/`Deserialize` derives on `Transform`, `Color`, `Rect`, `AABB`; pulls `glam/serde`).
  - `kaadan_renderer` / `kaadan_app` → `gltf` (default on).
  - `kaadan_app` → `profiling` (puffin scopes).
  - `kaadan_assets` → `hot-reload` (filesystem watching via `notify`).
- A feature must not silently change desktop behavior; it adds capability.
- No unused declared dependencies — `cargo machete` / clippy must stay clean. If a dep is only used under a feature, mark it `optional = true` and list it in that feature.

## Asset path conventions

- Runtime assets live under the project `assets/` directory; the `AssetResolver` resolves logical paths relative to it (`FilesystemResolver` walks up to find `assets/`).
- Shaders live in `assets/shaders/*.wgsl` and are embedded at build time via `include_str!` in `kaadan_renderer`.
- Logical asset paths are **forward-slash, relative, no leading slash** (e.g. `textures/player.png`) so the same path works across desktop and the Android `AAssetManager` resolver.
- Scenes are serialized as **RON** (`*.ron`) using the `kaadan_scene` format.

## Per-module requirements (every new module)

- A `lib.rs` (or module-level) doc comment stating the module's intent.
- At least one unit test.
- Its corresponding roadmap ticket marked complete, and `docs/audit/CURRENT_STATE.md` updated at the end of the phase.

## Logging & errors

- Use `tracing` macros (re-exported via `kaadan_core`). Initialize once with `kaadan_core::init_logging()` — it is idempotent and safe to call from both the editor and an embedded runtime.
- Fallible engine APIs return `kaadan_core::KaadanResult<T>`; avoid `unwrap`/`expect` outside tests and top-level binary setup.
