//! Background `cargo build` runner for the integrated code editor.
//!
//! Spawns `cargo build -p <package> --message-format=json` on a worker thread
//! and streams parsed compiler diagnostics back to the UI thread through an
//! `mpsc` channel. The UI polls the channel each frame (non-blocking) so egui
//! never stalls waiting for the compiler.
//!
//! ## Design
//!
//! * One build at a time. While [`BuildOutput::is_running`] is `true`, callers
//!   should disable the Build button.
//! * Communication is one-way (worker → UI). We do not support cancellation
//!   yet — once spawned a build runs to completion. (Trade-off: simpler, and
//!   `cargo build` on small gameplay crates finishes in seconds.)
//! * JSON parsing is line-by-line; partial / non-JSON lines are ignored. This
//!   matches `--message-format=json`'s newline-delimited output.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use serde::Deserialize;

/// Status of the most recent build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BuildStatus {
    #[default]
    Idle,
    Running,
    Ok,
    Failed,
}

impl BuildStatus {
    pub fn label(self) -> &'static str {
        match self {
            BuildStatus::Idle => "Idle",
            BuildStatus::Running => "Building…",
            BuildStatus::Ok => "Build OK",
            BuildStatus::Failed => "Build Failed",
        }
    }
}

/// Severity of a compiler diagnostic. Mirrors `rustc`'s `--error-format=json`
/// `level` field. Unknown levels collapse to [`DiagnosticLevel::Note`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
    Help,
}

impl DiagnosticLevel {
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "error" | "error: internal compiler error" => DiagnosticLevel::Error,
            "warning" => DiagnosticLevel::Warning,
            "help" => DiagnosticLevel::Help,
            _ => DiagnosticLevel::Note,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
            DiagnosticLevel::Note => "note",
            DiagnosticLevel::Help => "help",
        }
    }

    pub fn color(self) -> egui::Color32 {
        match self {
            DiagnosticLevel::Error => egui::Color32::from_rgb(0xff, 0x6b, 0x6b),
            DiagnosticLevel::Warning => egui::Color32::from_rgb(0xff, 0xc8, 0x4b),
            DiagnosticLevel::Note => egui::Color32::from_rgb(0x8c, 0xb4, 0xff),
            DiagnosticLevel::Help => egui::Color32::from_rgb(0x9c, 0xff, 0x9c),
        }
    }
}

/// A flattened compiler diagnostic: one [`Diagnostic`] per `(message, span)`
/// pair. Diagnostics without a span are emitted with empty `file_name` and
/// zero positions so the UI can still show the message text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub file_name: String,
    pub line_start: u32,
    pub line_end: u32,
    pub column_start: u32,
    pub column_end: u32,
}

/// One event the worker thread sends back. Kept tiny on purpose — the UI just
/// pushes diagnostics into its console and flips status on `Finished`.
pub enum BuildEvent {
    /// A parsed compiler diagnostic span.
    Diagnostic(Diagnostic),
    /// Build finished. `success` is `true` iff `cargo` exited 0.
    Finished { success: bool },
    /// Fatal worker error (cargo missing, IO failure). The UI displays it as
    /// a single error-level diagnostic and marks the build failed.
    Error(String),
}

/// Build output state held by the editor. The Code panel writes a fresh
/// [`BuildOutput`] (`status = Running`) when the user clicks Build; the UI
/// polls [`BuildOutput::poll`] each frame to drain the worker channel.
#[derive(Default)]
pub struct BuildOutput {
    pub status: BuildStatus,
    pub diagnostics: Vec<Diagnostic>,
    /// Receiver attached to the running worker. `None` when no build is in
    /// flight (idle / finished). Polling is non-blocking and cheap.
    receiver: Option<Receiver<BuildEvent>>,
}

impl BuildOutput {
    pub fn is_running(&self) -> bool {
        self.status == BuildStatus::Running
    }

    /// Spawn `cargo build -p <package> --message-format=json` on a worker
    /// thread. Resets state and sets [`BuildStatus::Running`]. If `cargo` can't
    /// be found the worker reports a single [`BuildEvent::Error`] and the
    /// build is marked failed on the next [`Self::poll`].
    pub fn start(&mut self, package: &str, manifest_dir: PathBuf) {
        self.status = BuildStatus::Running;
        self.diagnostics.clear();
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);

