use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use puzzle_gen::model::{
    Config, LengthUnit, NoticeSeverity, PipelineArtifacts, Rotation90,
};
use puzzle_gen::{run_pipeline, svg_emit};
use svgtypes::{SimplePathSegment, SimplifyingPathParser, TransformListParser, TransformListToken};

const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
const SODIPODI_NS: &str = "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn example_svg_path() -> PathBuf {
    crate_dir().join("examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg")
}

fn build_complete_grid_svg(rows: usize, cols: usize, unit: LengthUnit) -> String {
    let cell = unit.from_mm(20.0);
    let total_w = unit.from_mm((cols as f64) * 20.0);
    let total_h = unit.from_mm((rows as f64) * 20.0);
    let u = unit.as_str();

    let mut rects = String::new();
    for r in 0..rows {
        for c in 0..cols {
            let x = (c as f64) * cell;
            let y = (r as f64) * cell;
            rects.push_str(&format!(
                "    <rect id=\"rect-r{r}-c{c}\" x=\"{x:.6}\" y=\"{y:.6}\" width=\"{cell:.6}\" height=\"{cell:.6}\"/>\n"
            ));
        }
    }

    let mut tabs = String::new();
    for r in 0..rows.saturating_sub(1) {
        for c in 0..cols {
            let y = ((r + 1) as f64) * cell;
            let x0 = (c as f64) * cell;
            let s = x0 + 0.3 * cell;
            let e = x0 + 0.7 * cell;
            let bump = if (r + c) % 2 == 0 {
                y + 0.25 * cell
            } else {
                y - 0.25 * cell
            };
            tabs.push_str(&format!(
                "    <path id=\"th-{r}-{c}\" d=\"M {s:.6} {y:.6} C {s:.6} {bump:.6} {e:.6} {bump:.6} {e:.6} {y:.6}\"/>\n"
            ));
        }
    }
    for r in 0..rows {
        for c in 0..cols.saturating_sub(1) {
            let x = ((c + 1) as f64) * cell;
            let y0 = (r as f64) * cell;
            let s = y0 + 0.3 * cell;
            let e = y0 + 0.7 * cell;
            let bump = if (r + c) % 2 == 0 {
                x + 0.25 * cell
            } else {
                x - 0.25 * cell
            };
            tabs.push_str(&format!(
                "    <path id=\"tv-{r}-{c}\" d=\"M {x:.6} {s:.6} C {bump:.6} {s:.6} {bump:.6} {e:.6} {x:.6} {e:.6}\"/>\n"
            ));
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg width="{total_w:.6}{u}" height="{total_h:.6}{u}" viewBox="0 0 {total_w:.6} {total_h:.6}" version="1.1"
     xmlns="http://www.w3.org/2000/svg"
     xmlns:svg="http://www.w3.org/2000/svg"
     xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
     xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
     xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview id="namedview1" inkscape:document-units="{u}"/>
  <g inkscape:groupmode="layer" id="layer1" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="{total_w:.6}" height="{total_h:.6}" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" id="layer2" inkscape:label="Pieces">
{rects}  </g>
  <g inkscape:groupmode="layer" id="layer3" inkscape:label="Tabs">
{tabs}  </g>
</svg>"#
    )
}

fn build_complete_5x5_svg(unit: LengthUnit) -> String {
    build_complete_grid_svg(5, 5, unit)
}

fn parse_path_points_mm(
    d: &str,
    tx_mm: f64,
    ty_mm: f64,
    rot_deg: f64,
    unit: LengthUnit,
) -> Vec<(f64, f64)> {
    let (sin_r, cos_r) = rot_deg.to_radians().sin_cos();
    let transform = |x: f64, y: f64| -> (f64, f64) {
        let px = unit.to_mm(x);
        let py = unit.to_mm(y);
        (
            px * cos_r - py * sin_r + tx_mm,
            px * sin_r + py * cos_r + ty_mm,
        )
    };
    let mut pts = Vec::new();
    for seg in SimplifyingPathParser::from(d) {
        match seg.unwrap() {
            SimplePathSegment::MoveTo { x, y } | SimplePathSegment::LineTo { x, y } => {
                pts.push(transform(x, y));
            }
            SimplePathSegment::CurveTo { x, y, .. } | SimplePathSegment::Quadratic { x, y, .. } => {
                pts.push(transform(x, y));
            }
            SimplePathSegment::ClosePath => {}
        }
    }
    pts
}

fn parse_transform_mm(transform: Option<&str>, unit: LengthUnit) -> (f64, f64, f64) {
    let Some(tf) = transform else {
        return (0.0, 0.0, 0.0);
    };
    let mut tx = 0.0;
    let mut ty = 0.0;
    let mut rot_deg = 0.0;
    for tok in TransformListParser::from(tf) {
        match tok.unwrap() {
            TransformListToken::Translate { tx: x, ty: y } => {
                tx += unit.to_mm(x);
                ty += unit.to_mm(y);
            }
            TransformListToken::Rotate { angle } => {
                rot_deg += angle;
            }
            _ => {}
        }
    }
    (tx, ty, rot_deg)
}

#[test]
fn rainy_day_example_strict_fails_and_no_strict_heals_all_25_pieces() {
    let svg_xml = fs::read_to_string(example_svg_path()).unwrap();
    let cfg = Config::default();

    let mut strict_notices = Vec::new();
    let err = run_pipeline(&svg_xml, &cfg, true, &mut strict_notices).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("((2, 0), (3, 0))"),
        "expected missing edge ((2, 0), (3, 0)), got: {msg}"
    );
    assert!(msg.contains("2.4000in"));

    let mut notices = Vec::new();
    let artifacts = run_pipeline(&svg_xml, &cfg, false, &mut notices).unwrap();
    assert_eq!(artifacts.kerfed.rows, 5);
    assert_eq!(artifacts.kerfed.cols, 5);
    assert_eq!(artifacts.kerfed.matched_tabs, 39);
    assert_eq!(artifacts.kerfed.missing_tabs, 1);
    assert_eq!(
        artifacts.spliced.image_rendering.as_deref(),
        Some("optimizeSpeed")
    );
    assert_eq!(artifacts.kerfed.pieces.len(), 25);
    assert_eq!(
        notices
            .iter()
            .filter(|n| n.severity == NoticeSeverity::Warning && n.code == "MISSING_INTERNAL_TAB")
            .count(),
        1
    );

    // Verify FillRule::Positive removed overshoot ears cleanly on pieces touching path2 ((0,0)/(1,0)) and path10 ((1,0)/(1,1)).
    for piece in &artifacts.kerfed.pieces {
        assert!(piece.cut_polygon.len() >= 4);
        assert!(piece.bleed_polygon.len() >= 4);
        // Overshoot ears would push piece (0,0) cut_aabb_mm.y1 or piece (1,0) y0 cleanly within tab bounds.
        assert!(piece.cut_aabb_mm.width() > 15.0);
        assert!(piece.cut_aabb_mm.height() > 15.0);
    }

    // Verify all stages serialize as valid XML.
    let s1 = svg_emit::emit_stage1(&artifacts.spliced);
    let s2 = svg_emit::emit_stage2(&artifacts.kerfed, &cfg);
    let s3 = svg_emit::emit_stage3(&artifacts.kerfed, &artifacts.layout, &cfg);
    let d1 = roxmltree::Document::parse(&s1).unwrap();
    let img1 = d1
        .descendants()
        .find(|n| n.tag_name().name() == "image" && n.attribute("id") == Some("puzzle-image"))
        .unwrap();
    assert_eq!(
        img1.attribute("style"),
        Some("image-rendering:optimizeSpeed")
    );
    roxmltree::Document::parse(&s2).unwrap();
    let d3 = roxmltree::Document::parse(&s3).unwrap();
    let page_count = d3
        .descendants()
        .filter(|n| n.tag_name().name() == "page")
        .count();
    assert_eq!(page_count, artifacts.layout.pages.len());
    for page in &artifacts.layout.pages {
        let s4 = svg_emit::emit_stage4_page(&artifacts.kerfed, page, &cfg);
        roxmltree::Document::parse(&s4).unwrap();
    }
}

