use std::sync::atomic::AtomicBool;

use kurbo::{Point, Rect};

use crate::model::{
    Config, CornerAnchor, KerfedGrid, NestProgress, PagePlan, PageSlot, PaginateOptions,
    PaginatedLayout, PipelineError,
};
use crate::nest;
use crate::stage2::{perp_dist_point_segment, point_in_polygon, polygon_aabb};

pub const CORNER_KEEP_OUT_H_MM: f64 = 6.0;
pub(crate) const CONTAINMENT_TOL_MM: f64 = 1e-4;

type PolylineGlyph = &'static [&'static [(f64, f64)]];

const DIGIT_0: PolylineGlyph = &[&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)]];
const DIGIT_1: PolylineGlyph = &[
    &[(0.2, 0.25), (0.5, 0.0), (0.5, 1.0)],
    &[(0.15, 1.0), (0.85, 1.0)],
];
const DIGIT_2: PolylineGlyph = &[&[
    (0.0, 0.2),
    (0.2, 0.0),
    (0.8, 0.0),
    (1.0, 0.2),
    (1.0, 0.4),
    (0.0, 1.0),
    (1.0, 1.0),
]];
const DIGIT_3: PolylineGlyph = &[
    &[
        (0.0, 0.15),
        (0.2, 0.0),
        (0.8, 0.0),
        (1.0, 0.2),
        (0.8, 0.45),
        (0.35, 0.45),
    ],
    &[(0.8, 0.45), (1.0, 0.7), (0.8, 1.0), (0.2, 1.0), (0.0, 0.85)],
];
const DIGIT_4: PolylineGlyph = &[&[(0.8, 1.0), (0.8, 0.0), (0.0, 0.65), (1.0, 0.65)]];
const DIGIT_5: PolylineGlyph = &[&[
    (1.0, 0.0),
    (0.0, 0.0),
    (0.0, 0.45),
    (0.8, 0.45),
    (1.0, 0.65),
    (1.0, 0.8),
    (0.8, 1.0),
    (0.2, 1.0),
    (0.0, 0.85),
]];
const DIGIT_6: PolylineGlyph = &[&[
    (0.9, 0.0),
    (0.2, 0.0),
    (0.0, 0.2),
    (0.0, 0.85),
    (0.2, 1.0),
    (0.8, 1.0),
    (1.0, 0.85),
    (1.0, 0.55),
    (0.8, 0.45),
    (0.0, 0.45),
]];
const DIGIT_7: PolylineGlyph = &[&[(0.0, 0.0), (1.0, 0.0), (0.35, 1.0)]];
const DIGIT_8: PolylineGlyph = &[&[
    (0.2, 0.45),
    (0.0, 0.25),
    (0.0, 0.15),
    (0.2, 0.0),
    (0.8, 0.0),
    (1.0, 0.15),
    (1.0, 0.25),
    (0.8, 0.45),
    (0.2, 0.45),
    (0.0, 0.65),
    (0.0, 0.85),
    (0.2, 1.0),
    (0.8, 1.0),
    (1.0, 0.85),
    (1.0, 0.65),
    (0.8, 0.45),
]];
const DIGIT_9: PolylineGlyph = &[&[
    (0.1, 1.0),
    (0.8, 1.0),
    (1.0, 0.8),
    (1.0, 0.15),
    (0.8, 0.0),
    (0.2, 0.0),
    (0.0, 0.15),
    (0.0, 0.45),
    (0.2, 0.55),
    (1.0, 0.55),
]];

fn digit_glyph(ch: char) -> PolylineGlyph {
    match ch {
        '0' => DIGIT_0,
        '1' => DIGIT_1,
        '2' => DIGIT_2,
        '3' => DIGIT_3,
        '4' => DIGIT_4,
        '5' => DIGIT_5,
        '6' => DIGIT_6,
        '7' => DIGIT_7,
        '8' => DIGIT_8,
        '9' => DIGIT_9,
        _ => DIGIT_0,
    }
}

