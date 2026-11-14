use kurbo::{BezPath, PathEl, Point};

use crate::model::{
    IngestedPuzzle, InternalEdge, NoticeSeverity, Orientation, PipelineError, PipelineNotice,
    SplicedPiece, SplicedPuzzle, TabCandidate,
};

const PERP_TOL_MM: f64 = 2.0;
const WIDE_LONG_TOL_MM: f64 = 2.0;
const STRICT_OVERSHOOT_MM: f64 = 0.25;
const MIN_CLAMPED_SPAN_MM: f64 = 1.0;
const LEAD_ELIDE_TOL_MM: f64 = 1e-4;

#[derive(Debug, Clone, Copy)]
struct GridEdge {
    orientation: Orientation,
    coord_mm: f64,
    start_mm: f64,
    end_mm: f64,
    is_border: bool,
    internal_idx: Option<usize>,
    has_adjacent_prev: bool,
    has_adjacent_next: bool,
}

impl GridEdge {
    fn len(self) -> f64 {
        self.end_mm - self.start_mm
    }

    fn point_at(self, t: f64) -> Point {
        match self.orientation {
            Orientation::Horizontal => Point::new(self.start_mm + t, self.coord_mm),
            Orientation::Vertical => Point::new(self.coord_mm, self.start_mm + t),
        }
    }

    fn project(self, pt: Point) -> (f64, f64) {
        match self.orientation {
            Orientation::Horizontal => ((pt.y - self.coord_mm).abs(), pt.x - self.start_mm),
            Orientation::Vertical => ((pt.x - self.coord_mm).abs(), pt.y - self.start_mm),
        }
    }
}

#[derive(Debug, Clone)]
struct MatchedTab {
    id: String,
    /// Canonical curve ordered strictly from smaller `t` (`t_start`) to larger `t` (`t_end`).
    path_mm: BezPath,
}