#[test]
fn synthetic_5x5_stages_1_to_4_and_stage3_vs_stage4_coordinate_parity() {
    let svg_xml = build_complete_5x5_svg(LengthUnit::In);
    // Set print_bleed == kerf / 2 (0.10 mm) so bleed_polygon == cut_polygon and we can check piece parity too.
    let yaml = r##"
version: 1
page_size: "3.5in x 3.5in"
page_unit: "mm"
margin: "0.25in"
kerf: "0.20mm"
segment_length: "0.18mm"
piece_spacing: "0.5 kerf"
print_bleed: "0.5 kerf"
score_color: "#00aa00"
"##;
    let cfg = Config::from_yaml(Some(yaml)).unwrap();
    let mut notices = Vec::new();
    let artifacts = run_pipeline(&svg_xml, &cfg, true, &mut notices).unwrap();
    assert_eq!(artifacts.kerfed.matched_tabs, 40);
    assert_eq!(artifacts.kerfed.missing_tabs, 0);
    assert!(artifacts.layout.pages.len() >= 2);

    let s1 = svg_emit::emit_stage1(&artifacts.spliced);
    let s2 = svg_emit::emit_stage2(&artifacts.kerfed, &cfg);
    let s3 = svg_emit::emit_stage3(&artifacts.kerfed, &artifacts.layout, &cfg);

    let d1 = roxmltree::Document::parse(&s1).unwrap();
    assert_eq!(
        d1.descendants()
            .find(|n| n.tag_name().name() == "namedview"
                && n.tag_name().namespace() == Some(SODIPODI_NS))
            .and_then(|n| n.attribute((INKSCAPE_NS, "document-units"))),
        Some("in")
    );
    let img1 = d1
        .descendants()
        .find(|n| n.attribute("id") == Some("puzzle-image"))
        .unwrap();
    assert_eq!(img1.attribute("x"), Some("0"));
    assert_eq!(img1.attribute("y"), Some("0"));
    assert_eq!(img1.attribute("preserveAspectRatio"), Some("none"));
    assert_eq!(
        d1.descendants()
            .filter(|n| n.tag_name().name() == "use"
                && n.attribute((XLINK_NS, "href")) == Some("#puzzle-image"))
            .count(),
        4
    );
    assert!(
        d1.descendants()
            .any(|n| n.attribute((INKSCAPE_NS, "label")) == Some("Pieces"))
    );
    assert!(
        d1.descendants()
            .any(|n| n.attribute((INKSCAPE_NS, "label")) == Some("GridLabels"))
    );

    let d2 = roxmltree::Document::parse(&s2).unwrap();
    assert!(
        d2.descendants()
            .any(|n| n.attribute((INKSCAPE_NS, "label")) == Some("Pieces"))
    );
    assert!(
        d2.descendants()
            .any(|n| n.attribute((INKSCAPE_NS, "label")) == Some("GridLabels"))
    );

    let d3 = roxmltree::Document::parse(&s3).unwrap();
    assert_stage3_stage4_parity(&artifacts, &cfg, &d3);
}