/// Returns `(width_mm, height_mm)` of the localized corner keep-out box for `page_number`.
pub fn corner_keepout_size_mm(page_number: usize) -> (f64, f64) {
    let d_count = page_number.max(1).to_string().len() as f64;
    let num_w = d_count * 2.4 + (d_count - 1.0).max(0.0) * 0.48;
    let keepout_w = (1.0 + num_w).max(2.5) + 0.7;
    (keepout_w, CORNER_KEEP_OUT_H_MM)
}

/// Returns the `[TopLeft, BottomRight]` localized keep-out rectangles in sheet `mm`.
pub fn corner_keepout_rects_mm(page_number: usize, config: &Config, br_anchor: Point) -> [Rect; 2] {
    let (kw, kh) = corner_keepout_size_mm(page_number);
    let tl = Rect::new(
        config.margin_mm,
        config.margin_mm,
        config.margin_mm + kw,
        config.margin_mm + kh,
    );
    let br = Rect::new(br_anchor.x - kw, br_anchor.y - kh, br_anchor.x, br_anchor.y);
    [tl, br]
}

/// Generates single-stroke vector page numbers, baseline bar, and L-ticks anchored at `(anchor_x, anchor_y)`.
pub fn layout_corner_page_number_at_anchor(
    page_number: usize,
    anchor_x: f64,
    anchor_y: f64,
    corner: CornerAnchor,
) -> Vec<Vec<Point>> {
    let digit_h = 4.0;
    let digit_w = 2.4;
    let digit_gap = 0.48;
    let text = page_number.to_string();
    let d_count = text.len() as f64;
    let num_w = d_count * digit_w + (d_count - 1.0).max(0.0) * digit_gap;

    let mut strokes = Vec::new();

    let (origin_x, origin_y, tick) = match corner {
        CornerAnchor::TopLeft => (
            anchor_x + 1.0,
            anchor_y + 0.7,
            vec![
                Point::new(anchor_x, anchor_y + 2.5),
                Point::new(anchor_x, anchor_y),
                Point::new(anchor_x + 2.5, anchor_y),
            ],
        ),
        CornerAnchor::BottomRight => (
            anchor_x - 1.0 - num_w,
            anchor_y - 5.3,
            vec![
                Point::new(anchor_x - 2.5, anchor_y),
                Point::new(anchor_x, anchor_y),
                Point::new(anchor_x, anchor_y - 2.5),
            ],
        ),
    };

    strokes.push(tick);

    for (idx, ch) in text.chars().enumerate() {
        let x0 = origin_x + (idx as f64) * (digit_w + digit_gap);
        for polyline in digit_glyph(ch) {
            let pts = polyline
                .iter()
                .map(|&(ux, uy)| Point::new(x0 + ux * digit_w, origin_y + uy * digit_h))
                .collect();
            strokes.push(pts);
        }
    }

    // Horizontal baseline orientation bar beneath digits.
    let bar_y = origin_y + digit_h + 0.6;
    strokes.push(vec![
        Point::new(origin_x, bar_y),
        Point::new(origin_x + num_w, bar_y),
    ]);

    strokes
}

/// Generates single-stroke vector page numbers, baseline bar, and L-shaped corner registration ticks in `mm`.
pub fn layout_corner_page_number(
    page_number: usize,
    page_width_mm: f64,
    page_height_mm: f64,
    margin_mm: f64,
    corner: CornerAnchor,
) -> Vec<Vec<Point>> {
    let (ax, ay) = match corner {
        CornerAnchor::TopLeft => (margin_mm, margin_mm),
        CornerAnchor::BottomRight => (page_width_mm - margin_mm, page_height_mm - margin_mm),
    };
    layout_corner_page_number_at_anchor(page_number, ax, ay, corner)
}

/// Computes unified symmetric cut-centered slot dimensions across all `cut_polygon` and `bleed_polygon` vertices.
pub fn compute_slot_bounds(kerfed: &KerfedGrid) -> (f64, f64) {
    let mut max_half_w = 0.0_f64;
    let mut max_half_h = 0.0_f64;
    for piece in &kerfed.pieces {
        let center = piece.cut_aabb_mm.center();
        for pt in piece.cut_polygon.iter().chain(piece.bleed_polygon.iter()) {
            max_half_w = max_half_w.max((pt.x - center.x).abs());
            max_half_h = max_half_h.max((pt.y - center.y).abs());
        }
    }
    (2.0 * max_half_w, 2.0 * max_half_h)
}

