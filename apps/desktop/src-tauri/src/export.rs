use happyjlc_core::error::AppError;
use happyjlc_core::kicad::SymbolFillColor;
use happyjlc_core::{
    ComponentConversionRequest, ConversionReporter, FailedItem, FootprintExportOptions,
    Model3dExportOptions, RunOptions, RunReporter, RunRequest, RunSummary, SymbolExportOptions,
    run_with_reporter,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::config::ExportPathMode;
use crate::monitor::MonitorState;

pub type NotifyFn = Arc<dyn Fn() + Send + Sync + 'static>;

#[derive(Clone, Serialize)]
pub struct ExportFinishedPayload {
    pub tool: &'static str,
    pub success: bool,
    pub message: String,
    pub failed_items: Vec<FailedItem>,
}

#[derive(Clone, Serialize)]
pub struct ExportProgressPayload {
    pub tool: &'static str,
    pub message: String,
    pub determinate: bool,
    pub current: Option<usize>,
    pub total: Option<usize>,
}

#[derive(Clone, Default)]
pub struct ExportCallbacks {
    pub on_progress: Option<Arc<dyn Fn(ExportProgressPayload) + Send + Sync + 'static>>,
    pub on_finished: Option<Arc<dyn Fn(ExportFinishedPayload) + Send + Sync + 'static>>,
    pub on_state_changed: Option<NotifyFn>,
}

pub struct ExportRequest {
    pub ids: Vec<String>,
    pub output_path: String,
    pub show_terminal: bool,
    pub parallel: usize,
    pub path_mode: ExportPathMode,
    pub export_symbol: bool,
    pub export_footprint: bool,
    pub export_model_3d: bool,
    pub overwrite_symbol: bool,
    pub overwrite_footprint: bool,
    pub overwrite_model_3d: bool,
    pub symbol_fill_color: Option<String>,
}

pub fn spawn_export(
    state: Arc<Mutex<MonitorState>>,
    req: ExportRequest,
    callbacks: ExportCallbacks,
) -> Result<(), String> {
    let core_request = build_core_request(&req)?;

    if let Ok(mut monitor) = state.lock() {
        monitor.export_running = true;
        monitor.export_last_result = None;
    }

    emit_progress(
        &callbacks,
        if req.show_terminal {
            "Preparing HappyJLC export in embedded background mode..."
        } else {
            "Preparing HappyJLC export..."
        },
        false,
        None,
        Some(req.ids.len()),
    );

    thread::spawn(move || {
        let reporter = Arc::new(CoreReporter::new(callbacks.clone()));
        let result = run_core_export(core_request, reporter);
        finish_export(&state, &callbacks, result, req.ids.len());
    });

    Ok(())
}

fn build_core_request(req: &ExportRequest) -> Result<RunRequest, String> {
    let output = PathBuf::from(&req.output_path);
    let symbol_fill_color = req
        .symbol_fill_color
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(SymbolFillColor::parse)
        .transpose()
        .map_err(|error| error.to_string())?;
    let project_relative = match req.path_mode {
        ExportPathMode::ProjectRelative => true,
        ExportPathMode::LibraryRelative => false,
        ExportPathMode::Auto => should_use_project_relative(&output),
    };
    let component = ComponentConversionRequest::new(
        req.export_symbol,
        req.export_footprint,
        req.export_model_3d,
        SymbolExportOptions::new(symbol_fill_color, req.overwrite_symbol),
        FootprintExportOptions::new(
            req.export_model_3d,
            project_relative,
            req.overwrite_footprint,
        ),
        Model3dExportOptions::new(req.overwrite_model_3d),
    )
    .map_err(|error| error.to_string())?;

    RunRequest::new(
        req.ids.clone(),
        RunOptions {
            output,
            continue_on_error: true,
            parallel: req.parallel,
        },
        component,
    )
    .map_err(|error| error.to_string())
}

fn run_core_export(
    request: RunRequest,
    reporter: Arc<CoreReporter>,
) -> Result<Option<RunSummary>, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Failed to create export runtime: {error}"))?;
    runtime
        .block_on(run_with_reporter(request, reporter))
        .map_err(|error| error.to_string())
}

fn finish_export(
    state: &Arc<Mutex<MonitorState>>,
    callbacks: &ExportCallbacks,
    result: Result<Option<RunSummary>, String>,
    requested_count: usize,
) {
    let (success, message, failed_items) = match result {
        Ok(Some(summary)) if summary.failed == 0 => (
            true,
            format!(
                "HappyJLC completed: {} item(s) -> {}",
                summary.success,
                summary.output_dir.display()
            ),
            Vec::new(),
        ),
        Ok(Some(summary)) => {
            let failed_items = summary.failed_items.clone();
            (
                false,
                format!(
                    "HappyJLC completed with {} failure(s) out of {} item(s): {}",
                    summary.failed,
                    summary.total,
                    summary.failed_ids.join(", ")
                ),
                failed_items,
            )
        }
        Ok(None) => (
            true,
            "All components already completed.".to_string(),
            Vec::new(),
        ),
        Err(error) => (
            false,
            format!("HappyJLC export failed: {error}"),
            Vec::new(),
        ),
    };

    if let Ok(mut monitor) = state.lock() {
        monitor.export_running = false;
        monitor.export_last_result = Some(message.clone());
        monitor.add_debug_log(message.clone());
    }
    notify_state_changed(callbacks);
    emit_finished(
        callbacks,
        ExportFinishedPayload {
            tool: "export",
            success,
            message: if requested_count == 0 {
                "No components were requested.".to_string()
            } else {
                message
            },
            failed_items,
        },
    );
}

