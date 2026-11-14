use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, Ordering};

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};
use kurbo::{Point, Rect, Vec2};

use crate::model::{
    Config, KerfedGrid, KerfedPiece, NestProgress, PageSlot, PaginateOptions, PipelineError,
    Rotation90,
};
use crate::paginate;
use crate::stage2;

const MAX_OPEN_PAGES: usize = 2;
const TAIL_PAGES: usize = 2;

pub(crate) struct RawPagePlacement {
    pub slots: Vec<PageSlot>,
    pub br_anchor_mm: Point,
    pub content_bbox_mm: Rect,
}

#[derive(Debug, Clone, Copy)]
struct SheetSpec {
    page_width_mm: f64,
    page_height_mm: f64,
    margin_mm: f64,
    usable_w: f64,
    usable_h: f64,
    spacing_mm: f64,
    step_mm: f64,
    usable_cols: usize,
    usable_rows: usize,
    pad_cells: usize,
    total_cols: usize,
    total_rows: usize,
}

#[derive(Debug, Clone)]
struct OrientedVariant {
    env_min_x: f64,
    env_min_y: f64,
    env_w: f64,
    env_h: f64,
    outer_env_area: f64,
    dilated_aabb_area: f64,
    max_gx: usize,
    max_gy: usize,
    variant_cols: usize,
    variant_rows: usize,
    words_per_row: usize,
    words: Vec<u64>,
    row_max_one_run: Vec<u16>,
    active_vy_start: usize,
    active_vy_end: usize,
    anchor: Option<(usize, usize)>,
}

#[derive(Debug, Clone)]
struct SheetBitmask {
    page_number: usize,
    kw: f64,
    kh: f64,
    total_cols: usize,
    total_rows: usize,
    words_per_row: usize,
    words: Vec<u64>,
    row_max_free_run: Vec<u16>,
    pieces_max_x: f64,
    pieces_max_y: f64,
    cur_extent_w: f64,
    cur_extent_h: f64,
    extent_area_mm2: f64,
    placed_env_area_sum: f64,
    slots: Vec<PageSlot>,
}

#[derive(Debug, Clone)]
struct DecodedLayout {
    pages: Vec<SheetBitmask>,
    order: Vec<usize>,
    rot_prefs: Vec<Rotation90>,
    extent_area_sum: f64,
}

#[derive(Debug, Clone, Copy)]
struct PlacementScore {
    extent_area_mm2: f64,
    gx: usize,
    gy: usize,
    rot_rank: usize,
    rot: Rotation90,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GravityMode {
    TopFirst,
    LeftFirst,
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn gen_range(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        let bound = upper as u64;
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let r = self.next_u64();
            if r >= threshold {
                return (r % bound) as usize;
            }
        }
    }
}

pub(crate) fn pack_irregular_pages<F>(
    kerfed: &KerfedGrid,
    config: &Config,
    options: &PaginateOptions,
    cancel: &AtomicBool,
    on_progress: &mut F,
) -> Result<Vec<RawPagePlacement>, PipelineError>
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

    let usable_w = config.page_width_mm - 2.0 * config.margin_mm;
    let usable_h = config.page_height_mm - 2.0 * config.margin_mm;
    let (kw_1, kh) = paginate::corner_keepout_size_mm(1);
    let min_w = 2.0 * config.margin_mm + 2.0 * kw_1 + config.piece_spacing_mm;
    let min_h = 2.0 * config.margin_mm + 2.0 * kh + config.piece_spacing_mm;

    if usable_w <= 0.0 || usable_h <= 0.0 {
        return Err(PipelineError::new(
            "paginate",
            format!(
                "usable sheet area {usable_w:.2} x {usable_h:.2} mm is non-positive after margins; minimum required page_size is {min_w:.2}mm x {min_h:.2}mm"
            ),
        ));
    }

    if usable_w < 2.0 * kw_1 + config.piece_spacing_mm
        && usable_h < 2.0 * kh + config.piece_spacing_mm
    {
        return Err(PipelineError::new(
            "paginate",
            format!(
                "TopLeft and BottomRight corner keep-out boxes overlap on usable sheet {usable_w:.2} x {usable_h:.2} mm; minimum required page_size is {min_w:.2}mm x {min_h:.2}mm"
            ),
        ));
    }

    if kerfed.pieces.is_empty() {
        return Ok(Vec::new());
    }

    let raw_step = if config.kerf_mm > 0.0 {
        config.kerf_mm
    } else {
        config.segment_length_mm
    };
    let step_mm = raw_step.max(0.10).max(usable_w.max(usable_h) / 2048.0);
    let usable_cols = (usable_w / step_mm).floor().clamp(1.0, 2048.0) as usize;
    let usable_rows = (usable_h / step_mm).floor().clamp(1.0, 2048.0) as usize;

    let dilation_eps_mm = 0.005 + 0.002 * config.piece_spacing_mm;
    let pad_mm = if config.piece_spacing_mm > 0.0 {
        0.5 * config.piece_spacing_mm + dilation_eps_mm
    } else {
        0.0
    };
    let pad_cells = (pad_mm / step_mm).ceil() as usize + 1;
    let spec = SheetSpec {
        page_width_mm: config.page_width_mm,
        page_height_mm: config.page_height_mm,
        margin_mm: config.margin_mm,
        usable_w,
        usable_h,
        spacing_mm: config.piece_spacing_mm,
        step_mm,
        usable_cols,
        usable_rows,
        pad_cells,
        total_cols: usable_cols + 2 * pad_cells,
        total_rows: usable_rows + 2 * pad_cells,
    };

    let mut variants = Vec::with_capacity(kerfed.pieces.len());
    for piece in &kerfed.pieces {
        variants.push(build_piece_variants(piece, pad_mm, &spec)?);
    }

    // Single-piece pre-flight feasibility against page-1 keep-outs.
    let empty_p1 = SheetBitmask::new(1, &spec)?;
    for (p_idx, piece_vars) in variants.iter().enumerate() {
        let mut fits = false;
        for var in piece_vars {
            if var.env_w <= spec.usable_w + paginate::CONTAINMENT_TOL_MM
                && var.env_h <= spec.usable_h + paginate::CONTAINMENT_TOL_MM
                && find_first_fit_on_empty_sheet(&empty_p1, var)
            {
                fits = true;
                break;
            }
        }
        if !fits {
            return Err(piece_unfit_error(&kerfed.pieces[p_idx], piece_vars, 1, &spec));
        }
    }

    let n = kerfed.pieces.len();
    let constructive_configs = build_constructive_starts(kerfed, &variants);
    let mut best: Option<DecodedLayout> = None;

    for (order, rot_prefs) in constructive_configs {
        let cand = decode_layout(
            &order,
            &rot_prefs,
            &[],
            &variants,
            kerfed,
            &spec,
            true,
        )?
        .expect("constructive decode with fail_on_unplaceable=true returns Some or Err");

        if is_better_layout(&cand, best.as_ref()) {
            let mut canonical = cand;
            canonicalize_order(&mut canonical);
            best = Some(canonical);
        }
    }

    let mut best = best.expect("at least one constructive pass produced a layout");
    let usable_area = spec.usable_w * spec.usable_h;

    on_progress(NestProgress {
        rows: kerfed.rows,
        cols: kerfed.cols,
        iter: 0,
        total_iters: options.max_iters,
        pages: best.pages.len(),
        total_bbox_area_mm2: best.extent_area_sum,
        puzzle_area_mm2,
        bbox_area_pct: (best.extent_area_sum / puzzle_area_mm2) * 100.0,
        page_fill_ratios: compute_fill_ratios(&best.pages, usable_area),
    });

    let mut rng = SplitMix64::new(options.seed);
    let mut iter = 1usize;
    loop {
        if let Some(max_iters) = options.max_iters
            && iter > max_iters
        {
            break;
        }
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        let use_tail = best.pages.len() > TAIL_PAGES && rng.gen_range(100) < 80;
        let (prefix_pages, prefix_len): (&[SheetBitmask], usize) = if use_tail {
            let split = best.pages.len() - TAIL_PAGES;
            let plen = best.pages[..split].iter().map(|p| p.slots.len()).sum();
            (&best.pages[..split], plen)
        } else {
            (&[], 0)
        };

        let mut cand_order = best.order.clone();
        let mut cand_rot_prefs = best.rot_prefs.clone();
        mutate_slice(
            &mut cand_order[prefix_len..],
            &mut cand_rot_prefs,
            &mut rng,
        );

        let cand_opt = decode_layout(
            &cand_order[prefix_len..],
            &cand_rot_prefs,
            prefix_pages,
            &variants,
            kerfed,
            &spec,
            false,
        )?;

        let mut emitted = false;
        if let Some(mut cand) = cand_opt {
            cand.order = cand_order;
            cand.rot_prefs = cand_rot_prefs;
            if is_better_layout(&cand, Some(&best)) {
                canonicalize_order(&mut cand);
                best = cand;
                on_progress(NestProgress {
                    rows: kerfed.rows,
                    cols: kerfed.cols,
                    iter,
                    total_iters: options.max_iters,
                    pages: best.pages.len(),
                    total_bbox_area_mm2: best.extent_area_sum,
                    puzzle_area_mm2,
                    bbox_area_pct: (best.extent_area_sum / puzzle_area_mm2) * 100.0,
                    page_fill_ratios: compute_fill_ratios(&best.pages, usable_area),
                });
                emitted = true;
            }
        }

        if !emitted && iter.is_multiple_of(8) && Some(iter) != options.max_iters {
            on_progress(NestProgress {
                rows: kerfed.rows,
                cols: kerfed.cols,
                iter,
                total_iters: options.max_iters,
                pages: best.pages.len(),
                total_bbox_area_mm2: best.extent_area_sum,
                puzzle_area_mm2,
                bbox_area_pct: (best.extent_area_sum / puzzle_area_mm2) * 100.0,
                page_fill_ratios: compute_fill_ratios(&best.pages, usable_area),
            });
        }

        iter += 1;
    }

    // Post-pass compaction: compact non-final marks, and compare Top- vs Left-gravity on final page.
    let total_pages = best.pages.len();
    let mut final_pages = Vec::with_capacity(total_pages);

    for (idx, sheet) in best.pages.iter().enumerate() {
        let is_last = idx + 1 == total_pages;
        if !is_last {
            final_pages.push(compact_bottom_right_mark(
                sheet.page_number,
                &sheet.slots,
                kerfed,
                &spec,
            ));
        } else {
            let top_compacted =
                compact_bottom_right_mark(sheet.page_number, &sheet.slots, kerfed, &spec);
            let mut chosen = top_compacted;

            if let Some(left_slots) = repack_final_page_left_gravity(
                sheet.page_number,
                &sheet.slots,
                &variants,
                &spec,
            ) {
                let left_compacted =
                    compact_bottom_right_mark(sheet.page_number, &left_slots, kerfed, &spec);
                if left_compacted.content_bbox_mm.area()
                    < chosen.content_bbox_mm.area() - 1e-9
                {
                    chosen = left_compacted;
                }
            }

            final_pages.push(chosen);
        }
    }

    let total_bbox_area_mm2: f64 = final_pages.iter().map(|p| p.content_bbox_mm.area()).sum();
    let final_iter = options
        .max_iters
        .map_or(iter.saturating_sub(1), |m| iter.saturating_sub(1).min(m));
    on_progress(NestProgress {
        rows: kerfed.rows,
        cols: kerfed.cols,
        iter: final_iter,
        total_iters: options.max_iters,
        pages: final_pages.len(),
        total_bbox_area_mm2,
        puzzle_area_mm2,
        bbox_area_pct: (total_bbox_area_mm2 / puzzle_area_mm2) * 100.0,
        page_fill_ratios: compute_fill_ratios(&best.pages, usable_area),
    });

    debug_assert_eq!(
        final_pages.iter().map(|p| p.slots.len()).sum::<usize>(),
        n
    );

    Ok(final_pages)
}

