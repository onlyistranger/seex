use crate::cli::KicadVersion;
use crate::easyeda::ComponentData;
use crate::error::{ConversionError, Result};

pub struct Converter {
    _kicad_version: KicadVersion,
}

impl Converter {
    pub fn new(kicad_version: KicadVersion) -> Self {
        Self {
            _kicad_version: kicad_version,
        }
    }

    /// Convert pixels to mils (1 px = 10 mils in EasyEDA)
    pub fn px_to_mil(&self, px: f64) -> i32 {
        (10.0 * px) as i32
    }

    /// Convert pixels to millimeters (1 px = 10 mils = 0.254 mm)
    pub fn px_to_mm(&self, px: f64) -> f64 {
        10.0 * px * 0.0254
    }

    /// Flip Y coordinate (EasyEDA uses top-left origin, KiCad uses bottom-left)
    pub fn flip_y(&self, y: f64) -> f64 {
        -y
    }

    /// Normalize coordinate to bounding box origin
    pub fn normalize_to_bbox(&self, coord: f64, bbox_origin: f64) -> f64 {
        coord - bbox_origin
    }

    /// Convert degrees to radians
    pub fn deg_to_rad(&self, degrees: f64) -> f64 {
        degrees * std::f64::consts::PI / 180.0
    }

    /// Convert radians to degrees
    pub fn rad_to_deg(&self, radians: f64) -> f64 {
        radians * 180.0 / std::f64::consts::PI
    }

    /// Compute arc center from SVG elliptical arc endpoint parameters
    /// Based on W3C SVG specification for arc conversion
    /// Returns (center_x, center_y, start_angle_deg, end_angle_deg)
    pub fn compute_arc_center(
        &self,
        start: (f64, f64),
        end: (f64, f64),
        radii: (f64, f64),
        x_axis_rotation: f64,
        large_arc: bool,
        sweep: bool,
    ) -> Result<(f64, f64, f64, f64)> {
        let (x1, y1) = start;
        let (x2, y2) = end;
        let (mut rx, mut ry) = radii;

        // Handle degenerate cases
        if (x1 - x2).abs() < 1e-10 && (y1 - y2).abs() < 1e-10 {
            return Err(ConversionError::ArcConversion(
                "Start and end points are identical".to_string(),
            )
            .into());
        }

        if rx.abs() < 1e-10 || ry.abs() < 1e-10 {
            return Err(ConversionError::ArcConversion("Radii are too small".to_string()).into());
        }

        // Ensure radii are positive
        rx = rx.abs();
        ry = ry.abs();

        // Convert rotation angle to radians
        let phi = self.deg_to_rad(x_axis_rotation);
        let cos_phi = phi.cos();
        let sin_phi = phi.sin();

        // Step 1: Compute (x1', y1') - transformed start point
        let dx = (x1 - x2) / 2.0;
        let dy = (y1 - y2) / 2.0;
        let x1_prime = cos_phi * dx + sin_phi * dy;
        let y1_prime = -sin_phi * dx + cos_phi * dy;

        // Step 2: Correct radii if needed
        let lambda = (x1_prime / rx).powi(2) + (y1_prime / ry).powi(2);
        if lambda > 1.0 {
            rx *= lambda.sqrt();
            ry *= lambda.sqrt();
        }

        // Step 3: Compute center point (cx', cy') in transformed space
        let sq = ((rx * ry).powi(2) - (rx * y1_prime).powi(2) - (ry * x1_prime).powi(2))
            / ((rx * y1_prime).powi(2) + (ry * x1_prime).powi(2));

        let sq = if sq < 0.0 { 0.0 } else { sq.sqrt() };

        let sign = if large_arc == sweep { -1.0 } else { 1.0 };
        let cx_prime = sign * sq * rx * y1_prime / ry;
        let cy_prime = -sign * sq * ry * x1_prime / rx;

        // Step 4: Compute center point (cx, cy) in original space
        let cx = cos_phi * cx_prime - sin_phi * cy_prime + (x1 + x2) / 2.0;
        let cy = sin_phi * cx_prime + cos_phi * cy_prime + (y1 + y2) / 2.0;

        // Step 5: Compute start and end angles
        let ux = (x1_prime - cx_prime) / rx;
        let uy = (y1_prime - cy_prime) / ry;
        let vx = (-x1_prime - cx_prime) / rx;
        let vy = (-y1_prime - cy_prime) / ry;

        // Compute start angle
        let n = (ux.powi(2) + uy.powi(2)).sqrt();
        let p = ux;
        let sign = if uy < 0.0 { -1.0 } else { 1.0 };
        let mut theta1 = sign * (p / n).acos();

        // Compute angle extent
        let n = ((ux.powi(2) + uy.powi(2)) * (vx.powi(2) + vy.powi(2))).sqrt();
        let p = ux * vx + uy * vy;
        let sign = if ux * vy - uy * vx < 0.0 { -1.0 } else { 1.0 };
        let mut dtheta = sign * (p / n).acos();

        if !sweep && dtheta > 0.0 {
            dtheta -= 2.0 * std::f64::consts::PI;
        } else if sweep && dtheta < 0.0 {
            dtheta += 2.0 * std::f64::consts::PI;
        }

        let theta2 = theta1 + dtheta;

        // Convert to degrees
        theta1 = self.rad_to_deg(theta1);
        let mut theta2 = self.rad_to_deg(theta2);

        // Normalize angles
        while theta1 < 0.0 {
            theta1 += 360.0;
        }
        while theta2 < 0.0 {
            theta2 += 360.0;
        }
        while theta1 >= 360.0 {
            theta1 -= 360.0;
        }
        while theta2 >= 360.0 {
            theta2 -= 360.0;
        }

        Ok((cx, cy, theta1, theta2))
    }

