use std::str::FromStr;

use serde::Deserialize;

use crate::model::{Config, CoordFrame, LengthUnit, PipelineError};

const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
const SODIPODI_NS: &str = "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawConfig {
    version: Option<i64>,
    page_size: Option<String>,
    page_unit: Option<String>,
    margin: Option<String>,
    kerf: Option<String>,
    segment_length: Option<String>,
    piece_spacing: Option<String>,
    print_bleed: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present_nullable_string")]
    score_color: Option<Option<String>>,
    seed: Option<u64>,
}

fn deserialize_present_nullable_string<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = Option::<String>::deserialize(deserializer)?;
    Ok(Some(val))
}

impl Config {
    /// Parses and validates an optional YAML configuration file into canonical `mm`.
    pub fn from_yaml(yaml: Option<&str>) -> Result<Self, PipelineError> {
        let Some(yaml_text) = yaml else {
            return Ok(Self::default());
        };

        let raw_opt: Option<RawConfig> = serde_saphyr::from_str(yaml_text).map_err(|err| {
            PipelineError::new("config", format!("invalid YAML configuration: {err}"))
        })?;

        let Some(raw) = raw_opt else {
            return Ok(Self::default());
        };

        if let Some(version) = raw.version
            && version != 1
        {
            return Err(PipelineError::new(
                "config",
                format!("unsupported config version {version}; expected version: 1"),
            ));
        }

        let mut cfg = Self::default();

        if let Some(page_size) = raw.page_size.as_deref() {
            let (w_mm, h_mm) = parse_page_size(page_size)?;
            cfg.page_width_mm = w_mm;
            cfg.page_height_mm = h_mm;
        }

        if let Some(page_unit) = raw.page_unit.as_deref() {
            cfg.page_unit = parse_page_unit(page_unit, "page_unit")?;
        }

        if let Some(margin) = raw.margin.as_deref() {
            let margin_mm = parse_scalar_length(margin, "margin")?;
            if margin_mm < 0.0 {
                return Err(PipelineError::new(
                    "config",
                    format!("margin must be >= 0.0 mm, got \"{margin}\""),
                ));
            }
            cfg.margin_mm = margin_mm;
        }

        if let Some(kerf) = raw.kerf.as_deref() {
            let kerf_mm = parse_scalar_length(kerf, "kerf")?;
            if kerf_mm < 0.0 {
                return Err(PipelineError::new(
                    "config",
                    format!("kerf must be >= 0.0 mm, got \"{kerf}\""),
                ));
            }
            cfg.kerf_mm = kerf_mm;
            // Default kerf-relative fields track configured kerf unless overridden.
            cfg.segment_length_mm = kerf_mm;
            cfg.piece_spacing_mm = kerf_mm;
            cfg.print_bleed_mm = kerf_mm;
        }

        let mut seg_used_kerf = raw.segment_length.is_none();
        if let Some(segment_length) = raw.segment_length.as_deref() {
            let (seg_mm, used_kerf) =
                parse_kerf_relative_length(segment_length, cfg.kerf_mm, "segment_length")?;
            seg_used_kerf = used_kerf;
            if seg_mm < 0.0 {
                return Err(PipelineError::new(
                    "config",
                    format!("segment_length must be >= 0.02 mm, got \"{segment_length}\""),
                ));
            }
            cfg.segment_length_mm = seg_mm;
        }

        if cfg.kerf_mm == 0.0 && seg_used_kerf {
            return Err(PipelineError::new(
                "config",
                "kerf is 0.0 mm and segment_length relies on a kerf multiplier (yielding 0.0 mm); set an explicit positive segment_length >= 0.02mm",
            ));
        }

        if cfg.segment_length_mm < 0.02 {
            return Err(PipelineError::new(
                "config",
                format!(
                    "segment_length must be >= 0.02 mm, got {:.4} mm",
                    cfg.segment_length_mm
                ),
            ));
        }

        if let Some(piece_spacing) = raw.piece_spacing.as_deref() {
            let (spacing_mm, _) =
                parse_kerf_relative_length(piece_spacing, cfg.kerf_mm, "piece_spacing")?;
            if spacing_mm < 0.0 {
                return Err(PipelineError::new(
                    "config",
                    format!("piece_spacing must be >= 0.0 mm, got \"{piece_spacing}\""),
                ));
            }
            cfg.piece_spacing_mm = spacing_mm;
        }

        if let Some(print_bleed) = raw.print_bleed.as_deref() {
            let (bleed_mm, _) =
                parse_kerf_relative_length(print_bleed, cfg.kerf_mm, "print_bleed")?;
            if bleed_mm < 0.0 {
                return Err(PipelineError::new(
                    "config",
                    format!("print_bleed must be >= 0.0 mm, got \"{print_bleed}\""),
                ));
            }
            cfg.print_bleed_mm = bleed_mm;
        }

        if let Some(score_color_opt) = raw.score_color {
            let Some(score_color) = score_color_opt else {
                return Err(PipelineError::new(
                    "config",
                    "score_color is null or empty; quote hex colors in YAML as \"#RRGGBB\" (for example, score_color: \"#0000ff\")",
                ));
            };
            if score_color.trim().is_empty() {
                return Err(PipelineError::new(
                    "config",
                    "score_color is empty; quote hex colors in YAML as \"#RRGGBB\" (for example, score_color: \"#0000ff\")",
                ));
            }
            cfg.score_color = parse_score_color(&score_color)?;
        }

        if let Some(seed) = raw.seed {
            cfg.seed = seed;
        }

        Ok(cfg)
    }
}