fn build_piece_variants(
    piece: &KerfedPiece,
    pad_mm: f64,
    spec: &SheetSpec,
) -> Result<[OrientedVariant; 4], PipelineError> {
    let cut_io: Vec<[f64; 2]> = piece.cut_polygon.iter().map(|p| [p.x, p.y]).collect();
    let bleed_io: Vec<[f64; 2]> = piece.bleed_polygon.iter().map(|p| [p.x, p.y]).collect();
    let union_shapes = cut_io.overlay(&bleed_io, OverlayRule::Union, FillRule::Positive);
    let interior_pt = stage2::find_interior_point(&piece.cut_polygon)
        .or_else(|| stage2::find_interior_point(&piece.bleed_polygon))
        .unwrap_or(piece.cut_polygon[0]);

    let outer_env = stage2::select_outer_ring(union_shapes, interior_pt).ok_or_else(|| {
        PipelineError::new(
            "paginate",
            format!("failed to compute outer envelope for piece \"{}\"", piece.id),
        )
    })?;

    let dilated_poly = if pad_mm > 0.0 {
        let outer_io: Vec<[f64; 2]> = outer_env.iter().map(|p| [p.x, p.y]).collect();
        let style = OutlineStyle::new(pad_mm).line_join(LineJoin::Round(0.1));
        let dil_shapes = outer_io.outline_custom_as::<i64>(&style, stage2::outline_options_i64());
        stage2::select_outer_ring(dil_shapes, interior_pt).ok_or_else(|| {
            PipelineError::new(
                "paginate",
                format!(
                    "failed to dilate outer envelope (+{pad_mm:.4} mm) for piece \"{}\"",
                    piece.id
                ),
            )
        })?
    } else {
        outer_env.clone()
    };

    let mut out = Vec::with_capacity(4);
    for rot in Rotation90::ALL {
        out.push(rasterize_variant(&outer_env, &dilated_poly, rot, spec));
    }

    Ok(out.try_into().expect("exactly 4 orientations"))
}

fn rasterize_variant(
    outer_env: &[Point],
    dilated_poly: &[Point],
    rot: Rotation90,
    spec: &SheetSpec,
) -> OrientedVariant {
    let rot_env: Vec<Point> = outer_env.iter().map(|&p| rot.rotate_point(p)).collect();
    let env_aabb = stage2::polygon_aabb(&rot_env).expect("non-empty outer envelope");
    let env_min_x = env_aabb.x0;
    let env_min_y = env_aabb.y0;
    let env_w = env_aabb.width();
    let env_h = env_aabb.height();
    let outer_env_area = stage2::signed_area(&rot_env).abs();

    let rot_dil: Vec<Point> = dilated_poly.iter().map(|&p| rot.rotate_point(p)).collect();
    let dil_aabb = stage2::polygon_aabb(&rot_dil).expect("non-empty dilated polygon");
    let dilated_aabb_area = dil_aabb.area();

    let max_gx = ((spec.usable_w - env_w).max(0.0) / spec.step_mm)
        .floor()
        .min((spec.usable_cols - 1) as f64) as usize;
    let max_gy = ((spec.usable_h - env_h).max(0.0) / spec.step_mm)
        .floor()
        .min((spec.usable_rows - 1) as f64) as usize;

    let pad_shift = (spec.pad_cells as f64) * spec.step_mm;
    let max_local_x = (dil_aabb.x1 - env_min_x + pad_shift).max(env_w + 2.0 * pad_shift);
    let max_local_y = (dil_aabb.y1 - env_min_y + pad_shift).max(env_h + 2.0 * pad_shift);

    let variant_cols = ((max_local_x / spec.step_mm).ceil() as usize + 1)
        .min(spec.total_cols.saturating_sub(max_gx))
        .max(1)
        .min(spec.total_cols);
    let variant_rows = ((max_local_y / spec.step_mm).ceil() as usize + 1)
        .min(spec.total_rows.saturating_sub(max_gy))
        .max(1)
        .min(spec.total_rows);

    let shifted: Vec<Point> = rot_dil
        .into_iter()
        .map(|p| Point::new(p.x - env_min_x + pad_shift, p.y - env_min_y + pad_shift))
        .collect();

    let words_per_row = variant_cols.div_ceil(64);
    let mut words = vec![0u64; variant_rows * words_per_row];
    rasterize_polygon_conservative(
        &shifted,
        spec.step_mm,
        variant_cols,
        variant_rows,
        words_per_row,
        &mut words,
    );

    let mut row_max_one_run = vec![0u16; variant_rows];
    let mut active_vy_start = variant_rows;
    let mut active_vy_end = 0usize;
    let mut anchor: Option<(usize, usize)> = None;

    for vy in 0..variant_rows {
        let row_slice = &words[vy * words_per_row..(vy + 1) * words_per_row];
        let run = longest_one_run(row_slice, variant_cols);
        row_max_one_run[vy] = run;
        if run > 0 {
            active_vy_start = active_vy_start.min(vy);
            active_vy_end = vy + 1;
            if anchor.is_none() {
                for (w_idx, &w) in row_slice.iter().enumerate() {
                    if w != 0 {
                        let vx = (w_idx << 6) + w.trailing_zeros() as usize;
                        if vx < variant_cols {
                            anchor = Some((vx, vy));
                        }
                        break;
                    }
                }
            }
        }
    }

    OrientedVariant {
        env_min_x,
        env_min_y,
        env_w,
        env_h,
        outer_env_area,
        dilated_aabb_area,
        max_gx,
        max_gy,
        variant_cols,
        variant_rows,
        words_per_row,
        words,
        row_max_one_run,
        active_vy_start,
        active_vy_end,
        anchor,
    }
}