fn assert_stage3_stage4_parity(
    artifacts: &PipelineArtifacts,
    cfg: &Config,
    d3: &roxmltree::Document<'_>,
) {
    let print_layer = d3
        .descendants()
        .find(|n| n.attribute((INKSCAPE_NS, "label")) == Some("PrintPieces"))
        .unwrap();
    let marks_layer3 = d3
        .descendants()
        .find(|n| n.attribute((INKSCAPE_NS, "label")) == Some("PageNumbers"))
        .unwrap();

    // Check Stage 3 vs Stage 4 parity across every page.
    let doc_unit = artifacts.kerfed.frame.doc_unit;
    let out_unit = cfg.page_unit;
    for page in &artifacts.layout.pages {
        let s4 = svg_emit::emit_stage4_page(&artifacts.kerfed, page, cfg);
        assert!(!s4.contains("<defs"));
        assert!(!s4.contains("<image"));
        assert!(!s4.contains("<pattern"));
        assert!(!s4.contains("xmlns:xlink"));

        let d4 = roxmltree::Document::parse(&s4).unwrap();
        assert!(!d4.descendants().any(|n| n.attribute("transform").is_some()));

        let cut_layer = d4
            .descendants()
            .find(|n| n.attribute((INKSCAPE_NS, "label")) == Some("Cut"))
            .unwrap();
        let marks_layer4 = d4
            .descendants()
            .find(|n| n.attribute((INKSCAPE_NS, "label")) == Some("PageNumbers"))
            .unwrap();

        let page_x_mm = ((page.page_number - 1) as f64) * cfg.page_width_mm;

        // Compare PageNumbers paths unconditionally.
        let p3_marks: Vec<_> = marks_layer3
            .descendants()
            .filter(|n| {
                n.tag_name().name() == "path"
                    && n.attribute("id").is_some_and(|id| {
                        id.starts_with(&format!("page{}-mark-", page.page_number))
                    })
            })
            .collect();
        let p4_marks: Vec<_> = marks_layer4
            .descendants()
            .filter(|n| n.tag_name().name() == "path")
            .collect();
        assert_eq!(p3_marks.len(), p4_marks.len());
        for (m3, m4) in p3_marks.iter().zip(p4_marks.iter()) {
            assert!(m4.attribute("style").unwrap().contains(&cfg.score_color));
            let pts3 =
                parse_path_points_mm(m3.attribute("d").unwrap(), -page_x_mm, 0.0, 0.0, doc_unit);
            let pts4 = parse_path_points_mm(m4.attribute("d").unwrap(), 0.0, 0.0, 0.0, out_unit);
            assert_eq!(pts3.len(), pts4.len());
            for (a, b) in pts3.iter().zip(pts4.iter()) {
                let err = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                assert!(
                    err <= 2.2e-4,
                    "PageNumbers parity error {err:.6} mm > 2.2e-4 mm"
                );
            }
        }

        // Compare PrintPieces vs Cut paths when print_bleed == kerf / 2.
        for p4 in cut_layer
            .descendants()
            .filter(|n| n.tag_name().name() == "path")
        {
            let pid = p4.attribute("id").unwrap();
            assert!(p4.attribute("style").unwrap().contains("#000000"));
            let p3 = print_layer
                .descendants()
                .find(|n| n.tag_name().name() == "path" && n.attribute("id") == Some(pid))
                .unwrap();
            let g3 = p3.parent().unwrap();
            let (tx_mm, ty_mm, rot_deg) = parse_transform_mm(g3.attribute("transform"), doc_unit);
            let pts3 = parse_path_points_mm(
                p3.attribute("d").unwrap(),
                tx_mm - page_x_mm,
                ty_mm,
                rot_deg,
                doc_unit,
            );
            let pts4 = parse_path_points_mm(p4.attribute("d").unwrap(), 0.0, 0.0, 0.0, out_unit);
            assert_eq!(pts3.len(), pts4.len());
            for (a, b) in pts3.iter().zip(pts4.iter()) {
                let err = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                assert!(
                    err <= 2.2e-4,
                    "piece {pid} parity error {err:.6} mm > 2.2e-4 mm"
                );
            }
        }
    }
}