        let package = package.to_string();
        thread::spawn(move || {
            run_cargo_build(&package, &manifest_dir, tx);
        });
    }

    /// Drain any pending events from the worker without blocking. Call once
    /// per UI frame. Returns the number of events processed (useful for
    /// requesting a repaint when something arrived).
    pub fn poll(&mut self) -> usize {
        let mut count = 0;
        let mut finished_status: Option<BuildStatus> = None;
        if let Some(rx) = self.receiver.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(BuildEvent::Diagnostic(d)) => {
                        self.diagnostics.push(d);
                        count += 1;
                    }
                    Ok(BuildEvent::Finished { success }) => {
                        finished_status = Some(if success {
                            BuildStatus::Ok
                        } else {
                            BuildStatus::Failed
                        });
                        count += 1;
                    }
                    Ok(BuildEvent::Error(msg)) => {
                        self.diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            message: msg,
                            file_name: String::new(),
                            line_start: 0,
                            line_end: 0,
                            column_start: 0,
                            column_end: 0,
                        });
                        finished_status = Some(BuildStatus::Failed);
                        count += 1;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // Worker dropped without sending Finished. Treat the
                        // build as failed if we don't already have an answer.
                        if finished_status.is_none() && self.status == BuildStatus::Running {
                            finished_status = Some(BuildStatus::Failed);
                        }
                        break;
                    }
                }
            }
        }
        if let Some(status) = finished_status {
            self.status = status;
            self.receiver = None;
        }
        count
    }
}

/// Run `cargo build -p <package> --message-format=json`, stream parsed
/// diagnostics through `tx`, then send a final [`BuildEvent::Finished`].
fn run_cargo_build(
    package: &str,
    manifest_dir: &std::path::Path,
    tx: std::sync::mpsc::Sender<BuildEvent>,
) {
    // Capture stderr too so non-JSON cargo errors (e.g. "could not find
    // package") aren't lost. We don't currently surface stderr to the UI, but
    // discarding it would hang `cargo` on a full pipe for long enough to be
    // user-visible.
    let mut child = match Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg(package)
        .arg("--message-format=json")
        .current_dir(manifest_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            // `NotFound` -> cargo isn't installed / on PATH. Any other error
            // is also fatal for this build.
            let msg = if e.kind() == std::io::ErrorKind::NotFound {
                "cargo not found on PATH".to_string()
            } else {
                format!("failed to spawn cargo: {e}")
            };
            let _ = tx.send(BuildEvent::Error(msg));
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = tx.send(BuildEvent::Error("cargo produced no stdout".to_string()));
            let _ = child.wait();
            return;
        }
    };

    use std::io::{BufRead, BufReader};
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        for d in parse_cargo_json_line(&line) {
            if tx.send(BuildEvent::Diagnostic(d)).is_err() {
                // UI dropped the receiver; bail out, the user no longer cares.
                let _ = child.wait();
                return;
            }
        }
    }
    let status = child.wait();
    let success = matches!(status, Ok(s) if s.success());
    let _ = tx.send(BuildEvent::Finished { success });
}

// ----- JSON parsing -------------------------------------------------------

#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    #[serde(default)]
    message: Option<RustcMessage>,
}

#[derive(Deserialize)]
struct RustcMessage {
    message: String,
    level: String,
    #[serde(default)]
    spans: Vec<RustcSpan>,
}

#[derive(Deserialize)]
struct RustcSpan {
    file_name: String,
    line_start: u32,
    line_end: u32,
    column_start: u32,
    column_end: u32,
    /// `is_primary` lets us prefer the user-visible span when a diagnostic has
    /// several (e.g. the error site vs the macro definition site).
    #[serde(default)]
    is_primary: bool,
}