/// Parses a strict decimal literal (rejecting scientific notation, percentages, NaN, and Inf).
fn parse_strict_decimal(
    num_str: &str,
    field_name: &str,
    full_input: &str,
) -> Result<f64, PipelineError> {
    let s = num_str.trim();
    if s.is_empty() {
        return Err(PipelineError::new(
            "config",
            format!(
                "invalid {field_name} \"{full_input}\": missing numeric value (examples: \"0.18mm\", \"0.25 in\")"
            ),
        ));
    }
    if s.contains(['e', 'E', '%']) {
        return Err(PipelineError::new(
            "config",
            format!(
                "invalid {field_name} \"{full_input}\": scientific notation and percentages are not allowed (examples: \"0.18mm\", \"0.25 in\")"
            ),
        ));
    }

    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
    if unsigned.is_empty() {
        return Err(PipelineError::new(
            "config",
            format!(
                "invalid {field_name} \"{full_input}\": invalid number (examples: \"0.18mm\", \"0.25 in\")"
            ),
        ));
    }

    let mut dot_count = 0usize;
    let mut digit_count = 0usize;
    for b in unsigned.bytes() {
        if b.is_ascii_digit() {
            digit_count += 1;
        } else if b == b'.' {
            dot_count += 1;
            if dot_count > 1 {
                return Err(PipelineError::new(
                    "config",
                    format!(
                        "invalid {field_name} \"{full_input}\": malformed decimal number (examples: \"0.18mm\", \"0.25 in\")"
                    ),
                ));
            }
        } else {
            return Err(PipelineError::new(
                "config",
                format!(
                    "invalid {field_name} \"{full_input}\": unexpected character in number (examples: \"0.18mm\", \"0.25 in\")"
                ),
            ));
        }
    }

    if digit_count == 0 {
        return Err(PipelineError::new(
            "config",
            format!(
                "invalid {field_name} \"{full_input}\": missing digits (examples: \"0.18mm\", \"0.25 in\")"
            ),
        ));
    }

    let val: f64 = s.parse().map_err(|_| {
        PipelineError::new(
            "config",
            format!("invalid {field_name} \"{full_input}\": cannot parse number"),
        )
    })?;
    if !val.is_finite() {
        return Err(PipelineError::new(
            "config",
            format!("invalid {field_name} \"{full_input}\": non-finite value is not allowed"),
        ));
    }
    Ok(val)
}

fn parse_unit_suffix(
    unit_str: &str,
    field_name: &str,
    full_input: &str,
) -> Result<LengthUnit, PipelineError> {
    match unit_str.to_ascii_lowercase().as_str() {
        "mm" => Ok(LengthUnit::Mm),
        "cm" => Ok(LengthUnit::Cm),
        "in" => Ok(LengthUnit::In),
        "px" => Ok(LengthUnit::Px),
        _ => Err(PipelineError::new(
            "config",
            format!(
                "invalid unit in {field_name} \"{full_input}\"; expected one of mm, cm, in, px (examples: \"0.18mm\", \"0.25 in\")"
            ),
        )),
    }
}