fn rasterize_polygon_conservative(
    poly: &[Point],
    step_mm: f64,
    cols: usize,
    rows: usize,
    words_per_row: usize,
    words: &mut [u64],
) {
    if poly.is_empty() || cols == 0 || rows == 0 {
        return;
    }

    let max_x_idx = (cols - 1) as isize;
    let max_y_idx = (rows - 1) as isize;

    let set_cell = |words: &mut [u64], ix: usize, iy: usize| {
        let row_base = iy * words_per_row;
        words[row_base + (ix >> 6)] |= 1u64 << (ix & 63);
    };

    // 1. Vertices.
    for &p in poly {
        let ix = ((p.x / step_mm).floor() as isize).clamp(0, max_x_idx) as usize;
        let iy = ((p.y / step_mm).floor() as isize).clamp(0, max_y_idx) as usize;
        set_cell(words, ix, iy);
    }

    // 2. Supercover edge traversal.
    let mut ts = Vec::new();
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let ax = a.x / step_mm;
        let ay = a.y / step_mm;
        let bx = b.x / step_mm;
        let by = b.y / step_mm;

        ts.clear();
        ts.push(0.0);
        ts.push(1.0);

        let dx = bx - ax;
        if dx.abs() > 1e-12 {
            let x_lo = ax.min(bx).floor() as isize + 1;
            let x_hi = ax.max(bx).ceil() as isize - 1;
            for kx in x_lo..=x_hi {
                let t = (kx as f64 - ax) / dx;
                if t > 0.0 && t < 1.0 {
                    ts.push(t);
                }
            }
        }

        let dy = by - ay;
        if dy.abs() > 1e-12 {
            let y_lo = ay.min(by).floor() as isize + 1;
            let y_hi = ay.max(by).ceil() as isize - 1;
            for ky in y_lo..=y_hi {
                let t = (ky as f64 - ay) / dy;
                if t > 0.0 && t < 1.0 {
                    ts.push(t);
                }
            }
        }

        ts.sort_by(f64::total_cmp);

        for &t in &ts {
            let cx = ax + dx * t;
            let cy = ay + dy * t;
            let ix0 = ((cx - 1e-9).floor() as isize).clamp(0, max_x_idx) as usize;
            let ix1 = ((cx + 1e-9).floor() as isize).clamp(0, max_x_idx) as usize;
            let iy0 = ((cy - 1e-9).floor() as isize).clamp(0, max_y_idx) as usize;
            let iy1 = ((cy + 1e-9).floor() as isize).clamp(0, max_y_idx) as usize;
            for iy in iy0..=iy1 {
                for ix in ix0..=ix1 {
                    set_cell(words, ix, iy);
                }
            }
        }

        for pair in ts.windows(2) {
            let tm = 0.5 * (pair[0] + pair[1]);
            let cx = ax + dx * tm;
            let cy = ay + dy * tm;
            let ix = (cx.floor() as isize).clamp(0, max_x_idx) as usize;
            let iy = (cy.floor() as isize).clamp(0, max_y_idx) as usize;
            set_cell(words, ix, iy);
        }
    }

    // 3. Scanline interior fill at row centers `(vy + 0.5) * step_mm`.
    let mut xs = Vec::new();
    for vy in 0..rows {
        let y = (vy as f64 + 0.5) * step_mm;
        xs.clear();
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                let t = (y - a.y) / (b.y - a.y);
                xs.push(a.x + t * (b.x - a.x));
            }
        }
        xs.sort_by(f64::total_cmp);
        let row_base = vy * words_per_row;
        for pair in xs.as_chunks::<2>().0 {
            let x0 = ((pair[0] / step_mm).floor() as isize).clamp(0, max_x_idx) as usize;
            let x1 = ((pair[1] / step_mm).floor() as isize).clamp(0, max_x_idx) as usize;
            set_bit_range(&mut words[row_base..row_base + words_per_row], x0, x1);
        }
    }
}

fn set_bit_range(row_words: &mut [u64], x0: usize, x1: usize) {
    if x0 > x1 {
        return;
    }
    let w0 = x0 >> 6;
    let w1 = x1 >> 6;
    let b0 = x0 & 63;
    let b1 = x1 & 63;
    if w0 == w1 {
        let mask = (u64::MAX << b0) & (u64::MAX >> (63 - b1));
        row_words[w0] |= mask;
    } else {
        row_words[w0] |= u64::MAX << b0;
        for word in &mut row_words[w0 + 1..w1] {
            *word = u64::MAX;
        }
        row_words[w1] |= u64::MAX >> (63 - b1);
    }
}

fn longest_one_run(row_words: &[u64], total_cols: usize) -> u16 {
    let mut best = 0usize;
    let mut cur = 0usize;
    let mut remaining = total_cols;

    for &word in row_words {
        if remaining == 0 {
            break;
        }
        let bits = remaining.min(64);
        remaining -= bits;
        let valid_mask = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let mut w = word & valid_mask;
        if w == valid_mask {
            cur += bits;
            best = best.max(cur);
            continue;
        }
        if w == 0 {
            cur = 0;
            continue;
        }
        let mut rem_bits = bits;
        while rem_bits > 0 {
            let z = (w.trailing_zeros() as usize).min(rem_bits);
            if z > 0 {
                cur = 0;
                rem_bits -= z;
                if rem_bits == 0 {
                    break;
                }
                w >>= z;
            }
            let o = (w.trailing_ones() as usize).min(rem_bits);
            cur += o;
            best = best.max(cur);
            rem_bits -= o;
            if rem_bits == 0 {
                break;
            }
            w >>= o;
        }
    }

    best.min(u16::MAX as usize) as u16
}

fn longest_zero_run(row_words: &[u64], total_cols: usize) -> u16 {
    let mut best = 0usize;
    let mut cur = 0usize;
    let mut remaining = total_cols;

    for &word in row_words {
        if remaining == 0 {
            break;
        }
        let bits = remaining.min(64);
        remaining -= bits;
        let valid_mask = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let mut w = word & valid_mask;
        if w == 0 {
            cur += bits;
            best = best.max(cur);
            continue;
        }
        if w == valid_mask {
            cur = 0;
            continue;
        }
        let mut rem_bits = bits;
        while rem_bits > 0 {
            let z = (w.trailing_zeros() as usize).min(rem_bits);
            cur += z;
            best = best.max(cur);
            rem_bits -= z;
            if rem_bits == 0 {
                break;
            }
            w >>= z;
            let o = (w.trailing_ones() as usize).min(rem_bits);
            cur = 0;
            rem_bits -= o;
            if rem_bits == 0 {
                break;
            }
            w >>= o;
        }
    }

    best.min(u16::MAX as usize) as u16
}

