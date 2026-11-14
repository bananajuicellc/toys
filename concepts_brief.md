# `puzzle-gen`: Stage 3 & 4 Irregular Polygon Nesting and Scrap Compaction

* **Author:** Tim Sweña (Swast)
* **Date:** 2026-11-13

## Overview

`puzzle-gen` currently lays out puzzle pieces on Stage 3 print sheets and Stage 4 laser cut sheets using a uniform rectangular grid. Every piece is assigned a bounding box sized to the worst-case double-tabbed piece across the entire puzzle, and two full-width `6.0 mm` horizontal bands are reserved across the top and bottom of every sheet for corner registration marks. In addition, the default `6 in x 6 in` page size wastes 61.5% of an `8.5 in x 11 in` adhesive sticker sheet before layout even begins.

This proposal updates the default `page_size` to `8.5in x 11in`, preserves Stage 2 (`<prefix>-stage2.svg`) as an upright `(row, col)` inspection grid, and replaces the Stage 3 and Stage 4 uniform grid pagination with 2D irregular polygon bin packing across right-angle rotations (`0°`, `90°`, `180°`, `270°`) and localized corner registration keep-out boxes. Both the placed pieces and the bottom-right registration mark on each page are compacted tightly toward the top-left origin so any unused portion of a sheet remains completely blank and unscored for reuse. The solver runs a fixed deterministic iteration count by default for test reproducibility and offers an `--interactive` terminal convergence view plotting total page bounding-box area against the original puzzle area.

## Analysis

### 1. Eliminating Artificial Sheet Reservations (Corner Bands and Global Slots)
In `src/puzzle-gen/src/paginate.rs`, `CORNER_BAND_MM = 6.0` subtracts `12.0 mm` of usable height across the entire width of every page (`~2,438 mm²` on an `8.5 in x 11 in` sheet), even though the top-left and bottom-right page numbers and L-ticks only occupy two small corner boxes (`~4.1 mm x 6.0 mm` for single-digit pages, `~7.0 mm x 6.0 mm` for double-digit pages). Replacing the full-width bands with two localized corner keep-out rectangles inside `[margin_mm, page_width_mm - margin_mm] x [margin_mm, page_height_mm - margin_mm]` recovers over 96% of the top and bottom band width immediately.

Similarly, `compute_slot_bounds` inflates every piece slot to `2 * max_half_w x 2 * max_half_h` centered on `cut_aabb.center()`. On the `4 in x 4 in` 25-piece example (`20.32 mm x 20.32 mm` nominal cells), every piece reserves `37.15 mm x 35.54 mm` (`1,320 mm²`, over `3.2x` the nominal cell area). Moving to irregular polygon footprints allows straight border edges to sit flush against the page margin and allows protruding tabs (`~5–8 mm`) to interleave into adjacent concave sockets or into the empty corner shoulders beside neighboring tabs.

### 2. Algorithmic Contradiction: $O(N^2)$ Continuous NFP Complexity vs. 1,000-Piece Scaling
For a `50 cm x 70 cm` puzzle (`~875–1,000` unique hand-drawn pieces) with 4 cardinal rotations (`3,500–4,000` oriented polygons), precomputing exact continuous pairwise No-Fit Polygons (NFPs) requires up to $8 \times 10^6$ non-convex polygon convolutions, which exceeds a 60-second budget and risks numerical degeneracy on 100-vertex flattened Bezier contours.

To resolve this without sacrificing exact clearance guarantees, the nesting engine uses a two-tier hybrid representation:
1. **Conservative Bitmask Skyline Precomputation ($O(N)$ at `1 kerf` Grid Step):** For each piece, take its outer physical footprint (`cut_polygon ∪ bleed_polygon`) for margin containment and buffer it outward by `piece_spacing_mm / 2.0` using `i_overlay` `OutlineOffset` (`LineJoin::Round`). Pre-rotate each piece into `0°`, `90°`, `180°`, and `270°` and rasterize the buffered polygon conservatively onto a 2D bitset (`Vec<u64>`) at a grid step equal to `1 kerf` (`0.18 mm` by default, with a safe positive floor such as `segment_length_mm.max(0.10)` when `kerf == 0 mm`). Any grid cell intersecting the polygon interior or boundary is set to `1`. Because the bitmask strictly over-approximates the outward-buffered polygon, bitwise disjointness (`(mask_a & mask_b) == 0`) guarantees continuous Euclidean clearance `>= piece_spacing_mm` using 64-bit word operations (`u64::trailing_zeros`).
2. **Constructive Multi-Pass Baseline + Seeded Fixed-Iteration Local Search:** First run a deterministic constructive pass across several piece orderings (decreasing AABB area, border/corner pieces first, decreasing convexity defect) placing each piece at the best `(page, y, x, rotation)` position. Then run a seeded local search (`--seed <U64>`, default `0`) for a fixed deterministic iteration budget (continuing until user key-press stop when `--interactive` is enabled) that focuses repacking on the tail pages to reduce page count and shrink the sum of per-page bounding-box areas.

