use std::fmt::Write as _;

use kurbo::{BezPath, PathEl, Point, Rect, Shape, Vec2};
use xmlwriter::{Indent, Options, XmlWriter};

use crate::model::{Config, KerfedGrid, LengthUnit, PagePlan, PaginatedLayout, SplicedPuzzle};
use crate::paginate::compute_slot_bounds;

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
const SODIPODI_NS: &str = "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const LABEL_MARGIN_MM: f64 = 8.0;
const STROKE_WIDTH_MM: f64 = 0.1;

/// Formats a scalar coordinate in `unit` with fixed precision (`6` for `in`/`cm`, `4` for `mm`/`px`).
pub fn fmt_coord(val: f64, unit: LengthUnit) -> String {
    let dp = unit.decimal_places();
    let s = format!("{val:.dp$}");
    if let Some(rest) = s.strip_prefix('-')
        && rest.bytes().all(|b| b == b'0' || b == b'.')
    {
        return rest.to_string();
    }
    s
}

/// Converts canonical millimeters to `unit` and formats with fixed precision.
pub fn fmt_mm(mm: f64, unit: LengthUnit) -> String {
    fmt_coord(unit.from_mm(mm), unit)
}

fn new_svg_writer(
    include_xlink: bool,
    w_mm: f64,
    h_mm: f64,
    vb: Rect,
    unit: LengthUnit,
) -> XmlWriter {
    let mut w = XmlWriter::new(Options {
        use_single_quote: false,
        indent: Indent::Spaces(2),
        attributes_indent: Indent::None,
    });
    w.write_declaration();
    w.start_element("svg");
    w.write_attribute("width", &format!("{}{}", fmt_mm(w_mm, unit), unit.as_str()));
    w.write_attribute(
        "height",
        &format!("{}{}", fmt_mm(h_mm, unit), unit.as_str()),
    );
    w.write_attribute(
        "viewBox",
        &format!(
            "{} {} {} {}",
            fmt_mm(vb.x0, unit),
            fmt_mm(vb.y0, unit),
            fmt_mm(vb.width(), unit),
            fmt_mm(vb.height(), unit)
        ),
    );
    w.write_attribute("version", "1.1");
    w.write_attribute("xmlns", SVG_NS);
    w.write_attribute("xmlns:svg", SVG_NS);
    w.write_attribute("xmlns:inkscape", INKSCAPE_NS);
    w.write_attribute("xmlns:sodipodi", SODIPODI_NS);
    if include_xlink {
        w.write_attribute("xmlns:xlink", XLINK_NS);
    }
    w
}

fn write_namedview(
    w: &mut XmlWriter,
    unit: LengthUnit,
    pages: Option<&[(f64, f64, usize)]>,
    page_w_mm: f64,
    page_h_mm: f64,
) {
    w.start_element("sodipodi:namedview");
    w.write_attribute("id", "namedview1");
    w.write_attribute("inkscape:document-units", unit.as_str());
    if let Some(page_rects) = pages {
        for &(px_mm, py_mm, page_num) in page_rects {
            w.start_element("inkscape:page");
            w.write_attribute("x", &fmt_mm(px_mm, unit));
            w.write_attribute("y", &fmt_mm(py_mm, unit));
            w.write_attribute("width", &fmt_mm(page_w_mm, unit));
            w.write_attribute("height", &fmt_mm(page_h_mm, unit));
            w.write_attribute("id", &format!("page{page_num}"));
            w.end_element();
        }
    }
    w.end_element();
}

