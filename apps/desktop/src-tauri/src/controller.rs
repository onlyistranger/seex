use chrono::Local;
use regex::Regex;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::app_paths::AppPaths;
use crate::config::{AppConfig, ExportConfig, MonitorConfig};
use crate::export;
use crate::imported_symbols;
use crate::inventory::{
    BomPreview, ConfirmBomDeductionRequest, ImportBomRequest, ImportBomResult,
    InventoryLibraryPart, InventoryPartInput, InventoryRepository, InventoryResponse,
    InventoryStockAdjustment, ProductionRecord,
};
use crate::monitor::{MonitorHandle, MonitorState, NotifyFn};

const DEFAULT_KEYWORD: &str =
    "regex:\u{7f16}\u{53f7}[\u{ff1a}:]\\s*(C\\d+)||regex:(?m)^(C\\d{3,})$";

struct LcscPartCollectionSummary {
    normalized_parts: Vec<String>,
    matched_part_count: usize,
    duplicate_part_count: usize,
    invalid_entry_count: usize,
}

pub struct AppController {
    state: Arc<Mutex<MonitorState>>,
    paths: AppPaths,
    inventory: Arc<Mutex<InventoryRepository>>,
    _monitor: MonitorHandle,
}

impl AppController {
    pub fn new(paths: AppPaths, notifier: NotifyFn) -> Result<Self, String> {
        let config = AppConfig::load(&paths);
        let state = Arc::new(Mutex::new(MonitorState::new(&paths)));

        if let Ok(mut s) = state.lock() {
            apply_config(&mut s, &config, &paths);
            s.set_keyword(DEFAULT_KEYWORD.to_string());
        }

        if let Ok(s) = state.lock() {
            snapshot_config(&s).save(&paths);
        }

        let monitor = MonitorHandle::spawn_with_callback(Arc::clone(&state), notifier);
        let inventory = Arc::new(Mutex::new(InventoryRepository::open(&paths)?));

        Ok(Self {
            state,
            paths,
            inventory,
            _monitor: monitor,
        })
    }

    pub fn new_native() -> Result<Self, String> {
        Self::new(AppPaths::resolve_native()?, Arc::new(|| {}))
    }

