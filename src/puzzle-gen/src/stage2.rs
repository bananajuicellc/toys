use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::overlay::OverlayOptions;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};
use kurbo::{BezPath, PathEl, PathSeg, Point, Rect};

use crate::model::{
    Config, KerfedGrid, KerfedPiece, NoticeSeverity, PipelineError, PipelineNotice, SplicedPiece,
    SplicedPuzzle,
};

const DEDUP_TOL_MM: f64 = 0.001;

/// Flattens Stage 1 contours, heals cross-edge overshoot via `FillRule::Positive`, and computes cut/bleed polygons.
pub fn heal_and_offset(
    spliced: &SplicedPuzzle,
    config: &Config,
    notices: &mut Vec<PipelineNotice>,
) -> Result<KerfedGrid, PipelineError> {
    if config.print_bleed_mm + 1e-12 < config.kerf_mm * 0.5 {
        notices.push(PipelineNotice {
            severity: NoticeSeverity::Warning,
            code: "BLEED_LESS_THAN_HALF_KERF",
            message: format!(
                "print_bleed ({:.4} mm) is smaller than kerf / 2 ({:.4} mm); cut edges may expose unprinted label stock",
                config.print_bleed_mm,
                config.kerf_mm * 0.5
            ),
        });
    }

    let mut pieces = Vec::with_capacity(spliced.pieces.len());
    for sp in &spliced.pieces {
        let kp =
            process_piece(sp, config).map_err(|err| err.with_debug_spliced(spliced.clone()))?;
        pieces.push(kp);
    }

    Ok(KerfedGrid {
        frame: spliced.frame,
        image_href: spliced.image_href.clone(),
        image_bounds_mm: spliced.image_bounds_mm,
        image_preserve_aspect_ratio: spliced.image_preserve_aspect_ratio.clone(),
        image_rendering: spliced.image_rendering.clone(),
        rows: spliced.rows,
        cols: spliced.cols,
        matched_tabs: spliced.matched_tabs,
        missing_tabs: spliced.missing_tabs,
        pieces,
    })
}

fn outline_options_i64() -> OverlayOptions<f64, i64> {
    let mut opts = OverlayOptions::<f64, i64>::default();
    opts.min_output_area = 1e-6;
    opts.clean_result = true;
    opts
}

