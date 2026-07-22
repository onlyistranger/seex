use crate::error::{ConversionError, Result};
use regex::Regex;

#[derive(Debug, Clone)]
pub enum SvgCommand {
    MoveTo {
        x: f64,
        y: f64,
    },
    LineTo {
        x: f64,
        y: f64,
    },
    Arc {
        rx: f64,
        ry: f64,
        angle: f64,
        large_arc: bool,
        sweep: bool,
        x: f64,
        y: f64,
    },
    ClosePath,
}

pub fn parse_svg_path(path: &str) -> Result<Vec<SvgCommand>> {
    let mut commands = Vec::new();
    let path = path.trim();

    // Regex patterns for SVG commands
    let move_re = Regex::new(r"M\s*([-\d.]+)[,\s]+([-\d.]+)").unwrap();
    let line_re = Regex::new(r"L\s*([-\d.]+)[,\s]+([-\d.]+)").unwrap();
    let arc_re = Regex::new(
        r"A\s*([-\d.]+)[,\s]+([-\d.]+)[,\s]+([-\d.]+)[,\s]+([01])[,\s]+([01])[,\s]+([-\d.]+)[,\s]+([-\d.]+)"
    ).unwrap();

    let mut pos = 0;
    let chars: Vec<char> = path.chars().collect();

    while pos < chars.len() {
        let remaining = &path[pos..];

        if remaining.starts_with('M') {
            if let Some(cap) = move_re.captures(remaining) {
                let x = cap[1].parse::<f64>().map_err(|_| {
                    ConversionError::SvgParse("Invalid MoveTo X coordinate".to_string())
                })?;
                let y = cap[2].parse::<f64>().map_err(|_| {
                    ConversionError::SvgParse("Invalid MoveTo Y coordinate".to_string())
                })?;
                commands.push(SvgCommand::MoveTo { x, y });
                pos += cap.get(0).unwrap().len();
            } else {
                pos += 1;
            }
        } else if remaining.starts_with('L') {
            if let Some(cap) = line_re.captures(remaining) {
                let x = cap[1].parse::<f64>().map_err(|_| {
                    ConversionError::SvgParse("Invalid LineTo X coordinate".to_string())
                })?;
                let y = cap[2].parse::<f64>().map_err(|_| {
                    ConversionError::SvgParse("Invalid LineTo Y coordinate".to_string())
                })?;
                commands.push(SvgCommand::LineTo { x, y });
                pos += cap.get(0).unwrap().len();
            } else {
                pos += 1;
            }
        } else if remaining.starts_with('A') {
            if let Some(cap) = arc_re.captures(remaining) {
                let rx = cap[1]
                    .parse::<f64>()
                    .map_err(|_| ConversionError::SvgParse("Invalid Arc RX".to_string()))?;
                let ry = cap[2]
                    .parse::<f64>()
                    .map_err(|_| ConversionError::SvgParse("Invalid Arc RY".to_string()))?;
                let angle = cap[3]
                    .parse::<f64>()
                    .map_err(|_| ConversionError::SvgParse("Invalid Arc angle".to_string()))?;
                let large_arc = &cap[4] == "1";
                let sweep = &cap[5] == "1";
                let x = cap[6].parse::<f64>().map_err(|_| {
                    ConversionError::SvgParse("Invalid Arc X coordinate".to_string())
                })?;
                let y = cap[7].parse::<f64>().map_err(|_| {
                    ConversionError::SvgParse("Invalid Arc Y coordinate".to_string())
                })?;
                commands.push(SvgCommand::Arc {
                    rx,
                    ry,
                    angle,
                    large_arc,
                    sweep,
                    x,
                    y,
                });
                pos += cap.get(0).unwrap().len();
            } else {
                pos += 1;
            }
        } else if remaining.starts_with('Z') || remaining.starts_with('z') {
            commands.push(SvgCommand::ClosePath);
            pos += 1;
        } else {
            pos += 1;
        }
    }

    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_path() {
        let path = "M 10,20 L 30,40 Z";
        let commands = parse_svg_path(path).unwrap();
        assert_eq!(commands.len(), 3);
    }

    #[test]
    fn test_parse_arc() {
        let path = "M 0,0 A 10,10 0 0 1 20,20";
        let commands = parse_svg_path(path).unwrap();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_parse_footprint_arc_comma_no_space() {
        let path = "M3956.6958,3000.9151 A29.0043,29.0043 0 1 0 3898.9082,2999.9286";
        let commands = parse_svg_path(path).unwrap();
        assert_eq!(commands.len(), 2);
    }

    // --- below: value-correctness + edge cases ported from easyeda2kicad test_symbol_converter ---

    fn assert_move_to(cmd: &SvgCommand, expected_x: f64, expected_y: f64) {
        match cmd {
            SvgCommand::MoveTo { x, y } => {
                assert!(
                    (x - expected_x).abs() < 1e-9,
                    "MoveTo x: got {x}, want {expected_x}"
                );
                assert!(
                    (y - expected_y).abs() < 1e-9,
                    "MoveTo y: got {y}, want {expected_y}"
                );
            }
            other => panic!("expected MoveTo, got {other:?}"),
        }
    }

    fn assert_line_to(cmd: &SvgCommand, expected_x: f64, expected_y: f64) {
        match cmd {
            SvgCommand::LineTo { x, y } => {
                assert!(
                    (x - expected_x).abs() < 1e-9,
                    "LineTo x: got {x}, want {expected_x}"
                );
                assert!(
                    (y - expected_y).abs() < 1e-9,
                    "LineTo y: got {y}, want {expected_y}"
                );
            }
            other => panic!("expected LineTo, got {other:?}"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn assert_arc(
        cmd: &SvgCommand,
        rx: f64,
        ry: f64,
        angle: f64,
        large_arc: bool,
        sweep: bool,
        x: f64,
        y: f64,
    ) {
        match cmd {
            SvgCommand::Arc {
                rx: r,
                ry: rr,
                angle: a,
                large_arc: l,
                sweep: s,
                x: xx,
                y: yy,
            } => {
                assert!((r - rx).abs() < 1e-9, "Arc rx: got {r}, want {rx}");
                assert!((rr - ry).abs() < 1e-9, "Arc ry: got {rr}, want {ry}");
                assert!((a - angle).abs() < 1e-9, "Arc angle: got {a}, want {angle}");
                assert_eq!(*l, large_arc, "Arc large_arc: got {l}, want {large_arc}");
                assert_eq!(*s, sweep, "Arc sweep: got {s}, want {sweep}");
                assert!((xx - x).abs() < 1e-9, "Arc x: got {xx}, want {x}");
                assert!((yy - y).abs() < 1e-9, "Arc y: got {yy}, want {y}");
            }
            other => panic!("expected Arc, got {other:?}"),
        }
    }

    #[test]
    fn test_move_to_line_to_close_values() {
        // Ported from easyeda2kicad test_convert_ee_paths_mlz (M/L/Z closed polygon).
        // Our parser does not yet emit closedness metadata; we verify command order + coords.
        let path = "M 400 300 L 410 300 L 405 310 Z";
        let cmds = parse_svg_path(path).unwrap();
        assert_eq!(cmds.len(), 4);
        assert_move_to(&cmds[0], 400.0, 300.0);
        assert_line_to(&cmds[1], 410.0, 300.0);
        assert_line_to(&cmds[2], 405.0, 310.0);
        assert!(matches!(cmds[3], SvgCommand::ClosePath));
    }

    #[test]
    fn test_only_move_to_no_arc() {
        // Ported from easyeda2kicad test_arc_invalid_structure (M-only, no Arc token).
        // There, the converter returns no arcs; here the parser must still emit the MoveTo.
        let cmds = parse_svg_path("M 400 300").unwrap();
        assert_eq!(cmds.len(), 1);
        assert_move_to(&cmds[0], 400.0, 300.0);
    }

    #[test]
    fn test_zero_radius_arc_still_parsed() {
        // Ported from easyeda2kicad test_arc_zero_radius (A rx=0 triggers degenerate arc
        // downstream). The SVG parser is a pure tokenizer: it must still emit the Arc so
        // the converter can decide. Downstream degenerate-detection is covered by converter tests.
        let cmds = parse_svg_path("M 400 300 A 0 0 0 0 1 410 300").unwrap();
        assert_eq!(cmds.len(), 2);
        assert_move_to(&cmds[0], 400.0, 300.0);
        assert_arc(&cmds[1], 0.0, 0.0, 0.0, false, true, 410.0, 300.0);
    }

    #[test]
    fn test_arc_large_arc_and_sweep_flags_all_combinations() {
        // The (large_arc, sweep) flag pair carries real geometric meaning in the SVG arc
        // endpoint-parameterization; cover all 4 to catch any sign/bool inversion.
        for (large, sweep, path) in [
            (false, false, "M 0 0 A 10 10 0 0 0 20 20"),
            (false, true, "M 0 0 A 10 10 0 0 1 20 20"),
            (true, false, "M 0 0 A 10 10 0 1 0 20 20"),
            (true, true, "M 0 0 A 10 10 0 1 1 20 20"),
        ] {
            let cmds = parse_svg_path(path).unwrap();
            assert_eq!(cmds.len(), 2, "for path {path}");
            assert_arc(&cmds[1], 10.0, 10.0, 0.0, large, sweep, 20.0, 20.0);
        }
    }

    #[test]
    fn test_arc_angle_value() {
        // EasyEDA paths carry a rotation angle in degrees before the two flags.
        let cmds = parse_svg_path("M 0 0 A 5 7 45 0 1 12 15").unwrap();
        assert_arc(&cmds[1], 5.0, 7.0, 45.0, false, true, 12.0, 15.0);
    }

    #[test]
    fn test_negative_coordinates() {
        // EasyEDA axis can go negative for symbols anchored off-origin.
        let cmds = parse_svg_path("M -10 -20 L -30 -40 Z").unwrap();
        assert_eq!(cmds.len(), 3);
        assert_move_to(&cmds[0], -10.0, -20.0);
        assert_line_to(&cmds[1], -30.0, -40.0);
        assert!(matches!(cmds[2], SvgCommand::ClosePath));
    }

    #[test]
    fn test_decimal_coordinates() {
        // Real-world footprint data uses fractional coords (see test_parse_footprint_arc_comma_no_space).
        let cmds = parse_svg_path("M 1.5 2.7 L 7.5 5.67 Z").unwrap();
        assert_move_to(&cmds[0], 1.5, 2.7);
        assert_line_to(&cmds[1], 7.5, 5.67);
    }

    #[test]
    fn test_empty_string_returns_empty() {
        assert!(parse_svg_path("").unwrap().is_empty());
    }

    #[test]
    fn test_leading_close_path_emits_close() {
        // A bare Z without a prior M is malformed but the tokenizer must not panic:
        // it should emit a ClosePath and leave the caller to validate.
        let cmds = parse_svg_path("Z").unwrap();
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], SvgCommand::ClosePath));
    }

    #[test]
    fn test_unknown_token_is_skipped() {
        // Ported from easyeda2kicad test_convert_ee_paths_unknown_token.
        // 'X' is not a supported SVG command and must be ignored, not error.
        let cmds = parse_svg_path("M 10 20 X 999 L 30 40").unwrap();
        assert_eq!(cmds.len(), 2);
        assert_move_to(&cmds[0], 10.0, 20.0);
        assert_line_to(&cmds[1], 30.0, 40.0);
    }

    #[test]
    fn test_lowercase_z_close_path() {
        // SVG allows both Z and z for closepath; some EasyEDA exports use lowercase.
        let cmds = parse_svg_path("M 0 0 L 10 10 z").unwrap();
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[2], SvgCommand::ClosePath));
    }

    #[test]
    fn test_multiple_move_to_segments() {
        // EasyEDA multi-subpath geometry (e.g. rotated connectors) emits repeated M tokens;
        // each must produce its own MoveTo, not collapse or error.
        let cmds = parse_svg_path("M 0 0 M 10 10 L 20 20").unwrap();
        assert_eq!(cmds.len(), 3);
        assert_move_to(&cmds[0], 0.0, 0.0);
        assert_move_to(&cmds[1], 10.0, 10.0);
        assert_line_to(&cmds[2], 20.0, 20.0);
    }

    #[test]
    fn test_comma_separated_without_space() {
        // Common in EasyEDA footprint exports: "M3956,3000A29,29 0 1 0 3898,2999".
        // The regex must accept comma+opt-space separators, not require a literal space.
        let cmds = parse_svg_path("M1,2L3,4Z").unwrap();
        assert_eq!(cmds.len(), 3);
        assert_move_to(&cmds[0], 1.0, 2.0);
        assert_line_to(&cmds[1], 3.0, 4.0);
    }
}