fn orient_cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// Computes exact Euclidean distance between closed line segments `[a, b]` and `[c, d]`.
pub(crate) fn segment_to_segment_distance(a: Point, b: Point, c: Point, d: Point) -> f64 {
    let d1 = orient_cross(a, b, c);
    let d2 = orient_cross(a, b, d);
    let d3 = orient_cross(c, d, a);
    let d4 = orient_cross(c, d, b);

    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return 0.0;
    }

    perp_dist_point_segment(a, c, d)
        .min(perp_dist_point_segment(b, c, d))
        .min(perp_dist_point_segment(c, a, b))
        .min(perp_dist_point_segment(d, a, b))
}

/// Verifies two closed polygon rings are disjoint and separated by at least `(min_spacing - tol).max(0.0)`.
pub(crate) fn polygon_ring_clearance_ok(
    poly_a: &[Point],
    poly_b: &[Point],
    min_spacing: f64,
    tol: f64,
) -> bool {
    if poly_a.is_empty() || poly_b.is_empty() {
        return true;
    }

    let req = (min_spacing - tol).max(0.0);
    if let (Some(aabb_a), Some(aabb_b)) = (polygon_aabb(poly_a), polygon_aabb(poly_b)) {
        let gap_x = (aabb_b.x0 - aabb_a.x1).max(aabb_a.x0 - aabb_b.x1);
        let gap_y = (aabb_b.y0 - aabb_a.y1).max(aabb_a.y0 - aabb_b.y1);
        let fast_thresh = req.max(1e-9);
        if gap_x > fast_thresh || gap_y > fast_thresh {
            return true;
        }
    }

    for &pt in poly_a {
        if point_in_polygon(pt, poly_b) {
            return false;
        }
    }
    for &pt in poly_b {
        if point_in_polygon(pt, poly_a) {
            return false;
        }
    }

    for i in 0..poly_a.len() {
        let a = poly_a[i];
        let b = poly_a[(i + 1) % poly_a.len()];
        for j in 0..poly_b.len() {
            let c = poly_b[j];
            let d = poly_b[(j + 1) % poly_b.len()];
            let dist = segment_to_segment_distance(a, b, c, d);
            if dist <= 1e-12 || dist < req {
                return false;
            }
        }
    }

    true
}

/// Verifies a closed polygon ring clears an axis-aligned rectangle by at least `(min_spacing - tol).max(0.0)`.
pub(crate) fn polygon_rect_clearance_ok(
    poly: &[Point],
    rect: Rect,
    min_spacing: f64,
    tol: f64,
) -> bool {
    let rect_ring = [
        Point::new(rect.x0, rect.y0),
        Point::new(rect.x1, rect.y0),
        Point::new(rect.x1, rect.y1),
        Point::new(rect.x0, rect.y1),
    ];
    polygon_ring_clearance_ok(poly, &rect_ring, min_spacing, tol)
}

/// Packs irregular pieces onto margin-bounded sheets with localized corner keep-outs and default options.
pub fn paginate_pieces(
    kerfed: &KerfedGrid,
    config: &Config,
) -> Result<PaginatedLayout, PipelineError> {
    paginate_pieces_with_options(
        kerfed,
        config,
        &PaginateOptions::from_config(config),
        &AtomicBool::new(false),
        |_| {},
    )
}

