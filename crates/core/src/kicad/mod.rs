pub mod footprint;
pub mod footprint_exporter;
pub mod layers;
pub mod model_exporter;
pub mod symbol;
pub mod symbol_exporter;

pub use footprint::{
    Drill, Ki3dModel, KiArc as FootprintKiArc, KiCircle as FootprintKiCircle, KiFootprint, KiLine,
    KiPad, KiText, KiTrack, PadShape, PadType,
};
pub use footprint_exporter::FootprintExporter;
pub use layers::*;
pub use model_exporter::ModelExporter;
pub use symbol::KiArc as SymbolKiArc;
pub use symbol::KiText as SymbolKiText;
pub use symbol::{KiCircle, KiPin, KiPolyline, KiRectangle, KiSymbol, PinStyle, PinType};
pub use symbol_exporter::{SymbolExporter, SymbolFillColor};

pub(crate) fn escape_kicad_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character => escaped.push(character),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::escape_kicad_string;

    #[test]
    fn escapes_kicad_string_controls() {
        assert_eq!(
            escape_kicad_string("A\\B\"C\nD\rE\tF"),
            "A\\\\B\\\"C\\nD\\rE\\tF"
        );
    }
}