impl SheetBitmask {
    fn new(page_number: usize, spec: &SheetSpec) -> Result<Self, PipelineError> {
        let (kw, kh) = paginate::corner_keepout_size_mm(page_number);
        if spec.usable_w < 2.0 * kw + spec.spacing_mm
            && spec.usable_h < 2.0 * kh + spec.spacing_mm
        {
            let min_w = 2.0 * spec.margin_mm + 2.0 * kw + spec.spacing_mm;
            let min_h = 2.0 * spec.margin_mm + 2.0 * kh + spec.spacing_mm;
            return Err(PipelineError::new(
                "paginate",
                format!(
                    "TopLeft and BottomRight corner keep-out boxes overlap on page {page_number} (usable {:.2} x {:.2} mm); minimum required page_size is {min_w:.2}mm x {min_h:.2}mm",
                    spec.usable_w, spec.usable_h
                ),
            ));
        }

        let words_per_row = spec.total_cols.div_ceil(64);
        let words = vec![0u64; spec.total_rows * words_per_row];
        let row_max_free_run = vec![spec.total_cols.min(u16::MAX as usize) as u16; spec.total_rows];

        let mut sheet = Self {
            page_number,
            kw,
            kh,
            total_cols: spec.total_cols,
            total_rows: spec.total_rows,
            words_per_row,
            words,
            row_max_free_run,
            pieces_max_x: spec.margin_mm,
            pieces_max_y: spec.margin_mm,
            cur_extent_w: kw,
            cur_extent_h: kh,
            extent_area_mm2: kw * kh,
            placed_env_area_sum: 0.0,
            slots: Vec::new(),
        };

        let half_sp = 0.5 * spec.spacing_mm;
        sheet.mark_usable_rect(0.0, 0.0, kw + half_sp, kh + half_sp, spec);
        sheet.mark_usable_rect(
            spec.usable_w - kw - half_sp,
            spec.usable_h - kh - half_sp,
            spec.usable_w,
            spec.usable_h,
            spec,
        );

        Ok(sheet)
    }

    fn mark_usable_rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, spec: &SheetSpec) {
        let pad_shift = (spec.pad_cells as f64) * spec.step_mm;
        let px0 = x0 + pad_shift;
        let py0 = y0 + pad_shift;
        let px1 = x1 + pad_shift;
        let py1 = y1 + pad_shift;

        let cx0 = ((px0 / spec.step_mm).floor() as isize).clamp(0, self.total_cols as isize - 1)
            as usize;
        let cx1 = (((px1 / spec.step_mm).ceil() as isize) - 1)
            .max(cx0 as isize)
            .clamp(0, self.total_cols as isize - 1) as usize;
        let cy0 = ((py0 / spec.step_mm).floor() as isize).clamp(0, self.total_rows as isize - 1)
            as usize;
        let cy1 = (((py1 / spec.step_mm).ceil() as isize) - 1)
            .max(cy0 as isize)
            .clamp(0, self.total_rows as isize - 1) as usize;

        for sy in cy0..=cy1 {
            let row_start = sy * self.words_per_row;
            set_bit_range(
                &mut self.words[row_start..row_start + self.words_per_row],
                cx0,
                cx1,
            );
            self.row_max_free_run[sy] = longest_zero_run(
                &self.words[row_start..row_start + self.words_per_row],
                self.total_cols,
            );
        }
    }

    fn try_fit_or_skip_x(
        &self,
        var: &OrientedVariant,
        gx: usize,
        gy: usize,
        last_fail_vy: &mut usize,
    ) -> usize {
        if gx + var.variant_cols > self.total_cols || gy + var.variant_rows > self.total_rows {
            return usize::MAX;
        }
        let word_off = gx >> 6;
        let sh = gx & 63;

        let check_row = |vy: usize| -> usize {
            let s_row =
                &self.words[(gy + vy) * self.words_per_row..(gy + vy + 1) * self.words_per_row];
            let v_row = &var.words[vy * var.words_per_row..(vy + 1) * var.words_per_row];
            for (w_idx, &vw) in v_row.iter().enumerate() {
                if vw == 0 {
                    continue;
                }
                let sw_col = word_off + w_idx;
                let sw = (s_row[sw_col] >> sh)
                    | if sh > 0 && sw_col + 1 < self.words_per_row {
                        s_row[sw_col + 1] << (64 - sh)
                    } else {
                        0
                    };
                let coll = sw & vw;
                if coll != 0 {
                    let b_lo = coll.trailing_zeros();
                    let b_hi = 63 - coll.leading_zeros();
                    let skip_lo = (sw >> b_lo).trailing_ones() as usize;
                    let skip_hi = (sw >> b_hi).trailing_ones() as usize;
                    return skip_lo.max(skip_hi).max(1);
                }
            }
            0
        };

        let probe_vy = *last_fail_vy;
        if probe_vy >= var.active_vy_start && probe_vy < var.active_vy_end {
            let skip = check_row(probe_vy);
            if skip > 0 {
                return skip;
            }
        }

        for vy in var.active_vy_start..var.active_vy_end {
            if vy == probe_vy {
                continue;
            }
            let skip = check_row(vy);
            if skip > 0 {
                *last_fail_vy = vy;
                return skip;
            }
        }
        0
    }

    fn place_variant(
        &mut self,
        piece_index: usize,
        rot: Rotation90,
        var: &OrientedVariant,
        gx: usize,
        gy: usize,
        spec: &SheetSpec,
    ) {
        let word_off = gx >> 6;
        let sh = gx & 63;

        for vy in var.active_vy_start..var.active_vy_end {
            let sy = gy + vy;
            let row_start = sy * self.words_per_row;
            let v_row = &var.words[vy * var.words_per_row..(vy + 1) * var.words_per_row];
            let mut modified = false;
            for (w_idx, &vw) in v_row.iter().enumerate() {
                if vw == 0 {
                    continue;
                }
                modified = true;
                let sw_col = word_off + w_idx;
                self.words[row_start + sw_col] |= vw << sh;
                if sh > 0 && sw_col + 1 < self.words_per_row {
                    self.words[row_start + sw_col + 1] |= vw >> (64 - sh);
                }
            }
            if modified {
                self.row_max_free_run[sy] = longest_zero_run(
                    &self.words[row_start..row_start + self.words_per_row],
                    self.total_cols,
                );
            }
        }

        let placed_dx = ((gx as f64) * spec.step_mm).min((spec.usable_w - var.env_w).max(0.0));
        let placed_dy = ((gy as f64) * spec.step_mm).min((spec.usable_h - var.env_h).max(0.0));
        let offset_mm = Vec2::new(
            spec.margin_mm + placed_dx - var.env_min_x,
            spec.margin_mm + placed_dy - var.env_min_y,
        );

        self.pieces_max_x = self.pieces_max_x.max(spec.margin_mm + placed_dx + var.env_w);
        self.pieces_max_y = self.pieces_max_y.max(spec.margin_mm + placed_dy + var.env_h);
        self.cur_extent_w = (self.pieces_max_x - spec.margin_mm).max(self.kw);
        self.cur_extent_h = (self.pieces_max_y - spec.margin_mm).max(self.kh);
        self.extent_area_mm2 = self.cur_extent_w * self.cur_extent_h;
        self.placed_env_area_sum += var.outer_env_area;

        self.slots.push(PageSlot {
            piece_index,
            rotation: rot,
            offset_mm,
        });
    }
}

fn find_first_fit_on_empty_sheet(sheet: &SheetBitmask, var: &OrientedVariant) -> bool {
    let mut last_fail_vy = (var.active_vy_start + var.active_vy_end) / 2;
    for gy in 0..=var.max_gy {
        if (var.active_vy_start..var.active_vy_end)
            .any(|vy| sheet.row_max_free_run[gy + vy] < var.row_max_one_run[vy])
        {
            continue;
        }
        let mut gx = 0usize;
        while gx <= var.max_gx {
            if let Some((avx, avy)) = var.anchor {
                let ax = gx + avx;
                let sw = sheet.words[(gy + avy) * sheet.words_per_row + (ax >> 6)] >> (ax & 63);
                if (sw & 1) != 0 {
                    gx = gx.saturating_add((sw.trailing_ones() as usize).max(1));
                    continue;
                }
            }
            let skip = sheet.try_fit_or_skip_x(var, gx, gy, &mut last_fail_vy);
            if skip == 0 {
                return true;
            }
            gx = gx.saturating_add(skip);
        }
    }
    false
}

