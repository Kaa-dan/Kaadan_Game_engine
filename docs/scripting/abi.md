# KaadanEngine scripting ABI

Gameplay in KaadanEngine is written as an ordinary Rust crate (see
`templates/game_template`). The same source compiles two ways:

- **`cdylib`** — loaded at runtime by `kaadan_script::ScriptHost` for hot-reload
  during desktop development.
- **`rlib`** — statically linked into the engine binary for shipping (mobile /
  iOS), where the plugin's `build` function is called directly. No
  `libloading`, no dylib, no on-device reload.

The seam between the host and the plugin is a single C-ABI symbol.

## The `kaadan_register` symbol

Every plugin exports exactly one symbol, emitted by the `kaadan_game!` macro:

```rust
#[no_mangle]
pub extern "C" fn kaadan_register(ctx: &mut kaadan_script::ScriptContext);
```

`kaadan_game!(build)` expands to this, forwarding to your
`fn build(ctx: &mut ScriptContext)`. The host resolves the symbol by the byte
string `b"kaadan_register"` and calls it once per load with a fresh
`ScriptContext` wrapping the host's `App`.

## Registration via `ScriptContext`

`ScriptContext<'a>` is the narrow, safe facade gameplay uses during
registration. It borrows the host's `App` and records the names of everything it
registers:

- `add_system(name, system)` — register on `Stage::Update`.
- `add_system_to_stage(stage, name, system)` — register on a specific `Stage`.
- `world()` / `resources()` / `insert_resource(r)` — set up initial state.
- `registered()` / `take_registered()` — the recorded system names.

`kaadan_script` re-exports `Stage` so plugins depend only on `kaadan_script` for
scheduling. Systems are `FnMut(&mut World, &mut Resources) + 'static`.

`ComponentRegistry` (also in `kaadan_script`) maps component *names* to
type-erased `has` / `remove` / `insert_default` operations, so future editor /
tooling code can manipulate components by name without the concrete type.

## Reload model

The **host owns the `App`** — and therefore the `World` and `Resources`. The
plugin only contributes systems. On reload (`ScriptHost::reload`, or
`ScriptHost::poll` when the source mtime changes):

1. Remove the plugin's previously registered systems **by name**
   (`App::remove_system`) — done **before** the old library is dropped, so no
   scheduled closure still points into code that is about to be unmapped.
2. Drop the old `Library` (and delete its temp copy).
3. Load the freshly built dylib and call `kaadan_register` again, re-registering
   systems against the **same** `App`.

Because the world/resources are never touched, **game state survives a code
swap**.

The dylib is **copied to a unique temp file before loading**, and the copy is
what gets `dlopen`'d. This lets `cargo` overwrite the original during a rebuild
without fighting a file lock (important on Windows; tidy everywhere).

## Safety contract

`dlopen` + calling a C-ABI symbol is `unsafe`. It is sound only under these
rules:

- **Same toolchain + same dependency versions.** Host and plugin must be built
  with the same `rustc` and identical versions of all shared crates
  (`kaadan_ecs`, `kaadan_math`, …). Rust has no stable ABI, so a mismatch makes
  `ScriptContext` (and every component type) have a different layout on each
  side — undefined behavior. Building both from one workspace satisfies this.
- **Do not leave dylib-defined component types in the `World` across a reload.**
  Components whose *type* lives in the plugin dylib would have their code (and
  `Drop` glue, vtables, layout) unmapped when the library is dropped, leaving
  dangling instances in the world. Keep gameplay components in shared crates
  (e.g. `kaadan_renderer`, `kaadan_math`) or as plain data, not as types private
  to the plugin.
- **Library lifetime.** `ScriptHost` keeps the loaded `Library` alive for as long
  as its systems are scheduled; the registered closures hold `fn` pointers into
  the library's code segment.
- **iOS ships statically linked.** There is no on-device hot-reload; the `rlib`
  path (call `build` directly) is the production path. Compile `kaadan_script`
  with `--no-default-features` to drop `libloading` for such targets.

## Build & test

- `scripts/build_game_template.sh [--release]` builds the cdylib.
- **CI-run:** `templates/game_template`'s `static_link_build_and_run` test proves
  the static-link path (`ScriptContext::new` + `build` + tick), and
  `kaadan_script`'s `ComponentRegistry` unit test. Both run under
  `cargo test --workspace`.
- **Manual only:** `kaadan_script/tests/hot_reload.rs` is `#[ignore]`d because it
  shells out to `cargo build -p game_template`. Run it with:

  ```sh
  cargo test -p kaadan_script -- --ignored
  ```

## `panic = "abort"` + cdylib

The release profile sets `panic = "abort"`. A `cdylib` built with
`panic = "abort"` must not unwind across the FFI boundary; our entry point does
not rely on unwinding, so this is fine. Tests use the dev profile
(`panic = "unwind"`), which is unaffected.
