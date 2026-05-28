# 07 — Input and Audio

## Description
Input abstraction (action mapping, touch gestures, gamepad) and audio playback (BGM, SFX, spatial audio basics). Input maps physical events to logical actions so game code never checks raw keycodes.

## Phase
3 — ECS & Sprites

## Prerequisites
- Skill 03 (`03-platform-abstraction`) — `InputEvent` enum, touch events
- Skill 05 (`05-ecs-world`) — ECS integration, Resources

## Complexity
Medium — touch gesture recognition is the trickiest part

## Architecture Decisions

### Why action mapping?
- Game code says `input.action_pressed("jump")`, not `input.key_pressed(KeyCode::Space)`
- Same action maps to Space on desktop, tap on mobile, A-button on gamepad
- Players can rebind controls without touching game logic
- Simplifies cross-platform: the game layer is platform-agnostic

### Touch-first design
- `InputMap` starts with touch gestures as first-class citizens
- Tap = "primary action", Swipe = "direction", Pinch = "zoom"
- Keyboard/mouse/gamepad are alternative bindings to the same actions
- This ensures mobile input isn't an afterthought bolted on later

### Audio architecture
- `rodio` handles audio decoding and playback across desktop/mobile
- Audio commands go through a channel to avoid blocking the main thread
- ECS integration: `AudioCommand` component or `AudioEvent` resource
- Spatial audio (volume based on distance) is a simple system, not a full audio engine

## Step-by-Step Implementation

### 1. Input Crate Setup

```toml
# crates/kaadan_input/Cargo.toml
[package]
name = "kaadan_input"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math" }
kaadan_core = { path = "../kaadan_core" }
kaadan_platform = { path = "../kaadan_platform" }
tracing = { workspace = true }

[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
gilrs = { workspace = true }
```

### 2. InputMap — Action Mapping

```rust
// crates/kaadan_input/src/input_map.rs
use kaadan_platform::{KeyCode, TouchPhase};
use std::collections::HashMap;

/// A physical input that can be bound to an action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputBinding {
    Key(KeyCode),
    Tap,             // Single touch tap
    DoubleTap,       // Double tap
    SwipeUp,
    SwipeDown,
    SwipeLeft,
    SwipeRight,
    PinchIn,         // Pinch to zoom out
    PinchOut,        // Pinch to zoom in
    GamepadButton(u32),
    GamepadAxis { axis: u32, positive: bool },
}

/// Maps physical inputs to named actions.
pub struct InputMap {
    /// action_name → list of bindings
    bindings: HashMap<String, Vec<InputBinding>>,
}

impl InputMap {
    pub fn new() -> Self {
        Self { bindings: HashMap::new() }
    }

    pub fn bind(&mut self, action: impl Into<String>, binding: InputBinding) -> &mut Self {
        self.bindings
            .entry(action.into())
            .or_default()
            .push(binding);
        self
    }

    pub fn bindings_for(&self, action: &str) -> &[InputBinding] {
        self.bindings.get(action).map_or(&[], |v| v.as_slice())
    }
}
```

### 3. InputState — Per-Frame State