fn piece_unfit_error(
    piece: &KerfedPiece,
    piece_vars: &[OrientedVariant; 4],
    page_number: usize,
    spec: &SheetSpec,
) -> PipelineError {
    let (kw, kh) = paginate::corner_keepout_size_mm(page_number);
    let mut best_area = f64::INFINITY;
    let mut best_dims = (piece_vars[0].env_w, piece_vars[0].env_h);
    let mut best_req = (0.0_f64, 0.0_f64);

    for var in piece_vars {
        let w_a = var.env_w + 2.0 * spec.margin_mm + 2.0 * (kw + spec.spacing_mm);
        let h_a = var.env_h + 2.0 * spec.margin_mm;
        let area_a = w_a * h_a;

        let w_b = var.env_w + 2.0 * spec.margin_mm;
        let h_b = var.env_h + 2.0 * spec.margin_mm + 2.0 * (kh + spec.spacing_mm);
        let area_b = w_b * h_b;

        let (req_w, req_h, area) = if area_a <= area_b {
            (w_a, h_a, area_a)
        } else {
            (w_b, h_b, area_b)
        };
        if area < best_area {
            best_area = area;
            best_dims = (var.env_w, var.env_h);
            best_req = (req_w, req_h);
        }
    }

    PipelineError::new(
        "paginate",
        format!(
            "piece \"{}\" ({:.2} x {:.2} mm) cannot fit on usable sheet {:.2} x {:.2} mm (page {page_number}); minimum required page_size is {:.2}mm x {:.2}mm",
            piece.id, best_dims.0, best_dims.1, spec.usable_w, spec.usable_h, best_req.0, best_req.1
        ),
    )
}

fn find_best_placement_on_sheet(
    sheet: &SheetBitmask,
    piece_vars: &[OrientedVariant; 4],
    pref: Rotation90,
    spec: &SheetSpec,
    gravity: GravityMode,
) -> Option<PlacementScore> {
    let rots = [
        pref,
        pref.next_cw(),
        pref.next_cw().next_cw(),
        pref.prev_cw(),
    ];
    let mut best: Option<PlacementScore> = None;

    for (rot_rank, rot) in rots.into_iter().enumerate() {
        let var = &piece_vars[rot as usize];
        if var.env_w > spec.usable_w + paginate::CONTAINMENT_TOL_MM
            || var.env_h > spec.usable_h + paginate::CONTAINMENT_TOL_MM
        {
            continue;
        }
        let mut last_fail_vy = (var.active_vy_start + var.active_vy_end) / 2;

        match gravity {
            GravityMode::TopFirst => {
                for gy in 0..=var.max_gy {
                    let placed_dy =
                        ((gy as f64) * spec.step_mm).min((spec.usable_h - var.env_h).max(0.0));
                    let row_min_h = (sheet
                        .pieces_max_y
                        .max(spec.margin_mm + placed_dy + var.env_h)
                        - spec.margin_mm)
                        .max(sheet.kh);
                    let min_possible_area = sheet.cur_extent_w.max(var.env_w) * row_min_h;

                    if let Some(cur_best) = best
                        && (min_possible_area > cur_best.extent_area_mm2 + 1e-9
                            || (min_possible_area >= cur_best.extent_area_mm2 - 1e-9
                                && gy > cur_best.gy))
                    {
                        break;
                    }

                    if (var.active_vy_start..var.active_vy_end)
                        .any(|vy| sheet.row_max_free_run[gy + vy] < var.row_max_one_run[vy])
                    {
                        continue;
                    }

                    let mut gx = 0usize;
                    while gx <= var.max_gx {
                        if let Some((avx, avy)) = var.anchor {
                            let ax = gx + avx;
                            let sw = sheet.words[(gy + avy) * sheet.words_per_row + (ax >> 6)]
                                >> (ax & 63);
                            if (sw & 1) != 0 {
                                gx = gx.saturating_add((sw.trailing_ones() as usize).max(1));
                                continue;
                            }
                        }
                        let placed_dx = ((gx as f64) * spec.step_mm)
                            .min((spec.usable_w - var.env_w).max(0.0));
                        let new_w = (sheet
                            .pieces_max_x
                            .max(spec.margin_mm + placed_dx + var.env_w)
                            - spec.margin_mm)
                            .max(sheet.kw);
                        let new_area = new_w * row_min_h;
                        let cand = PlacementScore {
                            extent_area_mm2: new_area,
                            gx,
                            gy,
                            rot_rank,
                            rot,
                        };
                        if !is_better_score(cand, best, gravity) {
                            break;
                        }
                        let skip = sheet.try_fit_or_skip_x(var, gx, gy, &mut last_fail_vy);
                        if skip == 0 {
                            best = Some(cand);
                            break;
                        }
                        gx = gx.saturating_add(skip);
                    }
                }
            }
            GravityMode::LeftFirst => {
                for gx in 0..=var.max_gx {
                    let placed_dx =
                        ((gx as f64) * spec.step_mm).min((spec.usable_w - var.env_w).max(0.0));
                    let col_min_w = (sheet
                        .pieces_max_x
                        .max(spec.margin_mm + placed_dx + var.env_w)
                        - spec.margin_mm)
                        .max(sheet.kw);
                    let min_possible_area = col_min_w * sheet.cur_extent_h.max(var.env_h);

                    if let Some(cur_best) = best
                        && (min_possible_area > cur_best.extent_area_mm2 + 1e-9
                            || (min_possible_area >= cur_best.extent_area_mm2 - 1e-9
                                && gx > cur_best.gx))
                    {
                        break;
                    }

                    for gy in 0..=var.max_gy {
                        let placed_dy = ((gy as f64) * spec.step_mm)
                            .min((spec.usable_h - var.env_h).max(0.0));
                        let new_h = (sheet
                            .pieces_max_y
                            .max(spec.margin_mm + placed_dy + var.env_h)
                            - spec.margin_mm)
                            .max(sheet.kh);
                        let new_area = col_min_w * new_h;
                        let cand = PlacementScore {
                            extent_area_mm2: new_area,
                            gx,
                            gy,
                            rot_rank,
                            rot,
                        };
                        if !is_better_score(cand, best, gravity) {
                            break;
                        }
                        if (var.active_vy_start..var.active_vy_end)
                            .any(|vy| sheet.row_max_free_run[gy + vy] < var.row_max_one_run[vy])
                        {
                            continue;
                        }
                        if let Some((avx, avy)) = var.anchor {
                            let ax = gx + avx;
                            let sw = sheet.words[(gy + avy) * sheet.words_per_row + (ax >> 6)]
                                >> (ax & 63);
                            if (sw & 1) != 0 {
                                continue;
                            }
                        }
                        if sheet.try_fit_or_skip_x(var, gx, gy, &mut last_fail_vy) == 0 {
                            best = Some(cand);
                            break;
                        }
                    }
                }
            }
        }
    }

    best
}

fn is_better_score(
    cand: PlacementScore,
    best: Option<PlacementScore>,
    gravity: GravityMode,
) -> bool {
    let Some(cur) = best else {
        return true;
    };
    if cand.extent_area_mm2 < cur.extent_area_mm2 - 1e-9 {
        return true;
    }
    if cand.extent_area_mm2 > cur.extent_area_mm2 + 1e-9 {
        return false;
    }
    match gravity {
        GravityMode::TopFirst => {
            (cand.gy, cand.gx, cand.rot_rank) < (cur.gy, cur.gx, cur.rot_rank)
        }
        GravityMode::LeftFirst => {
            (cand.gx, cand.gy, cand.rot_rank) < (cur.gx, cur.gy, cur.rot_rank)
        }
    }
}