    /// Calculate bounding box for a set of points
    pub fn calculate_bbox(&self, points: &[(f64, f64)]) -> Option<(f64, f64, f64, f64)> {
        if points.is_empty() {
            return None;
        }

        let mut min_x = points[0].0;
        let mut max_x = points[0].0;
        let mut min_y = points[0].1;
        let mut max_y = points[0].1;

        for &(x, y) in points.iter().skip(1) {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }

        Some((min_x, min_y, max_x, max_y))
    }
}

/// Sanitize a component name for use as a filename/identifier.
/// If `replace_dot` is true, replaces '.' with '_'.
/// If `replace_dot` is false, preserves '.' and restores underscore between digits to decimal point (e.g. 1_6 -> 1.6).
pub fn sanitize_name(name: &str, replace_dot: bool) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || (!replace_dot && c == '.') {
                c
            } else {
                '_'
            }
        })
        .collect();

    if replace_dot {
        sanitized.replace('.', "_")
    } else {
        let re_dot = regex::Regex::new(r"(\d)_(\d)").unwrap();
        let mut result = sanitized;
        while re_dot.is_match(&result) {
            result = re_dot.replace_all(&result, "${1}.${2}").to_string();
        }
        result
    }
}

/// Convert EasyEDA rotation to KiCad rotation
/// Negates angles > 180 degrees to match KiCad convention
pub fn angle_to_ki(rotation: f64) -> f64 {
    if rotation > 180.0 {
        -(360.0 - rotation)
    } else {
        rotation
    }
}

/// Strip part number suffix (like _C12345) and sanitize footprint name
pub fn restore_footprint_name(title: &str, replace_dot: bool) -> String {
    let sanitized = sanitize_name(title, replace_dot);
    let re_c = regex::Regex::new(r"(_C\d+)+$").unwrap();
    re_c.replace_all(&sanitized, "").to_string()
}

/// Strip part number suffix and restore/sanitize STEP model name
pub fn restore_step_model_name(title: &str, replace_dot: bool) -> String {
    let sanitized = sanitize_name(title, replace_dot);
    let re_c = regex::Regex::new(r"(_C\d+)+$").unwrap();
    re_c.replace_all(&sanitized, "").to_string()
}

/// Helper to generate suffixed footprint/component name
pub fn make_suffixed_name(title: &str, lcsc_id: &str, replace_dot: bool) -> String {
    let clean = restore_footprint_name(title, replace_dot);
    format!("{}_{}", clean, lcsc_id)
}

/// Helper to generate suffixed 3D model name
pub fn make_suffixed_model_name(title: &str, lcsc_id: &str, replace_dot: bool) -> String {
    let clean = restore_step_model_name(title, replace_dot);
    format!("{}_{}", clean, lcsc_id)
}

