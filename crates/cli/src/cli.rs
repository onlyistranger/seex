use clap::Parser;
use happyjlc_core::error::{AppError, EasyedaError, Result};
use happyjlc_core::kicad::SymbolFillColor;
use happyjlc_core::{
    ComponentConversionRequest, FootprintExportOptions, Model3dExportOptions, RunOptions,
    RunRequest, SymbolExportOptions,
};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "happyjlc")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Fast EasyEDA/LCSC to KiCad converter with parallel downloads", long_about = None)]
pub struct Cli {
    /// LCSC component ID (e.g., C2040)
    #[arg(long, value_name = "ID", conflicts_with = "batch")]
    pub lcsc_id: Option<String>,

    /// Batch mode: read LCSC IDs from a file (one ID per line)
    #[arg(long, value_name = "FILE", conflicts_with = "lcsc_id")]
    pub batch: Option<PathBuf>,

    /// Convert symbol only
    #[arg(long)]
    pub symbol: bool,

    /// Convert footprint only
    #[arg(long)]
    pub footprint: bool,

    /// Convert 3D model only
    #[arg(long = "3d")]
    pub model_3d: bool,

    /// Convert all (symbol + footprint + 3D model)
    #[arg(long)]
    pub full: bool,

    /// Output directory path
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,

    /// Overwrite existing components
    #[arg(long)]
    pub overwrite: bool,

    /// Overwrite symbol output only
    #[arg(long)]
    pub overwrite_symbol: bool,

    /// Overwrite footprint output only
    #[arg(long)]
    pub overwrite_footprint: bool,

    /// Overwrite 3D model output only
    #[arg(long = "overwrite-3d")]
    pub overwrite_model_3d: bool,

    /// Use project-relative paths (KIPRJMOD) instead of KiCad environment or absolute paths for 3D models
    #[arg(long)]
    pub project_relative: bool,

    /// Override filled symbol rectangle color with #RRGGBB or #RRGGBBAA
    #[arg(long, value_name = "HEX")]
    pub symbol_fill_color: Option<String>,

    /// Enable debug logging
    #[arg(long)]
    pub debug: bool,

    /// Continue on error in batch mode (skip failed components)
    #[arg(long)]
    pub continue_on_error: bool,

    /// Number of parallel downloads in batch mode (default: 4)
    #[arg(long, default_value = "4")]
    pub parallel: usize,
}

impl Cli {
    pub fn into_request(self) -> Result<RunRequest> {
        validate_cli(&self)?;
        let lcsc_ids = resolve_lcsc_ids(&self)?;
        let convert_symbol = self.symbol || self.full;
        let convert_footprint = self.footprint || self.full;
        let convert_model_3d = self.model_3d || self.full;
        let symbol_fill_color = self
            .symbol_fill_color
            .as_deref()
            .map(SymbolFillColor::parse)
            .transpose()?;
        let component = ComponentConversionRequest::new(
            convert_symbol,
            convert_footprint,
            convert_model_3d,
            SymbolExportOptions::new(symbol_fill_color, self.overwrite || self.overwrite_symbol),
            FootprintExportOptions::new(
                convert_model_3d,
                self.project_relative,
                self.overwrite || self.overwrite_footprint,
            ),
            Model3dExportOptions::new(self.overwrite || self.overwrite_model_3d),
        )?;
        RunRequest::new(
            lcsc_ids,
            RunOptions {
                output: self.output,
                continue_on_error: self.continue_on_error,
                parallel: self.parallel,
            },
            component,
        )
    }
}

fn validate_cli(cli: &Cli) -> Result<()> {
    if cli.lcsc_id.is_none() && cli.batch.is_none() {
        return Err(AppError::Other(
            "Either --lcsc-id or --batch must be specified".to_string(),
        ));
    }
    if let Some(id) = &cli.lcsc_id
        && (!id.starts_with('C') || id.len() < 2)
    {
        return Err(AppError::Easyeda(EasyedaError::InvalidLcscId(id.clone())));
    }
    if !cli.symbol && !cli.footprint && !cli.model_3d && !cli.full {
        return Err(AppError::Other(
            "At least one conversion option must be specified (--symbol, --footprint, --3d, or --full)"
                .to_string(),
        ));
    }
    if let Some(value) = cli.symbol_fill_color.as_deref() {
        SymbolFillColor::parse(value)?;
    }
    Ok(())
}

fn resolve_lcsc_ids(cli: &Cli) -> Result<Vec<String>> {
    if let Some(id) = &cli.lcsc_id {
        return Ok(vec![id.clone()]);
    }
    if let Some(batch_file) = &cli.batch {
        let content = std::fs::read_to_string(batch_file)
            .map_err(|error| AppError::io_context("read batch file", batch_file, error))?;
        let re = regex::Regex::new(r"C\d+").unwrap();
        let ids = dedupe_lcsc_ids(re.find_iter(&content).map(|m| m.as_str().to_string()));
        if ids.is_empty() {
            return Err(AppError::Other(
                "No valid LCSC IDs found in batch file".to_string(),
            ));
        }
        log::info!("Loaded {} LCSC IDs from batch file", ids.len());
        return Ok(ids);
    }
    Err(AppError::Other("No LCSC ID source specified".to_string()))
}

fn dedupe_lcsc_ids(ids: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect()
}