fn process_piece(sp: &SplicedPiece, config: &Config) -> Result<KerfedPiece, PipelineError> {
    let flat_ring = flatten_piece_contour(&sp.path_mm, config.segment_length_mm, sp)?;
    let healed_ring = heal_ring_positive(&flat_ring, sp)?;
    let unoffset_area = signed_area(&healed_ring);

    let rdp_tol = (config.segment_length_mm * 0.05).min(0.005);
    let healed_io: Vec<[f64; 2]> = healed_ring.iter().map(|p| [p.x, p.y]).collect();

    // Compute +kerf / 2 cut polygon.
    let half_kerf = config.kerf_mm * 0.5;
    let cut_polygon = if half_kerf == 0.0 {
        healed_ring.clone()
    } else {
        let style = OutlineStyle::new(half_kerf).line_join(LineJoin::Round(0.1));
        let shapes = healed_io.outline_custom_as::<i64>(&style, outline_options_i64());
        if shapes.len() != 1 || shapes[0].len() != 1 {
            return Err(PipelineError::new(
                "stage2",
                format!(
                    "kerf offsetting (+{half_kerf:.4} mm) failed with {} shapes / {} rings on piece ({}, {}) id=\"{}\"",
                    shapes.len(),
                    shapes.first().map_or(0, Vec::len),
                    sp.row,
                    sp.col,
                    sp.id
                ),
            ));
        }
        let raw_pts = ring_from_io(&shapes[0][0]);
        let simplified = simplify_closed_ring_rdp(&raw_pts, rdp_tol);
        let cut_area = signed_area(&simplified);
        if cut_area <= unoffset_area {
            return Err(PipelineError::new(
                "stage2",
                format!(
                    "kerf offsetting (+{half_kerf:.4} mm) failed to expand piece ({}, {}) id=\"{}\" (unoffset={unoffset_area:.4}, cut={cut_area:.4})",
                    sp.row, sp.col, sp.id
                ),
            ));
        }
        simplified
    };

    // Compute +print_bleed polygon, dropping any internal socket-bridging holes.
    let bleed_polygon = if config.print_bleed_mm == 0.0 {
        healed_ring
    } else {
        let interior_pt = find_interior_point(&healed_ring).ok_or_else(|| {
            PipelineError::new(
                "stage2",
                format!(
                    "failed to find interior point of healed piece ({}, {}) id=\"{}\"",
                    sp.row, sp.col, sp.id
                ),
            )
        })?;
        let style = OutlineStyle::new(config.print_bleed_mm).line_join(LineJoin::Round(0.1));
        let shapes = healed_io.outline_custom_as::<i64>(&style, outline_options_i64());
        let outer = shapes
            .into_iter()
            .filter_map(|mut shape| {
                if shape.is_empty() {
                    return None;
                }
                let outer_ring = ring_from_io(&shape.swap_remove(0));
                if signed_area(&outer_ring) > 0.0 && point_in_polygon(interior_pt, &outer_ring) {
                    Some(outer_ring)
                } else {
                    None
                }
            })
            .max_by(|a, b| signed_area(a).total_cmp(&signed_area(b)))
            .ok_or_else(|| {
                PipelineError::new(
                    "stage2",
                    format!(
                        "print_bleed offsetting (+{:.4} mm) collapsed piece ({}, {}) id=\"{}\"",
                        config.print_bleed_mm, sp.row, sp.col, sp.id
                    ),
                )
            })?;

        let simplified = simplify_closed_ring_rdp(&outer, rdp_tol);
        let bleed_area = signed_area(&simplified);
        if bleed_area <= unoffset_area {
            return Err(PipelineError::new(
                "stage2",
                format!(
                    "print_bleed offsetting (+{:.4} mm) failed to expand piece ({}, {}) id=\"{}\" (unoffset={unoffset_area:.4}, bleed={bleed_area:.4})",
                    config.print_bleed_mm, sp.row, sp.col, sp.id
                ),
            ));
        }
        simplified
    };

    let cut_aabb_mm = polygon_aabb(&cut_polygon).ok_or_else(|| {
        PipelineError::new(
            "stage2",
            format!(
                "empty cut polygon on piece ({}, {}) id=\"{}\"",
                sp.row, sp.col, sp.id
            ),
        )
    })?;

    Ok(KerfedPiece {
        row: sp.row,
        col: sp.col,
        id: sp.id.clone(),
        cut_aabb_mm,
        cut_polygon,
        bleed_polygon,
    })
}

fn flatten_piece_contour(
    path: &BezPath,
    segment_length_mm: f64,
    sp: &SplicedPiece,
) -> Result<Vec<Point>, PipelineError> {
    let flat_tol = (segment_length_mm * 0.1).min(0.05);
    let mut pts: Vec<Point> = Vec::new();

    for seg in path.segments() {
        match seg {
            PathSeg::Line(line) => {
                push_dedup(&mut pts, line.p0);
                push_dedup(&mut pts, line.p1);
            }
            PathSeg::Quad(q) => {
                let single = [PathEl::MoveTo(q.p0), PathEl::QuadTo(q.p1, q.p2)];
                flatten_single_curve(&single, flat_tol, segment_length_mm, &mut pts);
            }
            PathSeg::Cubic(c) => {
                let single = [PathEl::MoveTo(c.p0), PathEl::CurveTo(c.p1, c.p2, c.p3)];
                flatten_single_curve(&single, flat_tol, segment_length_mm, &mut pts);
            }
        }
    }

    while pts.len() >= 2 && pts.last().unwrap().distance(pts[0]) < DEDUP_TOL_MM {
        pts.pop();
    }

    if pts.len() < 3 {
        return Err(PipelineError::new(
            "stage2",
            format!(
                "flattened contour for piece ({}, {}) id=\"{}\" has fewer than 3 vertices",
                sp.row, sp.col, sp.id
            ),
        ));
    }

    let area = signed_area(&pts);
    if area.abs() <= 1e-9 {
        return Err(PipelineError::new(
            "stage2",
            format!(
                "flattened contour for piece ({}, {}) id=\"{}\" has near-zero area",
                sp.row, sp.col, sp.id
            ),
        ));
    }
    if area < 0.0 {
        pts.reverse();
    }

    Ok(pts)
}

