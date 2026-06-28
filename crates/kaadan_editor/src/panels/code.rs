//! Code panel: file tree of the gameplay crate + a syntax-highlighted Rust
//! editor for the active file. Pairs with the [`Build`](crate::panels::build)
//! panel which renders cargo diagnostics.
//!
//! Phase 8 scope: one active buffer, no multi-file tabs, no LSP, no search.
//! The crate path is configurable via [`CodePanelState::crate_dir`] (defaults
//! to `templates/game_template` relative to CWD).

use std::path::{Path, PathBuf};

use egui_code_editor::{CodeEditor, ColorTheme, Syntax};

use crate::cargo_build::BuildOutput;

/// Persistent state for the Code panel. Lives on `EditorState` so the buffer
/// survives tab focus changes and dock re-layouts.
pub struct CodePanelState {
    /// Root of the gameplay crate the editor is editing.
    pub crate_dir: PathBuf,
    /// Path of the file currently loaded into `buffer`, or `None` if no file
    /// is open yet (we show a hint instead of an editor).
    pub open_file: Option<PathBuf>,
    /// Editable text buffer for the open file.
    pub buffer: String,
    /// True if `buffer` differs from what was last loaded / saved from disk.
    pub modified: bool,
    /// Last status / error message shown above the editor (e.g. "saved",
    /// "failed to read", "jumped to line N"). Cleared when the user opens
    /// another file.
    pub status: Option<String>,
    /// If `Some`, on the next render the buffer scrolls so this 1-based line
    /// is visible and a banner is shown. Used by jump-to-error.
    pub scroll_to_line: Option<u32>,
}

impl Default for CodePanelState {
    fn default() -> Self {
        Self {
            crate_dir: default_gameplay_crate_dir(),
            open_file: None,
            buffer: String::new(),
            modified: false,
            status: None,
            scroll_to_line: None,
        }
    }
}

impl CodePanelState {
    /// Load `path` into `buffer`. On failure, sets `status` and leaves the
    /// buffer untouched. Always clears the `modified` flag on success.
    pub fn load_file(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.buffer = text;
                self.open_file = Some(path);
                self.modified = false;
                self.status = None;
                self.scroll_to_line = None;
            }
            Err(e) => {
                self.status = Some(format!("failed to read {}: {}", path.display(), e));
            }
        }
    }

    /// Write `buffer` to the open file. No-op (with a status) if nothing is
    /// open.
    pub fn save(&mut self) {
        let Some(path) = self.open_file.clone() else {
            self.status = Some("no file open to save".to_string());
            return;
        };
        match std::fs::write(&path, &self.buffer) {
            Ok(()) => {
                self.modified = false;
                self.status = Some(format!("saved {}", path.display()));
            }
            Err(e) => {
                self.status = Some(format!("save failed: {e}"));
            }
        }
    }

    /// Open `path` and request the editor scroll to `line` (1-based). Used by
    /// the Build panel's jump-to-error links. If `path` is the file already
    /// open the buffer is preserved (any unsaved edits stay) and we just set
    /// the scroll hint.
    pub fn jump_to(&mut self, path: PathBuf, line: u32) {
        let already_open = self.open_file.as_deref() == Some(path.as_path());
        if !already_open {
            self.load_file(path);
        }
        self.scroll_to_line = Some(line.max(1));
        self.status = Some(format!("jumped to line {line}"));
    }
}

/// Default gameplay-crate path: `<cwd>/templates/game_template`. We resolve
/// at call time rather than embedding `CARGO_MANIFEST_DIR` so the editor
/// binary works whether it was launched from the workspace root or a target
/// directory.
fn default_gameplay_crate_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("templates")
        .join("game_template")
}

