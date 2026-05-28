# KaadanEngine

A Rust game engine targeting Android, iOS, and HyperOS.

## Architecture

```
kaadan_app          <- Application entry point, ties everything together
  |
  +-- kaadan_scene       Scene graph & UI framework
  |     +-- kaadan_ui
  |     +-- kaadan_ecs
  |
  +-- kaadan_renderer    GPU rendering (wgpu)
  |     +-- kaadan_math
  |
  +-- kaadan_physics     2D physics (Rapier)
  +-- kaadan_input       Input handling (keyboard, touch, gamepad)
  +-- kaadan_audio       Audio playback
  +-- kaadan_assets      Asset loading & hot-reload
  +-- kaadan_platform    Platform abstraction (windowing, lifecycle)
  +-- kaadan_core        Logging, errors, shared types
  +-- kaadan_math        Linear algebra (glam)
```

## Building

```sh
# Build all crates
cargo build

# Run clippy
cargo clippy -- -D warnings

# Run tests
cargo test
```

## License

MIT OR Apache-2.0
