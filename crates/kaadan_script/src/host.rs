use std::path::{Path, PathBuf};
use std::time::SystemTime;

use kaadan_ecs::App;
use libloading::{Library, Symbol};

use crate::behaviour::{clear_all_behaviours, BehaviourRegistry};
use crate::context::ScriptContext;

const REGISTER_SYMBOL: &[u8] = b"kaadan_register";

const ABI_SYMBOL: &[u8] = b"kaadan_abi_version";

const RELOAD_SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

/// Errors raised while loading or reloading a gameplay plugin.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("io error handling plugin dylib: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to load plugin library: {0}")]
    Load(libloading::Error),

    #[error("plugin is missing the `kaadan_register` symbol: {0}")]
    MissingSymbol(libloading::Error),

    #[error(
        "plugin is missing the `kaadan_abi_version` symbol ({0}); \
         rebuild it against the current kaadan_script (use the `kaadan_game!` macro)"
    )]
    MissingAbiSymbol(libloading::Error),

    #[error(
        "plugin ABI mismatch: host expects {host}, plugin reports {plugin}; \
         rebuild the gameplay crate (stale artifact in target/?)"
    )]
    AbiMismatch { host: u64, plugin: u64 },
}

type RegisterFn = unsafe extern "C" fn(&mut ScriptContext);

/// Loads a gameplay cdylib at runtime and supports hot-reload.
///
/// The host owns the [`App`] (and thus the `World`/`Resources`); the plugin only
/// contributes systems through a [`ScriptContext`]. On reload the host removes
/// the plugin's previously registered systems *by name* before dropping the old
/// library, then loads the fresh build and re-registers — so game state (the
/// world, resources) survives the reload while code is swapped.
pub struct ScriptHost {
    path: PathBuf,
    lib: Option<Library>,
    loaded_copy: Option<PathBuf>,
    plugin_systems: Vec<String>,
    last_modified: Option<SystemTime>,
}