```rust
// crates/kaadan_input/src/input_state.rs
use kaadan_math::Vec2;
use kaadan_platform::{InputEvent, KeyCode, TouchPhase};
use std::collections::HashSet;

/// Per-frame input state, updated from platform events.
/// Inserted as an ECS Resource.
pub struct InputState {
    // Keyboard
    keys_pressed: HashSet<KeyCode>,
    keys_just_pressed: HashSet<KeyCode>,
    keys_just_released: HashSet<KeyCode>,

    // Touch
    touches: Vec<TouchState>,

    // Recognized gestures this frame
    gestures: Vec<Gesture>,

    // Mouse / pointer (desktop)
    pointer_position: Vec2,
    pointer_delta: Vec2,
}

#[derive(Debug, Clone)]
pub struct TouchState {
    pub id: u64,
    pub position: Vec2,
    pub start_position: Vec2,
    pub phase: TouchPhase,
}

#[derive(Debug, Clone)]
pub enum Gesture {
    Tap { position: Vec2 },
    DoubleTap { position: Vec2 },
    Swipe { direction: SwipeDirection, velocity: f32, start: Vec2, end: Vec2 },
    Pinch { scale: f32, center: Vec2 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Up, Down, Left, Right,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            keys_just_pressed: HashSet::new(),
            keys_just_released: HashSet::new(),
            touches: Vec::new(),
            gestures: Vec::new(),
            pointer_position: Vec2::ZERO,
            pointer_delta: Vec2::ZERO,
        }
    }

    /// Call at the start of each frame before processing events.
    pub fn begin_frame(&mut self) {
        self.keys_just_pressed.clear();
        self.keys_just_released.clear();
        self.gestures.clear();
        self.pointer_delta = Vec2::ZERO;
    }

    /// Process a platform input event.
    pub fn process_event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Key(key_event) => {
                if key_event.pressed {
                    if self.keys_pressed.insert(key_event.key) {
                        self.keys_just_pressed.insert(key_event.key);
                    }
                } else {
                    self.keys_pressed.remove(&key_event.key);
                    self.keys_just_released.insert(key_event.key);
                }
            }
            InputEvent::Touch(touch) => {
                self.process_touch(touch);
            }
            _ => {}
        }
    }

    // Keyboard queries
    pub fn key_pressed(&self, key: KeyCode) -> bool { self.keys_pressed.contains(&key) }
    pub fn key_just_pressed(&self, key: KeyCode) -> bool { self.keys_just_pressed.contains(&key) }
    pub fn key_just_released(&self, key: KeyCode) -> bool { self.keys_just_released.contains(&key) }

    // Touch queries
    pub fn touches(&self) -> &[TouchState] { &self.touches }
    pub fn gestures(&self) -> &[Gesture] { &self.gestures }

    // Action queries (uses InputMap)
    pub fn action_pressed(&self, action: &str, map: &super::InputMap) -> bool {
        map.bindings_for(action).iter().any(|binding| match binding {
            InputBinding::Key(key) => self.key_pressed(*key),
            InputBinding::Tap => self.gestures.iter().any(|g| matches!(g, Gesture::Tap { .. })),
            _ => false,
        })
    }

    pub fn action_just_pressed(&self, action: &str, map: &super::InputMap) -> bool {
        map.bindings_for(action).iter().any(|binding| match binding {
            InputBinding::Key(key) => self.key_just_pressed(*key),
            InputBinding::Tap => self.gestures.iter().any(|g| matches!(g, Gesture::Tap { .. })),
            _ => false,
        })
    }
}
```

### 4. Touch Gesture Recognizer

