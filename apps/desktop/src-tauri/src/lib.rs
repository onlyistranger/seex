pub mod app_paths;
pub mod config;
pub mod controller;
pub mod export;
pub mod extract;
pub mod imported_symbols;
pub mod inventory;
pub mod monitor;
pub mod tui;

use controller::AppController;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WindowEvent};

use crate::config::{AppConfig, ExportPathMode, ModelFormat};
use crate::inventory::{
    BomPreview, ConfirmBomDeductionRequest, ImportBomRequest, ImportBomResult, InventoryPartInput,
    InventoryResponse, InventoryStockAdjustment, ProductionRecord,
};

#[derive(Serialize, Clone)]
pub struct AppState {
    pub history: Vec<(String, String)>,
    pub matched: Vec<(String, String)>,
    pub keyword: String,
    pub export_output_path: String,
    pub export_last_result: Option<String>,
    pub export_show_terminal: bool,
    pub export_parallel: usize,
    pub export_path_mode: ExportPathMode,
    pub export_symbol: bool,
    pub export_footprint: bool,
    pub export_model_3d: bool,
    pub export_overwrite_symbol: bool,
    pub export_overwrite_footprint: bool,
    pub export_overwrite_model_3d: bool,
    pub export_symbol_fill_color: Option<String>,
    pub export_running: bool,
    pub monitoring: bool,
    pub always_on_top: bool,
    pub history_count: usize,
    pub matched_count: usize,
    pub history_save_path: String,
    pub matched_save_path: String,
    pub imported_parts_save_path: String,
    pub default_model_format: ModelFormat,
}

pub struct ManagedApp {
    pub controller: AppController,
}

fn build_app_state(controller: &AppController) -> AppState {
    let defaults = AppConfig::default();

    if let Ok(m) = controller.state().lock() {
        AppState {
            history_count: m.history.len(),
            matched_count: m.matched.len(),
            history: m.history.clone(),
            matched: m.matched.clone(),
            keyword: m.keyword.clone(),
            export_output_path: m.export_output_path.clone(),
            export_last_result: m.export_last_result.clone(),
            export_show_terminal: m.export_show_terminal,
            export_parallel: m.export_parallel,
            export_path_mode: m.export_path_mode,
            export_symbol: m.export_symbol,
            export_footprint: m.export_footprint,
            export_model_3d: m.export_model_3d,
            export_overwrite_symbol: m.export_overwrite_symbol,
            export_overwrite_footprint: m.export_overwrite_footprint,
            export_overwrite_model_3d: m.export_overwrite_model_3d,
            export_symbol_fill_color: m.export_symbol_fill_color.clone(),
            export_running: m.export_running,
            monitoring: m.monitoring,
            always_on_top: m.always_on_top,
            history_save_path: m.history_save_path.clone(),
            matched_save_path: m.matched_save_path.clone(),
            imported_parts_save_path: m.imported_parts_save_path.clone(),
            default_model_format: m.default_model_format,
        }
    } else {
        AppState {
            history: vec![],
            matched: vec![],
            keyword: String::new(),
            export_output_path: controller.paths().default_export_output_path_string(),
            export_last_result: None,
            export_show_terminal: defaults.export.show_terminal,
            export_parallel: defaults.export.parallel,
            export_path_mode: defaults.export.path_mode,
            export_symbol: defaults.export.export_symbol,
            export_footprint: defaults.export.export_footprint,
            export_model_3d: defaults.export.export_model_3d,
            export_overwrite_symbol: defaults.export.overwrite_symbol,
            export_overwrite_footprint: defaults.export.overwrite_footprint,
            export_overwrite_model_3d: defaults.export.overwrite_model_3d,
            export_symbol_fill_color: defaults.export.symbol_fill_color,
            export_running: false,
            monitoring: true,
            always_on_top: defaults.monitor.always_on_top,
            history_count: 0,
            matched_count: 0,
            history_save_path: controller.paths().default_history_save_path_string(),
            matched_save_path: controller.paths().default_matched_save_path_string(),
            imported_parts_save_path: controller.paths().default_imported_parts_save_path_string(),
            default_model_format: defaults.monitor.default_model_format,
        }
    }
}

