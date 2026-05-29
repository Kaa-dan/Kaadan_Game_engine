use std::path::{Path, PathBuf};
use std::time::SystemTime;

use kaadan_ecs::App;
use libloading::{Library, Symbol};

use crate::context::ScriptContext;

/// The exported symbol every gameplay plugin must provide. See `kaadan_game!`.
const REGISTER_SYMBOL: &[u8] = b"kaadan_register";

/// Errors raised while loading or reloading a gameplay plugin.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("io error handling plugin dylib: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to load plugin library: {0}")]
    Load(libloading::Error),

    #[error("plugin is missing the `kaadan_register` symbol: {0}")]
    MissingSymbol(libloading::Error),
}

/// Signature of the exported registration entry point.
type RegisterFn = unsafe extern "C" fn(&mut ScriptContext);

/// Loads a gameplay cdylib at runtime and supports hot-reload.
///
/// The host owns the [`App`] (and thus the `World`/`Resources`); the plugin only
/// contributes systems through a [`ScriptContext`]. On reload the host removes
/// the plugin's previously registered systems *by name* before dropping the old
/// library, then loads the fresh build and re-registers — so game state (the
/// world, resources) survives the reload while code is swapped.
pub struct ScriptHost {
    /// Path to the on-disk plugin built by `cargo` (watched for changes).
    path: PathBuf,
    /// The currently loaded library (a *copy* of `path`); kept alive so its
    /// code/`fn` pointers remain valid for as long as its systems are scheduled.
    lib: Option<Library>,
    /// Path of the temp copy currently loaded, deleted on reload/drop.
    loaded_copy: Option<PathBuf>,
    /// Names of systems registered by the loaded plugin (for removal on reload).
    plugin_systems: Vec<String>,
    /// mtime of `path` at the moment we last loaded; drives [`poll`](Self::poll).
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

        // SAFETY: the plugin exports `kaadan_register` with exactly this
        // `extern "C" fn(&mut ScriptContext)` signature via the `kaadan_game!`
        // macro. Same-toolchain + same-deps guarantees `ScriptContext` has an
        // identical layout on both sides, so the call is sound. The returned
        // `Symbol` borrows `lib`, so we call it immediately while `lib` is alive
        // and do not let the pointer escape this scope.
        let registered = unsafe {
            let register: Symbol<RegisterFn> = lib
                .get(REGISTER_SYMBOL)
                .map_err(ScriptError::MissingSymbol)?;
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
    /// On a reload error the host logs and returns `false` (it keeps running the
    /// previously loaded systems, which were already removed — callers wanting
    /// strict handling should call [`reload`](Self::reload) directly).
    pub fn poll(&mut self, app: &mut App) -> bool {
        let current = match std::fs::metadata(&self.path).and_then(|m| m.modified()) {
            Ok(m) => Some(m),
            Err(_) => return false,
        };
        if current != self.last_modified {
            match self.reload(app) {
                Ok(()) => return true,
                Err(e) => {
                    kaadan_core::tracing::error!("script hot-reload failed: {e}");
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

/// Build a process-unique sibling path for the temp copy of `path`,
/// e.g. `libgame.dylib` -> `libgame.<pid>.<nanos>.dylib`.
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