struct CoreReporter {
    callbacks: ExportCallbacks,
    current: AtomicUsize,
    total: AtomicUsize,
}

impl CoreReporter {
    fn new(callbacks: ExportCallbacks) -> Self {
        Self {
            callbacks,
            current: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
        }
    }

    fn emit_component_progress(&self, message: impl Into<String>, current: usize) {
        let total = self.total.load(Ordering::Relaxed);
        emit_progress(&self.callbacks, message, true, Some(current), Some(total));
    }
}

impl ConversionReporter for CoreReporter {
    fn emit_output_line(&self, line: &str) {
        emit_progress(
            &self.callbacks,
            line.to_string(),
            true,
            Some(self.current.load(Ordering::Relaxed)),
            Some(self.total.load(Ordering::Relaxed)),
        );
    }
}

impl RunReporter for CoreReporter {
    fn on_resume_skipped(&self, skipped: usize) {
        emit_progress(
            &self.callbacks,
            format!("Skipped {skipped} completed component(s)."),
            false,
            None,
            None,
        );
    }

    fn on_batch_started(&self, _is_batch: bool, total_count: usize, _parallel: usize) {
        self.total.store(total_count, Ordering::Relaxed);
        emit_progress(
            &self.callbacks,
            "Running HappyJLC export...",
            true,
            Some(0),
            Some(total_count),
        );
    }

    fn on_component_started(&self, lcsc_id: &str) {
        self.emit_component_progress(
            format!("Converting {lcsc_id}..."),
            self.current.load(Ordering::Relaxed),
        );
    }

    fn on_component_succeeded(&self, lcsc_id: &str) {
        let current = self.current.fetch_add(1, Ordering::Relaxed) + 1;
        self.emit_component_progress(format!("Converted {lcsc_id}"), current);
    }

    fn on_component_failed(&self, lcsc_id: &str, error: &AppError, _continued: bool) {
        let current = self.current.fetch_add(1, Ordering::Relaxed) + 1;
        self.emit_component_progress(format!("Failed {lcsc_id}: {error}"), current);
    }

    fn on_task_panicked(&self, error: &str) {
        emit_progress(
            &self.callbacks,
            format!("Export task failed: {error}"),
            false,
            None,
            None,
        );
    }

    fn finish(&self) {}
}

fn emit_progress(
    callbacks: &ExportCallbacks,
    message: impl Into<String>,
    determinate: bool,
    current: Option<usize>,
    total: Option<usize>,
) {
    if let Some(on_progress) = &callbacks.on_progress {
        on_progress(ExportProgressPayload {
            tool: "export",
            message: message.into(),
            determinate,
            current,
            total,
        });
    }
}

fn emit_finished(callbacks: &ExportCallbacks, payload: ExportFinishedPayload) {
    if let Some(on_finished) = &callbacks.on_finished {
        on_finished(payload);
    }
}

fn notify_state_changed(callbacks: &ExportCallbacks) {
    if let Some(on_state_changed) = &callbacks.on_state_changed {
        on_state_changed();
    }
}

fn should_use_project_relative(output_path: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(output_path) else {
        return false;
    };

    entries.filter_map(Result::ok).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| matches!(extension, "kicad_pro" | "kicad_pcb" | "pro"))
    })
}

#[cfg(test)]
mod tests {
    use super::{ExportFinishedPayload, ExportRequest, build_core_request};
    use crate::config::ExportPathMode;
    use happyjlc_core::{ErrorCategory, FailedItem};

    #[test]
    fn translates_desktop_export_options_to_core_request() {
        let request = build_core_request(&ExportRequest {
            ids: vec!["C2040".to_string()],
            output_path: "/tmp/happyjlc-library".to_string(),
            show_terminal: false,
            parallel: 4,
            path_mode: ExportPathMode::ProjectRelative,
            export_symbol: true,
            export_footprint: true,
            export_model_3d: true,
            overwrite_symbol: true,
            overwrite_footprint: false,
            overwrite_model_3d: true,
            symbol_fill_color: Some("#005C8FCC".to_string()),
        })
        .expect("desktop request should translate");

        assert_eq!(request.lcsc_ids, vec!["C2040"]);
        assert_eq!(request.run.parallel, 4);
        assert!(request.component.convert_symbol);
        assert!(request.component.footprint.project_relative_3d);
        assert!(request.component.symbol.overwrite);
        assert!(!request.component.footprint.overwrite);
        assert!(request.component.model_3d.overwrite);
        assert!(request.component.symbol.symbol_fill_color.is_some());
    }

    #[test]
    fn rejects_requests_without_export_assets() {
        let result = build_core_request(&ExportRequest {
            ids: vec!["C2040".to_string()],
            output_path: "/tmp/happyjlc-library".to_string(),
            show_terminal: false,
            parallel: 4,
            path_mode: ExportPathMode::Auto,
            export_symbol: false,
            export_footprint: false,
            export_model_3d: false,
            overwrite_symbol: false,
            overwrite_footprint: false,
            overwrite_model_3d: false,
            symbol_fill_color: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn serializes_categorized_failed_items_in_finished_payload() {
        let payload = ExportFinishedPayload {
            tool: "export",
            success: false,
            message: "partial failure".to_string(),
            failed_items: vec![FailedItem {
                lcsc_id: "C2040".to_string(),
                category: ErrorCategory::NotFound,
            }],
        };

        let value = serde_json::to_value(payload).expect("payload should serialize");
        assert_eq!(value["failed_items"][0]["lcsc_id"], "C2040");
        assert_eq!(value["failed_items"][0]["category"], "not_found");
    }
}