/// Parses `<number>[ws]<mm|cm|in|px>` into millimeters.
pub fn parse_scalar_length(input: &str, field_name: &str) -> Result<f64, PipelineError> {
    let trimmed = input.trim();
    if trimmed.len() < 3 {
        return Err(PipelineError::new(
            "config",
            format!(
                "invalid {field_name} \"{input}\": expected decimal value with unit mm, cm, in, or px (examples: \"0.18mm\", \"0.25 in\")"
            ),
        ));
    }

    let split_idx = trimmed.len() - 2;
    if !trimmed.is_char_boundary(split_idx) {
        return Err(PipelineError::new(
            "config",
            format!(
                "invalid {field_name} \"{input}\": expected unit mm, cm, in, or px (examples: \"0.18mm\", \"0.25 in\")"
            ),
        ));
    }

    let (num_part, unit_part) = trimmed.split_at(split_idx);
    let unit = parse_unit_suffix(unit_part, field_name, input)?;
    let num = parse_strict_decimal(num_part, field_name, input)?;
    Ok(unit.to_mm(num))
}

/// Parses `<multiplier>[ws]kerf` or a scalar length with unit into millimeters.
pub fn parse_kerf_relative_length(
    input: &str,
    kerf_mm: f64,
    field_name: &str,
) -> Result<(f64, bool), PipelineError> {
    let trimmed = input.trim();
    if trimmed.len() >= 4 {
        let split_idx = trimmed.len() - 4;
        if trimmed.is_char_boundary(split_idx) {
            let (num_part, suffix) = trimmed.split_at(split_idx);
            if suffix.eq_ignore_ascii_case("kerf") {
                let mult = parse_strict_decimal(num_part, field_name, input)?;
                return Ok((mult * kerf_mm, true));
            }
        }
    }
    let mm = parse_scalar_length(trimmed, field_name)?;
    Ok((mm, false))
}

/// Parses `<width> x <height>` where each side is a scalar length with unit.
pub fn parse_page_size(input: &str) -> Result<(f64, f64), PipelineError> {
    let s = input.trim();
    let bytes = s.as_bytes();
    let mut i = 0usize;

    // Optional sign on width
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    // Width numeric body (digits and dots, rejecting scientific notation cleanly via scalar parser)
    while i < bytes.len()
        && (bytes[i].is_ascii_digit()
            || bytes[i] == b'.'
            || bytes[i] == b'e'
            || bytes[i] == b'E'
            || bytes[i] == b'%')
    {
        i += 1;
    }
    // Optional whitespace between width number and width unit
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    // Width 2-letter unit
    if i + 2 > bytes.len() {
        return Err(PipelineError::new(
            "config",
            format!(
                "invalid page_size \"{input}\": expected \"<width> x <height>\" with units (example: \"6in x 6in\")"
            ),
        ));
    }
    let width_end = i + 2;
    let width_slice = &s[..width_end];
    let width_mm = parse_scalar_length(width_slice, "page_size width")?;

    let mut rest = &s[width_end..];
    rest = rest.trim_start();
    if !rest.starts_with(['x', 'X']) {
        return Err(PipelineError::new(
            "config",
            format!(
                "invalid page_size \"{input}\": expected 'x' separator between width and height (example: \"6in x 6in\")"
            ),
        ));
    }
    let height_slice = rest[1..].trim();
    let height_mm = parse_scalar_length(height_slice, "page_size height")?;

    if width_mm <= 0.0 || height_mm <= 0.0 {
        return Err(PipelineError::new(
            "config",
            format!("page_size dimensions must be strictly positive, got \"{input}\""),
        ));
    }

    Ok((width_mm, height_mm))
}

/// Parses `page_unit` (`mm`, `cm`, `in`, `px`, case-insensitive).
pub fn parse_page_unit(input: &str, field_name: &str) -> Result<LengthUnit, PipelineError> {
    parse_unit_suffix(input.trim(), field_name, input)
}