/// Extract base package identifier from complex EasyEDA/raw package strings.
/// E.g. "SOT-223-3_L6.5-W3.4-P2.30-LS7.0-BR" -> "SOT-223"
/// "SOT-23-5_L2.9-W1.6-P0.95-LS2.8-BR" -> "SOT-23"
/// "TO-252-2_L6.5-W5.3-P2.30-LS9.7-BR" -> "TO-252"
/// "R0805" -> "0805"
pub fn extract_package_base(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // If it's like "SOT-223-3_L6.5-W3.4-P2.30-LS7.0-BR", split before dimension specs like "_L", "_W", "_D"
    let re_dim = regex::Regex::new(r"_(?:[LWDH]|LS|EP|BR|BL)\d").unwrap();
    let prefix = if let Some(mat) = re_dim.find(trimmed) {
        &trimmed[..mat.start()]
    } else if let Some(idx) = trimmed.find('_') {
        &trimmed[..idx]
    } else {
        trimmed
    };

    // Strip trailing pin count for SOT and TO packages: e.g. SOT-223-3 -> SOT-223, SOT-23-5 -> SOT-23, TO-252-2 -> TO-252
    let re_sot_to = regex::Regex::new(r"^(SOT-\d+|TO-\d+)(?:-\d+)+$").unwrap();
    if let Some(caps) = re_sot_to.captures(prefix) {
        return caps.get(1).unwrap().as_str().to_string();
    }

    prefix.to_string()
}

/// Resolve the clean base package name from ComponentData.
/// Prefers `supplier_footprint` (the page product package, e.g. "SOT-223") if present;
/// otherwise extracts base from `component_data.package`.
/// For chip packages (R0805, C0805, 0603, 0402, etc.), preserves the R or C indicator
/// from the EasyEDA footprint package so resistor and capacitor footprints remain differentiated.
pub fn resolve_package_name(component_data: &ComponentData, replace_dot: bool) -> Option<String> {
    let pkg_from_easyeda = extract_package_base(&component_data.package);
    let re_chip = regex::Regex::new(r"^[RCL]\d{4}$").unwrap();

    let base = if re_chip.is_match(&pkg_from_easyeda) {
        // Keep R/C/L identifier for chip components (e.g. R0805 vs C0805)
        pkg_from_easyeda
    } else if let Some(sf) = component_data
        .supplier_footprint
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        extract_package_base(sf)
    } else if !component_data.package.trim().is_empty() {
        pkg_from_easyeda
    } else {
        return None;
    };

    let sanitized = sanitize_name(&base, replace_dot);
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