fn update_state<T, F>(app: &State<ManagedApp>, update: F) -> Result<T, String>
where
    F: FnOnce(&mut crate::monitor::MonitorState) -> Result<T, String>,
{
    app.controller.update_state_and_save(update)
}

fn update_state_and_ignore_lock_error<F>(app: &State<ManagedApp>, update: F)
where
    F: FnOnce(&mut crate::monitor::MonitorState),
{
    let _ = update_state(app, |state| {
        update(state);
        Ok(())
    });
}

#[tauri::command]
fn get_state(app: State<ManagedApp>) -> AppState {
    build_app_state(&app.controller)
}

#[tauri::command]
fn set_keyword(app: State<ManagedApp>, keyword: String) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.set_keyword(keyword);
    }
}

#[tauri::command]
fn toggle_monitoring(app: State<ManagedApp>) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.monitoring = !m.monitoring;
        if m.monitoring {
            m.last_content.clear();
            m.initialized = true;
        }
    }
}

#[tauri::command]
fn delete_history(app: State<ManagedApp>, index: usize) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.delete_history(index);
    }
}

#[tauri::command]
fn delete_matched(app: State<ManagedApp>, index: usize) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.delete_matched(index);
    }
}

#[tauri::command]
fn clear_all(app: State<ManagedApp>) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.history.clear();
        m.matched.clear();
        m.last_content.clear();
        m.initialized = false;
        m.match_debug_log.clear();
        m.export_last_result = None;
        m.export_running = false;
    }
}

#[tauri::command]
fn save_history(app: State<ManagedApp>) -> String {
    app.controller.save_history()
}

#[tauri::command]
fn save_matched(app: State<ManagedApp>) -> String {
    app.controller.save_matched()
}

#[tauri::command]
fn save_imported_parts(app: State<ManagedApp>) -> String {
    app.controller.save_imported_parts()
}

#[tauri::command]
fn save_lcsc_parts(app: State<ManagedApp>, parts: Vec<String>) -> String {
    app.controller.save_lcsc_parts(parts)
}

#[tauri::command]
fn import_imported_parts(app: State<ManagedApp>) -> String {
    app.controller.import_imported_parts()
}

#[tauri::command]
fn queue_lcsc_parts(app: State<ManagedApp>, parts: Vec<String>) -> String {
    app.controller.queue_lcsc_parts(parts)
}

#[tauri::command]
fn set_history_save_path(app: State<ManagedApp>, path: String) {
    let paths = app.controller.paths().clone();
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_history_save_path(path, &paths);
    });
}

#[tauri::command]
fn set_matched_save_path(app: State<ManagedApp>, path: String) {
    let paths = app.controller.paths().clone();
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_matched_save_path(path, &paths);
    });
}

#[tauri::command]
fn set_imported_parts_save_path(app: State<ManagedApp>, path: String) {
    let paths = app.controller.paths().clone();
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_imported_parts_save_path(path, &paths);
    });
}

#[tauri::command]
fn set_export_path(app: State<ManagedApp>, path: String) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_export_output_path(path);
    });
}

#[tauri::command]
fn toggle_export_terminal(app: State<ManagedApp>) {
    update_state_and_ignore_lock_error(&app, |state| {
        state.toggle_export_show_terminal();
    });
}

#[tauri::command]
fn set_export_parallel(app: State<ManagedApp>, parallel: usize) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_export_parallel(parallel);
    });
}

#[tauri::command]
fn set_export_path_mode(app: State<ManagedApp>, path_mode: ExportPathMode) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_export_path_mode(path_mode);
    });
}