impl ScriptHost {
    /// Create a host pointing at the plugin dylib at `path`. Nothing is loaded
    /// until [`load`](Self::load) is called.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lib: None,
            loaded_copy: None,
            plugin_systems: Vec::new(),
            last_modified: None,
        }
    }

    /// Names of the systems registered by the currently loaded plugin.
    pub fn plugin_systems(&self) -> &[String] {
        &self.plugin_systems
    }

    /// The plugin dylib path this host watches.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load (or reload from scratch) the plugin and register its systems on `app`.
    ///
    /// The original dylib is copied to a unique temp file first; the *copy* is
    /// what we `dlopen`. This lets `cargo` overwrite the original (rebuild)
    /// without fighting a file lock, which matters on Windows but is also tidy
    /// elsewhere.
    pub fn load(&mut self, app: &mut App) -> Result<(), ScriptError> {
        // Record the source mtime *before* loading so a rebuild that lands
        // between stat and load is still detected on the next poll.
        let modified = std::fs::metadata(&self.path)?.modified().ok();

        let copy_path = unique_copy_path(&self.path);
        std::fs::copy(&self.path, &copy_path)?;

        // SAFETY: loading an arbitrary dynamic library is inherently unsafe —
        // its static initializers run on load. We trust the plugin because it is
        // built from this same workspace with the *same toolchain and dependency
        // versions* as the host (the documented ABI contract). A copy is loaded
        // so the original can be rebuilt; the copy path is owned by us.
        let lib = unsafe { Library::new(&copy_path) }.map_err(|e| {
            // Best-effort cleanup of the copy if the load fails.
            let _ = std::fs::remove_file(&copy_path);
            ScriptError::Load(e)
        })?;

        // Check the ABI version BEFORE handing the plugin a `&mut ScriptContext`.
        // This is the one call we can make whose signature (`fn() -> u64`) is
        // layout-independent, so it is safe to make even against a plugin that
        // disagrees with us about everything else.
        //
        // SAFETY: `kaadan_abi_version` is emitted by `kaadan_game!` with exactly
        // this signature and touches no shared types. The `Symbol` borrows `lib`
        // and is called immediately while `lib` is alive.
        let abi_check = unsafe {
            lib.get::<unsafe extern "C" fn() -> u64>(ABI_SYMBOL)
                .map_err(ScriptError::MissingAbiSymbol)
                .map(|f| f())
        };
        let abi = match abi_check {
            Ok(v) if v == crate::ABI_VERSION => v,
            Ok(v) => {
                drop(lib);
                let _ = std::fs::remove_file(&copy_path);
                return Err(ScriptError::AbiMismatch {
                    host: crate::ABI_VERSION,
                    plugin: v,
                });
            }
            Err(e) => {
                drop(lib);
                let _ = std::fs::remove_file(&copy_path);
                return Err(e);
            }
        };
        debug_assert_eq!(abi, crate::ABI_VERSION);

        // SAFETY: the plugin exports `kaadan_register` with exactly this
        // `extern "C" fn(&mut ScriptContext)` signature via the `kaadan_game!`
        // macro. Same-toolchain + same-deps guarantees `ScriptContext` has an
        // identical layout on both sides, so the call is sound — and the ABI
        // check above rejects the stale-artifact case that would break it. The
        // returned `Symbol` borrows `lib`, so we call it immediately while `lib`
        // is alive and do not let the pointer escape this scope.
        let registered = unsafe {
            let register: Symbol<RegisterFn> = match lib.get(REGISTER_SYMBOL) {
                Ok(f) => f,
                Err(e) => {
                    drop(lib);
                    let _ = std::fs::remove_file(&copy_path);
                    return Err(ScriptError::MissingSymbol(e));
                }
            };
            let mut ctx = ScriptContext::new(app);
            register(&mut ctx);
            ctx.take_registered()
        };

        // SAFETY: we keep `lib` alive in `self.lib` for as long as its systems
        // are in the schedule. The registered systems hold `fn` pointers into
        // this library's code segment; dropping the library would unmap that
        // code and turn those pointers dangling. `reload`/`drop` remove the
        // systems *before* dropping the library to uphold this.
        self.lib = Some(lib);
        self.loaded_copy = Some(copy_path);
        self.plugin_systems = registered;
        self.last_modified = modified;
        Ok(())
    }

    /// Reload the plugin: remove its systems, drop the old library, load anew.
    ///
    /// Game state (world / resources) is untouched and therefore preserved.
    pub fn reload(&mut self, app: &mut App) -> Result<(), ScriptError> {
        // Remove the old systems BEFORE dropping the library: the scheduled
        // closures point into the library's code, so they must be gone before
        // the code is unmapped.
        for name in &self.plugin_systems {
            app.remove_system(name);
        }
        self.plugin_systems.clear();

        // Behaviour instances carry vtables into the old library, and the
        // BehaviourRegistry holds factory `fn` pointers into it — both dangle the
        // moment `self.lib` drops. Tear them down (running `on_destroy`) and drop
        // the registry's factories before the code is unmapped. The fresh plugin
        // re-registers its factories and re-attaches behaviours from `build`.
        clear_all_behaviours(&mut app.world, &mut app.resources);
        if let Some(registry) = app.resources.get_mut::<BehaviourRegistry>() {
            registry.clear();
        }

        // Now it is sound to unload the old code.
        self.lib = None;
        if let Some(old_copy) = self.loaded_copy.take() {
            let _ = std::fs::remove_file(old_copy);
        }

        self.load(app)
    }

    /// If the source dylib's mtime changed since the last load, reload and
    /// return `true`; otherwise return `false`.
    ///
    /// A change is acted on only once the file has been quiet for
    /// [`RELOAD_SETTLE`], so a dylib still being written by the linker is left
    /// alone and picked up by a later poll.
    ///
    /// **On failure the plugin's systems and behaviours are gone.** [`reload`]
    /// removes them before it can know whether the new build loads, so a failed
    /// reload leaves gameplay inert until the *next* successful one. The failed
    /// mtime is recorded so a bad build is not retried every frame; the next
    /// rebuild changes the mtime again and triggers a fresh attempt. Callers
    /// wanting to handle this explicitly should call [`reload`](Self::reload).
    ///
    /// [`reload`]: Self::reload
    pub fn poll(&mut self, app: &mut App) -> bool {
        let current = match std::fs::metadata(&self.path).and_then(|m| m.modified()) {
            Ok(m) => Some(m),
            Err(_) => return false,
        };
        if current != self.last_modified {
            // Still being written? Leave `last_modified` alone and retry next
            // poll, rather than dlopen'ing a half-linked image.
            if let Some(modified) = current {
                match SystemTime::now().duration_since(modified) {
                    Ok(age) if age < RELOAD_SETTLE => return false,
                    // mtime in the future (clock skew / network fs): treat as
                    // unsettled rather than trusting it.
                    Err(_) => return false,
                    Ok(_) => {}
                }
            }
            match self.reload(app) {
                Ok(()) => return true,
                Err(e) => {
                    kaadan_core::tracing::error!(
                        "script hot-reload failed, gameplay is inert until the next \
                         successful build: {e}"
                    );
                    // Avoid hammering reload every frame on a bad build.
                    self.last_modified = current;
                    return false;
                }
            }
        }
        false
    }
}