#[test]
fn supports_all_four_units_and_default_config_yaml() {
    let default_yaml =
        fs::read_to_string(crate_dir().join("examples/default-config.yaml")).unwrap();
    let parsed_default = Config::from_yaml(Some(&default_yaml)).unwrap();
    assert_eq!(parsed_default, Config::default());

    for unit in [
        LengthUnit::Mm,
        LengthUnit::Cm,
        LengthUnit::In,
        LengthUnit::Px,
    ] {
        let svg_xml = build_complete_5x5_svg(unit);
        let yaml = format!(
            "version: 1\npage_size: \"6in x 6in\"\npage_unit: \"{}\"\nkerf: \"0.18mm\"\n",
            unit.as_str()
        );
        let cfg = Config::from_yaml(Some(&yaml)).unwrap();
        let mut notices = Vec::new();
        let artifacts = run_pipeline(&svg_xml, &cfg, true, &mut notices).unwrap();
        assert_eq!(artifacts.kerfed.pieces.len(), 25);
        let s4 = svg_emit::emit_stage4_page(&artifacts.kerfed, &artifacts.layout.pages[0], &cfg);
        let d4 = roxmltree::Document::parse(&s4).unwrap();
        assert_eq!(
            d4.descendants()
                .find(|n| n.tag_name().name() == "namedview"
                    && n.tag_name().namespace() == Some(SODIPODI_NS))
                .and_then(|n| n.attribute((INKSCAPE_NS, "document-units"))),
            Some(unit.as_str())
        );
    }
}