fn write_shared_defs(
    w: &mut XmlWriter,
    image_href: &str,
    image_bounds_mm: Rect,
    preserve_aspect_ratio: &str,
    image_rendering: Option<&str>,
    unit: LengthUnit,
) {
    let w_mm = image_bounds_mm.width();
    let h_mm = image_bounds_mm.height();
    let w_str = fmt_mm(w_mm, unit);
    let h_str = fmt_mm(h_mm, unit);
    let two_w_str = fmt_mm(2.0 * w_mm, unit);
    let two_h_str = fmt_mm(2.0 * h_mm, unit);

    w.start_element("defs");
    w.write_attribute("id", "defs1");

    w.start_element("image");
    w.write_attribute("id", "puzzle-image");
    w.write_attribute("x", "0");
    w.write_attribute("y", "0");
    w.write_attribute("width", &w_str);
    w.write_attribute("height", &h_str);
    w.write_attribute("preserveAspectRatio", preserve_aspect_ratio);
    if let Some(rendering) = image_rendering {
        w.write_attribute("style", &format!("image-rendering:{rendering}"));
    }
    w.write_attribute("xlink:href", image_href);
    w.end_element();

    w.start_element("pattern");
    w.write_attribute("id", "puzzle-pattern");
    w.write_attribute("patternUnits", "userSpaceOnUse");
    w.write_attribute("x", &fmt_mm(image_bounds_mm.x0, unit));
    w.write_attribute("y", &fmt_mm(image_bounds_mm.y0, unit));
    w.write_attribute("width", &two_w_str);
    w.write_attribute("height", &two_h_str);

    // Tile (0, 0): unmirrored source image.
    w.start_element("use");
    w.write_attribute("xlink:href", "#puzzle-image");
    w.end_element();

    // Tile (1, 0): horizontal mirror.
    w.start_element("use");
    w.write_attribute("xlink:href", "#puzzle-image");
    w.write_attribute(
        "transform",
        &format!("translate({two_w_str}, 0) scale(-1, 1)"),
    );
    w.end_element();

    // Tile (0, 1): vertical mirror.
    w.start_element("use");
    w.write_attribute("xlink:href", "#puzzle-image");
    w.write_attribute(
        "transform",
        &format!("translate(0, {two_h_str}) scale(1, -1)"),
    );
    w.end_element();

    // Tile (1, 1): horizontal + vertical mirror.
    w.start_element("use");
    w.write_attribute("xlink:href", "#puzzle-image");
    w.write_attribute(
        "transform",
        &format!("translate({two_w_str}, {two_h_str}) scale(-1, -1)"),
    );
    w.end_element();

    w.end_element(); // pattern
    w.end_element(); // defs
}

fn format_bezpath(path: &BezPath, unit: LengthUnit) -> String {
    let mut d = String::new();
    for el in path.elements() {
        if !d.is_empty() {
            d.push(' ');
        }
        match *el {
            PathEl::MoveTo(p) => {
                let _ = write!(d, "M {} {}", fmt_mm(p.x, unit), fmt_mm(p.y, unit));
            }
            PathEl::LineTo(p) => {
                let _ = write!(d, "L {} {}", fmt_mm(p.x, unit), fmt_mm(p.y, unit));
            }
            PathEl::CurveTo(c1, c2, p) => {
                let _ = write!(
                    d,
                    "C {} {} {} {} {} {}",
                    fmt_mm(c1.x, unit),
                    fmt_mm(c1.y, unit),
                    fmt_mm(c2.x, unit),
                    fmt_mm(c2.y, unit),
                    fmt_mm(p.x, unit),
                    fmt_mm(p.y, unit)
                );
            }
            PathEl::QuadTo(c, p) => {
                let _ = write!(
                    d,
                    "Q {} {} {} {}",
                    fmt_mm(c.x, unit),
                    fmt_mm(c.y, unit),
                    fmt_mm(p.x, unit),
                    fmt_mm(p.y, unit)
                );
            }
            PathEl::ClosePath => d.push('Z'),
        }
    }
    d
}

fn format_closed_polygon(pts: &[Point], offset_mm: Vec2, unit: LengthUnit) -> String {
    let mut d = String::new();
    for (idx, &p) in pts.iter().enumerate() {
        let q = p + offset_mm;
        if idx == 0 {
            let _ = write!(d, "M {} {}", fmt_mm(q.x, unit), fmt_mm(q.y, unit));
        } else {
            let _ = write!(d, " L {} {}", fmt_mm(q.x, unit), fmt_mm(q.y, unit));
        }
    }
    d.push_str(" Z");
    d
}