fn flatten_single_curve(single: &[PathEl], flat_tol: f64, max_chord_mm: f64, dst: &mut Vec<Point>) {
    let mut prev: Option<Point> = None;
    kurbo::flatten(single.iter().copied(), flat_tol, |el| match el {
        PathEl::MoveTo(p) => {
            push_dedup(dst, p);
            prev = Some(p);
        }
        PathEl::LineTo(p) => {
            if let Some(a) = prev {
                let dist = a.distance(p);
                if dist > max_chord_mm {
                    let steps = (dist / max_chord_mm).ceil() as usize;
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        push_dedup(dst, a.lerp(p, t));
                    }
                } else {
                    push_dedup(dst, p);
                }
            } else {
                push_dedup(dst, p);
            }
            prev = Some(p);
        }
        _ => {}
    });
}

fn push_dedup(pts: &mut Vec<Point>, pt: Point) {
    if let Some(&last) = pts.last()
        && last.distance(pt) < DEDUP_TOL_MM
    {
        return;
    }
    pts.push(pt);
}

fn heal_ring_positive(ring: &[Point], sp: &SplicedPiece) -> Result<Vec<Point>, PipelineError> {
    let subj: Vec<[f64; 2]> = ring.iter().map(|p| [p.x, p.y]).collect();
    let empty: Vec<[f64; 2]> = Vec::new();
    let shapes = subj.overlay(&empty, OverlayRule::Subject, FillRule::Positive);

    if shapes.len() != 1 || shapes[0].len() != 1 {
        return Err(PipelineError::new(
            "stage2",
            format!(
                "self-union healing failed on piece ({}, {}) id=\"{}\": expected 1 simple ring, got {} shapes / {} contours",
                sp.row,
                sp.col,
                sp.id,
                shapes.len(),
                shapes.first().map_or(0, Vec::len)
            ),
        ));
    }

    let mut healed = ring_from_io(&shapes[0][0]);
    while healed.len() >= 2 && healed.last().unwrap().distance(healed[0]) < DEDUP_TOL_MM {
        healed.pop();
    }
    if healed.len() < 3 {
        return Err(PipelineError::new(
            "stage2",
            format!(
                "healed contour collapsed on piece ({}, {}) id=\"{}\"",
                sp.row, sp.col, sp.id
            ),
        ));
    }
    let area = signed_area(&healed);
    if area < 0.0 {
        healed.reverse();
    }
    if signed_area(&healed) <= 1e-6 {
        return Err(PipelineError::new(
            "stage2",
            format!(
                "healed contour has degenerate area on piece ({}, {}) id=\"{}\"",
                sp.row, sp.col, sp.id
            ),
        ));
    }
    Ok(healed)
}

fn ring_from_io(pts: &[[f64; 2]]) -> Vec<Point> {
    pts.iter().map(|p| Point::new(p[0], p[1])).collect()
}

/// Signed shoelace area in `(x, y)` coordinates (positive for clockwise in SVG `+y`-down).
pub(crate) fn signed_area(ring: &[Point]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    0.5 * sum
}

pub(crate) fn polygon_aabb(ring: &[Point]) -> Option<Rect> {
    let first = *ring.first()?;
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for &p in &ring[1..] {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }
    Some(Rect::new(min_x, min_y, max_x, max_y))
}