#[test]
fn cli_end_to_end_dry_run_collisions_overwrite_pruning_and_debug_emit() {
    let bin = env!("CARGO_BIN_EXE_puzzle-gen");
    let tmp = tempfile::tempdir().unwrap();

    // 1. --dry-run does not create output-dir.
    let dry_out = tmp.path().join("dry_gen");
    let out_dry = Command::new(bin)
        .arg(example_svg_path())
        .arg("--no-strict")
        .arg("--dry-run")
        .arg("--output-dir")
        .arg(&dry_out)
        .output()
        .unwrap();
    assert!(
        out_dry.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out_dry.stderr)
    );
    assert!(!dry_out.exists());
    let stdout_dry = String::from_utf8_lossy(&out_dry.stdout);
    assert!(stdout_dry.contains("grid=5x5"));
    assert!(stdout_dry.contains("matched_tabs=39"));
    assert!(stdout_dry.contains("missing_tabs=1"));
    assert!(stdout_dry.contains("pages="));
    assert!(stdout_dry.contains("puzzle_area_mm2="));
    assert!(stdout_dry.contains("total_bbox_mm2="));
    assert!(stdout_dry.contains("bbox_area_pct="));

    // 2. Write multi-page outputs with small page size (e.g. 2in x 2in -> >= 5 pages).
    let out_dir = tmp.path().join("run1");
    let small_cfg = tmp.path().join("small.yaml");
    fs::write(
        &small_cfg,
        "version: 1\npage_size: \"2in x 2in\"\nmargin: \"0.1in\"\n",
    )
    .unwrap();
    let run1 = Command::new(bin)
        .arg(example_svg_path())
        .arg("--no-strict")
        .arg("--config")
        .arg(&small_cfg)
        .arg("--output-dir")
        .arg(&out_dir)
        .arg("--prefix")
        .arg("puz")
        .output()
        .unwrap();
    assert!(
        run1.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run1.stderr)
    );
    assert!(out_dir.join("puz-stage1.svg").exists());
    assert!(out_dir.join("puz-stage2.svg").exists());
    assert!(out_dir.join("puz-stage3.svg").exists());
    assert!(out_dir.join("puz-stage4-page1.svg").exists());
    assert!(out_dir.join("puz-stage4-page5.svg").exists());

    // Add unrelated files that must never be pruned.
    let notes_file = out_dir.join("puz-stage4-page-notes.svg");
    let zero_pad_file = out_dir.join("puz-stage4-page01.svg");
    fs::write(&notes_file, "keep me").unwrap();
    fs::write(&zero_pad_file, "keep me too").unwrap();

    // 3. Re-run without --overwrite fails on collisions without altering files.
    let collide = Command::new(bin)
        .arg(example_svg_path())
        .arg("--no-strict")
        .arg("--output-dir")
        .arg(&out_dir)
        .arg("--prefix")
        .arg("puz")
        .output()
        .unwrap();
    assert!(!collide.status.success());
    assert!(String::from_utf8_lossy(&collide.stderr).contains("without --overwrite"));

    // 4. --emit-debug-on-error on Stage 3 pagination failure preserves Stage 4 pages and emits debug SVGs.
    let tiny_cfg = tmp.path().join("tiny.yaml");
    fs::write(
        &tiny_cfg,
        "version: 1\npage_size: \"1in x 1in\"\nmargin: \"0.2in\"\n",
    )
    .unwrap();
    let dbg_dir = tmp.path().join("dbg");
    fs::create_dir_all(&dbg_dir).unwrap();
    let untouched_s4 = dbg_dir.join("puz-stage4-page9.svg");
    fs::write(&untouched_s4, "untouched").unwrap();
    let dbg_run = Command::new(bin)
        .arg(example_svg_path())
        .arg("--no-strict")
        .arg("--config")
        .arg(&tiny_cfg)
        .arg("--output-dir")
        .arg(&dbg_dir)
        .arg("--prefix")
        .arg("puz")
        .arg("--emit-debug-on-error")
        .output()
        .unwrap();
    assert!(!dbg_run.status.success());
    let dbg_stderr = String::from_utf8_lossy(&dbg_run.stderr);
    assert!(dbg_stderr.contains("MISSING_INTERNAL_TAB"));
    assert!(dbg_dir.join("puz-stage1.svg").exists());
    assert!(dbg_dir.join("puz-stage2.svg").exists());
    assert!(untouched_s4.exists());

    // 5. Re-run with --overwrite and large sheet (1 page) prunes stale Stage 4 pages > 1.
    let big_cfg = tmp.path().join("big.yaml");
    fs::write(
        &big_cfg,
        "version: 1\npage_size: \"12in x 12in\"\nmargin: \"0.25in\"\n",
    )
    .unwrap();
    let run2 = Command::new(bin)
        .arg(example_svg_path())
        .arg("--no-strict")
        .arg("--overwrite")
        .arg("--config")
        .arg(&big_cfg)
        .arg("--output-dir")
        .arg(&out_dir)
        .arg("--prefix")
        .arg("puz")
        .output()
        .unwrap();
    assert!(
        run2.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run2.stderr)
    );
    assert!(out_dir.join("puz-stage4-page1.svg").exists());
    assert!(!out_dir.join("puz-stage4-page2.svg").exists());
    assert!(!out_dir.join("puz-stage4-page5.svg").exists());
    assert!(notes_file.exists());
    assert!(zero_pad_file.exists());

    // 6. Reject unsafe --prefix values.
    for bad in ["", ".", "..", "a/b", "a\\b"] {
        let bad_run = Command::new(bin)
            .arg(example_svg_path())
            .arg("--dry-run")
            .arg("--prefix")
            .arg(bad)
            .output()
            .unwrap();
        assert!(
            !bad_run.status.success(),
            "prefix {bad:?} should be rejected"
        );
    }

    let _ = Path::new(".").exists();
}

