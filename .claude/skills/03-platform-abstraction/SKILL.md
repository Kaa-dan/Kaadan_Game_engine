# 03 — Platform Abstraction

## Description
Build `kaadan_platform` — a uniform `Platform` trait over desktop (winit), Android (android-activity + NDK), and iOS (objc2 UIKit). Handles window creation, event loops, input events, and raw window handle negotiation for the renderer.

## Phase
2 — Platform & Pixels

## Prerequisites
- Skill 01 (`01-project-foundation`) — workspace compiles
- Skill 02 (`02-math-and-core-types`) — Vec2, InputEvent uses math types

## Complexity
High — cross-platform windowing with mobile lifecycle management

## Architecture Decisions

### Why winit for desktop?
- Industry-standard Rust windowing library, used by wgpu, Bevy, and most Rust game engines
- Provides `raw-window-handle` integration that wgpu needs
- Handles X11/Wayland/macOS/Windows behind one API

### Why android-activity instead of raw NDK?
- `android-activity` provides the `NativeActivity` glue that handles JNI, looper, and `ANativeWindow` lifecycle
- Without it, you'd need manual JNI bindings and `android_main` setup
- Works with `cargo-ndk` for cross-compilation

### Why objc2 for iOS?
- Type-safe Rust bindings to Objective-C runtime
- Can create `UIWindow`, configure `CAMetalLayer` for Metal rendering
- Safer than raw `objc` crate with manual selectors

### Mobile-first design
- The `Platform` trait is designed around mobile constraints: touch input, lifecycle events (pause/resume), variable display density
- Desktop keyboard/mouse are mapped to the same `InputEvent` enum
- Surface can be lost/recreated (Android) — the trait accounts for this

## Step-by-Step Implementation

### 1. Define the Platform Trait

```toml
# crates/kaadan_platform/Cargo.toml
[package]
name = "kaadan_platform"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math" }
kaadan_core = { path = "../kaadan_core" }
tracing = { workspace = true }
raw-window-handle = { workspace = true }

[target.'cfg(not(target_os = "android"))'.dependencies]
winit = { workspace = true }

[target.'cfg(target_os = "android")'.dependencies]
android-activity = { workspace = true }
ndk = { workspace = true }
jni = { workspace = true }

[target.'cfg(target_os = "ios")'.dependencies]
# objc2 dependencies added when iOS backend is implemented
```

### 2. Unified Input Events

```rust
// crates/kaadan_platform/src/input_event.rs
use kaadan_math::Vec2;

/// Unified input event consumed identically on all platforms.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Touch began/moved/ended (mobile) or mouse click (desktop)
    Touch(TouchEvent),
    /// Physical key press/release
    Key(KeyEvent),
    /// Window resized
    Resize { width: u32, height: u32 },
    /// App lifecycle change
    Lifecycle(LifecycleEvent),
    /// Window close requested
    CloseRequested,
}

#[derive(Debug, Clone)]
pub struct TouchEvent {
    pub id: u64,
    pub phase: TouchPhase,
    pub position: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub pressed: bool,
}

/// Subset of common key codes. Expand as needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    Space, Enter, Escape, Backspace, Tab,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    ShiftLeft, ShiftRight, ControlLeft, ControlRight,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// App moved to foreground (resume)
    Resumed,
    /// App moved to background (pause) — save state, release GPU resources
    Suspended,
    /// Low memory warning — free caches
    LowMemory,
}
```

### 3. Platform Trait

```rust
// crates/kaadan_platform/src/platform.rs
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// Configuration for window creation.
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "KaadanEngine".to_string(),
            width: 800,
            height: 600,
            resizable: true,
        }
    }
}

/// Callback driven by the platform event loop.
pub trait AppHandler {
    /// Called once when the window is ready and surface is available.
    fn init(&mut self, window: &dyn PlatformWindow);
    /// Called every frame with accumulated input events.
    fn update(&mut self, events: &[InputEvent], dt: f32);
    /// Called when the surface is resized.
    fn resize(&mut self, width: u32, height: u32);
    /// Called on lifecycle events (suspend/resume).
    fn lifecycle(&mut self, event: LifecycleEvent);
    /// Return true to exit the event loop.
    fn should_exit(&self) -> bool;
}

/// Abstraction over the platform window.
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn scale_factor(&self) -> f64;
}
```

### 4. Desktop Backend (winit)

