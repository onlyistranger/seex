use serde::Serialize;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Network,
    NotFound,
    InvalidId,
    InvalidData,
    Symbol,
    Footprint,
    #[serde(rename = "model_3d")]
    Model3d,
    Conversion,
    Io,
    TaskPanic,
    Unknown,
}

#[derive(Error, Debug)]
pub enum EasyedaError {
    #[error("API request failed: {0}")]
    ApiRequest(#[from] reqwest::Error),

    #[error("Invalid LCSC ID format: {0}")]
    InvalidLcscId(String),

    #[error("Component not found: {0}")]
    ComponentNotFound(String),

    #[error("Failed to parse JSON response: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Invalid component data: {0}")]
    InvalidData(String),
}

#[derive(Error, Debug)]
pub enum KicadError {
    #[error("Failed to export symbol: {0}")]
    SymbolExport(String),

    #[error("Failed to export footprint: {0}")]
    FootprintExport(String),

    #[error("Failed to export 3D model: {0}")]
    ModelExport(String),

    #[error("Invalid KiCad version")]
    InvalidVersion,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Invalid coordinate: {0}")]
    InvalidCoordinate(String),

    #[error("Invalid unit conversion: {0}")]
    InvalidUnit(String),

    #[error("SVG path parse error: {0}")]
    SvgParse(String),

    #[error("Arc conversion failed: {0}")]
    ArcConversion(String),
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Easyeda(#[from] EasyedaError),

    #[error(transparent)]
    Kicad(#[from] KicadError),

    #[error(transparent)]
    Conversion(#[from] ConversionError),

    #[error("I/O error while {action} {path}: {source}")]
    IoContext {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn io_context(
        action: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::IoContext {
            action,
            path: path.into(),
            source,
        }
    }

    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::Easyeda(error) => match error {
                EasyedaError::ApiRequest(_) => ErrorCategory::Network,
                EasyedaError::InvalidLcscId(_) => ErrorCategory::InvalidId,
                EasyedaError::ComponentNotFound(_) => ErrorCategory::NotFound,
                EasyedaError::JsonParse(_) | EasyedaError::InvalidData(_) => {
                    ErrorCategory::InvalidData
                }
            },
            Self::Kicad(error) => match error {
                KicadError::SymbolExport(_) => ErrorCategory::Symbol,
                KicadError::FootprintExport(_) => ErrorCategory::Footprint,
                KicadError::ModelExport(_) => ErrorCategory::Model3d,
                KicadError::InvalidVersion => ErrorCategory::Conversion,
                KicadError::Io(_) => ErrorCategory::Io,
            },
            Self::Conversion(_) => ErrorCategory::Conversion,
            Self::IoContext { .. } => ErrorCategory::Io,
            Self::Regex(_) => ErrorCategory::InvalidData,
            Self::Other(_) => ErrorCategory::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, ConversionError, EasyedaError, ErrorCategory, KicadError};

    #[test]
    fn categorizes_stable_export_error_classes() {
        assert_eq!(
            AppError::from(EasyedaError::InvalidLcscId("bad".to_string())).category(),
            ErrorCategory::InvalidId
        );
        assert_eq!(
            AppError::from(EasyedaError::ComponentNotFound("C1".to_string())).category(),
            ErrorCategory::NotFound
        );
        assert_eq!(
            AppError::from(KicadError::SymbolExport("bad symbol".to_string())).category(),
            ErrorCategory::Symbol
        );
        assert_eq!(
            AppError::from(KicadError::FootprintExport("bad footprint".to_string())).category(),
            ErrorCategory::Footprint
        );
        assert_eq!(
            AppError::from(KicadError::ModelExport("bad model".to_string())).category(),
            ErrorCategory::Model3d
        );
        assert_eq!(
            AppError::from(ConversionError::InvalidCoordinate("x".to_string())).category(),
            ErrorCategory::Conversion
        );
        assert_eq!(
            AppError::Other("unexpected".to_string()).category(),
            ErrorCategory::Unknown
        );
    }
}