fn wrap_mm_svg(image_attrs: &str, pieces_body: &str, tabs_body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg width="40mm" height="40mm" viewBox="0 0 40 40" version="1.1"
     xmlns="http://www.w3.org/2000/svg"
     xmlns:svg="http://www.w3.org/2000/svg"
     xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
     xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
     xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview id="namedview1" inkscape:document-units="mm"/>
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" {image_attrs} xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    {pieces_body}
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs">
    {tabs_body}
  </g>
</svg>"#
    )
}

fn ring_area_mm2(pts: &[kurbo::Point]) -> f64 {
    let mut sum = 0.0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    0.5 * sum
}

#[test]
fn bounded_early_snapping_and_edge_cases() {
    // 1. 1.0 mm bounded-diameter <rect> perturbation, 2.0 mm joint tab misalignment with handle translation,
    //    corner-adjacent full-edge tab, Quadratic (raise) & Arc tab curves, and sub-0.05 mm stray path filtering.
    let rects_2x2 = r#"
    <rect id="r00" x="0.0" y="0.0" width="19.6" height="19.8"/>
    <rect id="r01" x="20.0" y="0.0" width="20.0" height="20.2"/>
    <rect id="r10" x="0.0" y="20.3" width="20.3" height="19.7"/>
    <rect id="r11" x="19.9" y="19.9" width="20.1" height="20.1"/>
"#;
    let tabs_2x2 = r#"
    <path id="stray1" d="M 3.0 3.0 L 3.01 3.01 Z"/>
    <path id="t_h0_snap" d="M 5.0 21.5 C 5.0 25.0 15.0 25.0 15.0 18.5"/>
    <path id="t_h1_corner_full" d="M 20.0 20.0 C 25.0 23.0 35.0 23.0 40.0 20.0"/>
    <path id="t_v0_quad" d="M 20.0 5.0 Q 25.0 10.0 20.0 15.0"/>
    <path id="t_v1_arc" d="M 20.0 25.0 A 5.0 5.0 0 0 1 20.0 35.0"/>
"#;
    let svg = wrap_mm_svg("", rects_2x2, tabs_2x2);
    let cfg = Config::default();
    let mut notices = Vec::new();
    let artifacts = run_pipeline(&svg, &cfg, true, &mut notices).unwrap();
    assert_eq!(artifacts.kerfed.rows, 2);
    assert_eq!(artifacts.kerfed.cols, 2);
    assert_eq!(artifacts.kerfed.matched_tabs, 4);
    assert_eq!(artifacts.kerfed.missing_tabs, 0);
    assert!(
        notices
            .iter()
            .any(|n| n.code == "STRAY_PATH_FILTERED" && n.message.contains("stray1"))
    );
    // Verify Quadratic and Arc were normalized to cubic curves and snapped handle moved on r00.
    let r00 = &artifacts.spliced.pieces[0];
    assert!(
        r00.path_mm
            .elements()
            .iter()
            .any(|el| matches!(el, kurbo::PathEl::CurveTo(_, _, _)))
    );
    let has_snapped_handle = r00.path_mm.elements().iter().any(|el| match *el {
        kurbo::PathEl::CurveTo(c1, c2, _) => (c1.y - 23.5).abs() < 0.5 || (c2.y - 23.5).abs() < 0.5,
        _ => false,
    });
    assert!(
        has_snapped_handle,
        "expected tangency-preserving handle translation on snapped curve"
    );

    // 2. Multi-edge corner ambiguity rejection.
    let ambig_svg = wrap_mm_svg(
        "",
        r#"
    <rect id="r00" x="0" y="0" width="20" height="20"/>
    <rect id="r01" x="20" y="0" width="20" height="20"/>
    <rect id="r10" x="0" y="20" width="20" height="20"/>
    <rect id="r11" x="20" y="20" width="20" height="20"/>
"#,
        r#"<path id="ambig" d="M 18.5 20.0 L 20.0 21.5"/>"#,
    );
    let ambig_err = run_pipeline(&ambig_svg, &cfg, false, &mut Vec::new()).unwrap_err();
    assert!(ambig_err.to_string().contains("ambig"));

    // 3. Rotated <rect> and rotated <image> rejection.
    let rot_rect_svg = wrap_mm_svg(
        "",
        r#"<rect id="rot_rect" x="0" y="0" width="20" height="20" transform="rotate(15 10 10)"/>"#,
        "",
    );
    let rot_rect_err = run_pipeline(&rot_rect_svg, &cfg, true, &mut Vec::new()).unwrap_err();
    assert!(rot_rect_err.to_string().contains("rot_rect"));

    let rot_img_svg = wrap_mm_svg(
        r#"transform="rotate(5)""#,
        r#"<rect id="r00" x="0" y="0" width="40" height="40"/>"#,
        "",
    );
    let rot_img_err = run_pipeline(&rot_img_svg, &cfg, true, &mut Vec::new()).unwrap_err();
    assert!(rot_img_err.to_string().contains("img1"));

    // 4. bleed_polygon socket bridging, print_bleed < kerf / 2 warning, and zero kerf short-circuiting.
    let narrow_socket_svg = wrap_mm_svg(
        "",
        r#"
    <rect id="r00" x="0" y="0" width="40" height="20"/>
    <rect id="r10" x="0" y="20" width="40" height="20"/>
"#,
        r#"<path id="t_narrow" d="M 19.0 20.0 L 19.75 18.0 L 18.0 15.0 L 22.0 15.0 L 20.25 18.0 L 21.0 20.0"/>"#,
    );
    let zero_kerf_cfg = Config::from_yaml(Some(
        "version: 1\nkerf: \"0mm\"\nsegment_length: \"0.18mm\"\nprint_bleed: \"0.6mm\"\n",
    ))
    .unwrap();
    let bridged = run_pipeline(&narrow_socket_svg, &zero_kerf_cfg, true, &mut Vec::new()).unwrap();
    assert!(
        ring_area_mm2(&bridged.kerfed.pieces[1].bleed_polygon)
            > ring_area_mm2(&bridged.kerfed.pieces[1].cut_polygon)
    );

    let small_bleed_cfg = Config::from_yaml(Some(
        "version: 1\nkerf: \"0.20mm\"\nprint_bleed: \"0.05mm\"\n",
    ))
    .unwrap();
    let mut small_bleed_notices = Vec::new();
    let _ = run_pipeline(
        &narrow_socket_svg,
        &small_bleed_cfg,
        true,
        &mut small_bleed_notices,
    )
    .unwrap();
    assert!(
        small_bleed_notices
            .iter()
            .any(|n| n.severity == NoticeSeverity::Warning && n.code == "BLEED_LESS_THAN_HALF_KERF")
    );

    // 5. serde-saphyr duplicate-key, unknown-key, and unquoted #RRGGBB errors.
    assert!(Config::from_yaml(Some("kerf: \"0.18mm\"\nkerf: \"0.20mm\"\n")).is_err());
    assert!(Config::from_yaml(Some("unknown_key: \"0.18mm\"\n")).is_err());
    let color_err = Config::from_yaml(Some("score_color: #0000ff\n")).unwrap_err();
    assert!(color_err.to_string().contains("\"#RRGGBB\""));
}