fn format_open_polyline(pts: &[Point], offset_mm: Vec2, unit: LengthUnit) -> String {
    let mut d = String::new();
    for (idx, &p) in pts.iter().enumerate() {
        let q = p + offset_mm;
        if idx == 0 {
            let _ = write!(d, "M {} {}", fmt_mm(q.x, unit), fmt_mm(q.y, unit));
        } else {
            let _ = write!(d, " L {} {}", fmt_mm(q.x, unit), fmt_mm(q.y, unit));
        }
    }
    d
}

fn write_grid_labels_layer(
    w: &mut XmlWriter,
    row_centers_mm: &[f64],
    col_centers_mm: &[f64],
    label_x_mm: f64,
    label_y_mm: f64,
    unit: LengthUnit,
) {
    w.start_element("g");
    w.write_attribute("inkscape:groupmode", "layer");
    w.write_attribute("id", "layer-grid-labels");
    w.write_attribute("inkscape:label", "GridLabels");

    let font_size = fmt_mm(3.5, unit);
    let style = format!(
        "font-family:sans-serif;font-size:{font_size};fill:#000000;text-anchor:middle;dominant-baseline:central"
    );

    w.set_preserve_whitespaces(true);
    for (r, &cy) in row_centers_mm.iter().enumerate() {
        w.start_element("text");
        w.write_attribute("id", &format!("label-row-{r}"));
        w.write_attribute("x", &fmt_mm(label_x_mm, unit));
        w.write_attribute("y", &fmt_mm(cy, unit));
        w.write_attribute("style", &style);
        w.write_text(&r.to_string());
        w.end_element();
    }
    for (c, &cx) in col_centers_mm.iter().enumerate() {
        w.start_element("text");
        w.write_attribute("id", &format!("label-col-{c}"));
        w.write_attribute("x", &fmt_mm(cx, unit));
        w.write_attribute("y", &fmt_mm(label_y_mm, unit));
        w.write_attribute("style", &style);
        w.write_text(&c.to_string());
        w.end_element();
    }
    w.set_preserve_whitespaces(false);

    w.end_element();
}

/// Emits `<prefix>-stage1.svg` in original `doc_unit` with spliced cubic piece contours and `2x2` pattern fill.
pub fn emit_stage1(spliced: &SplicedPuzzle) -> String {
    let unit = spliced.frame.doc_unit;

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut row_sums = vec![0.0_f64; spliced.rows];
    let mut row_counts = vec![0usize; spliced.rows];
    let mut col_sums = vec![0.0_f64; spliced.cols];
    let mut col_counts = vec![0usize; spliced.cols];

    for p in &spliced.pieces {
        let bb = p.path_mm.bounding_box();
        min_x = min_x.min(bb.x0);
        min_y = min_y.min(bb.y0);
        max_x = max_x.max(bb.x1);
        max_y = max_y.max(bb.y1);

        let c = bb.center();
        row_sums[p.row] += c.y;
        row_counts[p.row] += 1;
        col_sums[p.col] += c.x;
        col_counts[p.col] += 1;
    }

    let vb = Rect::new(
        min_x - LABEL_MARGIN_MM,
        min_y - LABEL_MARGIN_MM,
        max_x + LABEL_MARGIN_MM,
        max_y + LABEL_MARGIN_MM,
    );

    let mut w = new_svg_writer(true, vb.width(), vb.height(), vb, unit);
    write_namedview(&mut w, unit, None, 0.0, 0.0);
    write_shared_defs(
        &mut w,
        &spliced.image_href,
        spliced.image_bounds_mm,
        &spliced.image_preserve_aspect_ratio,
        spliced.image_rendering.as_deref(),
        unit,
    );

    let stroke_w = fmt_mm(STROKE_WIDTH_MM, unit);
    let piece_style = format!(
        "fill:url(#puzzle-pattern);stroke:#000000;stroke-width:{stroke_w};fill-rule:nonzero"
    );

    w.start_element("g");
    w.write_attribute("inkscape:groupmode", "layer");
    w.write_attribute("id", "layer-pieces");
    w.write_attribute("inkscape:label", "Pieces");

    for p in &spliced.pieces {
        w.start_element("path");
        w.write_attribute("id", &format!("piece-r{}-c{}", p.row, p.col));
        w.write_attribute("style", &piece_style);
        w.write_attribute("d", &format_bezpath(&p.path_mm, unit));
        w.end_element();
    }
    w.end_element();

    let row_centers: Vec<f64> = row_sums
        .into_iter()
        .zip(row_counts)
        .map(|(s, n)| s / (n.max(1) as f64))
        .collect();
    let col_centers: Vec<f64> = col_sums
        .into_iter()
        .zip(col_counts)
        .map(|(s, n)| s / (n.max(1) as f64))
        .collect();

    write_grid_labels_layer(
        &mut w,
        &row_centers,
        &col_centers,
        min_x - 4.0,
        min_y - 4.0,
        unit,
    );

    w.end_document()
}

