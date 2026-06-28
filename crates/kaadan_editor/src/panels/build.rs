//! Build panel: console-style view of the most recent `cargo build`. Each
//! diagnostic is a clickable link that loads the referenced file into the
//! Code panel and scrolls (banner-only, see [`crate::panels::code`]) to the
//! reported line.

use std::path::PathBuf;

use crate::cargo_build::BuildOutput;
use crate::panels::code::CodePanelState;

pub fn show(ui: &mut egui::Ui, build: &mut BuildOutput, code: &mut CodePanelState) {
    ui.horizontal(|ui| {
        ui.heading("Build");
        ui.label(build.status.label());
        if !build.diagnostics.is_empty() {
            ui.label(format!("({} diagnostics)", build.diagnostics.len()));
        }
        if ui
            .add_enabled(!build.is_running(), egui::Button::new("Clear"))
            .clicked()
        {
            build.diagnostics.clear();
        }
    });
    ui.separator();

    if build.diagnostics.is_empty() {
        let msg = if build.is_running() {
            "Running cargo build…"
        } else {
            "No diagnostics. Click Build in the Code tab to start."
        };
        ui.label(msg);
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for d in &build.diagnostics {
            ui.horizontal(|ui| {
                ui.colored_label(d.level.color(), d.level.label());
                if d.file_name.is_empty() {
                    // No span: just render the message text, not clickable.
                    ui.label(egui::RichText::new(&d.message).monospace());
                } else {
                    let location = format!("{}:{}:{}", d.file_name, d.line_start, d.column_start);
                    let link_text = format!("{location}   {}", d.message);
                    if ui
                        .add(egui::Link::new(egui::RichText::new(link_text).monospace()))
                        .clicked()
                    {
                        let target = resolve_diagnostic_path(&code.crate_dir, &d.file_name);
                        code.jump_to(target, d.line_start);
                    }
                }
            });
        }
    });
}

/// Diagnostics from `rustc` typically report paths relative to the package
/// manifest (e.g. `src/lib.rs`). Anchor those to the gameplay crate root so
/// the editor opens the right file regardless of CWD. Absolute paths are
/// returned unchanged.
fn resolve_diagnostic_path(crate_dir: &std::path::Path, file_name: &str) -> PathBuf {
    let p = PathBuf::from(file_name);
    if p.is_absolute() {
        p
    } else {
        crate_dir.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_resolved_against_crate_dir() {
        let crate_dir = std::path::Path::new("/tmp/game_template");
        let resolved = resolve_diagnostic_path(crate_dir, "src/lib.rs");
        assert_eq!(
            resolved,
            std::path::PathBuf::from("/tmp/game_template/src/lib.rs")
        );
    }

    #[test]
    fn absolute_paths_passed_through() {
        let crate_dir = std::path::Path::new("/tmp/game_template");
        let resolved = resolve_diagnostic_path(crate_dir, "/abs/src/lib.rs");
        assert_eq!(resolved, std::path::PathBuf::from("/abs/src/lib.rs"));
    }
}