/// Render the Code panel. `build` is read-only here (we render its status but
/// never touch its diagnostics); kicking off a build mutates it so it stays
/// `&mut`.
pub fn show(ui: &mut egui::Ui, state: &mut CodePanelState, build: &mut BuildOutput) {
    let src_dir = state.crate_dir.join("src");

    // Top strip: file path, save, modified marker, build button + status.
    ui.horizontal(|ui| {
        let label = match &state.open_file {
            Some(p) => p.display().to_string(),
            None => "(no file)".to_string(),
        };
        ui.label(egui::RichText::new(&label).monospace());
        if state.modified {
            ui.label(egui::RichText::new("●").color(egui::Color32::YELLOW))
                .on_hover_text("Unsaved changes");
        }
        if ui
            .add_enabled(state.open_file.is_some(), egui::Button::new("Save"))
            .clicked()
        {
            state.save();
        }
        ui.separator();
        if ui
            .add_enabled(!build.is_running(), egui::Button::new("Build"))
            .on_hover_text("cargo build -p game_template")
            .clicked()
        {
            // Use the crate-dir's parent (workspace root) so `cargo` finds
            // the workspace manifest. Fall back to CWD if the configured
            // crate dir is somehow at the filesystem root.
            let manifest_dir = state
                .crate_dir
                .parent()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let package = state
                .crate_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("game_template")
                .to_string();
            build.start(&package, manifest_dir);
        }
        ui.label(build.status.label());
    });

    if let Some(status) = &state.status {
        ui.colored_label(egui::Color32::LIGHT_BLUE, status);
    }

    ui.separator();

    // Left: file tree. Right: editor. Use a horizontal split sized as a
    // fixed-width left pane (a true splitter would require egui_dock nesting
    // which is overkill for an MVP).
    egui::SidePanel::left(egui::Id::new("kaadan_code_filetree"))
        .resizable(true)
        .default_width(220.0)
        .show_inside(ui, |ui| {
            ui.heading("Files");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                if src_dir.exists() {
                    show_dir(ui, &src_dir, state);
                } else {
                    ui.label(format!("(missing) {}", src_dir.display()));
                }
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        if state.open_file.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("Select a file from the tree on the left.");
            });
            return;
        }
        let mut editor = CodeEditor::default()
            .id_source("kaadan_code_editor")
            .with_rows(24)
            .with_fontsize(13.0)
            .with_theme(ColorTheme::GRUVBOX_DARK)
            .with_syntax(Syntax::rust())
            .with_numlines(true)
            .vscroll(true);
        let before = state.buffer.clone();
        editor.show(ui, &mut state.buffer);
        if state.buffer != before {
            state.modified = true;
        }
        // Jump-to-line: egui_code_editor wraps its TextEdit in a scroll area
        // we don't own, so we can't move the cursor reliably. We render a
        // small banner instead (set by `status` in `jump_to`) — this is the
        // documented MVP behaviour. Clear the hint after one frame so a
        // subsequent edit re-enables free scrolling.
        if state.scroll_to_line.take().is_some() {
            ui.ctx().request_repaint();
        }
    });
}

/// Recursive depth-first walk of the gameplay crate's `src/` tree, rendering
/// each directory as a `CollapsingHeader` and each `.rs` file as a clickable
/// label. We use `std::fs::read_dir` (no `walkdir` dep) — the gameplay crate
/// is small, depth is shallow, and avoiding a new dep keeps Cargo.toml lean.
fn show_dir(ui: &mut egui::Ui, dir: &Path, state: &mut CodePanelState) {
    let Ok(read) = std::fs::read_dir(dir) else {
        ui.label(format!("(unreadable) {}", dir.display()));
        return;
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    dirs.sort();
    files.sort();

    for d in dirs {
        let name = d
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        egui::CollapsingHeader::new(name)
            .default_open(true)
            .show(ui, |ui| show_dir(ui, &d, state));
    }
    for f in files {
        let name = f
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let is_open = state.open_file.as_deref() == Some(f.as_path());
        if ui.selectable_label(is_open, &name).clicked() {
            state.load_file(f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_file_then_save_round_trips() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!("kaadan_code_panel_test_{}.rs", std::process::id()));
        {
            let mut f = std::fs::File::create(&tmp).unwrap();
            writeln!(f, "fn main() {{}}").unwrap();
        }
        let mut s = CodePanelState::default();
        s.load_file(tmp.clone());
        assert_eq!(s.open_file.as_deref(), Some(tmp.as_path()));
        assert!(s.buffer.contains("fn main"));
        assert!(!s.modified);

        s.buffer.push_str("// edit\n");
        s.modified = true;
        s.save();
        assert!(!s.modified, "save should clear modified flag");

        let on_disk = std::fs::read_to_string(&tmp).unwrap();
        assert!(on_disk.contains("// edit"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn jump_to_sets_scroll_hint_and_status() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!("kaadan_code_jump_{}.rs", std::process::id()));
        std::fs::write(&tmp, "// a\n// b\n// c\n").unwrap();
        let mut s = CodePanelState::default();
        s.jump_to(tmp.clone(), 2);
        assert_eq!(s.scroll_to_line, Some(2));
        assert!(s.status.as_deref().unwrap_or("").contains("line 2"));
        let _ = std::fs::remove_file(&tmp);
    }
}