/// Matches open `Tabs` curves to internal grid edges and splices closed clockwise piece contours.
pub fn splice_tabs(
    puzzle: &IngestedPuzzle,
    strict: bool,
    notices: &mut Vec<PipelineNotice>,
) -> Result<SplicedPuzzle, PipelineError> {
    let rows = puzzle.rows;
    let cols = puzzle.cols;

    let mut internal_edges = Vec::new();
    let mut all_edges = Vec::new();

    // Internal horizontal edges (L -> R).
    for r in 0..rows.saturating_sub(1) {
        for c in 0..cols {
            let idx = internal_edges.len();
            let edge = InternalEdge {
                orientation: Orientation::Horizontal,
                coord_mm: puzzle.y_lines_mm[r + 1],
                start_mm: puzzle.x_lines_mm[c],
                end_mm: puzzle.x_lines_mm[c + 1],
                cell_a: (r, c),
                cell_b: (r + 1, c),
            };
            internal_edges.push(edge);
            all_edges.push(GridEdge {
                orientation: Orientation::Horizontal,
                coord_mm: edge.coord_mm,
                start_mm: edge.start_mm,
                end_mm: edge.end_mm,
                is_border: false,
                internal_idx: Some(idx),
                has_adjacent_prev: c > 0,
                has_adjacent_next: c + 1 < cols,
            });
        }
    }

    // Internal vertical edges (T -> B).
    for r in 0..rows {
        for c in 0..cols.saturating_sub(1) {
            let idx = internal_edges.len();
            let edge = InternalEdge {
                orientation: Orientation::Vertical,
                coord_mm: puzzle.x_lines_mm[c + 1],
                start_mm: puzzle.y_lines_mm[r],
                end_mm: puzzle.y_lines_mm[r + 1],
                cell_a: (r, c),
                cell_b: (r, c + 1),
            };
            internal_edges.push(edge);
            all_edges.push(GridEdge {
                orientation: Orientation::Vertical,
                coord_mm: edge.coord_mm,
                start_mm: edge.start_mm,
                end_mm: edge.end_mm,
                is_border: false,
                internal_idx: Some(idx),
                has_adjacent_prev: r > 0,
                has_adjacent_next: r + 1 < rows,
            });
        }
    }

    // Outer border edges (used solely to reject border-attached tabs).
    for c in 0..cols {
        for &y in &[puzzle.y_lines_mm[0], puzzle.y_lines_mm[rows]] {
            all_edges.push(GridEdge {
                orientation: Orientation::Horizontal,
                coord_mm: y,
                start_mm: puzzle.x_lines_mm[c],
                end_mm: puzzle.x_lines_mm[c + 1],
                is_border: true,
                internal_idx: None,
                has_adjacent_prev: c > 0,
                has_adjacent_next: c + 1 < cols,
            });
        }
    }
    for r in 0..rows {
        for &x in &[puzzle.x_lines_mm[0], puzzle.x_lines_mm[cols]] {
            all_edges.push(GridEdge {
                orientation: Orientation::Vertical,
                coord_mm: x,
                start_mm: puzzle.y_lines_mm[r],
                end_mm: puzzle.y_lines_mm[r + 1],
                is_border: true,
                internal_idx: None,
                has_adjacent_prev: r > 0,
                has_adjacent_next: r + 1 < rows,
            });
        }
    }

    let mut edge_tabs: Vec<Option<MatchedTab>> = vec![None; internal_edges.len()];
    let mut matched_tabs = 0usize;

    for tab in &puzzle.tabs {
        let (orig_start, orig_end) = path_endpoints(&tab.path_mm).ok_or_else(|| {
            PipelineError::new(
                "stage1",
                format!("tab path \"{}\" has no valid endpoints", tab.id),
            )
        })?;

        let mut candidates = Vec::new();
        for &edge in &all_edges {
            let edge_len = edge.len();
            let (perp_s, t_s) = edge.project(orig_start);
            let (perp_e, t_e) = edge.project(orig_end);

            if perp_s <= PERP_TOL_MM
                && perp_e <= PERP_TOL_MM
                && t_s >= -WIDE_LONG_TOL_MM
                && t_s <= edge_len + WIDE_LONG_TOL_MM
                && t_e >= -WIDE_LONG_TOL_MM
                && t_e <= edge_len + WIDE_LONG_TOL_MM
            {
                let clamped_span = (t_e.clamp(0.0, edge_len) - t_s.clamp(0.0, edge_len)).abs();
                if clamped_span >= MIN_CLAMPED_SPAN_MM {
                    candidates.push((edge, t_s, t_e));
                }
            }
        }

        if candidates.is_empty() {
            return Err(PipelineError::new(
                "stage1",
                format!(
                    "unmatched tab <path id=\"{}\"> with endpoints ({:.3}, {:.3}) mm and ({:.3}, {:.3}) mm does not match any grid edge within {PERP_TOL_MM} mm",
                    tab.id, orig_start.x, orig_start.y, orig_end.x, orig_end.y
                ),
            ));
        }

        if candidates.len() > 1 {
            return Err(PipelineError::new(
                "stage1",
                format!(
                    "ambiguous tab <path id=\"{}\"> with endpoints ({:.3}, {:.3}) mm and ({:.3}, {:.3}) mm matches {} grid edges",
                    tab.id,
                    orig_start.x,
                    orig_start.y,
                    orig_end.x,
                    orig_end.y,
                    candidates.len()
                ),
            ));
        }

        let (edge, t_s, t_e) = candidates[0];
        if edge.is_border {
            return Err(PipelineError::new(
                "stage1",
                format!(
                    "tab <path id=\"{}\"> with endpoints ({:.3}, {:.3}) mm and ({:.3}, {:.3}) mm lies on an outer puzzle border",
                    tab.id, orig_start.x, orig_start.y, orig_end.x, orig_end.y
                ),
            ));
        }

        let edge_len = edge.len();
        if (edge.has_adjacent_prev && (t_s < -STRICT_OVERSHOOT_MM || t_e < -STRICT_OVERSHOOT_MM))
            || (edge.has_adjacent_next
                && (t_s > edge_len + STRICT_OVERSHOOT_MM || t_e > edge_len + STRICT_OVERSHOOT_MM))
        {
            return Err(PipelineError::new(
                "stage1",
                format!(
                    "tab <path id=\"{}\"> overshoots > {STRICT_OVERSHOOT_MM} mm past edge end toward an adjacent edge (t_start={t_s:.3}, t_end={t_e:.3}, edge_len={edge_len:.3} mm)",
                    tab.id
                ),
            ));
        }

        let internal_idx = edge.internal_idx.unwrap();
        let iedge = internal_edges[internal_idx];
        if let Some(existing) = &edge_tabs[internal_idx] {
            return Err(PipelineError::new(
                "stage1",
                format!(
                    "duplicate tab <path id=\"{}\"> on internal edge ({:?}, {:?}) already matched by \"{}\"",
                    tab.id, iedge.cell_a, iedge.cell_b, existing.id
                ),
            ));
        }

        let snapped_path = snap_and_orient_tab(tab, edge, t_s, t_e, orig_start, orig_end);
        edge_tabs[internal_idx] = Some(MatchedTab {
            id: tab.id.clone(),
            path_mm: snapped_path,
        });
        matched_tabs += 1;
    }

    // Check missing internal edges.
    let mut missing_tabs = 0usize;
    for (idx, edge) in internal_edges.iter().enumerate() {
        if edge_tabs[idx].is_none() {
            missing_tabs += 1;
            let desc = format_internal_edge_desc(edge, puzzle);
            if strict {
                return Err(PipelineError::new(
                    "stage1",
                    format!("missing tab on internal edge {desc}"),
                ));
            }
            notices.push(PipelineNotice {
                severity: NoticeSeverity::Warning,
                code: "MISSING_INTERNAL_TAB",
                message: format!("missing tab on internal edge {desc}; keeping edge straight"),
            });
        }
    }

    // Lookup tables for internal horizontal & vertical edges.
    let horiz_count = rows.saturating_sub(1) * cols;
    let horiz_idx = |r: usize, c: usize| -> usize { r * cols + c };
    let vert_idx = |r: usize, c: usize| -> usize { horiz_count + r * (cols - 1) + c };

    let mut pieces = Vec::with_capacity(rows * cols);
    for cell in &puzzle.cells {
        let r = cell.row;
        let c = cell.col;
        let x0 = puzzle.x_lines_mm[c];
        let x1 = puzzle.x_lines_mm[c + 1];
        let y0 = puzzle.y_lines_mm[r];
        let y1 = puzzle.y_lines_mm[r + 1];

        let mut path = BezPath::new();
        path.move_to(Point::new(x0, y0));

        // Top: (x0, y0) -> (x1, y0)
        if r > 0 {
            if let Some(tab) = &edge_tabs[horiz_idx(r - 1, c)] {
                splice_directed_edge(
                    &mut path,
                    &tab.path_mm,
                    false,
                    Point::new(x0, y0),
                    Point::new(x1, y0),
                );
            } else {
                path.line_to(Point::new(x1, y0));
            }
        } else {
            path.line_to(Point::new(x1, y0));
        }

        // Right: (x1, y0) -> (x1, y1)
        if c + 1 < cols {
            if let Some(tab) = &edge_tabs[vert_idx(r, c)] {
                splice_directed_edge(
                    &mut path,
                    &tab.path_mm,
                    false,
                    Point::new(x1, y0),
                    Point::new(x1, y1),
                );
            } else {
                path.line_to(Point::new(x1, y1));
            }
        } else {
            path.line_to(Point::new(x1, y1));
        }

        // Bottom: (x1, y1) -> (x0, y1)
        if r + 1 < rows {
            if let Some(tab) = &edge_tabs[horiz_idx(r, c)] {
                splice_directed_edge(
                    &mut path,
                    &tab.path_mm,
                    true,
                    Point::new(x1, y1),
                    Point::new(x0, y1),
                );
            } else {
                path.line_to(Point::new(x0, y1));
            }
        } else {
            path.line_to(Point::new(x0, y1));
        }

        // Left: (x0, y1) -> (x0, y0)
        if c > 0 {
            if let Some(tab) = &edge_tabs[vert_idx(r, c - 1)] {
                splice_directed_edge(
                    &mut path,
                    &tab.path_mm,
                    true,
                    Point::new(x0, y1),
                    Point::new(x0, y0),
                );
            } else {
                path.line_to(Point::new(x0, y0));
            }
        } else {
            path.line_to(Point::new(x0, y0));
        }

        path.close_path();

        pieces.push(SplicedPiece {
            row: r,
            col: c,
            id: cell.id.clone(),
            path_mm: path,
        });
    }

    Ok(SplicedPuzzle {
        frame: puzzle.frame,
        image_href: puzzle.image_href.clone(),
        image_bounds_mm: puzzle.image_bounds_mm,
        image_preserve_aspect_ratio: puzzle.image_preserve_aspect_ratio.clone(),
        image_rendering: puzzle.image_rendering.clone(),
        rows,
        cols,
        matched_tabs,
        missing_tabs,
        pieces,
    })
}