```rust
// crates/kaadan_input/src/gesture.rs
use kaadan_math::Vec2;

/// Recognizes gestures from raw touch events.
pub struct GestureRecognizer {
    /// Minimum distance (pixels) for a swipe
    swipe_threshold: f32,
    /// Maximum time (seconds) between taps for double-tap
    double_tap_window: f32,
    /// Active touches being tracked
    active_touches: Vec<TrackedTouch>,
    /// Last tap time and position for double-tap detection
    last_tap: Option<(std::time::Instant, Vec2)>,
}

struct TrackedTouch {
    id: u64,
    start_position: Vec2,
    start_time: std::time::Instant,
    current_position: Vec2,
}

impl GestureRecognizer {
    pub fn new() -> Self {
        Self {
            swipe_threshold: 50.0,   // 50 logical pixels
            double_tap_window: 0.3,   // 300ms
            active_touches: Vec::new(),
            last_tap: None,
        }
    }

    /// Process a touch event. Returns recognized gestures (if any).
    pub fn process_touch(&mut self, touch: &TouchEvent) -> Vec<Gesture> {
        let mut gestures = Vec::new();
        match touch.phase {
            TouchPhase::Started => {
                self.active_touches.push(TrackedTouch {
                    id: touch.id,
                    start_position: touch.position,
                    start_time: std::time::Instant::now(),
                    current_position: touch.position,
                });
            }
            TouchPhase::Moved => {
                if let Some(t) = self.active_touches.iter_mut().find(|t| t.id == touch.id) {
                    t.current_position = touch.position;
                }
                // Check for pinch (2 active touches)
                // ...
            }
            TouchPhase::Ended => {
                if let Some(t) = self.active_touches.iter().find(|t| t.id == touch.id) {
                    let delta = t.current_position - t.start_position;
                    let distance = delta.length();
                    let duration = t.start_time.elapsed().as_secs_f32();

                    if distance < self.swipe_threshold && duration < 0.5 {
                        // Tap — check for double-tap
                        if let Some((last_time, last_pos)) = &self.last_tap {
                            if last_time.elapsed().as_secs_f32() < self.double_tap_window
                                && (t.current_position - *last_pos).length() < 30.0
                            {
                                gestures.push(Gesture::DoubleTap { position: t.current_position });
                                self.last_tap = None;
                            } else {
                                gestures.push(Gesture::Tap { position: t.current_position });
                                self.last_tap = Some((std::time::Instant::now(), t.current_position));
                            }
                        } else {
                            gestures.push(Gesture::Tap { position: t.current_position });
                            self.last_tap = Some((std::time::Instant::now(), t.current_position));
                        }
                    } else if distance >= self.swipe_threshold {
                        // Swipe
                        let direction = if delta.x.abs() > delta.y.abs() {
                            if delta.x > 0.0 { SwipeDirection::Right } else { SwipeDirection::Left }
                        } else {
                            if delta.y > 0.0 { SwipeDirection::Down } else { SwipeDirection::Up }
                        };
                        let velocity = distance / duration;
                        gestures.push(Gesture::Swipe {
                            direction, velocity,
                            start: t.start_position,
                            end: t.current_position,
                        });
                    }
                }
                self.active_touches.retain(|t| t.id != touch.id);
            }
            TouchPhase::Cancelled => {
                self.active_touches.retain(|t| t.id != touch.id);
            }
        }
        gestures
    }
}
```

### 5. Audio Crate Setup

```toml
# crates/kaadan_audio/Cargo.toml
[package]
name = "kaadan_audio"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_core = { path = "../kaadan_core" }
kaadan_math = { path = "../kaadan_math" }
rodio = { workspace = true }
tracing = { workspace = true }
```

### 6. AudioEngine

```rust
// crates/kaadan_audio/src/engine.rs
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;
use std::collections::HashMap;

pub struct AudioEngine {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    music_sink: Option<Sink>,
    sound_sinks: Vec<Sink>,
    master_volume: f32,
    music_volume: f32,
    sfx_volume: f32,
}

impl AudioEngine {
    pub fn new() -> Result<Self, kaadan_core::KaadanError> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio init failed: {e}")))?;
        Ok(Self {
            _stream: stream,
            stream_handle: handle,
            music_sink: None,
            sound_sinks: Vec::new(),
            master_volume: 1.0,
            music_volume: 0.7,
            sfx_volume: 1.0,
        })
    }

    /// Play a one-shot sound effect.
    pub fn play_sound(&mut self, data: Vec<u8>) -> Result<(), kaadan_core::KaadanError> {
        let cursor = Cursor::new(data);
        let source = Decoder::new(cursor)
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio decode: {e}")))?;
        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio sink: {e}")))?;
        sink.set_volume(self.master_volume * self.sfx_volume);
        sink.append(source);
        // Clean up finished sinks
        self.sound_sinks.retain(|s| !s.empty());
        self.sound_sinks.push(sink);
        Ok(())
    }

    /// Play background music (replaces current music).
    pub fn play_music(&mut self, data: Vec<u8>, looping: bool) -> Result<(), kaadan_core::KaadanError> {
        // Stop current music
        if let Some(sink) = self.music_sink.take() {
            sink.stop();
        }
        let cursor = Cursor::new(data);
        let source = Decoder::new(cursor)
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio decode: {e}")))?;
        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio sink: {e}")))?;
        sink.set_volume(self.master_volume * self.music_volume);
        if looping {
            sink.append(source.repeat_infinite());
        } else {
            sink.append(source);
        }
        self.music_sink = Some(sink);
        Ok(())
    }

    pub fn stop_music(&mut self) {
        if let Some(sink) = self.music_sink.take() {
            sink.stop();
        }
    }

    pub fn pause_music(&self) {
        if let Some(sink) = &self.music_sink {
            sink.pause();
        }
    }

    pub fn resume_music(&self) {
        if let Some(sink) = &self.music_sink {
            sink.play();
        }
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
        self.update_volumes();
    }

    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume.clamp(0.0, 1.0);
        self.update_volumes();
    }

    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.sfx_volume = volume.clamp(0.0, 1.0);
    }

    fn update_volumes(&self) {
        if let Some(sink) = &self.music_sink {
            sink.set_volume(self.master_volume * self.music_volume);
        }
    }
}
```