    pub fn state(&self) -> &Arc<Mutex<MonitorState>> {
        &self.state
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn get_imported_symbols(
        &self,
    ) -> Result<imported_symbols::ImportedSymbolsResponse, String> {
        let (output_path, configured_paths) = self.library_scan_config()?;
        imported_symbols::load_imported_symbols_with_paths(
            Path::new(&output_path),
            &configured_paths,
        )
    }

    pub fn add_kicad_library_path(&self, path: String) -> Result<Vec<String>, String> {
        let normalized = path.trim();
        if normalized.is_empty() {
            return Err("KiCad library path cannot be empty".to_string());
        }
        self.update_state_and_save(|state| {
            let path = PathBuf::from(normalized);
            if !path.exists() {
                return Err(format!("KiCad library path does not exist: {normalized}"));
            }
            let value = path
                .canonicalize()
                .map_err(|err| format!("Cannot access KiCad library path: {err}"))?
                .display()
                .to_string();
            if !state.kicad_library_paths.iter().any(|item| item == &value) {
                state.kicad_library_paths.push(value);
                state.kicad_library_paths.sort();
            }
            Ok(state.kicad_library_paths.clone())
        })
    }

    pub fn remove_kicad_library_path(&self, path: String) -> Result<Vec<String>, String> {
        self.update_state_and_save(|state| {
            state.kicad_library_paths.retain(|item| item != &path);
            Ok(state.kicad_library_paths.clone())
        })
    }

    fn library_scan_config(&self) -> Result<(String, Vec<String>), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "State lock failed".to_string())?;
        Ok((
            state.export_output_path.clone(),
            state.kicad_library_paths.clone(),
        ))
    }

    pub fn get_inventory(&self, query: &str) -> Result<InventoryResponse, String> {
        self.sync_inventory_library()?;
        let inventory = self
            .inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?;
        inventory.get_parts(query)
    }

    pub fn save_inventory_part(&self, input: InventoryPartInput) -> Result<(), String> {
        let mut input = input;
        let library_lcsc = input
            .library_lcsc
            .clone()
            .filter(|value| !value.trim().is_empty());
        let has_library_selection = library_lcsc.is_some()
            || input
                .library_source_file
                .as_deref()
                .is_some_and(|source| !source.trim().is_empty())
                && input
                    .library_symbol_name
                    .as_deref()
                    .is_some_and(|symbol| !symbol.trim().is_empty());
        if has_library_selection {
            let library_part = self
                .load_inventory_library()?
                .into_iter()
                .find(|part| {
                    library_lcsc.as_deref().is_some_and(|lcsc| {
                        !part.lcsc_part.is_empty() && part.lcsc_part.eq_ignore_ascii_case(lcsc)
                    }) || (input.library_source_file.as_deref() == Some(part.source_file.as_str())
                        && input.library_symbol_name.as_deref() == Some(part.symbol_name.as_str()))
                })
                .ok_or_else(|| "Selected component is no longer in the library".to_string())?;
            input.library_lcsc =
                (!library_part.lcsc_part.is_empty()).then_some(library_part.lcsc_part.clone());
            input.library_symbol_name = Some(library_part.symbol_name.clone());
            input.library_source_file = Some(library_part.source_file.clone());
            input.supplier_part_number =
                (!library_part.lcsc_part.is_empty()).then_some(library_part.lcsc_part.clone());
            input.name = library_part.value;
            input.package = library_part.package;
        }
        self.inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?
            .save_part(input)
    }

    pub fn delete_inventory_part(&self, id: &str) -> Result<(), String> {
        self.inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?
            .delete_part(id)
    }

    pub fn adjust_inventory_stock(
        &self,
        adjustment: InventoryStockAdjustment,
    ) -> Result<(), String> {
        self.inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?
            .adjust_stock(adjustment)
    }

    pub fn import_matched_to_inventory(&self) -> Result<String, String> {
        let ids = self
            .state
            .lock()
            .map_err(|_| "State lock failed".to_string())?
            .get_unique_ids();
        let library_parts = self
            .load_inventory_library()?
            .into_iter()
            .filter(|part| {
                ids.iter()
                    .any(|id| id.eq_ignore_ascii_case(&part.lcsc_part))
            })
            .collect::<Vec<_>>();
        let added = self
            .inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?
            .import_library_parts(&library_parts)?;
        Ok(format!("Added {} matched part(s) to inventory", added))
    }

    pub fn preview_inventory_bom(&self, path: &str, boards: u64) -> Result<BomPreview, String> {
        self.sync_inventory_library()?;
        let library_parts = self.load_inventory_library()?;
        let inventory = self
            .inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?;
        inventory.preview_csv(path, boards, &library_parts)
    }

    pub fn import_inventory_bom(
        &self,
        request: ImportBomRequest,
    ) -> Result<ImportBomResult, String> {
        self.sync_inventory_library()?;
        let library_parts = self.load_inventory_library()?;
        self.inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?
            .import_bom(request, &library_parts)
    }

    pub fn confirm_inventory_bom(
        &self,
        request: ConfirmBomDeductionRequest,
    ) -> Result<String, String> {
        let mut inventory = self
            .inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?;
        inventory.confirm_csv(request)
    }

    pub fn get_production_records(&self, limit: usize) -> Result<Vec<ProductionRecord>, String> {
        self.inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?
            .production_records(limit)
    }

    fn load_inventory_library(&self) -> Result<Vec<InventoryLibraryPart>, String> {
        let (output_path, configured_paths) = self.library_scan_config()?;
        let response = imported_symbols::load_imported_symbols_with_paths(
            Path::new(&output_path),
            &configured_paths,
        )?;
        Ok(response
            .items
            .into_iter()
            .map(|item| InventoryLibraryPart {
                library_key: item.library_key,
                lcsc_part: item.lcsc_part,
                value: item.value,
                symbol_name: item.symbol_name,
                package: item.package,
                source_file: item.source_file,
                has_model: item.model_available || !item.models.is_empty(),
                source_kind: item.source_kind,
                editable: item.editable,
            })
            .collect())
    }

    fn sync_inventory_library(&self) -> Result<(), String> {
        let library_parts = self.load_inventory_library()?;
        self.inventory
            .lock()
            .map_err(|_| "Inventory lock failed".to_string())?
            .sync_library(&library_parts)?;
        Ok(())
    }

    pub fn save_config(&self) {
        if let Ok(state) = self.state.lock() {
            snapshot_config(&state).save(&self.paths);
        }
    }

    pub fn update_state_and_save<T, F>(&self, update: F) -> Result<T, String>
    where
        F: FnOnce(&mut MonitorState) -> Result<T, String>,
    {
        let config_snapshot = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "State lock failed".to_string())?;
            let result = update(&mut state)?;
            let config = snapshot_config(&state);
            (result, config)
        };

        let (result, config) = config_snapshot;
        config.save(&self.paths);
        Ok(result)
    }

    pub fn save_history(&self) -> String {
        let (path, entries) = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            if m.history.is_empty() {
                return "No history to save".to_string();
            }
            (PathBuf::from(&m.history_save_path), m.history.clone())
        };

        if let Err(err) = ensure_parent_dir(&path) {
            return format!("Save failed: {}", err);
        }
        let file = match std::fs::File::create(&path) {
            Ok(file) => file,
            Err(err) => return format!("Save failed: {}", err),
        };
        let mut writer = std::io::BufWriter::new(file);
        for (time, content) in &entries {
            if let Err(err) = writeln!(writer, "[{}] {}", time, content) {
                return format!("Save failed: {}", err);
            }
        }
        if let Err(err) = writer.flush() {
            return format!("Save failed: {}", err);
        }
        format!("Saved to {}", path.display())
    }

    pub fn save_matched(&self) -> String {
        let (path, entries) = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            if m.matched.is_empty() {
                return "No matched results to export".to_string();
            }
            (PathBuf::from(&m.matched_save_path), m.matched.clone())
        };

        if let Err(err) = ensure_parent_dir(&path) {
            return format!("Export failed: {}", err);
        }
        let file = match std::fs::File::create(&path) {
            Ok(file) => file,
            Err(err) => return format!("Export failed: {}", err),
        };
        let mut writer = std::io::BufWriter::new(file);
        for (_, extracted) in &entries {
            if let Err(err) = writeln!(writer, "{}", extracted) {
                return format!("Export failed: {}", err);
            }
        }
        if let Err(err) = writer.flush() {
            return format!("Export failed: {}", err);
        }
        format!("Exported to {}", path.display())
    }

    pub fn save_imported_parts(&self) -> String {
        let (output_path, save_path) = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            (
                m.export_output_path.clone(),
                PathBuf::from(&m.imported_parts_save_path),
            )
        };

        let imported = match imported_symbols::load_imported_symbols(Path::new(&output_path)) {
            Ok(imported) => imported,
            Err(err) => return format!("Export failed: {}", err),
        };
        let parts = imported_symbols::unique_lcsc_parts(&imported.items);
        if parts.is_empty() {
            return "No imported LCSC Part values to export".to_string();
        }

        match save_parts_file(&save_path, &parts) {
            Ok(()) => format!(
                "Exported {} LCSC part(s) to {}",
                parts.len(),
                save_path.display()
            ),
            Err(message) => message,
        }
    }

    pub fn import_imported_parts(&self) -> String {
        let import_path = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            PathBuf::from(&m.imported_parts_save_path)
        };

        let content = match std::fs::read_to_string(&import_path) {
            Ok(content) => content,
            Err(err) => return format!("Import failed: {} ({})", import_path.display(), err),
        };
        let parsed = parse_imported_parts_text(&content);
        if parsed.normalized_parts.is_empty() {
            return format!("No LCSC parts found in {}", import_path.display());
        }

        let (added, already_queued, matched_count) = {
            let Ok(mut m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            let timestamp = Local::now().format("%H:%M:%S").to_string();
            let merge = m.merge_matched_ids(parsed.normalized_parts.iter().cloned(), timestamp);
            m.add_debug_log(format!(
                "Imported {} new matched parts from {}",
                merge.added,
                import_path.display()
            ));
            (merge.added, merge.already_present, m.matched.len())
        };

        format!(
            "Imported {} LCSC part(s) from {} ({} matched part occurrence(s), {} unique part(s), {} duplicate occurrence(s) in file, {} invalid line(s), {} already queued, matched queue now has {} item(s))",
            added,
            import_path.display(),
            parsed.matched_part_count,
            parsed.normalized_parts.len(),
            parsed.duplicate_part_count,
            parsed.invalid_entry_count,
            already_queued,
            matched_count
        )
    }

    pub fn save_lcsc_parts(&self, parts: Vec<String>) -> String {
        let save_path = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            PathBuf::from(&m.imported_parts_save_path)
        };

        let normalized = normalize_direct_lcsc_parts(parts);
        if normalized.normalized_parts.is_empty() {
            return "No valid LCSC parts to export".to_string();
        }

        match save_parts_file(&save_path, &normalized.normalized_parts) {
            Ok(()) => format!(
                "Exported {} LCSC part(s) to {} ({} duplicate input(s), {} invalid input(s))",
                normalized.normalized_parts.len(),
                save_path.display(),
                normalized.duplicate_part_count,
                normalized.invalid_entry_count
            ),
            Err(message) => message,
        }
    }

    pub fn queue_lcsc_parts(&self, parts: Vec<String>) -> String {
        let normalized = normalize_direct_lcsc_parts(parts);
        if normalized.normalized_parts.is_empty() {
            return "No valid LCSC parts to queue".to_string();
        }

        let (added, already_queued, matched_count) = {
            let Ok(mut m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            let timestamp = Local::now().format("%H:%M:%S").to_string();
            let merge = m.merge_matched_ids(normalized.normalized_parts.iter().cloned(), timestamp);
            m.add_debug_log(format!("Queued {} direct LCSC part(s)", merge.added));
            (merge.added, merge.already_present, m.matched.len())
        };

        format!(
            "Queued {} LCSC part(s) ({} unique part(s), {} duplicate input(s), {} invalid input(s), {} already queued, matched queue now has {} item(s))",
            added,
            normalized.normalized_parts.len(),
            normalized.duplicate_part_count,
            normalized.invalid_entry_count,
            already_queued,
            matched_count
        )
    }

    fn build_export_request(&self, ids: Vec<String>) -> Result<export::ExportRequest, String> {
        if ids.is_empty() {
            return Err("No matched results to export".to_string());
        }

        let m = self
            .state
            .lock()
            .map_err(|_| "State lock failed".to_string())?;
        if !m.export_has_export_targets() {
            return Err("Select at least one export type".to_string());
        }

        Ok(export::ExportRequest {
            ids,
            output_path: m.export_output_path.clone(),
            show_terminal: m.export_show_terminal,
            parallel: m.export_parallel,
            path_mode: m.export_path_mode,
            export_symbol: m.export_symbol,
            export_footprint: m.export_footprint,
            export_model_3d: m.export_model_3d,
            overwrite_symbol: m.export_overwrite_symbol,
            overwrite_footprint: m.export_overwrite_footprint,
            overwrite_model_3d: m.export_overwrite_model_3d,
            symbol_fill_color: m.export_symbol_fill_color.clone(),
        })
    }

    pub fn spawn_export(&self, callbacks: export::ExportCallbacks) -> String {
        let ids = match self.state.lock() {
            Ok(m) => m.get_unique_ids(),
            Err(_) => return "State lock failed".to_string(),
        };
        let request = match self.build_export_request(ids) {
            Ok(request) => request,
            Err(message) => return message,
        };

        match export::spawn_export(Arc::clone(&self.state), request, callbacks) {
            Ok(()) => "Export started".to_string(),
            Err(message) => message,
        }
    }

    pub fn spawn_export_parts(
        &self,
        parts: Vec<String>,
        callbacks: export::ExportCallbacks,
    ) -> String {
        let normalized = normalize_direct_lcsc_parts(parts);
        if normalized.normalized_parts.is_empty() {
            return "No valid LCSC parts to export".to_string();
        }

        let request = match self.build_export_request(normalized.normalized_parts) {
            Ok(request) => request,
            Err(message) => return message,
        };

        match export::spawn_export(Arc::clone(&self.state), request, callbacks) {
            Ok(()) => "Export started".to_string(),
            Err(message) => message,
        }
    }
}