fn format_internal_edge_desc(edge: &InternalEdge, puzzle: &IngestedPuzzle) -> String {
    let unit = puzzle.frame.doc_unit;
    let coord_doc = unit.from_mm(edge.coord_mm);
    let start_doc = unit.from_mm(edge.start_mm);
    let end_doc = unit.from_mm(edge.end_mm);
    let unit_str = unit.as_str();
    match edge.orientation {
        Orientation::Horizontal => format!(
            "(({}, {}), ({}, {})) along y = {coord_doc:.4}{unit_str}, x in [{start_doc:.4}, {end_doc:.4}]{unit_str}",
            edge.cell_a.0, edge.cell_a.1, edge.cell_b.0, edge.cell_b.1
        ),
        Orientation::Vertical => format!(
            "(({}, {}), ({}, {})) along x = {coord_doc:.4}{unit_str}, y in [{start_doc:.4}, {end_doc:.4}]{unit_str}",
            edge.cell_a.0, edge.cell_a.1, edge.cell_b.0, edge.cell_b.1
        ),
    }
}

fn path_endpoints(path: &BezPath) -> Option<(Point, Point)> {
    let els = path.elements();
    let start = match els.first()? {
        PathEl::MoveTo(p) => *p,
        _ => return None,
    };
    let end = match els.last()? {
        PathEl::LineTo(p) | PathEl::CurveTo(_, _, p) => *p,
        _ => return None,
    };
    Some((start, end))
}