#[tauri::command]
fn set_default_model_format(app: State<ManagedApp>, format: String) -> Result<(), String> {
    let format = ModelFormat::parse(&format)
        .ok_or_else(|| format!("Unsupported default model format: {}", format.trim()))?;
    update_state(&app, move |state| {
        state.default_model_format = format;
        Ok(())
    })
}

fn parse_export_asset_kind(target: &str) -> Result<&str, String> {
    match target.trim().to_ascii_lowercase().as_str() {
        "symbol" => Ok("symbol"),
        "footprint" => Ok("footprint"),
        "3d" | "model_3d" | "model-3d" => Ok("model_3d"),
        _ => Err(format!("Unknown export asset type: {}", target)),
    }
}

#[tauri::command]
fn set_export_enabled(app: State<ManagedApp>, target: String, enabled: bool) -> Result<(), String> {
    update_state(&app, move |state| {
        match parse_export_asset_kind(&target)? {
            "symbol" => state.set_export_symbol(enabled),
            "footprint" => state.set_export_footprint(enabled),
            "model_3d" => state.set_export_model_3d(enabled),
            _ => unreachable!(),
        }
        Ok(())
    })
}

#[tauri::command]
fn set_export_symbol(app: State<ManagedApp>, enabled: bool) -> Result<(), String> {
    set_export_enabled(app, "symbol".to_string(), enabled)
}

#[tauri::command]
fn set_export_footprint(app: State<ManagedApp>, enabled: bool) -> Result<(), String> {
    set_export_enabled(app, "footprint".to_string(), enabled)
}

#[tauri::command]
fn set_export_model_3d(app: State<ManagedApp>, enabled: bool) -> Result<(), String> {
    set_export_enabled(app, "model_3d".to_string(), enabled)
}

#[tauri::command]
fn set_export_overwrite_enabled(
    app: State<ManagedApp>,
    target: String,
    enabled: bool,
) -> Result<(), String> {
    update_state(&app, move |state| {
        match parse_export_asset_kind(&target)? {
            "symbol" => state.set_export_overwrite_symbol(enabled),
            "footprint" => state.set_export_overwrite_footprint(enabled),
            "model_3d" => state.set_export_overwrite_model_3d(enabled),
            _ => unreachable!(),
        }
        Ok(())
    })
}

#[tauri::command]
fn set_export_overwrite_symbol(app: State<ManagedApp>, overwrite: bool) -> Result<(), String> {
    set_export_overwrite_enabled(app, "symbol".to_string(), overwrite)
}

#[tauri::command]
fn set_export_overwrite_footprint(app: State<ManagedApp>, overwrite: bool) -> Result<(), String> {
    set_export_overwrite_enabled(app, "footprint".to_string(), overwrite)
}

#[tauri::command]
fn set_export_overwrite_model_3d(app: State<ManagedApp>, overwrite: bool) -> Result<(), String> {
    set_export_overwrite_enabled(app, "model_3d".to_string(), overwrite)
}

#[tauri::command]
fn set_export_symbol_fill_color(app: State<ManagedApp>, color: Option<String>) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_export_symbol_fill_color(color);
    });
}

#[tauri::command]
fn set_window_always_on_top(
    app: State<ManagedApp>,
    app_handle: AppHandle,
    always_on_top: bool,
) -> Result<(), String> {
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window
        .set_always_on_top(always_on_top)
        .map_err(|err| err.to_string())?;

    update_state(&app, move |state| {
        state.always_on_top = always_on_top;
        Ok(())
    })
}

#[tauri::command]
fn export(app: State<ManagedApp>, app_handle: AppHandle) -> String {
    let emit_handle = app_handle.clone();
    let state_handle = app_handle.clone();
    let finished_handle = app_handle;

    app.controller.spawn_export(export::ExportCallbacks {
        on_progress: Some(Arc::new(move |payload| {
            let _ = emit_handle.emit("export-progress", payload);
        })),
        on_finished: Some(Arc::new(move |payload| {
            let _ = finished_handle.emit("export-finished", payload);
        })),
        on_state_changed: Some(Arc::new(move || {
            let _ = state_handle.emit("clipboard-changed", ());
        })),
    })
}