/// Parse one line of `cargo build --message-format=json` output. Returns
/// `Vec` because one rustc diagnostic with N spans expands to N diagnostics.
/// Non-`compiler-message` JSON (e.g. `build-script-executed`,
/// `compiler-artifact`) yields an empty vec, as do non-JSON / blank lines.
pub fn parse_cargo_json_line(line: &str) -> Vec<Diagnostic> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }
    let parsed: CargoMessage = match serde_json::from_str(line) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    if parsed.reason != "compiler-message" {
        return Vec::new();
    }
    let Some(msg) = parsed.message else {
        return Vec::new();
    };
    let level = DiagnosticLevel::from_str_lossy(&msg.level);

    if msg.spans.is_empty() {
        return vec![Diagnostic {
            level,
            message: msg.message,
            file_name: String::new(),
            line_start: 0,
            line_end: 0,
            column_start: 0,
            column_end: 0,
        }];
    }

    // Prefer primary spans; if none are flagged primary, fall back to all
    // spans so the user still gets a clickable location.
    let primary: Vec<&RustcSpan> = msg.spans.iter().filter(|s| s.is_primary).collect();
    let spans: &[&RustcSpan] = if primary.is_empty() {
        // Borrow all as references for a uniform shape below.
        return msg
            .spans
            .iter()
            .map(|s| Diagnostic {
                level,
                message: msg.message.clone(),
                file_name: s.file_name.clone(),
                line_start: s.line_start,
                line_end: s.line_end,
                column_start: s.column_start,
                column_end: s.column_end,
            })
            .collect();
    } else {
        &primary
    };
    spans
        .iter()
        .map(|s| Diagnostic {
            level,
            message: msg.message.clone(),
            file_name: s.file_name.clone(),
            line_start: s.line_start,
            line_end: s.line_end,
            column_start: s.column_start,
            column_end: s.column_end,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real `cargo build --message-format=json` "compiler-message" line,
    /// trimmed and slightly redacted for stability. Verifies we extract the
    /// primary span and ignore the non-primary one.
    #[test]
    fn parses_compiler_message_with_primary_span() {
        let line = r#"{"reason":"compiler-message","package_id":"game_template 0.1.0","manifest_path":"/x/Cargo.toml","target":{"kind":["lib"],"crate_types":["rlib"],"name":"game_template","src_path":"/x/src/lib.rs","edition":"2021","doc":true,"doctest":true,"test":true},"message":{"rendered":"...","children":[],"code":null,"level":"error","message":"cannot find value `foo` in this scope","spans":[{"byte_end":42,"byte_start":39,"column_end":7,"column_start":4,"expansion":null,"file_name":"src/lib.rs","is_primary":true,"label":"not found in this scope","line_end":17,"line_start":17,"suggested_replacement":null,"suggestion_applicability":null,"text":[]},{"byte_end":10,"byte_start":0,"column_end":11,"column_start":1,"expansion":null,"file_name":"src/lib.rs","is_primary":false,"label":"defined here","line_end":1,"line_start":1,"suggested_replacement":null,"suggestion_applicability":null,"text":[]}]}}"#;
        let diags = parse_cargo_json_line(line);
        assert_eq!(diags.len(), 1, "primary-only filter should keep 1 span");
        let d = &diags[0];
        assert_eq!(d.level, DiagnosticLevel::Error);
        assert_eq!(d.message, "cannot find value `foo` in this scope");
        assert_eq!(d.file_name, "src/lib.rs");
        assert_eq!(d.line_start, 17);
        assert_eq!(d.line_end, 17);
        assert_eq!(d.column_start, 4);
        assert_eq!(d.column_end, 7);
    }

    /// `compiler-artifact` and other non-diagnostic reasons must be dropped,
    /// and malformed / blank lines must not panic.
    #[test]
    fn ignores_non_compiler_messages_and_garbage() {
        let artifact = r#"{"reason":"compiler-artifact","package_id":"game_template 0.1.0","manifest_path":"/x/Cargo.toml","target":{"kind":["lib"],"crate_types":["rlib"],"name":"game_template","src_path":"/x/src/lib.rs","edition":"2021"},"profile":{"opt_level":"0","debuginfo":2,"debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":[],"executable":null,"fresh":true}"#;
        assert!(parse_cargo_json_line(artifact).is_empty());
        assert!(parse_cargo_json_line("").is_empty());
        assert!(parse_cargo_json_line("not json at all").is_empty());
        assert!(parse_cargo_json_line("{ \"reason\": ").is_empty());
    }
}