/// Parses a `#RRGGBB` hex color and normalizes it to lowercase `#rrggbb`.
pub fn parse_score_color(input: &str) -> Result<String, PipelineError> {
    let trimmed = input.trim();
    let Some(hex) = trimmed.strip_prefix('#') else {
        return Err(PipelineError::new(
            "config",
            format!("invalid score_color \"{input}\": expected 6-digit hex color like \"#0000ff\""),
        ));
    };
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(PipelineError::new(
            "config",
            format!("invalid score_color \"{input}\": expected 6-digit hex color like \"#0000ff\""),
        ));
    }
    Ok(format!("#{}", hex.to_ascii_lowercase()))
}

impl CoordFrame {
    /// Resolves input SVG units, viewBox, and `user_to_mm` conversion factors from the SVG root.
    pub fn from_svg_root(doc: &roxmltree::Document<'_>) -> Result<Self, PipelineError> {
        let root = doc.root_element();
        if root.tag_name().name() != "svg" {
            return Err(PipelineError::new(
                "ingest",
                format!(
                    "expected root <svg> element, found <{}>",
                    root.tag_name().name()
                ),
            ));
        }

        let width_attr = root.attribute("width").ok_or_else(|| {
            PipelineError::new("ingest", "missing required <svg> width attribute")
        })?;
        let height_attr = root.attribute("height").ok_or_else(|| {
            PipelineError::new("ingest", "missing required <svg> height attribute")
        })?;

        let (w_val, w_unit_opt) = parse_svg_root_dimension(width_attr, "width")?;
        let (h_val, h_unit_opt) = parse_svg_root_dimension(height_attr, "height")?;

        let phys_w_mm = w_unit_opt.unwrap_or(LengthUnit::Px).to_mm(w_val);
        let phys_h_mm = h_unit_opt.unwrap_or(LengthUnit::Px).to_mm(h_val);

        // Determine document unit: namedview inkscape:document-units -> svg inkscape:document-units -> width/height unit.
        let namedview_unit_attr = root
            .children()
            .find(|n| {
                n.is_element()
                    && n.tag_name().name() == "namedview"
                    && n.tag_name().namespace() == Some(SODIPODI_NS)
            })
            .and_then(|nv| nv.attribute((INKSCAPE_NS, "document-units")));

        let svg_unit_attr = root.attribute((INKSCAPE_NS, "document-units"));

        let doc_unit = if let Some(u) = namedview_unit_attr.or(svg_unit_attr) {
            parse_page_unit(u, "inkscape:document-units")?
        } else {
            match (w_unit_opt, h_unit_opt) {
                (Some(wu), Some(hu)) if wu == hu => wu,
                (None, None) => LengthUnit::Px,
                (wu, hu) => {
                    return Err(PipelineError::new(
                        "ingest",
                        format!(
                            "mismatched <svg> width unit ({}) and height unit ({}) without inkscape:document-units",
                            wu.map(LengthUnit::as_str).unwrap_or("unitless/px"),
                            hu.map(LengthUnit::as_str).unwrap_or("unitless/px"),
                        ),
                    ));
                }
            }
        };

        let (vb_min_x, vb_min_y, vb_w, vb_h) = if let Some(vb_attr) = root.attribute("viewBox") {
            let vb = svgtypes::ViewBox::from_str(vb_attr).map_err(|err| {
                PipelineError::new(
                    "ingest",
                    format!("invalid <svg> viewBox \"{vb_attr}\": {err}"),
                )
            })?;
            if !vb.x.is_finite()
                || !vb.y.is_finite()
                || !vb.w.is_finite()
                || !vb.h.is_finite()
                || vb.w <= 0.0
                || vb.h <= 0.0
            {
                return Err(PipelineError::new(
                    "ingest",
                    format!("invalid non-positive <svg> viewBox \"{vb_attr}\""),
                ));
            }
            (vb.x, vb.y, vb.w, vb.h)
        } else {
            (
                0.0,
                0.0,
                LengthUnit::Px.from_mm(phys_w_mm),
                LengthUnit::Px.from_mm(phys_h_mm),
            )
        };

        let ux = phys_w_mm / vb_w;
        let uy = phys_h_mm / vb_h;
        let max_u = ux.max(uy);
        if (ux - uy).abs() / max_u > 1e-4 {
            return Err(PipelineError::new(
                "ingest",
                format!(
                    "non-uniform SVG aspect ratio between <svg> width/height and viewBox: x scale {ux:.6} mm/unit vs y scale {uy:.6} mm/unit"
                ),
            ));
        }

        Ok(Self {
            doc_unit,
            viewbox_min_user: (vb_min_x, vb_min_y),
            user_to_mm: (ux, uy),
            doc_to_mm: doc_unit.to_mm(1.0),
        })
    }
}