impl Drop for ScriptHost {
    fn drop(&mut self) {
        // We cannot remove systems from the App here (no handle to it), but the
        // common teardown order drops the App first. Best-effort temp cleanup:
        if let Some(copy) = self.loaded_copy.take() {
            let _ = std::fs::remove_file(copy);
        }
    }
}

fn unique_copy_path(path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("plugin");
    let ext = path.extension().and_then(|s| s.to_str());

    let file_name = match ext {
        Some(ext) => format!("{stem}.{pid}.{nanos}.{ext}"),
        None => format!("{stem}.{pid}.{nanos}"),
    };
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_load_cleans_up_its_temp_copy() {
        let dir = std::env::temp_dir().join(format!("kaadan_abi_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let fake = dir.join("libnot_a_plugin.dylib");
        std::fs::write(&fake, b"this is not a mach-o/elf/pe image").expect("write fake");

        let mut app = App::new();
        let mut host = ScriptHost::new(&fake);
        let err = host.load(&mut app).expect_err("garbage must not load");
        assert!(
            matches!(err, ScriptError::Load(_)),
            "expected a Load error, got: {err}"
        );

        // Only the original should remain in the directory.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .filter(|n| n != "libnot_a_plugin.dylib")
            .collect();
        assert!(
            leftovers.is_empty(),
            "failed load leaked temp copies: {leftovers:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_dylib_is_an_error() {
        let mut app = App::new();
        let mut host = ScriptHost::new("/definitely/not/here/libghost.dylib");
        assert!(matches!(host.load(&mut app), Err(ScriptError::Io(_))));
    }

    #[test]
    fn poll_without_a_dylib_is_a_noop() {
        let mut app = App::new();
        let mut host = ScriptHost::new("/definitely/not/here/libghost.dylib");
        assert!(!host.poll(&mut app));
    }

    #[test]
    fn poll_defers_while_the_dylib_is_still_settling() {
        let dir = std::env::temp_dir().join(format!("kaadan_settle_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("libsettling.dylib");
        std::fs::write(&path, b"pretend this is mid-link").expect("write");

        let mut app = App::new();
        let mut host = ScriptHost::new(&path);
        assert!(
            !host.poll(&mut app),
            "a just-written dylib must not be loaded immediately"
        );
        assert!(
            host.last_modified.is_none(),
            "deferring must not record the mtime, or the change would be lost"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
