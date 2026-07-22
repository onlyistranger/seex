use crate::checkpoint::CompletedAssets;
use crate::error::{AppError, Result};
use crate::kicad::symbol_exporter::SymbolFillColor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SymbolExportOptions {
    pub symbol_fill_color: Option<SymbolFillColor>,
    pub overwrite: bool,
}

impl SymbolExportOptions {
    pub fn new(symbol_fill_color: Option<SymbolFillColor>, overwrite: bool) -> Self {
        Self {
            symbol_fill_color,
            overwrite,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FootprintExportOptions {
    pub include_3d_model: bool,
    pub project_relative_3d: bool,
    pub overwrite: bool,
}

impl FootprintExportOptions {
    pub fn new(include_3d_model: bool, project_relative_3d: bool, overwrite: bool) -> Self {
        Self {
            include_3d_model,
            project_relative_3d,
            overwrite,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Model3dExportOptions {
    pub overwrite: bool,
}

impl Model3dExportOptions {
    pub fn new(overwrite: bool) -> Self {
        Self { overwrite }
    }
}

#[derive(Debug, Clone)]
pub struct ComponentConversionRequest {
    pub convert_symbol: bool,
    pub convert_footprint: bool,
    pub convert_model_3d: bool,
    pub symbol: SymbolExportOptions,
    pub footprint: FootprintExportOptions,
    pub model_3d: Model3dExportOptions,
}

impl ComponentConversionRequest {
    pub fn new(
        convert_symbol: bool,
        convert_footprint: bool,
        convert_model_3d: bool,
        symbol: SymbolExportOptions,
        footprint: FootprintExportOptions,
        model_3d: Model3dExportOptions,
    ) -> Result<Self> {
        if !convert_symbol && !convert_footprint && !convert_model_3d {
            return Err(AppError::Other(
                "At least one conversion asset must be selected".to_string(),
            ));
        }
        Ok(Self {
            convert_symbol,
            convert_footprint,
            convert_model_3d,
            symbol,
            footprint,
            model_3d,
        })
    }

    pub fn overwrite_any(&self) -> bool {
        (self.convert_symbol && self.symbol.overwrite)
            || (self.convert_footprint && self.footprint.overwrite)
            || (self.convert_model_3d && self.model_3d.overwrite)
    }

    pub fn checkpoint_assets(&self) -> CompletedAssets {
        CompletedAssets {
            symbol: self.convert_symbol,
            footprint: self.convert_footprint,
            model_3d: self.convert_model_3d,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub output: PathBuf,
    pub continue_on_error: bool,
    pub parallel: usize,
}

#[derive(Debug, Clone)]
pub struct RunRequest {
    pub lcsc_ids: Vec<String>,
    pub run: RunOptions,
    pub component: ComponentConversionRequest,
}

impl RunRequest {
    pub fn new(
        lcsc_ids: Vec<String>,
        run: RunOptions,
        component: ComponentConversionRequest,
    ) -> Result<Self> {
        if lcsc_ids.is_empty() {
            return Err(AppError::Other(
                "No component IDs were provided".to_string(),
            ));
        }
        if run.parallel == 0 {
            return Err(AppError::Other(
                "Parallel worker count must be greater than zero".to_string(),
            ));
        }
        Ok(Self {
            lcsc_ids,
            run,
            component,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component_request() -> ComponentConversionRequest {
        ComponentConversionRequest::new(
            true,
            true,
            true,
            SymbolExportOptions::new(None, true),
            FootprintExportOptions::new(true, true, true),
            Model3dExportOptions::new(true),
        )
        .unwrap()
    }

    #[test]
    fn run_request_keeps_explicit_options() {
        let request = RunRequest::new(
            vec!["C123456".to_string()],
            RunOptions {
                output: PathBuf::from("out"),
                continue_on_error: true,
                parallel: 8,
            },
            component_request(),
        )
        .unwrap();
        assert_eq!(request.lcsc_ids, vec!["C123456"]);
        assert!(request.component.convert_symbol);
        assert_eq!(request.run.output, PathBuf::from("out"));
    }

    #[test]
    fn component_request_requires_an_asset() {
        assert!(
            ComponentConversionRequest::new(
                false,
                false,
                false,
                SymbolExportOptions::new(None, false),
                FootprintExportOptions::new(false, false, false),
                Model3dExportOptions::new(false),
            )
            .is_err()
        );
    }
}