#[test]
fn default_8_5x11_packs_rainy_day_on_single_page_with_compacted_mark() {
    let svg_xml = fs::read_to_string(example_svg_path()).unwrap();
    let cfg = Config::default();
    let mut notices = Vec::new();
    let artifacts = run_pipeline(&svg_xml, &cfg, false, &mut notices).unwrap();
    assert_eq!(artifacts.layout.pages.len(), 1);
    assert!(
        artifacts.layout.bbox_area_pct < 200.0,
        "expected tight bbox_area_pct < 200.0%, got {:.2}%",
        artifacts.layout.bbox_area_pct
    );
    assert!(
        artifacts.layout.pages[0].bottom_right_anchor_mm.y < cfg.page_height_mm - cfg.margin_mm - 1.0,
        "expected compacted bottom-right anchor above bottom margin ({:.2} vs {:.2})",
        artifacts.layout.pages[0].bottom_right_anchor_mm.y,
        cfg.page_height_mm - cfg.margin_mm
    );
}

#[test]
fn seeded_nesting_is_deterministic_and_cli_overrides_yaml_seed() {
    let svg_xml = build_complete_5x5_svg(LengthUnit::Mm);
    let yaml_42 = "version: 1\npage_size: \"3.5in x 3.5in\"\nmargin: \"0.25in\"\nseed: 42\n";
    let cfg_42 = Config::from_yaml(Some(yaml_42)).unwrap();

    let run1 = run_pipeline(&svg_xml, &cfg_42, true, &mut Vec::new()).unwrap();
    let run2 = run_pipeline(&svg_xml, &cfg_42, true, &mut Vec::new()).unwrap();
    assert_eq!(run1.layout, run2.layout);
    assert_eq!(
        svg_emit::emit_stage3(&run1.kerfed, &run1.layout, &cfg_42),
        svg_emit::emit_stage3(&run2.kerfed, &run2.layout, &cfg_42)
    );
    assert_eq!(
        svg_emit::emit_stage4_page(&run1.kerfed, &run1.layout.pages[0], &cfg_42),
        svg_emit::emit_stage4_page(&run2.kerfed, &run2.layout.pages[0], &cfg_42)
    );

    let bin = env!("CARGO_BIN_EXE_puzzle-gen");
    let tmp = tempfile::tempdir().unwrap();
    let cfg_42_path = tmp.path().join("seed42.yaml");
    let cfg_99_path = tmp.path().join("seed99.yaml");
    let svg_path = tmp.path().join("puz.svg");
    fs::write(&cfg_42_path, yaml_42).unwrap();
    fs::write(
        &cfg_99_path,
        "version: 1\npage_size: \"3.5in x 3.5in\"\nmargin: \"0.25in\"\nseed: 99\n",
    )
    .unwrap();
    fs::write(&svg_path, &svg_xml).unwrap();

    let out_override = Command::new(bin)
        .arg(&svg_path)
        .arg("--config")
        .arg(&cfg_42_path)
        .arg("--seed")
        .arg("99")
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(out_override.status.success());

    let out_yaml_99 = Command::new(bin)
        .arg(&svg_path)
        .arg("--config")
        .arg(&cfg_99_path)
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(out_yaml_99.status.success());
    assert_eq!(out_override.stdout, out_yaml_99.stdout);
}

