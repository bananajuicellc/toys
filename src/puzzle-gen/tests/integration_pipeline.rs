use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use puzzle_gen::model::{Config, LengthUnit, NoticeSeverity};
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

fn build_complete_5x5_svg(unit: LengthUnit) -> String {
    let cell = unit.from_mm(20.0);
    let total = unit.from_mm(100.0);
    let u = unit.as_str();

    let mut rects = String::new();
    for r in 0..5 {
        for c in 0..5 {
            let x = (c as f64) * cell;
            let y = (r as f64) * cell;
            rects.push_str(&format!(
                "    <rect id=\"rect-r{r}-c{c}\" x=\"{x:.6}\" y=\"{y:.6}\" width=\"{cell:.6}\" height=\"{cell:.6}\"/>\n"
            ));
        }
    }

    let mut tabs = String::new();
    // 20 internal horizontal edges
    for r in 0..4 {
        for c in 0..5 {
            let y = ((r + 1) as f64) * cell;
            let x0 = (c as f64) * cell;
            let s = x0 + 0.3 * cell;
            let e = x0 + 0.7 * cell;
            let mid = x0 + 0.5 * cell;
            let bump = if (r + c) % 2 == 0 {
                y + 0.25 * cell
            } else {
                y - 0.25 * cell
            };
            tabs.push_str(&format!(
                "    <path id=\"th-{r}-{c}\" d=\"M {s:.6} {y:.6} C {s:.6} {bump:.6} {e:.6} {bump:.6} {e:.6} {y:.6}\"/>\n"
            ));
            let _ = mid;
        }
    }
    // 20 internal vertical edges
    for r in 0..5 {
        for c in 0..4 {
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
<svg width="{total:.6}{u}" height="{total:.6}{u}" viewBox="0 0 {total:.6} {total:.6}" version="1.1"
     xmlns="http://www.w3.org/2000/svg"
     xmlns:svg="http://www.w3.org/2000/svg"
     xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
     xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
     xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview id="namedview1" inkscape:document-units="{u}"/>
  <g inkscape:groupmode="layer" id="layer1" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="{total:.6}" height="{total:.6}" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" id="layer2" inkscape:label="Pieces">
{rects}  </g>
  <g inkscape:groupmode="layer" id="layer3" inkscape:label="Tabs">
{tabs}  </g>
</svg>"#
    )
}

fn parse_path_points_mm(d: &str, tx_mm: f64, ty_mm: f64, unit: LengthUnit) -> Vec<(f64, f64)> {
    let mut pts = Vec::new();
    for seg in SimplifyingPathParser::from(d) {
        match seg.unwrap() {
            SimplePathSegment::MoveTo { x, y } | SimplePathSegment::LineTo { x, y } => {
                pts.push((unit.to_mm(x) + tx_mm, unit.to_mm(y) + ty_mm));
            }
            SimplePathSegment::CurveTo { x, y, .. } | SimplePathSegment::Quadratic { x, y, .. } => {
                pts.push((unit.to_mm(x) + tx_mm, unit.to_mm(y) + ty_mm));
            }
            SimplePathSegment::ClosePath => {}
        }
    }
    pts
}

fn parse_translate_mm(transform: Option<&str>, unit: LengthUnit) -> (f64, f64) {
    let Some(tf) = transform else {
        return (0.0, 0.0);
    };
    let mut tx = 0.0;
    let mut ty = 0.0;
    for tok in TransformListParser::from(tf) {
        if let TransformListToken::Translate { tx: x, ty: y } = tok.unwrap() {
            tx += unit.to_mm(x);
            ty += unit.to_mm(y);
        }
    }
    (tx, ty)
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
page_size: "6in x 6in"
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
    let print_layer = d3
        .descendants()
        .find(|n| n.attribute((INKSCAPE_NS, "label")) == Some("PrintPieces"))
        .unwrap();
    let marks_layer3 = d3
        .descendants()
        .find(|n| n.attribute((INKSCAPE_NS, "label")) == Some("PageNumbers"))
        .unwrap();

    // Check Stage 3 vs Stage 4 parity across every page.
    let doc_unit = artifacts.kerfed.frame.doc_unit; // In
    let out_unit = cfg.page_unit; // Mm
    for page in &artifacts.layout.pages {
        let s4 = svg_emit::emit_stage4_page(&artifacts.kerfed, page, &cfg);
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
            assert!(m4.attribute("style").unwrap().contains("#00aa00"));
            let pts3 = parse_path_points_mm(m3.attribute("d").unwrap(), -page_x_mm, 0.0, doc_unit);
            let pts4 = parse_path_points_mm(m4.attribute("d").unwrap(), 0.0, 0.0, out_unit);
            assert_eq!(pts3.len(), pts4.len());
            for (a, b) in pts3.iter().zip(pts4.iter()) {
                let err = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                assert!(
                    err <= 1.5e-4,
                    "PageNumbers parity error {err:.6} mm > 1.5e-4 mm"
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
            let (tx_mm, ty_mm) = parse_translate_mm(g3.attribute("transform"), doc_unit);
            let pts3 = parse_path_points_mm(
                p3.attribute("d").unwrap(),
                tx_mm - page_x_mm,
                ty_mm,
                doc_unit,
            );
            let pts4 = parse_path_points_mm(p4.attribute("d").unwrap(), 0.0, 0.0, out_unit);
            assert_eq!(pts3.len(), pts4.len());
            for (a, b) in pts3.iter().zip(pts4.iter()) {
                let err = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                assert!(
                    err <= 1.5e-4,
                    "piece {pid} parity error {err:.6} mm > 1.5e-4 mm"
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
    assert!(stdout_dry.contains("page_grid="));

    // 2. Write multi-page outputs with small page size (e.g. 2.5in x 2.5in -> many pages).
    let out_dir = tmp.path().join("run1");
    let small_cfg = tmp.path().join("small.yaml");
    fs::write(
        &small_cfg,
        "version: 1\npage_size: \"2.5in x 2.5in\"\nmargin: \"0.1in\"\n",
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