/// Emits `<prefix>-stage2.svg` in original `doc_unit` with kerf-offset cut pieces spaced on a uniform AABB grid.
pub fn emit_stage2(kerfed: &KerfedGrid, config: &Config) -> String {
    let unit = kerfed.frame.doc_unit;
    let (slot_w_mm, slot_h_mm) = compute_slot_bounds(kerfed);
    let pitch_w = slot_w_mm + config.piece_spacing_mm;
    let pitch_h = slot_h_mm + config.piece_spacing_mm;

    let grid_w_mm = (kerfed.cols as f64) * slot_w_mm
        + (kerfed.cols.saturating_sub(1) as f64) * config.piece_spacing_mm;
    let grid_h_mm = (kerfed.rows as f64) * slot_h_mm
        + (kerfed.rows.saturating_sub(1) as f64) * config.piece_spacing_mm;
    let total_w_mm = 2.0 * LABEL_MARGIN_MM + grid_w_mm;
    let total_h_mm = 2.0 * LABEL_MARGIN_MM + grid_h_mm;
    let vb = Rect::new(0.0, 0.0, total_w_mm, total_h_mm);

    let mut w = new_svg_writer(true, total_w_mm, total_h_mm, vb, unit);
    write_namedview(&mut w, unit, None, 0.0, 0.0);
    write_shared_defs(
        &mut w,
        &kerfed.image_href,
        kerfed.image_bounds_mm,
        &kerfed.image_preserve_aspect_ratio,
        kerfed.image_rendering.as_deref(),
        unit,
    );

    let stroke_w = fmt_mm(STROKE_WIDTH_MM, unit);
    let piece_style = format!(
        "fill:url(#puzzle-pattern);stroke:#000000;stroke-width:{stroke_w};fill-rule:nonzero"
    );

    w.start_element("g");
    w.write_attribute("inkscape:groupmode", "layer");
    w.write_attribute("id", "layer-pieces");
    w.write_attribute("inkscape:label", "Pieces");

    for p in &kerfed.pieces {
        let cell_cx_mm = LABEL_MARGIN_MM + (p.col as f64) * pitch_w + 0.5 * slot_w_mm;
        let cell_cy_mm = LABEL_MARGIN_MM + (p.row as f64) * pitch_h + 0.5 * slot_h_mm;
        let cut_center = p.cut_aabb_mm.center();
        let dx_mm = cell_cx_mm - cut_center.x;
        let dy_mm = cell_cy_mm - cut_center.y;

        w.start_element("g");
        w.write_attribute(
            "transform",
            &format!(
                "translate({}, {})",
                fmt_mm(dx_mm, unit),
                fmt_mm(dy_mm, unit)
            ),
        );
        w.start_element("path");
        w.write_attribute("id", &format!("piece-r{}-c{}", p.row, p.col));
        w.write_attribute("style", &piece_style);
        w.write_attribute(
            "d",
            &format_closed_polygon(&p.cut_polygon, Vec2::ZERO, unit),
        );
        w.end_element();
        w.end_element();
    }
    w.end_element();

    let row_centers: Vec<f64> = (0..kerfed.rows)
        .map(|r| LABEL_MARGIN_MM + (r as f64) * pitch_h + 0.5 * slot_h_mm)
        .collect();
    let col_centers: Vec<f64> = (0..kerfed.cols)
        .map(|c| LABEL_MARGIN_MM + (c as f64) * pitch_w + 0.5 * slot_w_mm)
        .collect();

    write_grid_labels_layer(&mut w, &row_centers, &col_centers, 4.0, 4.0, unit);

    w.end_document()
}