/// Helper to generate package suffixed footprint name.
/// When `replace_dot` is false, uses `_` delimiter (e.g. `SOT-223_C6186`).
/// When `replace_dot` is true, uses `.` delimiter (e.g. `SOT-223.C6186`).
pub fn make_package_suffixed_name(package_base: &str, lcsc_id: &str, replace_dot: bool) -> String {
    let clean = restore_footprint_name(package_base, replace_dot);
    if replace_dot {
        format!("{}.{}", clean, lcsc_id)
    } else {
        format!("{}_{}", clean, lcsc_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_footprint_name() {
        assert_eq!(restore_footprint_name("XT30PW-M_C431092", false), "XT30PW-M");
        assert_eq!(restore_footprint_name("XT30PW-M", false), "XT30PW-M");
        assert_eq!(restore_footprint_name("0603WAF1001T5E_C21190", false), "0603WAF1001T5E");
        assert_eq!(restore_footprint_name("CH340N_C2977777_C2977777", false), "CH340N");
    }

    #[test]
    fn test_restore_step_model_name() {
        assert_eq!(restore_step_model_name("CONN-TH_XT30PW-M_C431092", false), "CONN-TH_XT30PW-M");
        assert_eq!(restore_step_model_name("CONN-TH_XT30PW-M", false), "CONN-TH_XT30PW-M");
        assert_eq!(restore_step_model_name("C0603_L1_6-W0_8-H0_8_C1653", false), "C0603_L1.6-W0.8-H0.8");
        assert_eq!(
            restore_step_model_name("SOP-8_L4_9-W3_9-P1_27-LS6_0-BL-1_C2977777", false),
            "SOP-8_L4.9-W3.9-P1.27-LS6.0-BL-1"
        );
        assert_eq!(
            restore_step_model_name("SOD-123_L2_8-W1_8-LS3_7-RD_C77978", false),
            "SOD-123_L2.8-W1.8-LS3.7-RD"
        );
    }

    #[test]
    fn test_replace_dot_with_underscore() {
        assert_eq!(restore_step_model_name("C0603_L1_6-W0_8-H0_8_C1653", true), "C0603_L1_6-W0_8-H0_8");
        assert_eq!(restore_step_model_name("C0603_L1.6-W0.8-H0.8_C1653", true), "C0603_L1_6-W0_8-H0_8");
        assert_eq!(restore_footprint_name("C0603_L1.6-W0.8-H0.8_C1653", true), "C0603_L1_6-W0_8-H0_8");
        assert_eq!(restore_footprint_name("C0603_L1_6-W0_8-H0_8_C1653", false), "C0603_L1.6-W0.8-H0.8");
        assert_eq!(make_suffixed_name("XT30PW-M", "C431092", false), "XT30PW-M_C431092");
        assert_eq!(make_suffixed_name("XT30PW-M_C431092", "C431092", false), "XT30PW-M_C431092");
    }

    #[test]
    fn test_package_naming_rules() {
        // C6186 package SOT-223
        assert_eq!(extract_package_base("SOT-223-3_L6.5-W3.4-P2.30-LS7.0-BR"), "SOT-223");
        assert_eq!(extract_package_base("SOT-223"), "SOT-223");
        assert_eq!(extract_package_base("SOT-23-5_L2.9-W1.6-P0.95-LS2.8-BR"), "SOT-23");
        assert_eq!(extract_package_base("TO-252-2_L6.5-W5.3-P2.30-LS9.7-BR"), "TO-252");
        assert_eq!(extract_package_base("SOD-123_L2.8-W1.8-LS3.7-RD"), "SOD-123");
        assert_eq!(extract_package_base("SOP-8_L4.9-W3.9-P1.27-LS6.0-BL"), "SOP-8");
        // Chip components preserve R / C identifier
        assert_eq!(extract_package_base("R0805"), "R0805");
        assert_eq!(extract_package_base("C0805"), "C0805");
        assert_eq!(extract_package_base("R0603_L1.6-W0.8"), "R0603");
        assert_eq!(extract_package_base("C0402_L1.0-W0.5"), "C0402");

        // Base filename (未开后缀)
        assert_eq!(restore_footprint_name("SOT-223", false), "SOT-223");
        // 开启后缀后为 SOT-223_C6186
        assert_eq!(make_package_suffixed_name("SOT-223", "C6186", false), "SOT-223_C6186");
        // 开启替换后为 SOT-223.C6186
        assert_eq!(make_package_suffixed_name("SOT-223", "C6186", true), "SOT-223.C6186");

        // R0805 with supplier_footprint as bare "0805" preserves R0805
        let res_comp = ComponentData {
            lcsc_id: "C3013955".to_string(),
            title: "AR05BTBW6200".to_string(),
            designator: "R?".to_string(),
            description: "Resistor".to_string(),
            data_str: Vec::new(),
            bbox_x: 0.0,
            bbox_y: 0.0,
            symbol_parts: Vec::new(),
            package_detail: Vec::new(),
            package_bbox_x: 0.0,
            package_bbox_y: 0.0,
            model_3d: None,
            manufacturer: "".to_string(),
            datasheet: "".to_string(),
            jlc_id: "".to_string(),
            package: "R0805".to_string(),
            supplier_footprint: Some("0805".to_string()),
        };
        assert_eq!(resolve_package_name(&res_comp, false).as_deref(), Some("R0805"));
        assert_eq!(make_package_suffixed_name("R0805", "C3013955", false), "R0805_C3013955");
        assert_eq!(make_package_suffixed_name("R0805", "C3013955", true), "R0805.C3013955");
    }

    #[test]
    fn test_px_to_mil() {
        let converter = Converter::new(KicadVersion::V6);
        assert_eq!(converter.px_to_mil(10.0), 100);
        assert_eq!(converter.px_to_mil(5.5), 55);
    }

    #[test]
    fn test_px_to_mm() {
        let converter = Converter::new(KicadVersion::V6);
        let result = converter.px_to_mm(10.0);
        assert!((result - 2.54).abs() < 0.001);
    }

    #[test]
    fn test_flip_y() {
        let converter = Converter::new(KicadVersion::V6);
        assert_eq!(converter.flip_y(10.0), -10.0);
        assert_eq!(converter.flip_y(-5.0), 5.0);
    }

    #[test]
    fn test_deg_to_rad() {
        let converter = Converter::new(KicadVersion::V6);
        let result = converter.deg_to_rad(180.0);
        assert!((result - std::f64::consts::PI).abs() < 0.001);
    }
}