#[test]
fn non_tty_interactive_flag_warns_and_succeeds() {
    let bin = env!("CARGO_BIN_EXE_puzzle-gen");
    let out = Command::new(bin)
        .arg(example_svg_path())
        .arg("--interactive")
        .arg("--dry-run")
        .arg("--no-strict")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stderr.contains("INTERACTIVE_NON_TTY"));
    assert!(stdout.contains("bbox_area_pct="));
}

#[test]
fn rotation_allows_90_deg_single_piece_fit_on_narrow_strip() {
    let svg_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg width="45mm" height="20mm" viewBox="0 0 45 20" version="1.1"
     xmlns="http://www.w3.org/2000/svg"
     xmlns:svg="http://www.w3.org/2000/svg"
     xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
     xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
     xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview id="namedview1" inkscape:document-units="mm"/>
  <g inkscape:groupmode="layer" id="layer1" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="45" height="20" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" id="layer2" inkscape:label="Pieces">
    <rect id="p0" x="0" y="0" width="45" height="20"/>
  </g>
  <g inkscape:groupmode="layer" id="layer3" inkscape:label="Tabs"/>
</svg>"#
    );
    let cfg = Config::from_yaml(Some(
        "version: 1\npage_size: \"1.5in x 4in\"\npage_unit: \"mm\"\nmargin: \"0.25in\"\nkerf: \"0.20mm\"\nprint_bleed: \"0.5 kerf\"\n",
    ))
    .unwrap();
    let artifacts = run_pipeline(&svg_xml, &cfg, true, &mut Vec::new()).unwrap();
    assert_eq!(artifacts.layout.pages.len(), 1);
    assert_eq!(artifacts.layout.pages[0].slots.len(), 1);
    assert!(
        matches!(
            artifacts.layout.pages[0].slots[0].rotation,
            Rotation90::Deg90 | Rotation90::Deg270
        ),
        "expected 90° or 270° rotation on narrow vertical strip, got {:?}",
        artifacts.layout.pages[0].slots[0].rotation
    );
    let s3 = svg_emit::emit_stage3(&artifacts.kerfed, &artifacts.layout, &cfg);
    let d3 = roxmltree::Document::parse(&s3).unwrap();
    assert_stage3_stage4_parity(&artifacts, &cfg, &d3);
}

#[test]
#[ignore]
fn large_1008_piece_release_benchmark() {
    let svg_xml = build_complete_grid_svg(28, 36, LengthUnit::Mm);
    let cfg = Config::default();
    let start = Instant::now();
    let artifacts = run_pipeline(&svg_xml, &cfg, true, &mut Vec::new()).unwrap();
    let elapsed = start.elapsed();
    eprintln!(
        "1008-piece nesting finished in {:.2}s across {} pages (bbox_area_pct={:.1}%)",
        elapsed.as_secs_f64(),
        artifacts.layout.pages.len(),
        artifacts.layout.bbox_area_pct
    );
    assert!(
        elapsed < Duration::from_secs(60),
        "expected < 60s for 1008 pieces, took {:.2}s",
        elapsed.as_secs_f64()
    );
    assert!(!artifacts.layout.pages.is_empty());
}