/// Emits `<prefix>-stage3.svg` in original `doc_unit` with Inkscape `<inkscape:page>` sheets, strokeless bleed fills, and black registration marks.
pub fn emit_stage3(kerfed: &KerfedGrid, layout: &PaginatedLayout, config: &Config) -> String {
    let unit = kerfed.frame.doc_unit;
    let page_w_mm = config.page_width_mm;
    let page_h_mm = config.page_height_mm;
    let vb = Rect::new(0.0, 0.0, page_w_mm, page_h_mm);

    let page_rects: Vec<(f64, f64, usize)> = layout
        .pages
        .iter()
        .map(|p| (((p.page_number - 1) as f64) * page_w_mm, 0.0, p.page_number))
        .collect();

    let mut w = new_svg_writer(true, page_w_mm, page_h_mm, vb, unit);
    write_namedview(&mut w, unit, Some(&page_rects), page_w_mm, page_h_mm);
    write_shared_defs(
        &mut w,
        &kerfed.image_href,
        kerfed.image_bounds_mm,
        &kerfed.image_preserve_aspect_ratio,
        kerfed.image_rendering.as_deref(),
        unit,
    );

    // PrintPieces layer (bleed polygons with pattern fill and stroke:none).
    w.start_element("g");
    w.write_attribute("inkscape:groupmode", "layer");
    w.write_attribute("id", "layer-print-pieces");
    w.write_attribute("inkscape:label", "PrintPieces");

    for page in &layout.pages {
        let page_x_mm = ((page.page_number - 1) as f64) * page_w_mm;
        for slot in &page.slots {
            let piece = &kerfed.pieces[slot.piece_index];
            let tx_mm = page_x_mm + slot.offset_mm.x;
            let ty_mm = slot.offset_mm.y;

            w.start_element("g");
            w.write_attribute(
                "transform",
                &format!(
                    "translate({}, {})",
                    fmt_mm(tx_mm, unit),
                    fmt_mm(ty_mm, unit)
                ),
            );
            w.start_element("path");
            w.write_attribute("id", &format!("piece-r{}-c{}", piece.row, piece.col));
            w.write_attribute(
                "style",
                "fill:url(#puzzle-pattern);stroke:none;fill-rule:nonzero",
            );
            w.write_attribute(
                "d",
                &format_closed_polygon(&piece.bleed_polygon, Vec2::ZERO, unit),
            );
            w.end_element();
            w.end_element();
        }
    }
    w.end_element();

    // PageNumbers layer (#000000 stroked vector digits and L-ticks shifted by (p - 1) * page_w).
    let stroke_w = fmt_mm(STROKE_WIDTH_MM, unit);
    let mark_style = format!(
        "fill:none;stroke:#000000;stroke-width:{stroke_w};stroke-linecap:round;stroke-linejoin:round"
    );

    w.start_element("g");
    w.write_attribute("inkscape:groupmode", "layer");
    w.write_attribute("id", "layer-page-numbers");
    w.write_attribute("inkscape:label", "PageNumbers");

    for page in &layout.pages {
        let page_shift = Vec2::new(((page.page_number - 1) as f64) * page_w_mm, 0.0);
        for (idx, stroke) in page.corner_strokes_mm.iter().enumerate() {
            w.start_element("path");
            w.write_attribute("id", &format!("page{}-mark-{idx}", page.page_number));
            w.write_attribute("style", &mark_style);
            w.write_attribute("d", &format_open_polyline(stroke, page_shift, unit));
            w.end_element();
        }
    }
    w.end_element();

    w.end_document()
}