/// Packs irregular pieces onto margin-bounded sheets with configurable optimization options and progress streaming.
pub fn paginate_pieces_with_options<F>(
    kerfed: &KerfedGrid,
    config: &Config,
    options: &PaginateOptions,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> Result<PaginatedLayout, PipelineError>
where
    F: FnMut(NestProgress),
{
    let puzzle_area_mm2 = kerfed.puzzle_bounds_mm.area();
    if !puzzle_area_mm2.is_finite() || puzzle_area_mm2 <= 0.0 {
        return Err(PipelineError::new(
            "paginate",
            "puzzle_bounds_mm must have strictly positive area",
        ));
    }

    let raw_pages =
        nest::pack_irregular_pages(kerfed, config, options, cancel, &mut on_progress)?;

    let mut pages = Vec::with_capacity(raw_pages.len());
    let mut assigned_count = 0usize;
    let mut total_bbox_area_mm2 = 0.0_f64;

    for (idx, raw) in raw_pages.into_iter().enumerate() {
        let page_number = idx + 1;
        verify_page_invariants(kerfed, page_number, &raw.slots, raw.br_anchor_mm, config)?;

        let mut corner_strokes_mm = layout_corner_page_number_at_anchor(
            page_number,
            config.margin_mm,
            config.margin_mm,
            CornerAnchor::TopLeft,
        );
        corner_strokes_mm.extend(layout_corner_page_number_at_anchor(
            page_number,
            raw.br_anchor_mm.x,
            raw.br_anchor_mm.y,
            CornerAnchor::BottomRight,
        ));

        assigned_count += raw.slots.len();
        total_bbox_area_mm2 += raw.content_bbox_mm.area();

        pages.push(PagePlan {
            page_number,
            slots: raw.slots,
            content_bbox_mm: raw.content_bbox_mm,
            bottom_right_anchor_mm: raw.br_anchor_mm,
            corner_strokes_mm,
        });
    }

    if assigned_count != kerfed.pieces.len() {
        return Err(PipelineError::new(
            "paginate",
            format!(
                "pagination piece count mismatch: assigned {assigned_count} vs {}",
                kerfed.pieces.len()
            ),
        ));
    }

    let bbox_area_pct = (total_bbox_area_mm2 / puzzle_area_mm2) * 100.0;

    Ok(PaginatedLayout {
        puzzle_area_mm2,
        total_bbox_area_mm2,
        bbox_area_pct,
        pages,
    })
}

/// Verifies sheet margins, corner keep-out disjointness/clearance, and pairwise piece spacing on one page.
pub(crate) fn verify_page_invariants(
    kerfed: &KerfedGrid,
    page_number: usize,
    slots: &[PageSlot],
    br_anchor: Point,
    config: &Config,
) -> Result<(), PipelineError> {
    let min_x = config.margin_mm - CONTAINMENT_TOL_MM;
    let max_x = config.page_width_mm - config.margin_mm + CONTAINMENT_TOL_MM;
    let min_y = config.margin_mm - CONTAINMENT_TOL_MM;
    let max_y = config.page_height_mm - config.margin_mm + CONTAINMENT_TOL_MM;

    let (kw, kh) = corner_keepout_size_mm(page_number);
    if br_anchor.x < config.margin_mm + kw - CONTAINMENT_TOL_MM
        || br_anchor.x > max_x
        || br_anchor.y < config.margin_mm + kh - CONTAINMENT_TOL_MM
        || br_anchor.y > max_y
    {
        return Err(PipelineError::new(
            "paginate",
            format!(
                "BottomRight registration anchor ({:.4}, {:.4}) mm on page {page_number} violates valid bounds",
                br_anchor.x, br_anchor.y
            ),
        ));
    }

    let [tl_rect, br_rect] = corner_keepout_rects_mm(page_number, config, br_anchor);
    let ko_gap_x = (br_rect.x0 - tl_rect.x1).max(tl_rect.x0 - br_rect.x1);
    let ko_gap_y = (br_rect.y0 - tl_rect.y1).max(tl_rect.y0 - br_rect.y1);
    if ko_gap_x < config.piece_spacing_mm - CONTAINMENT_TOL_MM
        && ko_gap_y < config.piece_spacing_mm - CONTAINMENT_TOL_MM
    {
        return Err(PipelineError::new(
            "paginate",
            format!(
                "TopLeft and BottomRight corner keep-out boxes overlap on page {page_number} (gap_x={ko_gap_x:.4}, gap_y={ko_gap_y:.4} mm)"
            ),
        ));
    }

    struct TransformedPiece<'a> {
        id: &'a str,
        cut: Vec<Point>,
        bleed: Vec<Point>,
        combined_aabb: Rect,
    }

    let mut transformed = Vec::with_capacity(slots.len());

    for slot in slots {
        let piece = &kerfed.pieces[slot.piece_index];
        let cut: Vec<Point> = piece
            .cut_polygon
            .iter()
            .map(|&pt| slot.transform_point(pt))
            .collect();
        let bleed: Vec<Point> = piece
            .bleed_polygon
            .iter()
            .map(|&pt| slot.transform_point(pt))
            .collect();

        for &t in cut.iter().chain(bleed.iter()) {
            if t.x < min_x || t.x > max_x || t.y < min_y || t.y > max_y {
                return Err(PipelineError::new(
                    "paginate",
                    format!(
                        "transformed vertex ({:.4}, {:.4}) mm of piece \"{}\" escaped usable sheet bounds [{min_x:.3}, {max_x:.3}] x [{min_y:.3}, {max_y:.3}]",
                        t.x, t.y, piece.id
                    ),
                ));
            }
        }

        for (poly_name, poly) in [("cut", &cut), ("bleed", &bleed)] {
            if !polygon_rect_clearance_ok(poly, tl_rect, config.piece_spacing_mm, CONTAINMENT_TOL_MM)
            {
                return Err(PipelineError::new(
                    "paginate",
                    format!(
                        "piece \"{}\" ({poly_name} polygon) violates TopLeft corner keep-out clearance on page {page_number}",
                        piece.id
                    ),
                ));
            }
            if !polygon_rect_clearance_ok(poly, br_rect, config.piece_spacing_mm, CONTAINMENT_TOL_MM)
            {
                return Err(PipelineError::new(
                    "paginate",
                    format!(
                        "piece \"{}\" ({poly_name} polygon) violates BottomRight corner keep-out clearance on page {page_number}",
                        piece.id
                    ),
                ));
            }
        }

        let cut_aabb = polygon_aabb(&cut).unwrap_or(Rect::ZERO);
        let bleed_aabb = polygon_aabb(&bleed).unwrap_or(Rect::ZERO);
        let combined_aabb = cut_aabb.union(bleed_aabb);
        transformed.push(TransformedPiece {
            id: piece.id.as_str(),
            cut,
            bleed,
            combined_aabb,
        });
    }

    let req_gap = (config.piece_spacing_mm - CONTAINMENT_TOL_MM).max(1e-9);
    for i in 0..transformed.len() {
        for j in (i + 1)..transformed.len() {
            let pa = &transformed[i];
            let pb = &transformed[j];
            let gap_x = (pb.combined_aabb.x0 - pa.combined_aabb.x1)
                .max(pa.combined_aabb.x0 - pb.combined_aabb.x1);
            let gap_y = (pb.combined_aabb.y0 - pa.combined_aabb.y1)
                .max(pa.combined_aabb.y0 - pb.combined_aabb.y1);
            if gap_x >= req_gap || gap_y >= req_gap {
                continue;
            }

            let ok = polygon_ring_clearance_ok(
                &pa.cut,
                &pb.cut,
                config.piece_spacing_mm,
                CONTAINMENT_TOL_MM,
            ) && polygon_ring_clearance_ok(
                &pa.cut,
                &pb.bleed,
                config.piece_spacing_mm,
                CONTAINMENT_TOL_MM,
            ) && polygon_ring_clearance_ok(
                &pa.bleed,
                &pb.cut,
                config.piece_spacing_mm,
                CONTAINMENT_TOL_MM,
            ) && polygon_ring_clearance_ok(
                &pa.bleed,
                &pb.bleed,
                config.piece_spacing_mm,
                CONTAINMENT_TOL_MM,
            );
            if !ok {
                return Err(PipelineError::new(
                    "paginate",
                    format!(
                        "piece spacing invariant violated between \"{}\" and \"{}\" on page {page_number} (min_spacing={:.4} mm)",
                        pa.id, pb.id, config.piece_spacing_mm
                    ),
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CoordFrame, KerfedPiece, LengthUnit, Rotation90};
    use kurbo::Vec2;

    fn rect_poly(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    fn simple_kerfed(pieces: Vec<KerfedPiece>) -> KerfedGrid {
        KerfedGrid {
            frame: CoordFrame {
                doc_unit: LengthUnit::Mm,
                viewbox_min_user: (0.0, 0.0),
                user_to_mm: (1.0, 1.0),
                doc_to_mm: 1.0,
            },
            image_href: String::new(),
            image_bounds_mm: Rect::ZERO,
            puzzle_bounds_mm: Rect::new(0.0, 0.0, 20.0, 20.0),
            image_preserve_aspect_ratio: "none".to_string(),
            image_rendering: None,
            rows: 1,
            cols: pieces.len(),
            matched_tabs: 0,
            missing_tabs: 0,
            pieces,
        }
    }

    #[test]
    fn corner_page_numbers_stay_in_bands_and_clear_ticks_under_zero_margin() {
        for &margin in &[0.0, 6.35] {
            for &page_num in &[1usize, 12, 123] {
                let page_w = 152.4;
                let page_h = 152.4;

                let tl = layout_corner_page_number(
                    page_num,
                    page_w,
                    page_h,
                    margin,
                    CornerAnchor::TopLeft,
                );
                let br = layout_corner_page_number(
                    page_num,
                    page_w,
                    page_h,
                    margin,
                    CornerAnchor::BottomRight,
                );

                // TopLeft tick is tl[0], remaining strokes are digits + baseline bar.
                for stroke in &tl[1..] {
                    for p in stroke {
                        assert!(p.x >= margin + 1.0 - 1e-9);
                        assert!(p.y >= margin + 0.7 - 1e-9);
                        assert!(p.y <= margin + 5.3 + 1e-9);
                    }
                }
                for stroke in &br[1..] {
                    for p in stroke {
                        assert!(p.x <= page_w - margin - 1.0 + 1e-9);
                        assert!(p.y >= page_h - margin - 5.3 - 1e-9);
                        assert!(p.y <= page_h - margin - 0.7 + 1e-9);
                    }
                }
                // Check multi-digit baseline width scales with D.
                let bar = br.last().unwrap();
                let expected_w = (page_num.to_string().len() as f64) * 2.4
                    + ((page_num.to_string().len() - 1) as f64) * 0.48;
                assert!(((bar[1].x - bar[0].x) - expected_w).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn corner_keepout_rects_and_anchor_strokes_scale_with_digits() {
        let cfg = Config {
            page_width_mm: 200.0,
            page_height_mm: 200.0,
            margin_mm: 6.35,
            ..Config::default()
        };
        let (kw1, kh1) = corner_keepout_size_mm(1);
        assert!((kw1 - 4.10).abs() < 1e-9);
        assert!((kh1 - 6.0).abs() < 1e-9);

        let (kw2, kh2) = corner_keepout_size_mm(12);
        assert!((kw2 - 6.98).abs() < 1e-9);
        assert!((kh2 - 6.0).abs() < 1e-9);

        let br_anchor = Point::new(100.0, 120.0);
        let [tl, br] = corner_keepout_rects_mm(12, &cfg, br_anchor);
        assert!((tl.x0 - 6.35).abs() < 1e-9);
        assert!((tl.x1 - (6.35 + kw2)).abs() < 1e-9);
        assert!((br.x1 - 100.0).abs() < 1e-9);
        assert!((br.x0 - (100.0 - kw2)).abs() < 1e-9);
        assert!((br.y1 - 120.0).abs() < 1e-9);
        assert!((br.y0 - (120.0 - kh2)).abs() < 1e-9);

        let strokes = layout_corner_page_number_at_anchor(
            12,
            br_anchor.x,
            br_anchor.y,
            CornerAnchor::BottomRight,
        );
        for stroke in &strokes {
            for &p in stroke {
                assert!(p.x >= br.x0 && p.x <= br.x1);
                assert!(p.y >= br.y0 && p.y <= br.y1);
            }
        }
    }

    #[test]
    fn crossing_segment_keepout_detection_works_even_with_vertices_outside() {
        // Long diagonal thin rectangle crossing through keepout [0, 4.1] x [0, 6.0]
        // with all vertices strictly outside the keepout rectangle.
        let keepout = Rect::new(0.0, 0.0, 4.1, 6.0);
        let crossing_poly = vec![
            Point::new(-1.0, 4.0),
            Point::new(6.0, -1.0),
            Point::new(6.5, -0.5),
            Point::new(-0.5, 4.5),
        ];
        assert!(!polygon_rect_clearance_ok(&crossing_poly, keepout, 0.0, 0.0));
        assert!(!polygon_rect_clearance_ok(&crossing_poly, keepout, 0.18, 1e-4));

        // Disjoint polygon separated by 0.25 mm > 0.18 mm passes.
        let clear_poly = rect_poly(4.35, 0.0, 8.0, 6.0);
        assert!(polygon_rect_clearance_ok(&clear_poly, keepout, 0.18, 1e-4));
    }

    #[test]
    fn verify_page_invariants_catches_overlap_keepout_violation_and_overlapping_keepouts() {
        let p0 = KerfedPiece {
            row: 0,
            col: 0,
            id: "p0".into(),
            cut_aabb_mm: Rect::new(0.0, 0.0, 10.0, 10.0),
            cut_polygon: rect_poly(0.0, 0.0, 10.0, 10.0),
            bleed_polygon: rect_poly(0.0, 0.0, 10.0, 10.0),
        };
        let p1 = KerfedPiece {
            row: 0,
            col: 1,
            id: "p1".into(),
            cut_aabb_mm: Rect::new(0.0, 0.0, 10.0, 10.0),
            cut_polygon: rect_poly(0.0, 0.0, 10.0, 10.0),
            bleed_polygon: rect_poly(0.0, 0.0, 10.0, 10.0),
        };
        let kerfed = simple_kerfed(vec![p0, p1]);
        let cfg = Config {
            page_width_mm: 100.0,
            page_height_mm: 100.0,
            margin_mm: 5.0,
            piece_spacing_mm: 0.18,
            ..Config::default()
        };

        // 1. Overlapping pieces fail.
        let overlap_slots = vec![
            PageSlot {
                piece_index: 0,
                rotation: Rotation90::Deg0,
                offset_mm: Vec2::new(15.0, 15.0),
            },
            PageSlot {
                piece_index: 1,
                rotation: Rotation90::Deg0,
                offset_mm: Vec2::new(24.0, 15.0),
            },
        ];
        let err_overlap =
            verify_page_invariants(&kerfed, 1, &overlap_slots, Point::new(95.0, 95.0), &cfg)
                .unwrap_err();
        assert!(err_overlap.to_string().contains("spacing"));

        // 2. BottomRight keep-out violation fails.
        let br_violation_slots = vec![PageSlot {
            piece_index: 0,
            rotation: Rotation90::Deg0,
            offset_mm: Vec2::new(15.0, 15.0),
        }];
        let err_br = verify_page_invariants(
            &kerfed,
            1,
            &br_violation_slots,
            Point::new(26.0, 26.0),
            &cfg,
        )
        .unwrap_err();
        assert!(err_br.to_string().contains("BottomRight"));

        // 3. Overlapping TopLeft and BottomRight keep-out boxes fail.
        let err_ko = verify_page_invariants(
            &kerfed,
            1,
            &[],
            Point::new(11.0, 13.0),
            &cfg,
        )
        .unwrap_err();
        assert!(err_ko.to_string().contains("overlap"));
    }

    #[test]
    fn compute_slot_bounds_is_symmetric_and_rejects_oversized_piece() {
        // Asymmetric bleed overhang on right side (+5 mm vs -1 mm on left) -> symmetric width 2*5 = 10 relative to cut_center.
        let cut = rect_poly(0.0, 0.0, 20.0, 20.0);
        let bleed = rect_poly(-1.0, -0.5, 25.0, 20.5);
        let kerfed = simple_kerfed(vec![KerfedPiece {
            row: 0,
            col: 0,
            id: "p0".into(),
            cut_aabb_mm: Rect::new(0.0, 0.0, 20.0, 20.0),
            cut_polygon: cut,
            bleed_polygon: bleed,
        }]);
        let (sw, sh) = compute_slot_bounds(&kerfed);
        assert!((sw - 30.0).abs() < 1e-9);
        assert!((sh - 21.0).abs() < 1e-9);

        let tiny_cfg = Config {
            page_width_mm: 35.0,
            page_height_mm: 30.0,
            margin_mm: 5.0,
            ..Config::default()
        };
        let err = paginate_pieces(&kerfed, &tiny_cfg).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("26.00 x 21.00"), "got: {msg}");
        assert!(msg.contains("p0"), "got: {msg}");
        assert!(msg.contains("minimum required page_size"), "got: {msg}");
    }
}