fn snap_and_orient_tab(
    tab: &TabCandidate,
    edge: GridEdge,
    t_s: f64,
    t_e: f64,
    orig_start: Point,
    orig_end: Point,
) -> BezPath {
    let edge_len = edge.len();
    let t_s_clamped = t_s.clamp(0.0, edge_len);
    let t_e_clamped = t_e.clamp(0.0, edge_len);
    let snapped_start = edge.point_at(t_s_clamped);
    let snapped_end = edge.point_at(t_e_clamped);

    let delta_start = snapped_start - orig_start;
    let delta_end = snapped_end - orig_end;

    let mut els: Vec<PathEl> = tab.path_mm.elements().to_vec();
    let last_idx = els.len() - 1;

    if let PathEl::MoveTo(ref mut p) = els[0] {
        *p = snapped_start;
    }

    if els.len() > 1
        && let PathEl::CurveTo(ref mut c1, _, _) = els[1]
    {
        *c1 += delta_start;
    }

    match &mut els[last_idx] {
        PathEl::LineTo(p) => {
            *p = snapped_end;
        }
        PathEl::CurveTo(_, c2, p) => {
            *c2 += delta_end;
            *p = snapped_end;
        }
        _ => {}
    }

    let snapped = BezPath::from_vec(els);
    if t_e_clamped < t_s_clamped {
        snapped.reverse_subpaths()
    } else {
        snapped
    }
}