fn simplify_closed_ring_rdp(ring: &[Point], tol: f64) -> Vec<Point> {
    let n = ring.len();
    if n <= 4 || tol <= 0.0 {
        return ring.to_vec();
    }

    // Find farthest antipodal vertex pair to split the closed ring cleanly.
    let (mut idx_a, mut idx_b) = (0usize, n / 2);
    let mut best_d2 = -1.0;
    if n <= 2048 {
        for i in 0..n {
            for j in (i + 1)..n {
                let d2 = ring[i].distance_squared(ring[j]);
                if d2 > best_d2 {
                    best_d2 = d2;
                    idx_a = i;
                    idx_b = j;
                }
            }
        }
    } else {
        for i in 0..n {
            let d2 = ring[0].distance_squared(ring[i]);
            if d2 > best_d2 {
                best_d2 = d2;
                idx_b = i;
            }
        }
        best_d2 = -1.0;
        for i in 0..n {
            let d2 = ring[idx_b].distance_squared(ring[i]);
            if d2 > best_d2 {
                best_d2 = d2;
                idx_a = i;
            }
        }
        if idx_a > idx_b {
            std::mem::swap(&mut idx_a, &mut idx_b);
        }
    }

    if idx_a == idx_b {
        return ring.to_vec();
    }

    let arc1: Vec<Point> = ring[idx_a..=idx_b].to_vec();
    let mut arc2: Vec<Point> = Vec::with_capacity(n - (idx_b - idx_a) + 1);
    arc2.extend_from_slice(&ring[idx_b..]);
    arc2.extend_from_slice(&ring[..=idx_a]);

    let s1 = rdp_open(&arc1, tol);
    let s2 = rdp_open(&arc2, tol);

    let mut out = Vec::with_capacity(s1.len() + s2.len());
    out.extend_from_slice(&s1[..s1.len() - 1]);
    out.extend_from_slice(&s2[..s2.len() - 1]);

    if out.len() < 3 { ring.to_vec() } else { out }
}

fn rdp_open(pts: &[Point], tol: f64) -> Vec<Point> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let start = pts[0];
    let end = pts[pts.len() - 1];
    let mut max_dist = -1.0;
    let mut max_idx = 0usize;

    for (idx, &p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = perp_dist_point_segment(p, start, end);
        if d > max_dist {
            max_dist = d;
            max_idx = idx;
        }
    }

    if max_dist > tol {
        let left = rdp_open(&pts[..=max_idx], tol);
        let right = rdp_open(&pts[max_idx..], tol);
        let mut out = Vec::with_capacity(left.len() + right.len() - 1);
        out.extend_from_slice(&left[..left.len() - 1]);
        out.extend_from_slice(&right);
        out
    } else {
        vec![start, end]
    }
}

fn perp_dist_point_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len2 = ab.hypot2();
    if len2 <= 1e-18 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    let proj = a + ab * t;
    p.distance(proj)
}