### 7. ECS Integration

```rust
// Input system — processes platform events into InputState resource
fn input_system(world: &mut World, resources: &mut Resources) {
    let input = resources.get_mut::<InputState>().unwrap();
    input.begin_frame();

    let events = resources.get::<PlatformEvents>().unwrap();
    for event in events.iter() {
        input.process_event(event);
    }
}

// Audio system — processes AudioCommand components
struct AudioCommand {
    kind: AudioCommandKind,
}

enum AudioCommandKind {
    PlaySound { data: Vec<u8> },
    PlayMusic { data: Vec<u8>, looping: bool },
    StopMusic,
}
```

## Deliverables Checklist

- [ ] `InputMap` mapping physical inputs to named actions with configurable bindings
- [ ] `InputState` resource with key/touch/gesture queries
- [ ] Touch gesture recognizer: tap, double-tap, swipe (direction + velocity), pinch-to-zoom
- [ ] `AudioEngine` wrapping `rodio` with `play_sound()`, `play_music()`, `set_volume()`
- [ ] Volume controls: master, music, SFX with clamping
- [ ] ECS integration: `InputState` resource, `AudioCommand` component
- [ ] Gamepad support via `gilrs` (desktop only, scaffold)
- [ ] Demo: sprite moves via touch/keyboard with SFX on action

## Common Pitfalls

1. **Touch IDs are not sequential** — On mobile, touch IDs can be any u64. Don't assume they start at 0 or are contiguous.

2. **Gesture timing on different frame rates** — Use wall-clock time (`Instant::now()`) for gesture timing, not frame-based counting. Frame rates vary on mobile.

3. **rodio OutputStream must stay alive** — The `OutputStream` struct must be kept alive for the entire audio lifetime. If it's dropped, all audio stops. Store it in the `AudioEngine` struct.

4. **Audio decoding is CPU-intensive** — Decode audio in a background thread or async task. Don't decode on the main thread during gameplay — it causes frame hitches.

5. **Mobile audio session** — On iOS, you need to configure the audio session (`AVAudioSession`). On Android, audio focus management matters. These are platform-specific concerns for Phase 6.

6. **Don't allocate in the gesture recognizer hot path** — Pre-allocate vectors, reuse buffers. The gesture recognizer runs every frame.

## References

- [rodio docs](https://docs.rs/rodio/latest/rodio/)
- [gilrs docs](https://docs.rs/gilrs/latest/gilrs/)
- [Touch gesture recognition patterns](https://developer.apple.com/documentation/uikit/touches_presses_and_gestures)
- [Android MotionEvent](https://developer.android.com/reference/android/view/MotionEvent)