fn decode_layout(
    order_slice: &[usize],
    rot_prefs: &[Rotation90],
    prefix_pages: &[SheetBitmask],
    variants: &[[OrientedVariant; 4]],
    kerfed: &KerfedGrid,
    spec: &SheetSpec,
    fail_on_unplaceable: bool,
) -> Result<Option<DecodedLayout>, PipelineError> {
    let mut pages = prefix_pages.to_vec();

    for &piece_idx in order_slice {
        let piece_vars = &variants[piece_idx];
        let pref = rot_prefs[piece_idx];
        let start_sheet = pages.len().saturating_sub(MAX_OPEN_PAGES);
        let mut placed = false;

        for sheet in &mut pages[start_sheet..] {
            if let Some(score) = find_best_placement_on_sheet(
                sheet,
                piece_vars,
                pref,
                spec,
                GravityMode::TopFirst,
            ) {
                let var = &piece_vars[score.rot as usize];
                sheet.place_variant(piece_idx, score.rot, var, score.gx, score.gy, spec);
                placed = true;
                break;
            }
        }

        if !placed {
            let next_page_num = pages.len() + 1;
            let mut new_sheet = match SheetBitmask::new(next_page_num, spec) {
                Ok(s) => s,
                Err(err) => {
                    if fail_on_unplaceable {
                        return Err(err);
                    }
                    return Ok(None);
                }
            };
            let Some(score) = find_best_placement_on_sheet(
                &new_sheet,
                piece_vars,
                pref,
                spec,
                GravityMode::TopFirst,
            ) else {
                if fail_on_unplaceable {
                    return Err(piece_unfit_error(
                        &kerfed.pieces[piece_idx],
                        piece_vars,
                        next_page_num,
                        spec,
                    ));
                }
                return Ok(None);
            };
            let var = &piece_vars[score.rot as usize];
            new_sheet.place_variant(piece_idx, score.rot, var, score.gx, score.gy, spec);
            pages.push(new_sheet);
        }
    }

    let extent_area_sum = pages.iter().map(|p| p.extent_area_mm2).sum();
    Ok(Some(DecodedLayout {
        pages,
        order: order_slice.to_vec(),
        rot_prefs: rot_prefs.to_vec(),
        extent_area_sum,
    }))
}

fn build_constructive_starts(
    kerfed: &KerfedGrid,
    variants: &[[OrientedVariant; 4]],
) -> Vec<(Vec<usize>, Vec<Rotation90>)> {
    let n = kerfed.pieces.len();
    let deg0_prefs = vec![Rotation90::Deg0; n];

    // 1. Decreasing dilated AABB area.
    let mut order1: Vec<usize> = (0..n).collect();
    order1.sort_by(|&a, &b| {
        variants[b][0]
            .dilated_aabb_area
            .total_cmp(&variants[a][0].dilated_aabb_area)
            .then_with(|| a.cmp(&b))
    });

    // 2. Border/corner pieces first with straight-edge rotation prefs.
    let rows = kerfed.rows;
    let cols = kerfed.cols;
    let grid_border_edges = |p: &KerfedPiece| -> u8 {
        (p.row == 0) as u8
            + (p.col == 0) as u8
            + (p.row + 1 == rows) as u8
            + (p.col + 1 == cols) as u8
    };
    let mut order2: Vec<usize> = (0..n).collect();
    order2.sort_by(|&a, &b| {
        grid_border_edges(&kerfed.pieces[b])
            .cmp(&grid_border_edges(&kerfed.pieces[a]))
            .then_with(|| {
                variants[b][0]
                    .outer_env_area
                    .total_cmp(&variants[a][0].outer_env_area)
            })
            .then_with(|| a.cmp(&b))
    });
    let rot_prefs2: Vec<Rotation90> = kerfed
        .pieces
        .iter()
        .map(|p| {
            let r = p.row;
            let c = p.col;
            if r == 0 && c == 0 {
                Rotation90::Deg0
            } else if r == 0 && c + 1 == cols && cols > 1 {
                Rotation90::Deg270
            } else if r + 1 == rows && c + 1 == cols && rows > 1 && cols > 1 {
                Rotation90::Deg180
            } else if r + 1 == rows && c == 0 && rows > 1 {
                Rotation90::Deg90
            } else if c + 1 == cols || r + 1 == rows {
                Rotation90::Deg180
            } else {
                Rotation90::Deg0
            }
        })
        .collect();

    // 3. Decreasing max(env_w, env_h).
    let mut order3: Vec<usize> = (0..n).collect();
    order3.sort_by(|&a, &b| {
        let ma = variants[a][0].env_w.max(variants[a][0].env_h);
        let mb = variants[b][0].env_w.max(variants[b][0].env_h);
        mb.total_cmp(&ma).then_with(|| a.cmp(&b))
    });

    // 4. Increasing convexity ratio = outer_env_area / (env_w * env_h).
    let mut order4: Vec<usize> = (0..n).collect();
    order4.sort_by(|&a, &b| {
        let ca = variants[a][0].outer_env_area
            / (variants[a][0].env_w * variants[a][0].env_h).max(1e-12);
        let cb = variants[b][0].outer_env_area
            / (variants[b][0].env_w * variants[b][0].env_h).max(1e-12);
        ca.total_cmp(&cb).then_with(|| a.cmp(&b))
    });

    vec![
        (order1, deg0_prefs.clone()),
        (order2, rot_prefs2),
        (order3, deg0_prefs.clone()),
        (order4, deg0_prefs),
    ]
}

fn is_better_layout(cand: &DecodedLayout, best: Option<&DecodedLayout>) -> bool {
    let Some(cur) = best else {
        return true;
    };
    if cand.pages.len() < cur.pages.len() {
        return true;
    }
    if cand.pages.len() > cur.pages.len() {
        return false;
    }
    cand.extent_area_sum < cur.extent_area_sum - 1e-6
}

fn canonicalize_order(layout: &mut DecodedLayout) {
    layout.order.clear();
    for page in &layout.pages {
        for slot in &page.slots {
            layout.order.push(slot.piece_index);
            layout.rot_prefs[slot.piece_index] = slot.rotation;
        }
    }
}

fn mutate_slice(slice: &mut [usize], rot_prefs: &mut [Rotation90], rng: &mut SplitMix64) {
    if slice.is_empty() {
        return;
    }
    if slice.len() >= 2 {
        let op = rng.gen_range(100);
        if op < 50 {
            let a = rng.gen_range(slice.len());
            let mut b = rng.gen_range(slice.len() - 1);
            if b >= a {
                b += 1;
            }
            slice.swap(a, b);
        } else if op < 80 {
            let from = rng.gen_range(slice.len());
            let mut to = rng.gen_range(slice.len() - 1);
            if to >= from {
                to += 1;
            }
            if from < to {
                slice[from..=to].rotate_left(1);
            } else {
                slice[to..=from].rotate_right(1);
            }
        } else {
            let idx = rng.gen_range(slice.len());
            let p_idx = slice[idx];
            let shift = 1 + rng.gen_range(3);
            for _ in 0..shift {
                rot_prefs[p_idx] = rot_prefs[p_idx].next_cw();
            }
        }
    } else {
        let p_idx = slice[0];
        let shift = 1 + rng.gen_range(3);
        for _ in 0..shift {
            rot_prefs[p_idx] = rot_prefs[p_idx].next_cw();
        }
    }
}

fn compute_fill_ratios(pages: &[SheetBitmask], usable_area: f64) -> Vec<f64> {
    if usable_area <= 0.0 {
        return vec![0.0; pages.len()];
    }
    pages
        .iter()
        .map(|p| (p.placed_env_area_sum / usable_area).clamp(0.0, 1.0))
        .collect()
}