```rust
// crates/kaadan_platform/src/desktop.rs
// (only compiled on non-Android, non-iOS targets)
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

pub struct DesktopPlatform;

impl DesktopPlatform {
    pub fn run(config: WindowConfig, handler: impl AppHandler + 'static) {
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        let mut app = WinitApp::new(config, handler);
        event_loop.run_app(&mut app).expect("Event loop failed");
    }
}

struct WinitApp<H: AppHandler> {
    config: WindowConfig,
    handler: H,
    window: Option<Window>,
    last_frame: std::time::Instant,
    pending_events: Vec<InputEvent>,
}

// ... implement ApplicationHandler for WinitApp, converting winit events
// to InputEvent and calling handler methods
```

### 5. Android Backend (scaffold)

```rust
// crates/kaadan_platform/src/android.rs
// (only compiled on target_os = "android")

#[cfg(target_os = "android")]
use android_activity::{AndroidApp, MainEvent, PollEvent};

/// Entry point for Android. Called by android-activity glue.
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    // Initialize logging to Android logcat
    // Create platform window wrapper around ANativeWindow
    // Run event loop polling MainEvent::InitWindow, MainEvent::TerminateWindow, etc.
    // Map touch events via ndk::event::MotionEvent → InputEvent::Touch
    todo!("Implement in Phase 2")
}
```

### 6. iOS Backend (scaffold)

```rust
// crates/kaadan_platform/src/ios.rs
// Scaffold — actual implementation in Phase 2/6

#[cfg(target_os = "ios")]
pub fn ios_main() {
    // Use objc2 to create UIApplication, UIWindow, UIViewController
    // Configure CAMetalLayer on the view
    // Run CADisplayLink-driven update loop
    // Map UITouch events → InputEvent::Touch
    todo!("Implement in Phase 6")
}
```

## Platform-Specific Details

### Android Surface Lifecycle
```
InitWindow → (surface available, init renderer)
    ↓
Running → (normal frame loop)
    ↓
TerminateWindow → (surface lost, drop renderer surface)
    ↓
Resume → InitWindow (re-create surface)
```

The renderer MUST handle surface loss gracefully. Never hold a reference to `ANativeWindow` after `TerminateWindow`.

### iOS Safe Areas
- `UIView.safeAreaInsets` defines regions not obscured by notch/home indicator
- Pass safe area insets through `PlatformWindow` so UI layout respects them

### Display Density
- Android: `ANativeWindow_getWidth/Height` gives physical pixels, density from `AConfiguration`
- iOS: `UIScreen.nativeScale` gives the scale factor
- Desktop: `winit::Window::scale_factor()`
- Always work in logical pixels in game code; convert to physical at the platform boundary

## Deliverables Checklist

- [ ] `Platform` trait with `AppHandler` callback interface
- [ ] `PlatformWindow` trait with `HasWindowHandle` + `HasDisplayHandle`
- [ ] Desktop backend using `winit` — opens window, prints input events
- [ ] Android backend scaffold with `android_main` entry point
- [ ] iOS backend scaffold with entry point
- [ ] Unified `InputEvent` enum: Touch, Key, Resize, Lifecycle
- [ ] `KeyCode` enum covering essential keys
- [ ] Window config with title, size, resizable flag
- [ ] `cargo build` succeeds on desktop
- [ ] `cargo ndk --target aarch64-linux-android build -p kaadan_platform` compiles (scaffold)

## Common Pitfalls

1. **winit 0.30 uses `ApplicationHandler` trait** — The older `run()` with closures is gone. You must implement `ApplicationHandler` and use `run_app()`.

2. **raw-window-handle 0.6 breaking changes** — `HasRawWindowHandle` is now `HasWindowHandle`. The trait method returns `Result<WindowHandle>` not a raw handle directly.

3. **Android ANativeWindow lifetime** — The native window is ONLY valid between `InitWindow` and `TerminateWindow` events. Accessing it outside this window causes segfaults.

4. **Don't use `std::fs` on Android** — App assets are inside the APK (a zip file). Use `ndk::asset::AssetManager` instead. `std::fs` only works for the app's private data directory.

5. **iOS requires main thread** — UIKit calls MUST happen on the main thread. The `objc2` bindings help but you must ensure the event loop runs on main.

6. **Display density varies widely** — Android devices range from 1.0x to 4.0x density. Never hardcode pixel sizes.

## References

- [winit docs](https://docs.rs/winit/latest/winit/)
- [android-activity docs](https://docs.rs/android-activity/latest/android_activity/)
- [raw-window-handle docs](https://docs.rs/raw-window-handle/latest/raw_window_handle/)
- [Android NDK native activity lifecycle](https://developer.android.com/ndk/guides/concepts#naa)
- [objc2 docs](https://docs.rs/objc2/latest/objc2/)