/// Emits a standalone `<prefix>-stage4-page<N>.svg` in `config.page_unit` without `<defs>`, `<image>`, `<pattern>`, or `transform` attributes.
pub fn emit_stage4_page(kerfed: &KerfedGrid, page: &PagePlan, config: &Config) -> String {
    let unit = config.page_unit;
    let page_w_mm = config.page_width_mm;
    let page_h_mm = config.page_height_mm;
    let vb = Rect::new(0.0, 0.0, page_w_mm, page_h_mm);

    let mut w = new_svg_writer(false, page_w_mm, page_h_mm, vb, unit);
    write_namedview(&mut w, unit, None, 0.0, 0.0);

    let stroke_w = fmt_mm(STROKE_WIDTH_MM, unit);

    // Cut layer: baked page-local cut polygons, #000000, ungrouped.
    let cut_style = format!("fill:none;stroke:#000000;stroke-width:{stroke_w}");
    w.start_element("g");
    w.write_attribute("inkscape:groupmode", "layer");
    w.write_attribute("id", "layer-cut");
    w.write_attribute("inkscape:label", "Cut");

    for slot in &page.slots {
        let piece = &kerfed.pieces[slot.piece_index];
        w.start_element("path");
        w.write_attribute("id", &format!("piece-r{}-c{}", piece.row, piece.col));
        w.write_attribute("style", &cut_style);
        w.write_attribute(
            "d",
            &format_closed_polygon(&piece.cut_polygon, slot.offset_mm, unit),
        );
        w.end_element();
    }
    w.end_element();

    // PageNumbers layer: identical page numbers + L-ticks in config.score_color.
    let mark_style = format!(
        "fill:none;stroke:{};stroke-width:{stroke_w};stroke-linecap:round;stroke-linejoin:round",
        config.score_color
    );
    w.start_element("g");
    w.write_attribute("inkscape:groupmode", "layer");
    w.write_attribute("id", "layer-page-numbers");
    w.write_attribute("inkscape:label", "PageNumbers");

    for (idx, stroke) in page.corner_strokes_mm.iter().enumerate() {
        w.start_element("path");
        w.write_attribute("id", &format!("page{}-mark-{idx}", page.page_number));
        w.write_attribute("style", &mark_style);
        w.write_attribute("d", &format_open_polyline(stroke, Vec2::ZERO, unit));
        w.end_element();
    }
    w.end_element();

    w.end_document()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::ingest_svg;
    use crate::paginate::paginate_pieces;
    use crate::stage1::splice_tabs;
    use crate::stage2::heal_and_offset;

    const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    const INKSCAPE_NS_TEST: &str = "http://www.inkscape.org/namespaces/inkscape";
    const SODIPODI_NS_TEST: &str = "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd";

    fn sample_svg_in() -> String {
        format!(
            r#"<svg width="2in" height="2in" viewBox="0 0 2 2" xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview inkscape:document-units="in"/>
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="2" height="2" style="image-rendering:optimizeSpeed" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r00" x="0" y="0" width="1" height="1"/>
    <rect id="r01" x="1" y="0" width="1" height="1"/>
    <rect id="r10" x="0" y="1" width="1" height="1"/>
    <rect id="r11" x="1" y="1" width="1" height="1"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs">
    <path id="t0" d="M 0.3 1.0 C 0.3 1.2 0.7 1.2 0.7 1.0"/>
    <path id="t1" d="M 1.3 1.0 C 1.3 1.2 1.7 1.2 1.7 1.0"/>
    <path id="t2" d="M 1.0 0.3 C 1.2 0.3 1.2 0.7 1.0 0.7"/>
    <path id="t3" d="M 1.0 1.3 C 1.2 1.3 1.2 1.7 1.0 1.7"/>
  </g>
</svg>"#
        )
    }

    #[test]
    fn emits_valid_stage1_to_stage4_xml_structure() {
        let mut notices = Vec::new();
        let ingested = ingest_svg(&sample_svg_in(), &mut notices).unwrap();
        let spliced = splice_tabs(&ingested, true, &mut notices).unwrap();
        // Force 2 pages (2 pieces per page) to test Stage 3 multi-page shift.
        let cfg = Config {
            page_width_mm: LengthUnit::In.to_mm(3.0),
            page_height_mm: LengthUnit::In.to_mm(2.0),
            margin_mm: LengthUnit::In.to_mm(0.1),
            ..Config::default()
        };
        let kerfed = heal_and_offset(&spliced, &cfg, &mut notices).unwrap();
        let layout = paginate_pieces(&kerfed, &cfg).unwrap();
        assert_eq!(layout.pages.len(), 2);

        let s1 = emit_stage1(&spliced);
        let d1 = roxmltree::Document::parse(&s1).unwrap();
        assert_eq!(
            d1.descendants()
                .find(
                    |n| n.tag_name().name() == "image" && n.attribute("id") == Some("puzzle-image")
                )
                .and_then(|n| n.attribute("style")),
            Some("image-rendering:optimizeSpeed")
        );
        assert_eq!(
            d1.descendants()
                .find(|n| n.tag_name().name() == "namedview"
                    && n.tag_name().namespace() == Some(SODIPODI_NS_TEST))
                .and_then(|n| n.attribute((INKSCAPE_NS_TEST, "document-units"))),
            Some("in")
        );
        assert_eq!(
            d1.descendants()
                .filter(|n| n.tag_name().name() == "use")
                .count(),
            4
        );

        let s2 = emit_stage2(&kerfed, &cfg);
        let d2 = roxmltree::Document::parse(&s2).unwrap();
        assert_eq!(
            d2.descendants()
                .find(
                    |n| n.tag_name().name() == "image" && n.attribute("id") == Some("puzzle-image")
                )
                .and_then(|n| n.attribute("style")),
            Some("image-rendering:optimizeSpeed")
        );
        assert!(
            d2.descendants()
                .any(|n| n.attribute((INKSCAPE_NS_TEST, "label")) == Some("GridLabels"))
        );

        let s3 = emit_stage3(&kerfed, &layout, &cfg);
        let d3 = roxmltree::Document::parse(&s3).unwrap();
        assert_eq!(
            d3.descendants()
                .find(
                    |n| n.tag_name().name() == "image" && n.attribute("id") == Some("puzzle-image")
                )
                .and_then(|n| n.attribute("style")),
            Some("image-rendering:optimizeSpeed")
        );
        let pages: Vec<_> = d3
            .descendants()
            .filter(|n| n.tag_name().name() == "page")
            .collect();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[1].attribute("x"), Some("3.000000"));
        assert!(
            d3.descendants()
                .any(|n| n.attribute((INKSCAPE_NS_TEST, "label")) == Some("PrintPieces"))
        );
        assert!(
            d3.descendants()
                .any(|n| n.attribute((INKSCAPE_NS_TEST, "label")) == Some("PageNumbers"))
        );

        // Verify page 2 marks in Stage 3 are shifted by 3.000000 in relative to page 1.
        let p2_mark = d3
            .descendants()
            .find(|n| n.attribute("id") == Some("page2-mark-0"))
            .unwrap()
            .attribute("d")
            .unwrap();
        assert!(p2_mark.starts_with("M 3.100000"));

        let s4 = emit_stage4_page(&kerfed, &layout.pages[1], &cfg);
        let d4 = roxmltree::Document::parse(&s4).unwrap();
        assert!(!s4.contains("xmlns:xlink"));
        assert!(!s4.contains("<defs"));
        assert!(!s4.contains("<image"));
        assert!(!s4.contains("<pattern"));
        assert!(!d4.descendants().any(|n| n.attribute("transform").is_some()));
        assert!(
            d4.descendants()
                .any(|n| n.attribute((INKSCAPE_NS_TEST, "label")) == Some("Cut"))
        );
        assert!(
            d4.descendants()
                .any(|n| n.attribute((INKSCAPE_NS_TEST, "label")) == Some("PageNumbers"))
        );
    }
}