### 3. Compacting the Bottom-Right Registration Mark With Page Content
If the `BottomRight` registration mark remained anchored at the far sheet corner `(page_width_mm - margin_mm, page_height_mm - margin_mm)` on a partially filled page, it would print and score inside the otherwise empty offcut region.

Instead, each page reserves the `TopLeft` registration mark at `(margin_mm, margin_mm)` and compacts the `BottomRight` registration mark inward to hug the bottom-right corner of the placed pieces on that page:
* Given the placed pieces' bounding box `[margin_mm, pieces_max_x] x [margin_mm, pieces_max_y]` on a page, the `BottomRight` mark anchor `(br_x, br_y)` is placed at the minimum coordinates `br_x >= pieces_max_x`, `br_y >= pieces_max_y` (within `[margin_mm + w_keepout, page_width_mm - margin_mm] x [margin_mm + 6.0, page_height_mm - margin_mm]`) such that its keep-out box `[br_x - w_keepout, br_x] x [br_y - 6.0, br_y]` does not intersect any placed piece on that page.
* Both Stage 3 (`#000000`) and Stage 4 (`score_color`) render the page's `BottomRight` mark at this identical compacted `(br_x, br_y)` coordinate, leaving the entire unused sheet area outside `[margin_mm, br_x] x [margin_mm, br_y]` completely unprinted and unscored for straight-cut scrap reuse.

### 4. Unified Bounding-Box Area Metric & Objective
The layout objective, CLI summary metric, and `--interactive` convergence chart all measure the **total page bounding-box area** compared against the **original Stage 0 puzzle area**:
* Let $A_{\text{puzzle}}$ be the original Stage 0 puzzle grid area (`grid_width_mm * grid_height_mm`).
* For each page $p \in 1..=P$, let $B_p$ be the axis-aligned bounding box enclosing all placed pieces (`cut_polygon ∪ bleed_polygon`) and both corner registration marks on page $p$, and let $A_{\text{bbox}} = \sum_{p=1}^{P} \text{area}(B_p)$.
* Minimizing $A_{\text{bbox}}$ (with primary priority on minimizing total page count $P$) directly rewards both eliminating tail pages and compacting pieces plus the bottom-right registration mark into the tightest possible rectangle on every page.
* The CLI and `--interactive` view report `bbox_area_ratio_pct` ($\frac{A_{\text{bbox}}}{A_{\text{puzzle}}} \times 100\%$, showing total page bounding-box area as a percentage of the original puzzle size, where lower is tighter) alongside `packing_efficiency_pct` ($\frac{A_{\text{puzzle}}}{A_{\text{bbox}}} \times 100\%$).

### 5. Exact Right-Angle Rotation Semantics Across Stage 3 (`<pattern>`) and Stage 4
In `src/puzzle-gen/src/svg_emit.rs`, Stage 3 fills pieces using `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse">` defined in original puzzle coordinates, while Stage 4 emits ungrouped, transform-free `<path d="...">` coordinates for laser CAM compatibility.

Defining right-angle rotations (`Rotation90::{Deg0, Deg90, Deg180, Deg270}`) around the canonical origin `(0, 0)` in SVG `+y`-down coordinates (`(x, y)`, `(-y, x)`, `(-x, -y)`, `(y, -x)`) followed by translation `offset_mm`:
* Trigonometric sin/cos values are exact integers (`-1`, `0`, `1`), introducing zero floating-point drift during rotation.
* Uniform unit scaling (`mm` to `doc_unit`) commutes with origin rotation.
* In Stage 3, `<path d="...">` remains in unshifted original puzzle coordinates while its parent `<g>` uses `transform="translate(tx, ty) rotate(90|180|270)"`, rotating both the piece bleed contour and its `#puzzle-pattern` artwork slice together.
* In Stage 4, `slot.rotation.rotate_point(p) + slot.offset_mm` is baked directly into `<path d="...">` vertices, preserving the `<= 1.5e-4 mm` physical coordinate parity between Stage 3 and Stage 4.

## Goals vs. Non-Goals