fn parse_svg_root_dimension(
    attr: &str,
    name: &str,
) -> Result<(f64, Option<LengthUnit>), PipelineError> {
    let trimmed = attr.trim();
    let len = svgtypes::Length::from_str(trimmed).map_err(|err| {
        PipelineError::new("ingest", format!("invalid <svg> {name} \"{attr}\": {err}"))
    })?;
    if !len.number.is_finite() || len.number <= 0.0 {
        return Err(PipelineError::new(
            "ingest",
            format!("<svg> {name} must be positive and finite, got \"{attr}\""),
        ));
    }

    let unit = match len.unit {
        svgtypes::LengthUnit::None => None,
        svgtypes::LengthUnit::Mm => Some(LengthUnit::Mm),
        svgtypes::LengthUnit::Cm => Some(LengthUnit::Cm),
        svgtypes::LengthUnit::In => Some(LengthUnit::In),
        svgtypes::LengthUnit::Px => Some(LengthUnit::Px),
        svgtypes::LengthUnit::Percent => {
            return Err(PipelineError::new(
                "ingest",
                format!("percentage <svg> {name} \"{attr}\" is not supported"),
            ));
        }
        other => {
            return Err(PipelineError::new(
                "ingest",
                format!(
                    "unsupported unit {other:?} in <svg> {name} \"{attr}\"; expected mm, cm, in, px, or unitless"
                ),
            ));
        }
    };

    Ok((len.number, unit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_grammar_accepts_case_and_whitespace() {
        assert!((parse_scalar_length("0.18mm", "kerf").unwrap() - 0.18).abs() < 1e-9);
        assert!((parse_scalar_length("  0.18  MM ", "kerf").unwrap() - 0.18).abs() < 1e-9);
        assert!((parse_scalar_length("2.5CM", "margin").unwrap() - 25.0).abs() < 1e-9);
        assert!((parse_scalar_length("1 In", "margin").unwrap() - 25.4).abs() < 1e-9);
        assert!((parse_scalar_length("96px", "margin").unwrap() - 25.4).abs() < 1e-9);
    }

    #[test]
    fn length_grammar_rejects_invalid_forms() {
        assert!(parse_scalar_length("0.18", "kerf").is_err());
        assert!(parse_scalar_length("1e-2mm", "kerf").is_err());
        assert!(parse_scalar_length("1E2mm", "kerf").is_err());
        assert!(parse_scalar_length("50%", "margin").is_err());
        assert!(parse_scalar_length("NaNmm", "margin").is_err());
        assert!(parse_scalar_length("infmm", "margin").is_err());
        assert!(parse_scalar_length("1pt", "margin").is_err());
    }

    #[test]
    fn page_size_handles_px_and_x_separator() {
        let (w, h) = parse_page_size(" 96px x 192 PX ").unwrap();
        assert!((w - 25.4).abs() < 1e-9);
        assert!((h - 50.8).abs() < 1e-9);

        let (w2, h2) = parse_page_size("8.5inX11in").unwrap();
        assert!((w2 - 215.9).abs() < 1e-9);
        assert!((h2 - 279.4).abs() < 1e-9);

        assert!(parse_page_size("0in x 6in").is_err());
        assert!(parse_page_size("6 x 6in").is_err());
    }

    #[test]
    fn yaml_defaults_and_kerf_multipliers() {
        let empty = Config::from_yaml(Some("# just a comment\n")).unwrap();
        assert_eq!(empty, Config::default());
        assert_eq!(empty.seed, 0);

        let yaml = r##"
version: 1
page_size: "8.5in x 11in"
page_unit: "in"
margin: "0.5in"
kerf: "0.24mm"
segment_length: "1.5 kerf"
piece_spacing: "2kerf"
print_bleed: "1 kerf"
score_color: "#FF00AA"
seed: 42
"##;
        let cfg = Config::from_yaml(Some(yaml)).unwrap();
        assert!((cfg.page_width_mm - 215.9).abs() < 1e-9);
        assert!((cfg.page_height_mm - 279.4).abs() < 1e-9);
        assert_eq!(cfg.page_unit, LengthUnit::In);
        assert!((cfg.margin_mm - 12.7).abs() < 1e-9);
        assert!((cfg.kerf_mm - 0.24).abs() < 1e-9);
        assert!((cfg.segment_length_mm - 0.36).abs() < 1e-9);
        assert!((cfg.piece_spacing_mm - 0.48).abs() < 1e-9);
        assert!((cfg.print_bleed_mm - 0.24).abs() < 1e-9);
        assert_eq!(cfg.score_color, "#ff00aa");
        assert_eq!(cfg.seed, 42);

        assert!(Config::from_yaml(Some("seed: -1\n")).is_err());
        assert!(Config::from_yaml(Some("seed: 1.5\n")).is_err());
    }

    #[test]
    fn yaml_rejects_zero_kerf_with_kerf_segment_length_and_small_segment() {
        let zero_kerf_default_seg = "kerf: \"0mm\"\n";
        let err = Config::from_yaml(Some(zero_kerf_default_seg)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("kerf"));
        assert!(msg.contains("segment_length"));

        let zero_kerf_explicit_seg = "kerf: \"0mm\"\nsegment_length: \"0.15mm\"\n";
        let cfg = Config::from_yaml(Some(zero_kerf_explicit_seg)).unwrap();
        assert_eq!(cfg.kerf_mm, 0.0);
        assert!((cfg.segment_length_mm - 0.15).abs() < 1e-9);

        let too_small_seg = "segment_length: \"0.019mm\"\n";
        assert!(Config::from_yaml(Some(too_small_seg)).is_err());
    }

    #[test]
    fn yaml_rejects_duplicate_unknown_and_unquoted_color() {
        let dup = "kerf: \"0.18mm\"\nkerf: \"0.20mm\"\n";
        assert!(Config::from_yaml(Some(dup)).is_err());

        let unknown = "kerf_width: \"0.18mm\"\n";
        assert!(Config::from_yaml(Some(unknown)).is_err());

        let unquoted_color = "score_color: #0000ff\n";
        let err = Config::from_yaml(Some(unquoted_color)).unwrap_err();
        assert!(err.to_string().contains("\"#RRGGBB\""));

        let multi_doc = "---\nkerf: \"0.18mm\"\n---\nkerf: \"0.2mm\"\n";
        assert!(Config::from_yaml(Some(multi_doc)).is_err());
    }

    #[test]
    fn coord_frame_resolves_units_and_aspect() {
        let svg_in = r#"<svg width="4in" height="4in" viewBox="0 0 4 4" xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"><sodipodi:namedview inkscape:document-units="in"/></svg>"#;
        let doc = roxmltree::Document::parse(svg_in).unwrap();
        let frame = CoordFrame::from_svg_root(&doc).unwrap();
        assert_eq!(frame.doc_unit, LengthUnit::In);
        assert!((frame.user_to_mm.0 - 25.4).abs() < 1e-9);
        assert!((frame.doc_to_mm - 25.4).abs() < 1e-9);

        // Missing viewBox defaults to CSS px.
        let svg_no_vb = r#"<svg width="4in" height="4in" xmlns="http://www.w3.org/2000/svg"/>"#;
        let doc_no_vb = roxmltree::Document::parse(svg_no_vb).unwrap();
        let frame_no_vb = CoordFrame::from_svg_root(&doc_no_vb).unwrap();
        assert_eq!(frame_no_vb.doc_unit, LengthUnit::In);
        assert!((frame_no_vb.user_to_mm.0 - (25.4 / 96.0)).abs() < 1e-9);

        // Mismatched width/height units without document-units fails.
        let svg_mismatch_units = r#"<svg width="100mm" height="10cm" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg"/>"#;
        let doc_mu = roxmltree::Document::parse(svg_mismatch_units).unwrap();
        assert!(CoordFrame::from_svg_root(&doc_mu).is_err());

        // Non-uniform aspect fails.
        let svg_nonuniform = r#"<svg width="100mm" height="200mm" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg"/>"#;
        let doc_nu = roxmltree::Document::parse(svg_nonuniform).unwrap();
        assert!(CoordFrame::from_svg_root(&doc_nu).is_err());
    }
}