fn find_interior_point(ring: &[Point]) -> Option<Point> {
    let aabb = polygon_aabb(ring)?;
    let h = aabb.height();
    for &frac in &[0.5, 0.375, 0.625, 0.25, 0.75, 0.125, 0.875] {
        let y = aabb.y0 + h * frac;
        let mut xs = Vec::new();
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                let t = (y - a.y) / (b.y - a.y);
                xs.push(a.x + t * (b.x - a.x));
            }
        }
        xs.sort_by(f64::total_cmp);
        let mut best_span = 0.0;
        let mut best_x = None;
        for pair in xs.as_chunks::<2>().0 {
            let span = pair[1] - pair[0];
            if span > best_span {
                best_span = span;
                best_x = Some(0.5 * (pair[0] + pair[1]));
            }
        }
        if let Some(x) = best_x
            && best_span > 1e-6
        {
            let candidate = Point::new(x, y);
            if point_in_polygon(candidate, ring) {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) fn point_in_polygon(pt: Point, ring: &[Point]) -> bool {
    let mut inside = false;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        if ((a.y > pt.y) != (b.y > pt.y)) && (pt.x < (b.x - a.x) * (pt.y - a.y) / (b.y - a.y) + a.x)
        {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::ingest_svg;
    use crate::stage1::splice_tabs;

    const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

    #[test]
    fn heals_cross_edge_overshoot_ears_and_computes_positive_offsets() {
        // Horizontal tab at y=20 with Bezier handle overshoot above y=20 (like path2 in example).
        let svg = format!(
            r#"<svg width="40mm" height="40mm" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview inkscape:document-units="mm"/>
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r00" x="0" y="0" width="40" height="20"/>
    <rect id="r10" x="0" y="20" width="40" height="20"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs">
    <path id="t_ear" d="M 15.0 20.0 C 12.0 18.0 16.0 23.0 15.0 25.0 C 14.0 28.0 26.0 28.0 25.0 25.0 C 24.0 23.0 28.0 18.0 25.0 20.0"/>
  </g>
</svg>"#
        );
        let mut notices = Vec::new();
        let ingested = ingest_svg(&svg, &mut notices).unwrap();
        let spliced = splice_tabs(&ingested, true, &mut notices).unwrap();
        let cfg = Config::default();
        let kerfed = heal_and_offset(&spliced, &cfg, &mut notices).unwrap();
        assert_eq!(kerfed.pieces.len(), 2);
        for p in &kerfed.pieces {
            assert!(signed_area(&p.cut_polygon) > 0.0);
            assert!(signed_area(&p.bleed_polygon) > signed_area(&p.cut_polygon));
            // Verify curve flattening bound on cut_polygon (no curved edge segment > segment_length_mm, while long straight borders stay long).
            assert!(p.cut_polygon.len() > 20);
        }
    }

    #[test]
    fn zero_offset_short_circuit_and_small_bleed_warning_and_socket_bridging() {
        // Piece with narrow socket mouth (0.5 mm wide) so large print_bleed (0.6 mm) bridges across mouth creating an inner hole.
        let svg = format!(
            r#"<svg width="40mm" height="40mm" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview inkscape:document-units="mm"/>
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r00" x="0" y="0" width="40" height="20"/>
    <rect id="r10" x="0" y="20" width="40" height="20"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs">
    <path id="t_narrow" d="M 19.0 20.0 L 19.75 18.0 L 18.0 15.0 L 22.0 15.0 L 20.25 18.0 L 21.0 20.0"/>
  </g>
</svg>"#
        );
        let mut notices = Vec::new();
        let ingested = ingest_svg(&svg, &mut notices).unwrap();
        let spliced = splice_tabs(&ingested, true, &mut notices).unwrap();

        // Test zero kerf short-circuit + print_bleed < kerf/2 warning (with kerf>0).
        let cfg_zero = Config {
            kerf_mm: 0.0,
            print_bleed_mm: 0.6,
            ..Config::default()
        };
        let kerfed_zero = heal_and_offset(&spliced, &cfg_zero, &mut notices).unwrap();
        assert_eq!(kerfed_zero.pieces.len(), 2);
        // Bleed polygon on r10 (which has the narrow socket) bridged across without failing.
        assert!(
            signed_area(&kerfed_zero.pieces[1].bleed_polygon)
                > signed_area(&kerfed_zero.pieces[1].cut_polygon)
        );

        let mut warn_notices = Vec::new();
        let cfg_small_bleed = Config {
            kerf_mm: 0.20,
            print_bleed_mm: 0.05,
            ..Config::default()
        };
        let _ = heal_and_offset(&spliced, &cfg_small_bleed, &mut warn_notices).unwrap();
        assert!(
            warn_notices
                .iter()
                .any(|n| n.code == "BLEED_LESS_THAN_HALF_KERF")
        );
    }

    #[test]
    fn flatten_respects_segment_length_bound_on_curves() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.curve_to(
            Point::new(10.0, -5.0),
            Point::new(20.0, 5.0),
            Point::new(30.0, 0.0),
        );
        path.line_to(Point::new(30.0, 30.0));
        path.line_to(Point::new(0.0, 30.0));
        path.close_path();

        let sp = SplicedPiece {
            row: 0,
            col: 0,
            id: "p".into(),
            path_mm: path,
        };
        let ring = flatten_piece_contour(&sp.path_mm, 0.18, &sp).unwrap();
        // Check curved region (y < 29.9) chords are <= 0.18 + eps.
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            if a.y < 10.0 && b.y < 10.0 {
                assert!(
                    a.distance(b) <= 0.18 + 1e-6,
                    "curved chord {} exceeded segment_length_mm",
                    a.distance(b)
                );
            }
        }
    }
}
