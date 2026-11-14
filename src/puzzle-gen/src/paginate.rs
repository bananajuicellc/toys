use kurbo::{Point, Vec2};

use crate::model::{
    Config, CornerAnchor, KerfedGrid, PagePlan, PageSlot, PaginatedLayout, PipelineError,
};
use crate::stage2::polygon_aabb;

pub const CORNER_BAND_MM: f64 = 6.0;
const CONTAINMENT_TOL_MM: f64 = 1e-4;

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

/// Generates single-stroke vector page numbers, baseline bar, and L-shaped corner registration ticks in `mm`.
pub fn layout_corner_page_number(
    page_number: usize,
    page_width_mm: f64,
    page_height_mm: f64,
    margin_mm: f64,
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
            margin_mm + 1.0,
            margin_mm + 0.7,
            vec![
                Point::new(margin_mm, margin_mm + 2.5),
                Point::new(margin_mm, margin_mm),
                Point::new(margin_mm + 2.5, margin_mm),
            ],
        ),
        CornerAnchor::BottomRight => (
            page_width_mm - margin_mm - 1.0 - num_w,
            page_height_mm - margin_mm - 5.3,
            vec![
                Point::new(page_width_mm - margin_mm - 2.5, page_height_mm - margin_mm),
                Point::new(page_width_mm - margin_mm, page_height_mm - margin_mm),
                Point::new(page_width_mm - margin_mm, page_height_mm - margin_mm - 2.5),
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

/// Reflows pieces in row-major order into margin-bounded and corner-band-reserved sheets.
pub fn paginate_pieces(
    kerfed: &KerfedGrid,
    config: &Config,
) -> Result<PaginatedLayout, PipelineError> {
    let (slot_w_mm, slot_h_mm) = compute_slot_bounds(kerfed);

    let usable_w = config.page_width_mm - 2.0 * config.margin_mm;
    let usable_h = config.page_height_mm - 2.0 * config.margin_mm - 2.0 * CORNER_BAND_MM;

    let page_cols = fit_slots(usable_w, slot_w_mm, config.piece_spacing_mm);
    let page_rows = fit_slots(usable_h, slot_h_mm, config.piece_spacing_mm);

    if page_cols < 1 || page_rows < 1 {
        let min_w = slot_w_mm + 2.0 * config.margin_mm;
        let min_h = slot_h_mm + 2.0 * config.margin_mm + 2.0 * CORNER_BAND_MM;
        return Err(PipelineError::new(
            "paginate",
            format!(
                "piece slot {slot_w_mm:.2} x {slot_h_mm:.2} mm exceeds usable sheet area {usable_w:.2} x {usable_h:.2} mm (after margins and 2x{CORNER_BAND_MM:.1} mm corner bands); minimum required page_size is {min_w:.2}mm x {min_h:.2}mm"
            ),
        ));
    }

    let per_page = page_cols * page_rows;
    let mut pages = Vec::new();
    let mut assigned_count = 0usize;

    for (page_idx, chunk) in kerfed.pieces.chunks(per_page).enumerate() {
        let page_number = page_idx + 1;
        let mut slots = Vec::with_capacity(chunk.len());

        for (slot_idx, piece) in chunk.iter().enumerate() {
            let piece_index = page_idx * per_page + slot_idx;
            let row = slot_idx / page_cols;
            let col = slot_idx % page_cols;

            let slot_cx = config.margin_mm
                + (col as f64) * (slot_w_mm + config.piece_spacing_mm)
                + 0.5 * slot_w_mm;
            let slot_cy = config.margin_mm
                + CORNER_BAND_MM
                + (row as f64) * (slot_h_mm + config.piece_spacing_mm)
                + 0.5 * slot_h_mm;
            let offset_mm = Vec2::new(
                slot_cx - piece.cut_aabb_mm.center().x,
                slot_cy - piece.cut_aabb_mm.center().y,
            );

            slots.push(PageSlot {
                piece_index,
                row,
                col,
                offset_mm,
            });
            assigned_count += 1;
        }

        verify_page_invariants(kerfed, &slots, config)?;

        let mut corner_strokes_mm = layout_corner_page_number(
            page_number,
            config.page_width_mm,
            config.page_height_mm,
            config.margin_mm,
            CornerAnchor::TopLeft,
        );
        corner_strokes_mm.extend(layout_corner_page_number(
            page_number,
            config.page_width_mm,
            config.page_height_mm,
            config.margin_mm,
            CornerAnchor::BottomRight,
        ));

        pages.push(PagePlan {
            page_number,
            slots,
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

    Ok(PaginatedLayout {
        slot_w_mm,
        slot_h_mm,
        page_cols,
        page_rows,
        pages,
    })
}

fn fit_slots(usable: f64, slot: f64, spacing: f64) -> usize {
    if usable <= 0.0 || slot <= 0.0 || usable + CONTAINMENT_TOL_MM < slot {
        return 0;
    }
    let pitch = slot + spacing;
    let mut n = ((usable + spacing + 1e-9) / pitch).floor() as usize;
    while n > 0 {
        let needed = (n as f64) * slot + ((n - 1) as f64) * spacing;
        if needed <= usable + CONTAINMENT_TOL_MM {
            break;
        }
        n -= 1;
    }
    n
}

fn verify_page_invariants(
    kerfed: &KerfedGrid,
    slots: &[PageSlot],
    config: &Config,
) -> Result<(), PipelineError> {
    let min_x = config.margin_mm - CONTAINMENT_TOL_MM;
    let max_x = config.page_width_mm - config.margin_mm + CONTAINMENT_TOL_MM;
    let min_y = config.margin_mm + CORNER_BAND_MM - CONTAINMENT_TOL_MM;
    let max_y = config.page_height_mm - config.margin_mm - CORNER_BAND_MM + CONTAINMENT_TOL_MM;

    let mut translated_aabbs = Vec::with_capacity(slots.len());

    for slot in slots {
        let piece = &kerfed.pieces[slot.piece_index];
        for pt in piece.cut_polygon.iter().chain(piece.bleed_polygon.iter()) {
            let t = *pt + slot.offset_mm;
            if t.x < min_x || t.x > max_x || t.y < min_y || t.y > max_y {
                return Err(PipelineError::new(
                    "paginate",
                    format!(
                        "translated vertex ({:.4}, {:.4}) mm of piece \"{}\" escaped usable sheet bounds [{min_x:.3}, {max_x:.3}] x [{min_y:.3}, {max_y:.3}]",
                        t.x, t.y, piece.id
                    ),
                ));
            }
        }

        let mut all_pts: Vec<Point> = piece
            .cut_polygon
            .iter()
            .chain(piece.bleed_polygon.iter())
            .map(|p| *p + slot.offset_mm)
            .collect();
        if let Some(aabb) = polygon_aabb(&all_pts) {
            translated_aabbs.push((piece.id.as_str(), aabb));
        }
        all_pts.clear();
    }

    let min_gap = config.piece_spacing_mm - CONTAINMENT_TOL_MM;
    for i in 0..translated_aabbs.len() {
        for j in (i + 1)..translated_aabbs.len() {
            let (id_a, a) = translated_aabbs[i];
            let (id_b, b) = translated_aabbs[j];
            let gap_x = (b.x0 - a.x1).max(a.x0 - b.x1);
            let gap_y = (b.y0 - a.y1).max(a.y0 - b.y1);
            if gap_x < min_gap && gap_y < min_gap {
                return Err(PipelineError::new(
                    "paginate",
                    format!(
                        "piece spacing invariant violated between \"{id_a}\" and \"{id_b}\" (gap_x={gap_x:.4}, gap_y={gap_y:.4} < {min_gap:.4} mm)"
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
    use crate::model::{CoordFrame, KerfedPiece, LengthUnit};
    use kurbo::Rect;

    fn rect_poly(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
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
    fn compute_slot_bounds_is_symmetric_and_rejects_oversized_piece() {
        // Asymmetric bleed overhang on right side (+5 mm vs -1 mm on left) -> symmetric width 2*5 = 10 relative to cut_center.
        let cut = rect_poly(0.0, 0.0, 20.0, 20.0);
        let bleed = rect_poly(-1.0, -0.5, 25.0, 20.5);
        let kerfed = KerfedGrid {
            frame: CoordFrame {
                doc_unit: LengthUnit::Mm,
                viewbox_min_user: (0.0, 0.0),
                user_to_mm: (1.0, 1.0),
                doc_to_mm: 1.0,
            },
            image_href: String::new(),
            image_bounds_mm: Rect::ZERO,
            image_preserve_aspect_ratio: "none".to_string(),
            rows: 1,
            cols: 1,
            matched_tabs: 0,
            missing_tabs: 0,
            pieces: vec![KerfedPiece {
                row: 0,
                col: 0,
                id: "p0".into(),
                cut_aabb_mm: Rect::new(0.0, 0.0, 20.0, 20.0),
                cut_polygon: cut,
                bleed_polygon: bleed,
            }],
        };
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
        assert!(msg.contains("30.00 x 21.00"));
        assert!(msg.contains("minimum required page_size"));
    }
}