fn repack_final_page_left_gravity(
    page_number: usize,
    slots: &[PageSlot],
    variants: &[[OrientedVariant; 4]],
    spec: &SheetSpec,
) -> Option<Vec<PageSlot>> {
    let mut sheet = SheetBitmask::new(page_number, spec).ok()?;
    for slot in slots {
        let piece_vars = &variants[slot.piece_index];
        let score = find_best_placement_on_sheet(
            &sheet,
            piece_vars,
            slot.rotation,
            spec,
            GravityMode::LeftFirst,
        )?;
        let var = &piece_vars[score.rot as usize];
        sheet.place_variant(slot.piece_index, score.rot, var, score.gx, score.gy, spec);
    }
    Some(sheet.slots)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OrdF64(f64);

impl Eq for OrdF64 {}

impl Ord for OrdF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialOrd for OrdF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn compact_bottom_right_mark(
    page_number: usize,
    slots: &[PageSlot],
    kerfed: &KerfedGrid,
    spec: &SheetSpec,
) -> RawPagePlacement {
    let (kw, kh) = paginate::corner_keepout_size_mm(page_number);
    let spacing = spec.spacing_mm;
    let min_ax = spec.margin_mm + kw;
    let min_ay = spec.margin_mm + kh;
    let fallback = Point::new(
        spec.page_width_mm - spec.margin_mm,
        spec.page_height_mm - spec.margin_mm,
    );

    let mut pieces_max_x = spec.margin_mm;
    let mut pieces_max_y = spec.margin_mm;
    let mut placed_polys: Vec<(Vec<Point>, Vec<Point>, Rect)> = Vec::with_capacity(slots.len());

    for slot in slots {
        let piece = &kerfed.pieces[slot.piece_index];
        let cut: Vec<Point> = piece
            .cut_polygon
            .iter()
            .map(|&p| slot.transform_point(p))
            .collect();
        let bleed: Vec<Point> = piece
            .bleed_polygon
            .iter()
            .map(|&p| slot.transform_point(p))
            .collect();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for &p in cut.iter().chain(bleed.iter()) {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        pieces_max_x = pieces_max_x.max(max_x);
        pieces_max_y = pieces_max_y.max(max_y);
        placed_polys.push((cut, bleed, Rect::new(min_x, min_y, max_x, max_y)));
    }

    let base_x = pieces_max_x.max(min_ax).min(fallback.x);
    let base_y = pieces_max_y.max(min_ay).min(fallback.y);

    let mut xs = vec![base_x, fallback.x];
    let mut ys = vec![base_y, fallback.y];
    for (_, _, aabb) in &placed_polys {
        let sx = aabb.x1 + kw + spacing;
        if sx >= min_ax && sx <= fallback.x {
            xs.push(sx);
        }
        let sy = aabb.y1 + kh + spacing;
        if sy >= min_ay && sy <= fallback.y {
            ys.push(sy);
        }
    }

    let sort_and_dedup = |vals: &mut Vec<f64>, pieces_max: f64| {
        vals.sort_by(|&a, &b| {
            let ea = pieces_max.max(a);
            let eb = pieces_max.max(b);
            ea.total_cmp(&eb).then_with(|| b.total_cmp(&a))
        });
        let mut deduped: Vec<f64> = Vec::with_capacity(vals.len());
        for &v in vals.iter() {
            if deduped.last().is_none_or(|&prev| (prev - v).abs() > 1e-9) {
                deduped.push(v);
            }
        }
        *vals = deduped;
    };

    sort_and_dedup(&mut xs, pieces_max_x);
    sort_and_dedup(&mut ys, pieces_max_y);

    let tl_rect = Rect::new(
        spec.margin_mm,
        spec.margin_mm,
        spec.margin_mm + kw,
        spec.margin_mm + kh,
    );

    let is_valid_anchor = |br_x: f64, br_y: f64, tol: f64| -> bool {
        let br_rect = Rect::new(br_x - kw, br_y - kh, br_x, br_y);
        let gap_x = (br_rect.x0 - tl_rect.x1).max(tl_rect.x0 - br_rect.x1);
        let gap_y = (br_rect.y0 - tl_rect.y1).max(tl_rect.y0 - br_rect.y1);
        if gap_x < spacing - tol && gap_y < spacing - tol {
            return false;
        }
        for (cut, bleed, aabb) in &placed_polys {
            let p_gap_x = (br_rect.x0 - aabb.x1).max(aabb.x0 - br_rect.x1);
            let p_gap_y = (br_rect.y0 - aabb.y1).max(aabb.y0 - br_rect.y1);
            let req_gap = (spacing - tol).max(1e-9);
            if p_gap_x >= req_gap || p_gap_y >= req_gap {
                continue;
            }
            if !paginate::polygon_rect_clearance_ok(cut, br_rect, spacing, tol)
                || !paginate::polygon_rect_clearance_ok(bleed, br_rect, spacing, tol)
            {
                return false;
            }
        }
        true
    };

    type AnchorKey = (OrdF64, Reverse<OrdF64>, usize, usize);
    let make_key = |i: usize, j: usize| -> AnchorKey {
        let br_x = xs[i];
        let br_y = ys[j];
        let w = pieces_max_x.max(br_x) - spec.margin_mm;
        let h = pieces_max_y.max(br_y) - spec.margin_mm;
        (OrdF64(w * h), Reverse(OrdF64(br_x + br_y)), i, j)
    };

    let mut heap: BinaryHeap<Reverse<AnchorKey>> = BinaryHeap::new();
    if !xs.is_empty() && !ys.is_empty() {
        heap.push(Reverse(make_key(0, 0)));
    }

    let mut chosen = fallback;
    while let Some(Reverse((_, _, i, j))) = heap.pop() {
        let br_x = xs[i];
        let br_y = ys[j];
        let is_fallback = (br_x - fallback.x).abs() < 1e-9 && (br_y - fallback.y).abs() < 1e-9;
        if is_fallback {
            let _ = is_valid_anchor(fallback.x, fallback.y, paginate::CONTAINMENT_TOL_MM);
            chosen = fallback;
            break;
        }
        if is_valid_anchor(br_x, br_y, 1e-9) {
            chosen = Point::new(br_x, br_y);
            break;
        }
        if j == 0 && i + 1 < xs.len() {
            heap.push(Reverse(make_key(i + 1, 0)));
        }
        if j + 1 < ys.len() {
            heap.push(Reverse(make_key(i, j + 1)));
        }
    }

    let content_bbox_mm = Rect::new(
        spec.margin_mm,
        spec.margin_mm,
        pieces_max_x.max(chosen.x),
        pieces_max_y.max(chosen.y),
    );

    RawPagePlacement {
        slots: slots.to_vec(),
        br_anchor_mm: chosen,
        content_bbox_mm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CoordFrame, LengthUnit};

    fn rect_poly(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    fn make_kerfed(pieces: Vec<KerfedPiece>, bounds: Rect) -> KerfedGrid {
        KerfedGrid {
            frame: CoordFrame {
                doc_unit: LengthUnit::Mm,
                viewbox_min_user: (0.0, 0.0),
                user_to_mm: (1.0, 1.0),
                doc_to_mm: 1.0,
            },
            image_href: String::new(),
            image_bounds_mm: bounds,
            puzzle_bounds_mm: bounds,
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
    fn rasterizer_covers_vertices_edges_and_interior() {
        let step_mm = 0.5;
        let cols = 12usize;
        let rows = 12usize;
        let words_per_row = cols.div_ceil(64);
        let mut words = vec![0u64; rows * words_per_row];
        let tri = vec![
            Point::new(0.2, 0.2),
            Point::new(5.3, 1.1),
            Point::new(2.7, 5.4),
        ];
        rasterize_polygon_conservative(&tri, step_mm, cols, rows, words_per_row, &mut words);

        let is_set = |ix: usize, iy: usize| -> bool {
            (words[iy * words_per_row + (ix >> 6)] >> (ix & 63)) & 1 != 0
        };

        // Vertices.
        assert!(is_set(0, 0));
        assert!(is_set(10, 2));
        assert!(is_set(5, 10));
        // Midpoint along top edge (2.75, 0.65) -> cell (5, 1).
        assert!(is_set(5, 1));
        // Interior cell (5, 4) -> center (2.75, 2.25).
        assert!(is_set(5, 4));
        // Outside cell (11, 11).
        assert!(!is_set(11, 11));
    }

    #[test]
    fn splitmix64_is_deterministic() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        let mut c = SplitMix64::new(43);

        let seq_a: Vec<u64> = (0..16).map(|_| a.next_u64()).collect();
        let seq_b: Vec<u64> = (0..16).map(|_| b.next_u64()).collect();
        let seq_c: Vec<u64> = (0..16).map(|_| c.next_u64()).collect();
        assert_eq!(seq_a, seq_b);
        assert_ne!(seq_a, seq_c);

        for upper in [1, 2, 7, 100] {
            assert_eq!(a.gen_range(upper), b.gen_range(upper));
        }
    }

    #[test]
    fn single_piece_rotates_90_deg_to_fit_narrow_sheet() {
        // 45 mm wide x 20 mm tall piece cannot fit upright in 38.1 mm x 101.6 mm (1.5in x 4in),
        // even with 0.25in margins (usable 25.4 mm x 88.9 mm), so it must rotate 90 or 270 deg.
        let piece = KerfedPiece {
            row: 0,
            col: 0,
            id: "wide".to_string(),
            cut_aabb_mm: Rect::new(0.0, 0.0, 45.0, 20.0),
            cut_polygon: rect_poly(0.0, 0.0, 45.0, 20.0),
            bleed_polygon: rect_poly(0.0, 0.0, 45.0, 20.0),
        };
        let kerfed = make_kerfed(vec![piece], Rect::new(0.0, 0.0, 45.0, 20.0));
        let cfg = Config {
            page_width_mm: LengthUnit::In.to_mm(1.5),
            page_height_mm: LengthUnit::In.to_mm(4.0),
            margin_mm: LengthUnit::In.to_mm(0.25),
            ..Config::default()
        };
        let layout = paginate::paginate_pieces(&kerfed, &cfg).unwrap();
        assert_eq!(layout.pages.len(), 1);
        assert!(matches!(
            layout.pages[0].slots[0].rotation,
            Rotation90::Deg90 | Rotation90::Deg270
        ));
    }

    #[test]
    fn post_pass_compacts_bottom_right_mark_and_breaks_ties() {
        // Sheet 100 x 100, margin 5 -> usable [5, 95] x [5, 95], fallback (95, 95).
        // Keepout page 1 is 4.10 x 6.00.
        // Piece spans [12, 30] x [12, 30] (completely outside TopLeft [5, 9.1] x [5, 11]),
        // so base = (30, 30) overlaps piece at [25.9, 30] x [24, 30].
        // Valid 1D X shift: (30 + 4.1 + 0.18, 30) = (34.28, 30) -> area 29.28 * 25 = 732.0.
        // Valid 1D Y shift: (30, 30 + 6.0 + 0.18) = (30, 36.18) -> area 25 * 31.18 = 779.5.
        // Tightest area is 1D X shift (34.28, 30).
        let cfg = Config {
            page_width_mm: 100.0,
            page_height_mm: 100.0,
            margin_mm: 5.0,
            piece_spacing_mm: 0.18,
            ..Config::default()
        };
        let spec = SheetSpec {
            page_width_mm: cfg.page_width_mm,
            page_height_mm: cfg.page_height_mm,
            margin_mm: cfg.margin_mm,
            usable_w: 90.0,
            usable_h: 90.0,
            spacing_mm: cfg.piece_spacing_mm,
            step_mm: 0.18,
            usable_cols: 500,
            usable_rows: 500,
            pad_cells: 2,
            total_cols: 504,
            total_rows: 504,
        };
        let p = KerfedPiece {
            row: 0,
            col: 0,
            id: "p0".to_string(),
            cut_aabb_mm: Rect::new(12.0, 12.0, 30.0, 30.0),
            cut_polygon: rect_poly(12.0, 12.0, 30.0, 30.0),
            bleed_polygon: rect_poly(12.0, 12.0, 30.0, 30.0),
        };
        let kerfed = make_kerfed(vec![p], Rect::new(0.0, 0.0, 18.0, 18.0));
        let slots = vec![PageSlot {
            piece_index: 0,
            rotation: Rotation90::Deg0,
            offset_mm: Vec2::ZERO,
        }];
        let res = compact_bottom_right_mark(1, &slots, &kerfed, &spec);
        assert!((res.br_anchor_mm.x - 34.28).abs() < 1e-6);
        assert!((res.br_anchor_mm.y - 30.0).abs() < 1e-6);

        // Tie-breaking test: piece is tiny [12, 13] x [12, 13] and another overhang piece is
        // at [40, 50] x [5, 10] and [5, 10] x [40, 50]. pieces_max = (50, 50), so base = (50, 50)
        // clears both pieces and ties any smaller anchor with area = 45 * 45 while maximizing br_x + br_y = 100.
        let p_a = KerfedPiece {
            row: 0,
            col: 0,
            id: "pa".to_string(),
            cut_aabb_mm: Rect::new(40.0, 5.0, 50.0, 10.0),
            cut_polygon: rect_poly(40.0, 5.0, 50.0, 10.0),
            bleed_polygon: rect_poly(40.0, 5.0, 50.0, 10.0),
        };
        let p_b = KerfedPiece {
            row: 0,
            col: 1,
            id: "pb".to_string(),
            cut_aabb_mm: Rect::new(5.0, 40.0, 10.0, 50.0),
            cut_polygon: rect_poly(5.0, 40.0, 10.0, 50.0),
            bleed_polygon: rect_poly(5.0, 40.0, 10.0, 50.0),
        };
        let kerfed_tie = make_kerfed(vec![p_a, p_b], Rect::new(0.0, 0.0, 45.0, 45.0));
        let tie_slots = vec![
            PageSlot {
                piece_index: 0,
                rotation: Rotation90::Deg0,
                offset_mm: Vec2::ZERO,
            },
            PageSlot {
                piece_index: 1,
                rotation: Rotation90::Deg0,
                offset_mm: Vec2::ZERO,
            },
        ];
        let tie_res = compact_bottom_right_mark(1, &tie_slots, &kerfed_tie, &spec);
        assert!((tie_res.br_anchor_mm.x - 50.0).abs() < 1e-6);
        assert!((tie_res.br_anchor_mm.y - 50.0).abs() < 1e-6);

        // 2D shift test: overhang [40, 50] x [5, 10], b1 [15, 25] x [20, 30],
        // b2 [25, 50] x [24.5, 30] (blocks all x<=50 at y=30), b3 [46, 50] x [14, 30] (blocks x=50 at y=16.18).
        // base=(50,30) and both 1D shifts hit pieces; 2D shift (29.28, 16.18) tucks into empty pocket
        // keeping content_bbox=[5,50]x[5,30] (area 45*25 = 1125.0).
        let p_over = KerfedPiece {
            row: 0,
            col: 0,
            id: "over".to_string(),
            cut_aabb_mm: Rect::new(40.0, 5.0, 50.0, 10.0),
            cut_polygon: rect_poly(40.0, 5.0, 50.0, 10.0),
            bleed_polygon: rect_poly(40.0, 5.0, 50.0, 10.0),
        };
        let p_b1 = KerfedPiece {
            row: 0,
            col: 1,
            id: "b1".to_string(),
            cut_aabb_mm: Rect::new(15.0, 20.0, 25.0, 30.0),
            cut_polygon: rect_poly(15.0, 20.0, 25.0, 30.0),
            bleed_polygon: rect_poly(15.0, 20.0, 25.0, 30.0),
        };
        let p_b2 = KerfedPiece {
            row: 0,
            col: 2,
            id: "b2".to_string(),
            cut_aabb_mm: Rect::new(25.0, 24.5, 50.0, 30.0),
            cut_polygon: rect_poly(25.0, 24.5, 50.0, 30.0),
            bleed_polygon: rect_poly(25.0, 24.5, 50.0, 30.0),
        };
        let p_b3 = KerfedPiece {
            row: 0,
            col: 3,
            id: "b3".to_string(),
            cut_aabb_mm: Rect::new(46.0, 14.0, 50.0, 30.0),
            cut_polygon: rect_poly(46.0, 14.0, 50.0, 30.0),
            bleed_polygon: rect_poly(46.0, 14.0, 50.0, 30.0),
        };
        let kerfed_2d = make_kerfed(
            vec![p_over, p_b1, p_b2, p_b3],
            Rect::new(0.0, 0.0, 45.0, 25.0),
        );
        let slots_2d = (0..4)
            .map(|piece_index| PageSlot {
                piece_index,
                rotation: Rotation90::Deg0,
                offset_mm: Vec2::ZERO,
            })
            .collect::<Vec<_>>();
        let res_2d = compact_bottom_right_mark(1, &slots_2d, &kerfed_2d, &spec);
        assert!((res_2d.br_anchor_mm.x - 29.28).abs() < 1e-6);
        assert!((res_2d.br_anchor_mm.y - 16.18).abs() < 1e-6);
        assert!((res_2d.content_bbox_mm.area() - 1125.0).abs() < 1e-6);
    }
}