#[tauri::command]
fn get_unique_ids(app: State<ManagedApp>) -> Vec<String> {
    if let Ok(m) = app.controller.state().lock() {
        m.get_unique_ids()
    } else {
        vec![]
    }
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    let mut clip = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clip.set_text(&text).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_imported_symbols(
    app: State<ManagedApp>,
) -> Result<imported_symbols::ImportedSymbolsResponse, String> {
    app.controller.get_imported_symbols()
}

#[tauri::command]
fn add_kicad_library_path(app: State<ManagedApp>, path: String) -> Result<Vec<String>, String> {
    app.controller.add_kicad_library_path(path)
}

#[tauri::command]
fn remove_kicad_library_path(app: State<ManagedApp>, path: String) -> Result<Vec<String>, String> {
    app.controller.remove_kicad_library_path(path)
}

#[tauri::command]
fn read_imported_model(
    app: State<ManagedApp>,
    request: imported_symbols::ImportedModelRequest,
) -> Result<Vec<u8>, String> {
    let output_path = {
        let state = app
            .controller
            .state()
            .lock()
            .map_err(|_| "State lock failed".to_string())?;
        state.export_output_path.clone()
    };

    imported_symbols::read_imported_model(std::path::Path::new(&output_path), request)
}

#[tauri::command]
fn update_imported_symbol(
    app: State<ManagedApp>,
    request: imported_symbols::ImportedSymbolUpdateRequest,
) -> Result<String, String> {
    let output_path = {
        let state = app
            .controller
            .state()
            .lock()
            .map_err(|_| "State lock failed".to_string())?;
        state.export_output_path.clone()
    };

    imported_symbols::update_imported_symbol(std::path::Path::new(&output_path), request)
}

#[tauri::command]
fn delete_imported_symbol(
    app: State<ManagedApp>,
    request: imported_symbols::ImportedSymbolDeleteRequest,
) -> Result<String, String> {
    let output_path = {
        let state = app
            .controller
            .state()
            .lock()
            .map_err(|_| "State lock failed".to_string())?;
        state.export_output_path.clone()
    };

    imported_symbols::delete_imported_symbol(std::path::Path::new(&output_path), request)
}

#[tauri::command]
fn get_inventory(
    app: State<ManagedApp>,
    query: Option<String>,
) -> Result<InventoryResponse, String> {
    app.controller.get_inventory(query.as_deref().unwrap_or(""))
}

#[tauri::command]
fn save_inventory_part(app: State<ManagedApp>, input: InventoryPartInput) -> Result<(), String> {
    app.controller.save_inventory_part(input)
}

#[tauri::command]
fn delete_inventory_part(app: State<ManagedApp>, id: String) -> Result<(), String> {
    app.controller.delete_inventory_part(&id)
}

#[tauri::command]
fn adjust_inventory_stock(
    app: State<ManagedApp>,
    adjustment: InventoryStockAdjustment,
) -> Result<(), String> {
    app.controller.adjust_inventory_stock(adjustment)
}

#[tauri::command]
fn import_matched_to_inventory(app: State<ManagedApp>) -> Result<String, String> {
    app.controller.import_matched_to_inventory()
}

#[tauri::command]
fn preview_inventory_bom(
    app: State<ManagedApp>,
    path: String,
    boards: u64,
) -> Result<BomPreview, String> {
    app.controller.preview_inventory_bom(&path, boards)
}

#[tauri::command]
fn import_inventory_bom(
    app: State<ManagedApp>,
    request: ImportBomRequest,
) -> Result<ImportBomResult, String> {
    app.controller.import_inventory_bom(request)
}

#[tauri::command]
fn confirm_inventory_bom(
    app: State<ManagedApp>,
    request: ConfirmBomDeductionRequest,
) -> Result<String, String> {
    app.controller.confirm_inventory_bom(request)
}

#[tauri::command]
fn get_production_records(
    app: State<ManagedApp>,
    limit: Option<usize>,
) -> Result<Vec<ProductionRecord>, String> {
    app.controller.get_production_records(limit.unwrap_or(20))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let paths = app_paths::AppPaths::resolve(&app_handle).map_err(std::io::Error::other)?;
            let emit_handle = app_handle;
            let controller = AppController::new(
                paths,
                Arc::new(move || {
                    let _ = emit_handle.emit("clipboard-changed", ());
                }),
            )
            .map_err(std::io::Error::other)?;
            let (always_on_top, window_width, window_height, window_x, window_y) = controller
                .state()
                .lock()
                .map(|state| {
                    (
                        state.always_on_top,
                        state.window_width,
                        state.window_height,
                        state.window_x,
                        state.window_y,
                    )
                })
                .unwrap_or((false, None, None, None, None));
            app.manage(ManagedApp { controller });
            if let Some(window) = app.get_webview_window("main") {
                if let (Some(width), Some(height)) = (window_width, window_height) {
                    let _ = window.set_size(PhysicalSize::new(width, height));
                }
                if let (Some(x), Some(y)) = (window_x, window_y) {
                    let _ = window.set_position(PhysicalPosition::new(x, y));
                }
                let _ = window.set_always_on_top(always_on_top);
                let app_handle = app.handle().clone();
                let event_window = window.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::Resized(size) => {
                        let is_maximized = event_window.is_maximized().unwrap_or(false);
                        let is_minimized = event_window.is_minimized().unwrap_or(false);
                        if !is_maximized
                            && !is_minimized
                            && let Ok(mut state) =
                                app_handle.state::<ManagedApp>().controller.state().lock()
                        {
                            state.set_window_size(size.width, size.height);
                        }
                    }
                    WindowEvent::Moved(position) => {
                        let is_maximized = event_window.is_maximized().unwrap_or(false);
                        let is_minimized = event_window.is_minimized().unwrap_or(false);
                        if !is_maximized
                            && !is_minimized
                            && let Ok(mut state) =
                                app_handle.state::<ManagedApp>().controller.state().lock()
                        {
                            state.set_window_position(position.x, position.y);
                        }
                    }
                    WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
                        if let Ok(position) = event_window.outer_position()
                            && let Ok(mut state) =
                                app_handle.state::<ManagedApp>().controller.state().lock()
                        {
                            state.set_window_position(position.x, position.y);
                        }
                        app_handle.state::<ManagedApp>().controller.save_config();
                    }
                    _ => {}
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_keyword,
            toggle_monitoring,
            delete_history,
            delete_matched,
            clear_all,
            save_history,
            save_matched,
            save_imported_parts,
            save_lcsc_parts,
            import_imported_parts,
            queue_lcsc_parts,
            set_history_save_path,
            set_matched_save_path,
            set_imported_parts_save_path,
            set_export_path,
            toggle_export_terminal,
            set_export_parallel,
            set_export_path_mode,
            set_default_model_format,
            set_export_enabled,
            set_export_symbol,
            set_export_footprint,
            set_export_model_3d,
            set_export_overwrite_enabled,
            set_export_overwrite_symbol,
            set_export_overwrite_footprint,
            set_export_overwrite_model_3d,
            set_export_symbol_fill_color,
            set_window_always_on_top,
            export,
            get_unique_ids,
            copy_to_clipboard,
            get_imported_symbols,
            add_kicad_library_path,
            remove_kicad_library_path,
            read_imported_model,
            update_imported_symbol,
            delete_imported_symbol,
            get_inventory,
            save_inventory_part,
            delete_inventory_part,
            adjust_inventory_stock,
            import_matched_to_inventory,
            preview_inventory_bom,
            import_inventory_bom,
            confirm_inventory_bom,
            get_production_records,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
