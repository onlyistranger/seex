use serde::{Deserialize, Serialize};
use std::fs;

use crate::app_paths::AppPaths;

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat {
    Step,
    Stp,
    #[default]
    Wrl,
}

impl ModelFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "step" => Some(Self::Step),
            "stp" => Some(Self::Stp),
            "wrl" => Some(Self::Wrl),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportPathMode {
    #[default]
    Auto,
    ProjectRelative,
    LibraryRelative,
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ModelFormat};
    use crate::app_paths::AppPaths;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "happyjlc_config_tests_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn loads_legacy_export_config_from_executable_dir() {
        let root = test_root("legacy_json");
        let legacy_dir = root.join("legacy");
        fs::create_dir_all(&legacy_dir).unwrap();

        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            Some(legacy_dir.clone()),
        );

        fs::write(
            legacy_dir.join("export_config.json"),
            r##"{
  "export": {
    "output_path": "/tmp/export",
    "show_terminal": false,
    "parallel": 8,
    "path_mode": "project_relative",
    "overwrite": true,
    "symbol_fill_color": "#005C8FCC"
  },
            "monitor": {
    "always_on_top": true,
    "history_save_path": "/tmp/history.txt",
    "matched_save_path": "/tmp/matched.txt",
    "imported_parts_save_path": "/tmp/imported-parts.txt"
  }
}"##,
        )
        .unwrap();

        let config = AppConfig::load(&paths);
        assert_eq!(config.export.output_path, "/tmp/export");
        assert!(!config.export.show_terminal);
        assert_eq!(config.export.parallel, 8);
        assert!(config.export.export_symbol);
        assert!(config.export.export_footprint);
        assert!(config.export.export_model_3d);
        assert!(config.export.overwrite_symbol);
        assert!(config.export.overwrite_footprint);
        assert!(config.export.overwrite_model_3d);
        assert_eq!(
            config.export.symbol_fill_color.as_deref(),
            Some("#005C8FCC")
        );
        assert!(config.monitor.always_on_top);
        assert_eq!(config.monitor.history_save_path, "/tmp/history.txt");
        assert_eq!(
            config.monitor.imported_parts_save_path,
            "/tmp/imported-parts.txt"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_omits_unset_symbol_fill_color() {
        let root = test_root("save_without_fill_color");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );

        AppConfig::default().save(&paths);

        let saved = fs::read_to_string(paths.config_file()).unwrap();
        assert!(!saved.contains("symbol_fill_color"));
        assert!(!saved.contains("\"overwrite\":"));
        assert!(saved.contains("\"export_symbol\": true"));
        assert!(saved.contains("\"always_on_top\""));
        assert!(saved.contains("\"imported_parts_save_path\""));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn saves_and_loads_window_position() {
        let root = test_root("window_position");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );

        let mut config = AppConfig::default();
        config.monitor.window_x = Some(-1280);
        config.monitor.window_y = Some(48);
        config.save(&paths);

        let loaded = AppConfig::load(&paths);
        assert_eq!(loaded.monitor.window_x, Some(-1280));
        assert_eq!(loaded.monitor.window_y, Some(48));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn defaults_to_wrl_model_format_and_round_trips() {
        let root = test_root("default_model_format");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );

        let config = AppConfig::default();
        assert_eq!(config.monitor.default_model_format, ModelFormat::Wrl);
        config.save(&paths);

        let loaded = AppConfig::load(&paths);
        assert_eq!(loaded.monitor.default_model_format, ModelFormat::Wrl);

        let _ = fs::remove_dir_all(root);
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub export: ExportConfig,
    pub monitor: MonitorConfig,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MonitorConfig {
    pub always_on_top: bool,
    pub history_save_path: String,
    pub matched_save_path: String,
    pub imported_parts_save_path: String,
    #[serde(default)]
    pub kicad_library_paths: Vec<String>,
    pub default_model_format: ModelFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_y: Option<i32>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportConfig {
    pub output_path: String,
    pub show_terminal: bool,
    pub parallel: usize,
    pub path_mode: ExportPathMode,
    #[serde(default = "default_true")]
    pub export_symbol: bool,
    #[serde(default = "default_true")]
    pub export_footprint: bool,
    #[serde(default = "default_true")]
    pub export_model_3d: bool,
    pub overwrite_symbol: bool,
    pub overwrite_footprint: bool,
    pub overwrite_model_3d: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_fill_color: Option<String>,
    #[serde(default, rename = "overwrite", skip_serializing)]
    pub(crate) legacy_overwrite: Option<bool>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            output_path: String::new(),
            show_terminal: true,
            parallel: 4,
            path_mode: ExportPathMode::Auto,
            export_symbol: true,
            export_footprint: true,
            export_model_3d: true,
            overwrite_symbol: false,
            overwrite_footprint: false,
            overwrite_model_3d: false,
            symbol_fill_color: None,
            legacy_overwrite: None,
        }
    }
}

impl AppConfig {
    pub fn load(paths: &AppPaths) -> Self {
        let path = paths.config_file();
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content)
                .map(Self::normalize)
                .unwrap_or_else(|_| Self::with_legacy(paths)),
            Err(_) => Self::with_legacy(paths),
        }
    }

    pub fn save(&self, paths: &AppPaths) {
        let path = paths.config_file();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }

    fn with_legacy(paths: &AppPaths) -> Self {
        if let Some(config) = Self::load_legacy_export_config(paths) {
            return config;
        }

        Self {
            export: ExportConfig::load_legacy(paths),
            ..Self::default()
        }
    }

    fn load_legacy_export_config(paths: &AppPaths) -> Option<Self> {
        let path = paths.legacy_config_file()?;
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok().map(Self::normalize)
    }

    fn normalize(mut self) -> Self {
        self.export.normalize_legacy();
        self
    }
}

impl ExportConfig {
    fn normalize_legacy(&mut self) {
        if self.legacy_overwrite == Some(true) {
            self.overwrite_symbol = true;
            self.overwrite_footprint = true;
            self.overwrite_model_3d = true;
        }
        self.legacy_overwrite = None;
    }

    fn load_legacy(paths: &AppPaths) -> Self {
        let content = paths
            .legacy_config_text_file()
            .and_then(|path| fs::read_to_string(path).ok());

        let Some(content) = content else {
            return Self::default();
        };

        let lines: Vec<&str> = content.lines().collect();
        let defaults = Self::default();
        let output_path = if !lines.is_empty() && !lines[0].is_empty() {
            lines[0].to_string()
        } else {
            defaults.output_path
        };
        let show_terminal = if lines.len() >= 2 {
            lines[1] == "true"
        } else {
            defaults.show_terminal
        };
        let parallel = if lines.len() >= 3 {
            lines[2]
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|value| *value >= 1)
                .unwrap_or(defaults.parallel)
        } else {
            defaults.parallel
        };
        Self {
            output_path,
            show_terminal,
            parallel,
            path_mode: defaults.path_mode,
            export_symbol: defaults.export_symbol,
            export_footprint: defaults.export_footprint,
            export_model_3d: defaults.export_model_3d,
            overwrite_symbol: defaults.overwrite_symbol,
            overwrite_footprint: defaults.overwrite_footprint,
            overwrite_model_3d: defaults.overwrite_model_3d,
            symbol_fill_color: defaults.symbol_fill_color,
            legacy_overwrite: None,
        }
    }
}