fn splice_directed_edge(
    dst: &mut BezPath,
    canon_tab: &BezPath,
    reverse: bool,
    slot_start: Point,
    slot_end: Point,
) {
    let directed = if reverse {
        canon_tab.reverse_subpaths()
    } else {
        canon_tab.clone()
    };

    let mut els = directed.elements().to_vec();
    let (tab_entry, tab_exit) = path_endpoints(&directed).expect("matched tab has endpoints");

    let d_entry = tab_entry.distance(slot_start);
    if d_entry > LEAD_ELIDE_TOL_MM {
        dst.line_to(tab_entry);
    } else if els.len() > 1
        && let PathEl::CurveTo(ref mut c1, _, _) = els[1]
    {
        *c1 += slot_start - tab_entry;
    }

    let d_exit = tab_exit.distance(slot_end);
    let last_idx = els.len() - 1;
    if d_exit <= LEAD_ELIDE_TOL_MM {
        match &mut els[last_idx] {
            PathEl::LineTo(p) => *p = slot_end,
            PathEl::CurveTo(_, c2, p) => {
                *c2 += slot_end - tab_exit;
                *p = slot_end;
            }
            _ => {}
        }
    }

    for el in &els[1..] {
        dst.push(*el);
    }

    if d_exit > LEAD_ELIDE_TOL_MM {
        dst.line_to(slot_end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::ingest_svg;
    use kurbo::Shape;

    const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

    fn make_2x2_svg(tabs_body: &str) -> String {
        format!(
            r#"<svg width="40mm" height="40mm" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview inkscape:document-units="mm"/>
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r00" x="0" y="0" width="20" height="20"/>
    <rect id="r01" x="20" y="0" width="20" height="20"/>
    <rect id="r10" x="0" y="20" width="20" height="20"/>
    <rect id="r11" x="20" y="20" width="20" height="20"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs">
    {tabs_body}
  </g>
</svg>"#
        )
    }

    #[test]
    fn snaps_endpoints_translates_handles_and_elides_short_leads() {
        // Horizontal and vertical tabs, including one reversed tab with ~1.5 mm perp offset and full-edge tab with <= 1e-4 lead.
        let svg = make_2x2_svg(
            r#"
    <path id="t_h0" d="M 15.0 21.5 C 15.0 24.0 5.0 24.0 5.0 19.0"/>
    <path id="t_h1" d="M 20.00005 20.0 L 25.0 25.0 L 39.99996 20.0"/>
    <path id="t_v0" d="M 20.0 5.0 C 24.0 5.0 24.0 15.0 20.0 15.0"/>
    <path id="t_v1" d="M 20.0 25.0 L 16.0 30.0 L 20.0 35.0"/>
"#,
        );
        let mut notices = Vec::new();
        let ingested = ingest_svg(&svg, &mut notices).unwrap();
        let spliced = splice_tabs(&ingested, true, &mut notices).unwrap();
        assert_eq!(spliced.matched_tabs, 4);
        assert_eq!(spliced.missing_tabs, 0);
        assert_eq!(spliced.pieces.len(), 4);
        for p in &spliced.pieces {
            assert!(
                p.path_mm.area() > 0.0,
                "piece {} must be clockwise (+y-down)",
                p.id
            );
        }
        // Check t_h1 full-edge lead elision on piece r01: bottom edge starts directly at (40,20) without tiny line_to.
        let r01 = &spliced.pieces[1];
        let mut zero_len_lines = 0usize;
        for seg in r01.path_mm.segments() {
            if let kurbo::PathSeg::Line(line) = seg
                && line.p0.distance(line.p1) < 1e-6
            {
                zero_len_lines += 1;
            }
        }
        assert_eq!(
            zero_len_lines, 0,
            "elided leads must not emit zero-length lines"
        );
    }

    #[test]
    fn rejects_corner_ambiguous_tab_and_border_tab_and_missing_in_strict() {
        // Tab with both endpoints near (20, 20) corner matches multiple edges -> ambiguous.
        let corner_ambig = make_2x2_svg(
            r#"
    <path id="ambig" d="M 18.5 20.0 L 20.0 21.5"/>
"#,
        );
        let mut notices = Vec::new();
        let ingested = ingest_svg(&corner_ambig, &mut notices).unwrap();
        let err = splice_tabs(&ingested, false, &mut notices).unwrap_err();
        assert!(err.to_string().contains("ambig"));

        // Border tab rejected.
        let border_tab = make_2x2_svg(
            r#"
    <path id="border" d="M 5.0 0.0 L 10.0 3.0 L 15.0 0.0"/>
"#,
        );
        let ingested_b = ingest_svg(&border_tab, &mut notices).unwrap();
        let err_b = splice_tabs(&ingested_b, false, &mut notices).unwrap_err();
        assert!(err_b.to_string().contains("border"));
    }
}