pub fn snapshot_config(state: &MonitorState) -> AppConfig {
    AppConfig {
        export: ExportConfig {
            output_path: state.export_output_path.clone(),
            show_terminal: state.export_show_terminal,
            parallel: state.export_parallel,
            path_mode: state.export_path_mode,
            export_symbol: state.export_symbol,
            export_footprint: state.export_footprint,
            export_model_3d: state.export_model_3d,
            overwrite_symbol: state.export_overwrite_symbol,
            overwrite_footprint: state.export_overwrite_footprint,
            overwrite_model_3d: state.export_overwrite_model_3d,
            symbol_fill_color: state.export_symbol_fill_color.clone(),
            legacy_overwrite: None,
        },
        monitor: MonitorConfig {
            always_on_top: state.always_on_top,
            history_save_path: state.history_save_path.clone(),
            matched_save_path: state.matched_save_path.clone(),
            imported_parts_save_path: state.imported_parts_save_path.clone(),
            kicad_library_paths: state.kicad_library_paths.clone(),
            default_model_format: state.default_model_format,
            window_width: state.window_width,
            window_height: state.window_height,
            window_x: state.window_x,
            window_y: state.window_y,
        },
    }
}

fn apply_config(state: &mut MonitorState, config: &AppConfig, paths: &AppPaths) {
    state.set_export_output_path(config.export.output_path.clone());
    state.export_show_terminal = config.export.show_terminal;
    state.set_export_parallel(config.export.parallel);
    state.set_export_path_mode(config.export.path_mode);
    state.set_export_symbol(config.export.export_symbol);
    state.set_export_footprint(config.export.export_footprint);
    state.set_export_model_3d(config.export.export_model_3d);
    state.set_export_overwrite_symbol(config.export.overwrite_symbol);
    state.set_export_overwrite_footprint(config.export.overwrite_footprint);
    state.set_export_overwrite_model_3d(config.export.overwrite_model_3d);
    state.set_export_symbol_fill_color(config.export.symbol_fill_color.clone());
    state.always_on_top = config.monitor.always_on_top;
    state.set_history_save_path(config.monitor.history_save_path.clone(), paths);
    state.set_matched_save_path(config.monitor.matched_save_path.clone(), paths);
    state.set_imported_parts_save_path(config.monitor.imported_parts_save_path.clone(), paths);
    state.kicad_library_paths = config.monitor.kicad_library_paths.clone();
    state.default_model_format = config.monitor.default_model_format;
    state.window_width = config.monitor.window_width;
    state.window_height = config.monitor.window_height;
    state.window_x = config.monitor.window_x;
    state.window_y = config.monitor.window_y;
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn save_parts_file(path: &Path, parts: &[String]) -> Result<(), String> {
    ensure_parent_dir(path).map_err(|err| format!("Export failed: {}", err))?;
    let file = std::fs::File::create(path).map_err(|err| format!("Export failed: {}", err))?;
    let mut writer = std::io::BufWriter::new(file);
    for part in parts {
        writeln!(writer, "{}", part).map_err(|err| format!("Export failed: {}", err))?;
    }
    writer
        .flush()
        .map_err(|err| format!("Export failed: {}", err))?;
    Ok(())
}

fn parse_imported_parts_text(content: &str) -> LcscPartCollectionSummary {
    let regex = Regex::new(r"(?i)c\d+").expect("import regex should compile");
    let mut seen = HashSet::new();
    let mut normalized_parts = Vec::new();
    let mut matched_part_count = 0usize;
    let mut invalid_entry_count = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut matched = false;
        for capture in regex.find_iter(trimmed) {
            let start = capture.start();
            let end = capture.end();
            let before = trimmed[..start].chars().next_back();
            let after = trimmed[end..].chars().next();

            if before.is_some_and(is_lcsc_boundary_blocker)
                || after.is_some_and(is_lcsc_boundary_blocker)
            {
                continue;
            }

            matched = true;
            matched_part_count += 1;
            let normalized = capture.as_str().to_ascii_uppercase();
            if seen.insert(normalized.clone()) {
                normalized_parts.push(normalized);
            }
        }

        if !matched {
            invalid_entry_count += 1;
        }
    }

    LcscPartCollectionSummary {
        duplicate_part_count: matched_part_count.saturating_sub(normalized_parts.len()),
        normalized_parts,
        matched_part_count,
        invalid_entry_count,
    }
}