* **In-Scope Goals:**
  * Change default `page_size` in `Config::default()` and `examples/default-config.yaml` from `6in x 6in` to `8.5in x 11in`.
  * Keep Stage 2 (`<prefix>-stage2.svg`) as an upright, unscrambled `(row, col)` inspection grid with `GridLabels`.
  * Replace full-width top/bottom `CORNER_BAND_MM` page reservations in Stages 3 and 4 with localized top-left and bottom-right corner keep-out boxes, and compact each page's bottom-right registration mark to hug the bottom-right bounds of the placed pieces on that page.
  * Implement 2D irregular polygon bin packing for Stages 3 and 4 using a conservative `1 kerf` bitmask skyline packer across `0°`, `90°`, `180°`, and `270°` piece rotations while maintaining `>= piece_spacing_mm` clearance between all `cut_polygon ∪ bleed_polygon` footprints.
  * Optimize layout to minimize total page count $P$ and total per-page content bounding-box area $A_{\text{bbox}} = \sum_{p=1}^{P} \text{area}(B_p)$, reporting total page bounding-box area vs. original puzzle area as a percentage in CLI output.
  * Maintain Stage 3 (`<g transform="translate(...) rotate(...)">` + `#puzzle-pattern`) and Stage 4 (baked transform-free `<path d="...">`) physical coordinate parity within `<= 1.5e-4 mm`.
  * Run a fixed deterministic iteration budget with `--seed <U64>` (default `0`) in non-interactive mode, and provide an `--interactive` CLI flag that displays a live terminal convergence chart on `stderr` and allows stopping early (`q`/`Enter`/`Ctrl+C`) to write the best verified layout found so far.
  * Upgrade `verify_page_invariants` to validate exact polygon-to-polygon clearance, margin containment, and corner keep-out avoidance.

* **Out-of-Scope Non-Goals:**
  * Arbitrary non-`90°` free-angle piece rotations (rejected to preserve pixel-grid alignment on printed artwork).
  * Wall-clock timeout flags (`--optimize-time`); non-interactive runs use fixed iteration counts for deterministic testability, and interactive runs stop on user input.
  * Multi-sheet-size heterogeneous bin packing within a single CLI run (scrap strips such as `3.5in x 11in` are handled by running `puzzle-gen` with a custom `page_size`).
  * Modifying Stage 1 or Stage 2 piece geometry, kerf/bleed offsetting rules, or Stage 2 grid ordering.
  * GUI or browser-based interactive manual piece dragging.

## Alternatives Weighed

* **Keep `6 in x 6 in` default page size:** Rejected by author: only one `6 in x 6 in` page fits on an `8.5 in x 11 in` sticker sheet, wasting 61.5% of the sticker stock before layout begins; switching the default to `8.5 in x 11 in` uses the full sticker sheet and leaves a reusable straight-cut strip on `12 in x 12 in` chipboard.
* **Arbitrary continuous-angle rotations (`0°..360°`):** Rejected by author: right-angle rotations (`0°`, `90°`, `180°`, `270°`) are required to keep piece artwork aligned with the printer and image pixel grid.
* **Preserve row-major piece ordering on Stage 3 and Stage 4 sheets:** Rejected by author: scrambling piece positions and right-angle orientations across pages is acceptable to reduce material waste, provided Stage 2 remains an unscrambled `(row, col)` inspection grid.
* **Keep the bottom-right registration mark fixed at `(page_width_mm - margin_mm, page_height_mm - margin_mm)` on partially filled pages:** Rejected by author: anchoring the bottom-right mark at the far sheet corner scores into the empty offcut region; compacting the bottom-right registration mark to the bottom-right bounds of the placed pieces keeps the offcut completely blank and unscored.
* **Wall-clock optimization timeout (`--optimize-time <SECONDS>`):** Rejected by author: fixed iteration counts with a deterministic seed provide reproducible test runs, while `--interactive` lets the user watch convergence and stop manually whenever satisfied.
* **Per-piece tight AABB rectangle packing without polygon nesting:** Considered during review and rejected as the primary solver: ~30–40% of a tabbed piece's bounding box consists of empty corner shoulders beside protruding tabs; rectangle-only packing cannot interleave tabs into neighboring empty shoulders or concave sockets.
* **All-pairs continuous No-Fit Polygon (NFP) precomputation ($O((4N)^2)$):** Considered during review and rejected for 1,000-piece scale: computing millions of exact non-convex pairwise NFPs for unique hand-drawn pieces is too slow; a conservative 2D bitmask skyline representation (`1 kerf` grid step) paired with tail-page local search bounds precomputation to $O(N)$ while preserving exact clearance guarantees.

## Key Decisions & Staging for Next Steps

* **Default Sheet Size:** `8.5in x 11in` (`215.9 mm x 279.4 mm`) in both `Config::default()` and `examples/default-config.yaml`.
* **Allowed Orientations:** Cardinal right-angle rotations (`0°`, `90°`, `180°`, `270°`) around canonical `(0, 0)` + translation `offset_mm` stored on `PageSlot`.
* **Bitmask Resolution:** `1 kerf` (`0.18 mm` default, with a safe positive floor when `kerf == 0 mm`).
* **Compacted Registration Marks:** `TopLeft` remains anchored at `(margin_mm, margin_mm)`, while `BottomRight` compacts to the tightest collision-free bottom-right corner `(br_x, br_y)` enclosing the placed pieces on each page.
* **Primary Layout Metric:** Total per-page content bounding-box area ($\sum_{p=1}^{P} \text{area}(B_p)$) compared against the original Stage 0 puzzle area ($A_{\text{puzzle}}$) as a percentage.
* **CLI Controls:** `--seed <U64>` (default `0`, fixed deterministic iteration count in non-interactive runs) and `--interactive` (live `stderr` convergence chart of the bounding-box area percentage with early stop on key-press).