fn normalize_direct_lcsc_parts(parts: Vec<String>) -> LcscPartCollectionSummary {
    let mut seen = HashSet::new();
    let mut normalized_parts = Vec::new();
    let mut matched_part_count = 0usize;
    let mut invalid_entry_count = 0usize;

    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !is_strict_lcsc_part(trimmed) {
            invalid_entry_count += 1;
            continue;
        }

        matched_part_count += 1;
        let normalized = trimmed.to_ascii_uppercase();
        if seen.insert(normalized.clone()) {
            normalized_parts.push(normalized);
        }
    }

    LcscPartCollectionSummary {
        duplicate_part_count: matched_part_count.saturating_sub(normalized_parts.len()),
        normalized_parts,
        matched_part_count,
        invalid_entry_count,
    }
}

fn is_lcsc_boundary_blocker(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

fn is_strict_lcsc_part(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('c' | 'C')) && chars.all(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{AppController, normalize_direct_lcsc_parts, parse_imported_parts_text};
    use crate::app_paths::AppPaths;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "happyjlc_controller_tests_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn native_controller_snapshot_uses_default_keyword() {
        let root = test_root("keyword");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");
        let state = controller
            .state()
            .lock()
            .expect("state lock should succeed");
        assert!(state.keyword.contains("regex"));
        drop(state);
        let _ = fs::remove_dir_all(root);
    }

    fn write_symbol_library(root: &PathBuf, name: &str, body: &str) {
        fs::create_dir_all(root).unwrap();
        fs::write(root.join(name), body).unwrap();
    }

    #[test]
    fn save_imported_parts_writes_unique_ids_one_per_line() {
        let root = test_root("save_imported_parts");
        let export_dir = root.join("export");
        write_symbol_library(
            &export_dir,
            "parts.kicad_sym",
            "(kicad_symbol_lib\n  (version 20211014)\n  (generator happyjlc-test)\n  (symbol \"Alpha\"\n    (property \"LCSC Part\" \"C123\" (id 5) (at 0 0 0))\n    (symbol \"Alpha_0_1\")\n  )\n  (symbol \"Beta\"\n    (property \"LCSC Part\" \"C123\" (id 5) (at 0 0 0))\n    (symbol \"Beta_0_1\")\n  )\n  (symbol \"Gamma\"\n    (property \"LCSC Part\" \"C456\" (id 5) (at 0 0 0))\n    (symbol \"Gamma_0_1\")\n  )\n)\n",
        );

        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");
        let save_path = root.join("out").join("imported.txt");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state.set_export_output_path(export_dir.display().to_string());
            state.set_imported_parts_save_path(save_path.display().to_string(), controller.paths());
        }

        let result = controller.save_imported_parts();
        assert_eq!(
            result,
            format!("Exported 2 LCSC part(s) to {}", save_path.display())
        );
        assert_eq!(fs::read_to_string(&save_path).unwrap(), "C123\nC456\n");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_imported_parts_reports_empty_when_scan_has_no_parts() {
        let root = test_root("save_imported_parts_empty");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");

        let result = controller.save_imported_parts();
        assert_eq!(result, "No imported LCSC Part values to export");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parse_imported_parts_text_reports_unique_duplicates_and_invalid_lines() {
        let parsed = parse_imported_parts_text("C123\njunk\nc123 C456\n\nC456\nXC789\n");

        assert_eq!(
            parsed.normalized_parts,
            vec!["C123".to_string(), "C456".to_string()]
        );
        assert_eq!(parsed.matched_part_count, 4);
        assert_eq!(parsed.duplicate_part_count, 2);
        assert_eq!(parsed.invalid_entry_count, 2);
    }

    #[test]
    fn normalize_direct_lcsc_parts_deduplicates_and_rejects_invalid_values() {
        let parsed = normalize_direct_lcsc_parts(vec![
            "C123".to_string(),
            "c123".to_string(),
            " C456 ".to_string(),
            "junk".to_string(),
            "XC789".to_string(),
        ]);

        assert_eq!(
            parsed.normalized_parts,
            vec!["C123".to_string(), "C456".to_string()]
        );
        assert_eq!(parsed.matched_part_count, 3);
        assert_eq!(parsed.duplicate_part_count, 1);
        assert_eq!(parsed.invalid_entry_count, 2);
    }

    #[test]
    fn retry_export_request_keeps_matched_queue_unchanged() {
        let root = test_root("retry_export_request");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state
                .matched
                .push(("10:00:00".to_string(), "C999".to_string()));
        }

        let request = controller
            .build_export_request(vec!["C123".to_string()])
            .expect("retry request should use the selected IDs");
        assert_eq!(request.ids, vec!["C123"]);

        let state = controller
            .state()
            .lock()
            .expect("state lock should succeed");
        assert_eq!(state.get_unique_ids(), vec!["C999"]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_imported_parts_merges_ids_into_matched_queue() {
        let root = test_root("import_imported_parts");
        let import_path = root.join("incoming").join("parts.txt");
        fs::create_dir_all(import_path.parent().unwrap()).unwrap();
        fs::write(&import_path, "C123\ninvalid\nc123\nC456\n").unwrap();

        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state.set_imported_parts_save_path(
                import_path.display().to_string(),
                controller.paths(),
            );
            state
                .matched
                .push(("10:00:00".to_string(), "C456".to_string()));
        }

        let result = controller.import_imported_parts();
        assert_eq!(
            result,
            format!(
                "Imported 1 LCSC part(s) from {} (3 matched part occurrence(s), 2 unique part(s), 1 duplicate occurrence(s) in file, 1 invalid line(s), 1 already queued, matched queue now has 2 item(s))",
                import_path.display()
            )
        );

        let state = controller
            .state()
            .lock()
            .expect("state lock should succeed");
        assert_eq!(
            state.get_unique_ids(),
            vec!["C123".to_string(), "C456".to_string()]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_imported_parts_keeps_large_batches_in_matched_queue() {
        let root = test_root("import_imported_parts_large");
        let import_path = root.join("incoming").join("many-parts.txt");
        fs::create_dir_all(import_path.parent().unwrap()).unwrap();
        let content = (1..=150)
            .map(|value| format!("C{value}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&import_path, content).unwrap();

        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state.set_imported_parts_save_path(
                import_path.display().to_string(),
                controller.paths(),
            );
        }

        let result = controller.import_imported_parts();
        assert!(result.contains("Imported 150 LCSC part(s)"));

        let state = controller
            .state()
            .lock()
            .expect("state lock should succeed");
        assert_eq!(state.matched.len(), 150);
        assert_eq!(
            state.get_unique_ids().first().map(String::as_str),
            Some("C1")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_lcsc_parts_uses_direct_selection() {
        let root = test_root("save_direct_parts");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");
        let save_path = root.join("out").join("selected.txt");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state.set_imported_parts_save_path(save_path.display().to_string(), controller.paths());
        }

        let result = controller.save_lcsc_parts(vec![
            "C123".to_string(),
            "c123".to_string(),
            "bad".to_string(),
            "C456".to_string(),
        ]);

        assert_eq!(
            result,
            format!(
                "Exported 2 LCSC part(s) to {} (1 duplicate input(s), 1 invalid input(s))",
                save_path.display()
            )
        );
        assert_eq!(fs::read_to_string(&save_path).unwrap(), "C123\nC456\n");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn queue_lcsc_parts_merges_selection_into_matched_queue() {
        let root = test_root("queue_direct_parts");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state
                .matched
                .push(("10:00:00".to_string(), "C456".to_string()));
        }

        let result = controller.queue_lcsc_parts(vec![
            "C123".to_string(),
            "c123".to_string(),
            "C456".to_string(),
            "invalid".to_string(),
        ]);

        assert_eq!(
            result,
            "Queued 1 LCSC part(s) (2 unique part(s), 1 duplicate input(s), 1 invalid input(s), 1 already queued, matched queue now has 2 item(s))"
        );

        let state = controller
            .state()
            .lock()
            .expect("state lock should succeed");
        assert_eq!(
            state.get_unique_ids(),
            vec!["C123".to_string(), "C456".to_string()]
        );

        let _ = fs::remove_dir_all(root);
    }
}
