# `puzzle-gen`: Stage 3 & 4 Irregular Polygon Nesting and Scrap Compaction

*   **Author:** Tim Sweña (Swast)
*   **Status:** Draft
*   **Date:** 2026-11-13

--------------------------------------------------------------------------------

## Overview

`puzzle-gen` is a local Rust CLI and geometric pipeline in [`src/puzzle-gen`](./src/puzzle-gen) that prepares custom jigsaw puzzle artwork drawn in Inkscape for physical fabrication: printing artwork onto adhesive sticker sheets via Inkscape multi-page PDF export (Stage 3), adhering the sheets to chipboard backing, and cutting individual pieces on a laser cutter (Stage 4).

Today, Stages 3 and 4 arrange pieces on a uniform rectangular grid where every piece occupies a worst-case global bounding box and two full-width horizontal bands are reserved across the top and bottom of every sheet. On standard `8.5 in x 11 in` adhesive sticker stock and `12 in x 12 in` chipboard, the current `6 in x 6 in` default page size and uniform grid pagination waste most of the material on every sheet.

This design updates the default `page_size` to `8.5in x 11in`, keeps Stage 2 (`<prefix>-stage2.svg`) as an upright `(row, col)` inspection grid, and replaces Stage 3 and Stage 4 uniform grid pagination with 2D irregular polygon bin packing across right-angle rotations (`0°`, `90°`, `180°`, `270°`) and localized corner registration keep-out boxes. The layout engine minimizes the total number of sheets and compacts both the placed pieces and the bottom-right registration mark on each page toward the top-left origin so any unused portion of a sheet stays unprinted and unscored for straight-cut scrap reuse. It runs a fixed deterministic iteration budget by default for reproducible tests and adds an `--interactive` CLI flag that displays a live terminal convergence chart of total page bounding-box area relative to the original puzzle area.

## The Problem

Fabricating a custom jigsaw puzzle—ranging from a `4 in x 4 in` (`25`-piece) test puzzle up to a `50 cm x 70 cm` (`28x36`, `1,008`-piece) puzzle—on `8.5 in x 11 in` adhesive sticker paper and `12 in x 12 in` chipboard currently suffers from four compounding sources of material waste in Stages 3 and 4:

*   **Default sheet size mismatch with sticker stock:** The current default `page_size` is `6 in x 6 in` (`36 sq in`). Because `6 + 6 = 12 in` exceeds both dimensions of an `8.5 in x 11 in` (`93.5 sq in`) sticker sheet, only one `6 in x 6 in` page can be cut from each sticker sheet, discarding 61.5% of the adhesive label stock before `puzzle-gen` places a single piece.
*   **Full-width horizontal registration bands:** `paginate_pieces` in [`src/puzzle-gen/src/paginate.rs`](./src/puzzle-gen/src/paginate.rs) subtracts two full-width `6.0 mm` horizontal bands (`12.0 mm` total height) across the entire width of every page (`~2,438 mm²` on an `8.5 in x 11 in` sheet), even though the top-left and bottom-right vector page numbers and L-ticks only occupy two small corner rectangles (`~4.1 mm x 6.0 mm` for 1-digit pages, `~7.0 mm x 6.0 mm` for 2-digit pages).
*   **Global worst-case rectangular slot inflation:** `compute_slot_bounds` assigns every piece on the sheet a uniform rectangular slot equal to `2 * max_half_w x 2 * max_half_h` around `cut_aabb.center()` across the entire puzzle. On the included `4 in x 4 in` (`16 sq in`, 25-piece) example where each grid cell is `20.32 mm x 20.32 mm` (`413 mm²`), every piece is allocated a `37.15 mm x 35.54 mm` (`1,320 mm²`) slot—over `3.2x` the nominal cell area—even for straight-edged border pieces or pieces with inward sockets. Furthermore, ~30–40% of a tabbed piece's bounding box consists of empty corner shoulders beside protruding tabs where neighboring pieces' tabs could interleave. As a result, a `16 sq in` puzzle spills across three `6 in x 6 in` sheets (`108 sq in` total, ~85% waste).
*   **Uncompacted tail pages and fixed bottom-right registration marks:** On partially filled pages, leaving the bottom-right registration mark fixed at `(page_width_mm - margin_mm, page_height_mm - margin_mm)` prints and scores into the far corner of the sheet, preventing the operator from severing the unused portion of the sticker sheet and chipboard with a single straight guillotine or laser cut for reuse on smaller runs.

## Goals

*   **`8.5 in x 11 in` Default Sheet Size:** Update the default `page_size` in `Config::default()` and [`src/puzzle-gen/examples/default-config.yaml`](./src/puzzle-gen/examples/default-config.yaml) to `"8.5in x 11in"` (`215.9 mm x 279.4 mm`) so default runs use the full sticker sheet and leave a clean straight-cut offcut strip on `12 in x 12 in` chipboard.
*   **Unscrambled Stage 2 Inspection Grid:** Preserve Stage 2 (`<prefix>-stage2.svg`) as an upright, unscrambled `(row, col)` inspection grid with `GridLabels`.
*   **Localized Corner Keep-Outs & Compacted Bottom-Right Registration Mark:** Replace the full-width top and bottom `6.0 mm` bands in Stages 3 and 4 with localized top-left and bottom-right corner keep-out boxes, and compact each page's bottom-right registration mark inward to the tightest collision-free bottom-right corner `(br_x, br_y)` enclosing the placed pieces on that page.
*   **Right-Angle Irregular Polygon Nesting (`0°`, `90°`, `180°`, `270°`):** Pack pieces on Stage 3 and Stage 4 sheets using their actual outward-buffered polygons (`cut_polygon ∪ bleed_polygon` with `>= piece_spacing_mm` clearance) across the four pixel-grid-aligned right-angle rotations (`0°`, `90°`, `180°`, `270°`) rasterized onto a conservative `1 kerf` bitmask grid.
*   **Unified Page Bounding-Box Area Minimization & Reporting:** Minimize total page count first and minimize the sum of per-page content bounding-box areas ($A_{\text{bbox}} = \sum_{p=1}^{P} \text{area}(B_p)$, where $B_p$ encloses all placed pieces and compacted registration marks on page $p$). Report `bbox_area_pct` ($\frac{A_{\text{bbox}}}{A_{\text{puzzle}}} \times 100\%$, comparing total page bounding-box area against original Stage 0 puzzle area $A_{\text{puzzle}}$) in CLI output.
*   **Stage 3 / Stage 4 Coordinate & Pattern Parity:** Rotate and translate Stage 3 pieces and their `#puzzle-pattern` fills via `<g transform="translate(tx, ty) rotate(deg)">` while baking identical right-angle rotations and translations into ungrouped, transform-free Stage 4 `<path d="...">` coordinates within `<= 1.5e-4 mm` physical parity.
*   **Deterministic Seeded Default & `--interactive` Convergence Mode:** Execute a fixed deterministic iteration budget with `--seed <U64>` (default `0`) in standard non-interactive runs, and support an `--interactive` CLI flag that renders a live `stderr` convergence chart of `bbox_area_pct` over iterations with early key-press stop (`q`, `Enter`, or `Ctrl+C`).
*   **Exact Polygon Invariant Verification:** Validate in `verify_page_invariants` that every placed piece stays within page margins, avoids both corner registration keep-out boxes, and maintains exact polygon-to-polygon Euclidean clearance `>= piece_spacing_mm`.

## Non-Goals

*   **Arbitrary non-`90°` piece rotations:** Continuous or non-cardinal rotation angles are excluded so printed raster artwork stays aligned with the printer and image pixel grid.
*   **Wall-clock optimization timeout flags:** Non-interactive runs use fixed iteration counts for deterministic testability rather than machine-dependent wall-clock timers.
*   **Heterogeneous multi-sheet-size packing within a single CLI run:** Every page in a single invocation shares `Config.page_size`; packing onto accumulated scrap strips (such as `3.5in x 11in` chipboard offcuts) is performed by passing a custom `page_size` in `--config`.
*   **Changes to Stage 0, Stage 1, or Stage 2 piece geometry:** Tab splicing, curve flattening, self-union healing, and kerf/bleed polygon offsetting remain unchanged.
*   **GUI or browser-based manual piece arrangement:** Interactive inspection is limited to the terminal convergence chart under `--interactive`.

## Open Questions

None.

## Critical User Journeys (CUJs)

### CUJ 1: Default `8.5 in x 11 in` Nested Generation and Scrap Compaction

*   **Given:** A user has a Stage 0 Inkscape SVG (`rainy-day-complete.svg`, `4 in x 4 in`, 25 pieces) and runs with default configuration (`page_size: 8.5in x 11in`, `margin: 0.25in`, `kerf: 0.18mm`, `piece_spacing: 1 kerf`, `print_bleed: 1 kerf`).
*   **When:** The user runs `puzzle-gen rainy-day-complete.svg`.
*   **Then:**
    *   `puzzle-gen` keeps `generated/rainy-day-complete-stage2.svg` as an upright `5x5` inspection grid, packs all 25 pieces onto a single `8.5 in x 11 in` page in Stages 3 and 4 using right-angle rotations (`0°`, `90°`, `180°`, `270°`) and `1 kerf` bitmask nesting, and pulls the page's bottom-right registration mark up and left to hug the bottom-right corner `(br_x, br_y)` of the packed pieces.
    *   `puzzle-gen` prints deterministic summary metrics including `pages=1` and `bbox_area_pct=<percentage>` comparing the page content bounding-box area against the `16 sq in` original puzzle area.
    *   The operator prints Stage 3 onto an `8.5 in x 11 in` sticker sheet, adheres it to `12 in x 12 in` chipboard, aligns Stage 4 using the top-left and compacted bottom-right corner marks, cuts the pieces, and severs the unprinted, unscored lower portion of the sheet and the `3.5 in x 12 in` side strip with straight cuts for future reuse.

### CUJ 2: Large Puzzle Optimization with `--interactive` Convergence and Early Stop

*   **Given:** A user is fabricating a `50 cm x 70 cm` (`28x36`, `1,008`-piece) puzzle configured in `config.yaml` and wants to monitor layout compaction and stop once the bounding-box area ratio levels off.
*   **When:** The user runs `puzzle-gen large-puzzle.svg --config config.yaml --interactive`.
*   **Then:**
    *   `puzzle-gen` computes the initial constructive multi-pass packing and continuously runs seeded local search iterations, rendering a live terminal chart on `stderr` showing current iteration count, total `pages`, and a convergence plot of `bbox_area_pct` over iterations.
    *   When the user presses `q` (or `Enter` / `Ctrl+C`), the optimizer finishes its current iteration, verifies exact polygon clearance and corner keep-out invariants on the best layout found so far, writes the final Stage 1–4 SVG files, and prints the final summary metrics to `stdout`.

### CUJ 3: Reproducible Seeded Runs and Offcut Strip Packing

*   **Given:** A user has accumulated `3.5 in x 11 in` chipboard offcut strips from previous `12 in x 12 in` boards and creates `offcut-config.yaml` with `page_size: "3.5in x 11in"`.
*   **When:** The user runs `puzzle-gen small-puzzle.svg --config offcut-config.yaml --seed 42 --dry-run` twice on different machines.
*   **Then:**
    *   Both runs execute the exact same number of deterministic search iterations under seed `42` and produce identical `pages` and `bbox_area_pct` values on `stdout`.
    *   If any piece cannot fit inside the `3.5 in x 11 in` margin-bounded sheet (with corner keep-outs) under any of the four right-angle rotations (`0°`, `90°`, `180°`, `270°`), `puzzle-gen` exits non-zero before writing files and reports the offending piece `id` and minimum required `page_size`.


--------------------------------------------------------------------------------

## Technical Plan (Architecture)

### Architecture Overview

The pipeline retains its clean split between the I/O-free geometric library ([`src/puzzle-gen/src/lib.rs`](./src/puzzle-gen/src/lib.rs)) and the CLI/TUI driver ([`src/puzzle-gen/src/main.rs`](./src/puzzle-gen/src/main.rs)). Stages 0, 1, and 2 continue to execute unchanged in canonical millimeters (`mm`), except that `SplicedPuzzle` and `KerfedGrid` forward `puzzle_bounds_mm` (the Stage 0 grid bounding box) so Stage 3/4 pagination knows the exact original puzzle area $A_{\text{puzzle}}$.

Stage 2 SVG emission ([`emit_stage2`](./src/puzzle-gen/src/svg_emit.rs)) continues to call `paginate::compute_slot_bounds(kerfed)` to render the upright, unscrambled `(row, col)` inspection grid with `GridLabels`. Stage 3 and Stage 4 pagination ([`paginate.rs`](./src/puzzle-gen/src/paginate.rs)) is replaced by a deterministic irregular polygon nesting engine that packs rotated pieces (`0°`, `90°`, `180°`, `270°`) onto `8.5 in x 11 in` (default) sheets using a `1 kerf` conservative bitmask representation, compacts each page's `BottomRight` registration mark to hug the page's placed pieces, and emits optional progress snapshots to a `ratatui` + `crossterm` terminal UI in `main.rs` when `--interactive` is enabled.

```text
Stage 0 Inkscape SVG ─┐
                      ▼
Optional YAML Config ─▶ [ Config (serde-saphyr, default 8.5x11in, seed: 0) ]
                                       │
                                       ▼
                        [ Stage 0–2: Ingest -> Splice -> Heal & Offset ]
                        (Retains upright KerfedGrid + puzzle_bounds_mm)
                                       │
           ┌───────────────────────────┴───────────────────────────┐
           ▼                                                       ▼
 [ Stage 2 Inspection SVG ]                     [ Stage 3/4 Irregular Polygon Nester ]
 (compute_slot_bounds:                          (1-kerf bitmask, 0/90/180/270 rot,
  upright (row, col) grid)                       tail-page search, compacted BR mark,
                                                 exact polygon clearance check)
                                                                   │
                                       ┌───────────────────────────┴───────────────────┐
                                       ▼                                               ▼
                        [ <prefix>-stage3.svg ]                        [ <prefix>-stage4-page<N>.svg ]
                        (<g translate rotate> + #puzzle-pattern)       (Transform-free Cut + PageNumbers)
```

New crates added to [`src/puzzle-gen/Cargo.toml`](./src/puzzle-gen/Cargo.toml):
*   **`crossterm` & `ratatui`:** Used exclusively inside the CLI binary layer (`main.rs` / interactive TUI helper) for alternate-screen `stderr` convergence charts and non-blocking single-key early stop (`q`, `Esc`, `Enter`, `Ctrl+C`) under `--interactive`. The core library remains terminal-I/O-free.

### CLI Interface & Configuration Schema Updates

*   **YAML Configuration (`src/puzzle-gen/src/config.rs`, `examples/default-config.yaml`):**
    *   Update `Config::default().page_width_mm` and `page_height_mm` to `8.5in x 11in` (`215.9 mm x 279.4 mm`), and update [`src/puzzle-gen/examples/default-config.yaml`](./src/puzzle-gen/examples/default-config.yaml) to `page_size: "8.5in x 11in"`.
    *   Add optional `seed: <u64>` (default `0`) to `RawConfig` and `Config`, rejected if negative or non-integer.
*   **CLI Flags (`src/puzzle-gen/src/main.rs`):**
    *   `--seed <U64>`: Optional deterministic PRNG seed overriding `config.seed`.
    *   `--interactive`: Optional flag launching a `ratatui` + `crossterm` convergence chart on `stderr` while running local search continuously until the user presses `q`, `Esc`, `Enter`, or `Ctrl+C`. When `--interactive` is set but `stdin` or `stderr` is not a TTY, emit a warning notice and run the default fixed-iteration deterministic pass.
*   **CLI `stdout` Summary Metrics:**
    *   Retain `grid=RxC`, `matched_tabs=M`, `missing_tabs=K`, `slot_mm=WxH` (Stage 2 grid slot), and `pages=P`.
    *   Replace `page_grid=CxR` with irregular layout compaction metrics:
        *   `puzzle_area_mm2=<f64>` (Stage 0 `puzzle_bounds_mm` area)
        *   `total_bbox_mm2=<f64>` (sum of per-page `content_bbox_mm` areas across pages `1..=P`)
        *   `bbox_area_pct=<f64>` (`(total_bbox_mm2 / puzzle_area_mm2) * 100.0`)

### Core Domain Data Structure Updates (`src/puzzle-gen/src/model.rs`)

| Type | Changes | Invariants |
| --- | --- | --- |
| `Config` | Default `page_size` changed to `8.5in x 11in`; add `pub seed: u64` (`0` default) | `seed` is deterministic across platforms; CLI `--seed` overrides YAML `seed`. |
| `SplicedPuzzle` & `KerfedGrid` | Add `pub puzzle_bounds_mm: kurbo::Rect` | Computed from Stage 0 `x_lines_mm` and `y_lines_mm`; positive area (`> 0.0`). |
| `Rotation90` | New enum `{ Deg0, Deg90, Deg180, Deg270 }` | Exact integer 90° clockwise rotation around canonical `(0, 0)` in `+y`-down coordinates. |
| `PageSlot` | Replace `row`/`col` with `pub rotation: Rotation90` | `transform_point(p) = rotation.rotate_point(p) + offset_mm`. |
| `PagePlan` | Add `pub content_bbox_mm: kurbo::Rect` and `pub bottom_right_anchor_mm: kurbo::Point` | `content_bbox_mm` encloses all placed pieces' `cut_polygon ∪ bleed_polygon` plus both corner marks on that page. |
| `PaginatedLayout` | Remove `page_cols` and `page_rows`; add `puzzle_area_mm2`, `total_bbox_area_mm2`, `bbox_area_pct`; retain `slot_w_mm`, `slot_h_mm` for Stage 2 | `bbox_area_pct == (total_bbox_area_mm2 / puzzle_area_mm2) * 100.0`. |
| `PaginateOptions` & `NestProgress` | New public library control/telemetry structs (`seed`, `max_iters: Option<usize>`) | Decouples optimization budget and progress reporting from `ratatui`/`crossterm`. |

### Right-Angle Irregular Polygon Nesting Engine (`src/puzzle-gen/src/paginate.rs`)

1.  **Outer Envelope & Conservative Bitmask Precomputation ($O(N)$):**
    *   For each piece `i`, compute the outer footprint `outer_env[i]` by taking the `i_overlay` union (`OverlayRule::Union`, `FillRule::Positive`) of `cut_polygon` and `bleed_polygon`, ensuring both cut kerf and print bleed are respected even when `print_bleed_mm < kerf_mm / 2.0`.
    *   Compute the collision polygon `dilated[i]` by buffering `outer_env[i]` outward by `piece_spacing_mm / 2.0` (`OutlineOffset`, `LineJoin::Round(0.1)`), or cloning `outer_env[i]` when `piece_spacing_mm == 0.0`.
    *   Determine the rasterization grid step `step_mm`: `let raw = if config.kerf_mm > 0.0 { config.kerf_mm } else { config.segment_length_mm };`, floored at `0.10 mm` and capped dynamically so the sheet grid never exceeds `2048` cells along either axis (`raw.max(0.10).max(usable_w.max(usable_h) / 2048.0)`). Under default `8.5in x 11in` and `0.18 mm` kerf, `step_mm` is `0.18 mm` (`1 kerf`).
    *   For each `(piece, rotation)` (`4N` variants), shift its rotated `outer_env` AABB to `(0, 0)` and conservatively rasterize `dilated[i, r]` onto a row-major `Vec<u64>` bitmask: mark every grid cell `(lc, lr)` touched by any polygon vertex, any polygon edge (supercover grid traversal), or any interior row scanline at `(lr + 0.5) * step_mm`. Because integer grid shifts preserve grid cell alignment, bitwise disjointness `(sheet_mask & piece_mask) == 0` guarantees continuous Euclidean spacing `>= piece_spacing_mm` between piece footprints.

2.  **Localized Corner Keep-Outs & Worst-Case Fallback Reservation:**
    *   Replace `CORNER_BAND_MM` full-width sheet reservations with two localized corner keep-out boxes of height `MARK_BAND_H_MM = 6.0 mm` and width `keepout_w(page_number) = (1.0 + num_w).max(2.5) + 0.7`, plus `piece_spacing_mm` clearance margin around the keep-out box.
    *   Before placing pieces on page `p`, pre-mark the `TopLeft` keep-out `[margin_mm, margin_mm + keepout_w]` x `[margin_mm, margin_mm + 6.0]` and the fallback `BottomRight` keep-out `[page_w - margin_mm - keepout_w, page_w - margin_mm]` x `[page_h - margin_mm - 6.0, page_h - margin_mm]` as occupied in page `p`'s bitmask.
    *   Allow rotated piece `outer_env` AABBs to span `[margin_mm, page_w - margin_mm]` x `[margin_mm, page_h - margin_mm]` so straight puzzle borders sit flush against `margin_mm`. Before starting the search, verify each piece fits on an empty sheet with corner keep-outs under at least one rotation in `{Deg0, Deg90, Deg180, Deg270}`; otherwise fail immediately naming the offending piece `id` and minimum required `page_size`.

3.  **Constructive Multi-Pass Baseline & Seeded Suffix Local Search:**
    *   **Constructive Decoder:** Given a piece order and per-piece rotation preference order, place each piece onto the earliest open page where its bitmask fits without collision, testing all four rotations and scanning valid `(gx, gy)` offsets to minimize local sheet fill extent. Maintain `first_free_y` and `remaining_free_cells` per sheet and test piece words against `!sheet_word` trailing zeros to skip occupied spans in $O(1)$ per 64 grid cells.
    *   **Multi-Start Baseline:** Evaluate four deterministic initial orderings: (1) decreasing dilated AABB area, (2) straight-edge border/corner pieces first followed by interior pieces, (3) decreasing maximum bounding side length, and (4) increasing convexity ratio (irregular pieces first).
    *   **Seeded Iterative Refinement (`SplitMix64`):** Starting from the best baseline ordering, run `max_iters` deterministic mutations (`DEFAULT_OPTIMIZE_ITERS = 64` in non-interactive mode; unbounded until `cancel` flag is true in `--interactive`), prioritizing swaps, insertions, and rotation-preference flips involving the last 2–3 tail pages and incrementally re-decoding only from the earliest mutated page index.
    *   **Dual-Gravity Page Compaction & Compacted `BottomRight` Mark:** On the final page (and whenever compacting a partially filled page), evaluate both Top-gravity (`y`-major scan) and Left-gravity (`x`-major scan). Remove the fallback `BottomRight` bitmask reservation and select the collision-free `BottomRight` mark anchor `(br_x, br_y)` inside `[margin_mm + keepout_w, page_w - margin_mm]` x `[margin_mm + 6.0, page_h - margin_mm]` that avoids `TopLeft` and all placed pieces (`piece_spacing_mm` clearance) while minimizing the page content bounding-box area `content_bbox_mm = (max(pieces_max_x, br_x) - margin_mm) * (max(pieces_max_y, br_y) - margin_mm)`. Keep the candidate layout minimizing `(pages.len(), total_bbox_area_mm2)`.

4.  **Exact Continuous Verification (`verify_page_invariants`):**
    *   Verify every placed piece's `cut_polygon ∪ bleed_polygon` vertices stay inside `[margin_mm - TOL, page_w - margin_mm + TOL] x [margin_mm - TOL, page_h - margin_mm + TOL]`.
    *   Verify zero intersection between any placed piece polygon and the page's `TopLeft` and compacted `BottomRight` keep-out rectangles (with `piece_spacing_mm` clearance).
    *   For all same-page piece pairs whose AABBs are within `piece_spacing_mm`, verify point-in-polygon disjointness and minimum edge-to-edge Euclidean distance `>= piece_spacing_mm - CONTAINMENT_TOL_MM`.

### Stage 3 & Stage 4 SVG Emission & Interactive TUI (`src/puzzle-gen/src/svg_emit.rs`, `main.rs`)

*   **Rotation Transform & Parity (`Rotation90`):**
    *   Because `Rotation90` rotates around canonical `(0, 0)` with exact integer matrix coefficients `{-1, 0, 1}`, rotation commutes with unit scaling (`mm` to `doc_unit`).
    *   [`emit_stage3`](./src/puzzle-gen/src/svg_emit.rs) continues writing `piece.bleed_polygon` at `Vec2::ZERO` while wrapping each piece in `<g transform="translate(tx_doc, ty_doc)">` (for `Deg0`) or `<g transform="translate(tx_doc, ty_doc) rotate(90|180|270)">`, rotating both the bleed polygon and its `url(#puzzle-pattern)` image slice in lockstep.
    *   [`emit_stage4_page`](./src/puzzle-gen/src/svg_emit.rs) bakes `slot.rotation.rotate_point(p) + slot.offset_mm` directly into transform-free `<path d="...">` coordinates, maintaining `<= 1.5e-4 mm` physical parity with Stage 3.
    *   Both Stage 3 and Stage 4 render `page.corner_strokes_mm` using the identical compacted `BottomRight` mark coordinates `(br_x, br_y)`.
*   **`--interactive` TUI Loop (`main.rs`):**
    *   Run `run_pipeline_with_options` on a `std::thread::scope` worker thread sending `NestProgress` snapshots over an `mpsc` channel, while the main thread owns a `ratatui::Terminal<CrosstermBackend<Stderr>>` inside an alternate-screen + raw-mode RAII guard.
    *   Render a header summary (`pages`, `bbox_area_pct`, iteration count, elapsed time), a `ratatui::widgets::Chart` plotting `bbox_area_pct` over iterations, and per-page fill gauges. Pressing `q`, `Esc`, `Enter`, or `Ctrl+C` sets `cancel` (`AtomicBool`), waits for the worker to finish verifying and return the best layout, restores the terminal cleanly, and writes Stages 1–4.


--------------------------------------------------------------------------------

## Alternatives Considered

### Alternative A: Keep `6 in x 6 in` Default Page Size

*   **Pros:** Matches the initial proof-of-concept sheet size and divides a `12 in x 12 in` chipboard into four equal quadrants.
*   **Cons:** Because `6 + 6 = 12 in` exceeds both sides of an `8.5 in x 11 in` (`93.5 sq in`) adhesive sticker sheet, only one `6 in x 6 in` (`36 sq in`) page can be cut from each sticker sheet, discarding 61.5% of the label stock before layout begins.
*   **Reason for Rejection:** Rejected by author: switching the default `page_size` to `8.5in x 11in` uses 100% of each sticker sheet, reduces margin loss per unit area, and leaves a single straight-cut `3.5 in x 12 in` offcut strip on `12 in x 12 in` chipboard.

### Alternative B: Arbitrary Continuous-Angle Piece Rotations (`0°..360°`)

*   **Pros:** Allows slightly tighter polygon nesting on diagonal tab/socket edges.
*   **Cons:** Rotates raster artwork at non-cardinal angles relative to the printer and image pixel grid, introducing resampling artifacts across adjacent puzzle pieces and requiring arbitrary-angle trigonometric transforms in Stage 3 `<pattern>` fills.
*   **Reason for Rejection:** Rejected by author: restricting rotations to right angles (`0°`, `90°`, `180°`, `270°`) keeps every piece aligned with the image pixel grid and uses exact integer coordinate transforms.

### Alternative C: Preserve Row-Major Piece Ordering on Stage 3 and Stage 4 Sheets

*   **Pros:** Keeps pieces sorted in puzzle `(row, col)` order on the laser bed after cutting.
*   **Cons:** Prevents interleaving compatible piece shapes from different parts of the puzzle and blocks moving smaller or flatter pieces forward to fill gaps on earlier sheets.
*   **Reason for Rejection:** Rejected by author: scrambling piece positions and right-angle orientations on Stages 3 and 4 is acceptable to minimize material waste, provided Stage 2 (`<prefix>-stage2.svg`) remains an upright, unscrambled `(row, col)` inspection grid.

### Alternative D: Keep the Bottom-Right Registration Mark Fixed at `(page_width_mm - margin_mm, page_height_mm - margin_mm)`

*   **Pros:** Places the bottom-right registration mark at the same sheet coordinate regardless of how many pieces are on the page.
*   **Cons:** Prints and scores into the far corner of partially filled tail pages, contaminating the otherwise empty offcut region.
*   **Reason for Rejection:** Rejected by author: compacting the bottom-right registration mark to hug the bottom-right bounds of the placed pieces keeps the unused portion of the sheet completely blank and unscored for straight-cut scrap reuse.

### Alternative E: Wall-Clock Optimization Timeout Flag (`--optimize-time <SECONDS>`)

*   **Pros:** Bounds non-interactive optimization runs to a fixed number of wall-clock seconds.
*   **Cons:** Produces different iteration counts, page counts, and layouts across machines with different CPU speeds or background load, breaking deterministic test assertions.
*   **Reason for Rejection:** Rejected by author: fixed iteration counts with a deterministic seed provide reproducible test runs across machines, while `--interactive` lets the operator monitor convergence and stop early whenever satisfied.

### Alternative F: CLI-Only `seed` Parameter (Omitting `seed` from `config.yaml`)

*   **Pros:** Keeps `config.yaml` strictly limited to physical sheet and laser dimensions.
*   **Cons:** Requires passing `--seed <U64>` on the command line every time a project needs a specific non-default layout seed.
*   **Reason for Rejection:** Rejected by author: supporting an optional `seed` field in `config.yaml` (overridable via CLI `--seed`) allows project configurations to version a reproducible layout seed alongside physical sheet settings.

### Alternative G: Dependency-Free `std` Line-Buffered Terminal Output for `--interactive`

*   **Pros:** Avoids adding `crossterm` and `ratatui` to `Cargo.toml`.
*   **Cons:** Cooked-mode `stdin` blocks until `Enter` is pressed (so single-key `q` does not stop the solver), and pure `std` cannot intercept `Ctrl+C` to finish verifying and writing the best layout found so far instead of terminating immediately.
*   **Reason for Rejection:** Rejected by author: using `crossterm` and `ratatui` supports single-key early stop (`q`, `Esc`, `Enter`, `Ctrl+C`) and renders an in-place terminal convergence chart on `stderr`.

### Alternative H: Per-Piece Tight AABB Rectangle Packing Without Polygon Nesting

*   **Pros:** Simpler 2D rectangle bin packing (skyline or guillotine) without polygon rasterization.
*   **Cons:** Roughly 30–40% of a tabbed piece's axis-aligned bounding box consists of empty corner shoulders beside protruding tabs; rectangle-only packing treats those empty shoulders as solid obstacles and cannot interleave neighboring tabs.
*   **Reason for Rejection:** Irregular polygon nesting is required to pack protruding tabs into adjacent empty corner shoulders and concave sockets.

### Alternative I: All-Pairs Continuous No-Fit Polygon (NFP) Precomputation ($O((4N)^2)$)

*   **Pros:** Computes exact continuous sliding contact boundaries between every pair of oriented pieces.
*   **Cons:** For a `1,008`-piece puzzle (`4,032` oriented polygons of ~80–120 vertices each), computing $\sim 8.1 \times 10^6$ non-convex pairwise NFPs exceeds a 60-second runtime budget and is prone to floating-point degeneracies on narrow undercut sockets.
*   **Reason for Rejection:** Precomputing $O(N)$ conservative `1 kerf` (`0.18 mm`) 2D bitmasks with suffix-incremental local search scales linearly in piece count while guaranteeing exact continuous clearance `>= piece_spacing_mm`.

--------------------------------------------------------------------------------

## Non-Functional Requirements

*   **1,008-Piece Scaling & Memory Footprint:** Precomputing `4N` bitmasks (`4,032` oriented variants for a `28x36`, `1,008`-piece puzzle at `1 kerf` = `0.18 mm`) must allocate `< 25 MB` of memory and complete constructive + fixed-iteration local search within `< 60 seconds` in release builds (`< 250 ms` on a `5x5`, `25`-piece puzzle).
*   **Cross-Platform Determinism:** Non-interactive runs with the same input SVG, `Config`, and `seed` must produce identical page counts, piece placements, rotations, compacted registration mark coordinates, and `bbox_area_pct` values across Windows, macOS, and Linux.
*   **Stage 3 / Stage 4 Coordinate Parity (`<= 1.5e-4 mm`):** Every piece vertex in Stage 3 (`<g transform="translate(...) rotate(...)">` + `<path d>`) and Stage 4 (baked transform-free `<path d>`), as well as every `PageNumbers` registration stroke, must agree in physical page coordinates within `<= 1.5e-4 mm`.
*   **Terminal State Restoration & Non-TTY Safety:** `--interactive` must restore raw mode and leave the alternate screen via an RAII `Drop` guard on normal completion, early user stop, error return, or panic, and must fall back to non-interactive fixed-iteration execution with a `Warning` notice when `stdin` or `stderr` is not a terminal.

## Risks & Limits

*   **Zero or Ultra-Small `kerf` Bitmask Resolution:** If `step_mm` followed `kerf_mm` without bounds, setting `kerf: "0mm"` or `kerf: "0.01mm"` on a large sheet would cause divide-by-zero or excessive bitmask memory allocation. Flooring `step_mm` at `0.10 mm` (using `segment_length_mm` when `kerf_mm == 0.0`) and capping sheet grid dimensions at `<= 2048` cells per axis bounds each sheet bitmask to `<= 524 KB`.
*   **Bottom-Right Registration Mark Contention on Full Pages:** If pieces packed into the extreme bottom-right corner of a sheet, no collision-free anchor inside `[margin_mm, page_width_mm - margin_mm] x [margin_mm, page_height_mm - margin_mm]` would remain for the `BottomRight` mark. Pre-reserving the fallback corner keep-out box at `(page_width_mm - margin_mm, page_height_mm - margin_mm)` during piece placement guarantees a valid anchor always exists before attempting inward compaction.
*   **Interactive Cancellation Mid-Iteration:** Stopping the process immediately on `Ctrl+C` would leave output files unwritten or incomplete. Routing `q`, `Esc`, `Enter`, and `Ctrl+C` through an `AtomicBool` cancellation flag checked between local search iterations guarantees the solver always returns a fully verified `PaginatedLayout` before writing Stage 1–4 files atomically.

--------------------------------------------------------------------------------

### Additional Resources

*   [`docs/designs/puzzle-gen.md`](./docs/designs/puzzle-gen.md): Initial four-stage `puzzle-gen` architecture and Stage 0–2 geometric specifications.
*   [`concepts_brief.md`](./concepts_brief.md): Multi-lens EGM brainstorm brief for Stage 3 & 4 irregular polygon nesting and scrap compaction.

### Future Improvements

*   **Stage 2 Diagnostic Problem-Stroke Highlighting:** Color and bold specific edges or pieces in `<prefix>-stage2.svg` where non-fatal warnings (`MISSING_INTERNAL_TAB` under `--no-strict`, self-union overshoot healing, or bleed socket bridging) or `--emit-debug-on-error` Stage 2 offsetting failures occurred, discussed as a separate follow-up design.


--------------------------------------------------------------------------------

## Detailed Implementation Plan

All work sits within [`src/puzzle-gen`](./src/puzzle-gen). No other workspace crate is modified. Phases must be executed sequentially so `cargo check` and `cargo test` stay green as domain models, nesting geometry, SVG emission, and CLI/TUI integration land.

### Target Scope & File Boundary

1.  **[`./src/puzzle-gen/Cargo.toml`](./src/puzzle-gen/Cargo.toml) (Modify):**
    *   **Rationale:** Add `crossterm` and `ratatui` to power the `--interactive` terminal convergence chart and non-blocking early-stop input loop in the CLI binary without touching the core geometric pipeline crates.
    *   **Proposed Changes:** Add `crossterm = "0.29"` and `ratatui = "0.30"` under `[dependencies]`.
2.  **[`./src/puzzle-gen/examples/default-config.yaml`](./src/puzzle-gen/examples/default-config.yaml) (Modify):**
    *   **Rationale:** Align the documented default config file with the new `8.5in x 11in` default page size and explicit `seed: 0` parameter so `Config::from_yaml` equality tests pass.
    *   **Proposed Changes:** Change `page_size: "6in x 6in"` to `page_size: "8.5in x 11in"`, update the `piece_spacing` comment to remove references to bounding-box slots across Stages 3–4, and add `seed: 0` with a comment documenting deterministic seeded layout search.
3.  **[`./src/puzzle-gen/src/model.rs`](./src/puzzle-gen/src/model.rs) (Modify):**
    *   **Rationale:** Extend core immutable domain types to represent `8.5in x 11in` defaults, YAML/CLI layout `seed`, Stage 0 `puzzle_bounds_mm`, right-angle `Rotation90` transforms, compacted `BottomRight` corner mark anchors, `bbox_area_pct` layout metrics, and I/O-free solver control/progress structs.
    *   **Proposed Changes:**
        *   In `Config`, add `pub seed: u64`; in `Config::default()`, set `page_width_mm: LengthUnit::In.to_mm(8.5)`, `page_height_mm: LengthUnit::In.to_mm(11.0)`, and `seed: 0`.
        *   Add `pub puzzle_bounds_mm: kurbo::Rect` to both `SplicedPuzzle` and `KerfedGrid`.
        *   Define `pub enum Rotation90 { Deg0, Deg90, Deg180, Deg270 }` (`#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`) with `pub const ALL: [Rotation90; 4]`, `pub fn degrees(self) -> u16`, and `pub fn rotate_point(self, p: kurbo::Point) -> kurbo::Point` mapping `(x, y)` to `(x, y)`, `(-y, x)`, `(-x, -y)`, and `(y, -x)` respectively.
        *   In `PageSlot`, replace `row: usize` and `col: usize` with `pub rotation: Rotation90`, and add `pub fn transform_point(self, p: kurbo::Point) -> kurbo::Point`.
        *   In `PagePlan`, add `pub content_bbox_mm: kurbo::Rect` and `pub bottom_right_anchor_mm: kurbo::Point`.
        *   In `PaginatedLayout`, remove `page_cols: usize` and `page_rows: usize`, retain `slot_w_mm` and `slot_h_mm` (for Stage 2 inspection grid), and add `pub puzzle_area_mm2: f64`, `pub total_bbox_area_mm2: f64`, and `pub bbox_area_pct: f64`.
        *   Define `pub const DEFAULT_OPTIMIZE_ITERS: usize = 64;`, `pub struct PaginateOptions { pub seed: u64, pub max_iters: Option<usize> }` (with `PaginateOptions::from_config(config: &Config)` setting `max_iters: Some(DEFAULT_OPTIMIZE_ITERS)`), and `pub struct NestProgress { pub iter: usize, pub total_iters: Option<usize>, pub pages: usize, pub total_bbox_area_mm2: f64, pub puzzle_area_mm2: f64, pub bbox_area_pct: f64, pub page_fill_ratios: Vec<f64> }`.
4.  **[`./src/puzzle-gen/src/config.rs`](./src/puzzle-gen/src/config.rs) (Modify):**
    *   **Rationale:** Parse and validate optional `seed` in `config.yaml` while preserving strict unknown-key and duplicate-key rejection.
    *   **Proposed Changes:**
        *   Add `seed: Option<u64>` to `RawConfig`. Because `serde_saphyr` deserializes `Option<u64>`, negative values, floats, or non-numeric strings fail schema parsing automatically.
        *   In `Config::from_yaml`, copy `raw.seed` onto `cfg.seed` when `Some`.
        *   Update unit tests in `config.rs` (`yaml_defaults_and_kerf_multipliers`, plus assertions verifying default `seed == 0`, explicit `seed: 42`, and rejection of `seed: -1`).
5.  **[`./src/puzzle-gen/src/stage1.rs`](./src/puzzle-gen/src/stage1.rs) (Modify):**
    *   **Rationale:** Carry exact Stage 0 grid bounds forward into `SplicedPuzzle` so `bbox_area_pct` measures against true Stage 0 grid area rather than image bounds or kerf-expanded areas.
    *   **Proposed Changes:** In `splice_tabs`, compute `puzzle_bounds_mm` from `(x_lines_mm.first(), y_lines_mm.first())` to `(x_lines_mm.last(), y_lines_mm.last())` and assign `puzzle_bounds_mm` on `SplicedPuzzle`.
6.  **[`./src/puzzle-gen/src/stage2.rs`](./src/puzzle-gen/src/stage2.rs) (Modify):**
    *   **Rationale:** Forward `puzzle_bounds_mm` into `KerfedGrid` and expose `outline_options_i64` to sibling crate modules for uniform `i_overlay` buffering.
    *   **Proposed Changes:**
        *   In `heal_and_offset`, populate `puzzle_bounds_mm: spliced.puzzle_bounds_mm` on `KerfedGrid`.
        *   Promote `outline_options_i64` and `signed_area` visibility to `pub(crate)` so `nest.rs` and `paginate.rs` reuse `stage2.rs` helpers rather than duplicating `i_overlay` configuration or shoelace math.
7.  **[`./src/puzzle-gen/src/nest.rs`](./src/puzzle-gen/src/nest.rs) (Create):**
    *   **Rationale:** Isolate the `1 kerf` conservative bitmask precomputation, 4-start constructive decoder, `SplitMix64` suffix-incremental local search, and dual-gravity `BottomRight` mark compaction into a cohesive module called by `paginate.rs`.
    *   **Proposed Changes:**
        *   Declare `pub(crate)` module helpers invoked by `paginate.rs`.
        *   Define `SplitMix64` seeded deterministic PRNG (`next_u64`, `gen_range`).
        *   Define `grid_step_mm(config: &Config) -> f64`: `raw = if config.kerf_mm > 0.0 { config.kerf_mm } else { config.segment_length_mm }; raw.max(0.10).max(usable_w.max(usable_h) / 2048.0)`.
        *   Compute per-piece `outer_env` via `cut_polygon.overlay(&bleed_polygon, OverlayRule::Union, FillRule::Positive)` (reusing `stage2::outline_options_i64`), and compute `dilated_poly` via `OutlineOffset` at `0.5 * piece_spacing_mm + 0.005` when `piece_spacing_mm > 0.0` (or cloning `outer_env` when `0.0`).
        *   Build `PieceVariant` for each `(piece_index, rotation)` (`4N` items): rotate `outer_env` and `dilated_poly` via `Rotation90::rotate_point`, shift by `(-env_min_x, -env_min_y)`, add `pad_cells = (pad_mm / step_mm).ceil() as usize`, and conservatively rasterize `dilated_poly` onto a row-major `Vec<u64>` bitmask covering all vertices, supercover edge traversals, and horizontal scanline interiors at `(row + 0.5) * step_mm`.
        *   Implement `SheetBitmask` storing sheet dimensions `(usable_w, usable_h)`, `pad_cells`, `Vec<u64>` row-major words, `first_free_y`, and `free_cells`. Pre-mark the localized `TopLeft` keep-out `[margin_mm, margin_mm + kw + spacing]` x `[margin_mm, margin_mm + 6.0 + spacing]` and fallback `BottomRight` keep-out `[page_w - margin_mm - kw - spacing, page_w - margin_mm]` x `[page_h - margin_mm - 6.0 - spacing, page_h - margin_mm]`.
        *   Check single-piece feasibility across all 4 rotations on a blank `SheetBitmask` with corner keep-outs; if any piece fails all 4 rotations, return `PipelineError::new("paginate", ...)` naming `piece.id`, its minimum rotated bounding dimensions, and `minimum required page_size`.
        *   Implement `decode_layout(order, rot_prefs, ...)` placing pieces onto earliest fitting open pages (`(sheet_words & variant_words) == 0` using `!word` trailing zeros to skip solid runs) and compacting each page's `BottomRight` mark anchor `(br_x, br_y)` after removing the fallback corner reservation (testing `(pieces_max_x, pieces_max_y)` pockets, horizontal/vertical expansions, and the fallback sheet corner while checking exact `TopLeft` and piece `>= piece_spacing_mm` clearance, keeping the anchor minimizing `(max(pieces_max_x, br_x) - margin_mm) * (max(pieces_max_y, br_y) - margin_mm)`).
        *   Implement `compact_final_page` comparing Top-gravity (`y`-major) vs. Left-gravity (`x`-major) placement orderings on the final page.
        *   Run the 4 constructive initial orderings (decreasing dilated AABB area, border/corner pieces first, decreasing max side, increasing convexity ratio), then run `SplitMix64` local search (`options.max_iters` iterations when `Some`, or until `cancel.load(Ordering::Relaxed)` when `None`) mutating tail-page orderings/rotations (`80%` within the last `2–3` pages, `20%` global swaps) with suffix re-decoding from the earliest changed page, invoking `on_progress` after each outer improvement/batch.
        *   Add unit tests in `nest.rs`: conservative rasterizer guarantees `segment/interior` coverage, `SplitMix64` determinism, single-piece `90°` fit when `0°` exceeds sheet width, and `BottomRight` mark inward compaction on a partially filled page.
8.  **[`./src/puzzle-gen/src/paginate.rs`](./src/puzzle-gen/src/paginate.rs) (Modify):**
    *   **Rationale:** Replace uniform `CORNER_BAND_MM` slot pagination with localized corner keep-outs, compacted `BottomRight` mark stroke generation, entry points for `nest.rs`, and exact continuous polygon clearance verification while preserving `compute_slot_bounds` for Stage 2.
    *   **Proposed Changes:**
        *   Replace `CORNER_BAND_MM` with `pub const CORNER_KEEP_OUT_H_MM: f64 = 6.0;`.
        *   Add `pub fn corner_keepout_size_mm(page_number: usize) -> (f64, f64)` and `pub fn corner_keepout_rects_mm(page_number: usize, config: &Config, br_anchor: Point) -> [Rect; 2]`.
        *   Add `pub fn layout_corner_page_number_at_anchor(page_number: usize, anchor_x: f64, anchor_y: f64, corner: CornerAnchor) -> Vec<Vec<Point>>` and delegate existing `layout_corner_page_number` to it for backwards-compatible unit testing.
        *   Keep `pub fn compute_slot_bounds(kerfed: &KerfedGrid) -> (f64, f64)` untouched for Stage 2.
        *   Add `pub fn paginate_pieces_with_options<F>(kerfed: &KerfedGrid, config: &Config, options: &PaginateOptions, cancel: &AtomicBool, on_progress: F) -> Result<PaginatedLayout, PipelineError>` calling `nest::pack_irregular_pages`, verifying every page via `verify_page_invariants`, and validating `assigned_count == kerfed.pieces.len()`. Keep `paginate_pieces(kerfed, config)` delegating to `paginate_pieces_with_options` with `PaginateOptions::from_config(config)`.
        *   Rewrite `verify_page_invariants` to check: (a) all transformed `cut_polygon` and `bleed_polygon` vertices inside `[margin_mm - TOL, page_width_mm - margin_mm + TOL] x [margin_mm - TOL, page_height_mm - margin_mm + TOL]`, (b) `polygon_rect_distance` against both `TopLeft` and compacted `BottomRight` keep-out boxes `>= config.piece_spacing_mm - TOL` (and `TopLeft` vs `BottomRight` keep-out boxes disjoint), and (c) pairwise `polygon_distance` between outer piece polygons whose AABBs are within `config.piece_spacing_mm` is `>= config.piece_spacing_mm - TOL` (using `stage2::point_in_polygon` and exact segment-to-segment distance).
        *   Update unit tests in `paginate.rs` for `corner_keepout_rects_mm`, `layout_corner_page_number_at_anchor`, `compute_slot_bounds`, `verify_page_invariants` catching polygon overlap and keep-out violations, and single-piece rotation-aware failure messages.
9.  **[`./src/puzzle-gen/src/lib.rs`](./src/puzzle-gen/src/lib.rs) (Modify):**
    *   **Rationale:** Declare `mod nest;` and expose `run_pipeline_with_options` so the binary crate can stream progress and cancel optimization cleanly without coupling the library to TUI crates.
    *   **Proposed Changes:**
        *   Add `mod nest;`.
        *   Implement `pub fn run_pipeline_with_options<F>(...) -> Result<PipelineArtifacts, PipelineError>` forwarding `options`, `cancel`, and `on_progress` into `paginate::paginate_pieces_with_options`.
        *   Delegate `run_pipeline` to `run_pipeline_with_options` with `PaginateOptions::from_config(config)`, `&AtomicBool::new(false)`, and `|_| {}`.
10. **[`./src/puzzle-gen/src/svg_emit.rs`](./src/puzzle-gen/src/svg_emit.rs) (Modify):**
    *   **Rationale:** Apply `Rotation90` transforms cleanly in Stage 3 (`<g transform="translate(...) rotate(...)">` preserving `#puzzle-pattern` orientation lockstep) and bake exact rotated points into transform-free Stage 4 `<path d="...">` coordinates.
    *   **Proposed Changes:**
        *   In `emit_stage3`, emit `transform="translate(tx, ty)"` when `slot.rotation == Rotation90::Deg0`, or `transform="translate(tx, ty) rotate(deg)"` for `Deg90`, `Deg180`, `Deg270`.
        *   In `emit_stage4_page`, add helper `format_transformed_closed_polygon(pts: &[Point], rot: Rotation90, offset_mm: Vec2, unit: LengthUnit) -> String` applying `rot.rotate_point(p) + offset_mm` directly into `<path d="...">` vertices with zero `<g>` or `transform` attributes.
        *   Update `emits_valid_stage1_to_stage4_xml_structure` unit test in `svg_emit.rs` to construct `PageSlot` with `rotation: Rotation90::Deg90` (testing non-zero rotation output in Stage 3 and Stage 4) and populate `puzzle_bounds_mm` / `content_bbox_mm`.
11. **[`./src/puzzle-gen/src/tui.rs`](./src/puzzle-gen/src/tui.rs) (Create):**
    *   **Rationale:** Isolate `ratatui` + `crossterm` terminal initialization, RAII terminal state restoration, event polling, and convergence chart rendering inside the CLI binary without polluting `main.rs` or library modules.
    *   **Proposed Changes:**
        *   Define `TerminalGuard` RAII struct entering alternate screen + raw mode on `io::stderr()` during construction and unconditionally calling `disable_raw_mode()` + `LeaveAlternateScreen` in `Drop`.
        *   Expose `pub(crate) fn run_interactive_pipeline(svg_xml: &str, config: &Config, strict: bool, notices: &mut Vec<PipelineNotice>) -> Result<PipelineArtifacts, PipelineError>`:
            *   Create `Arc<AtomicBool>` (`cancel`), `mpsc::channel::<NestProgress>()`, and run `run_pipeline_with_options` with `PaginateOptions { seed: config.seed, max_iters: None }` on a `std::thread::scope` worker thread.
            *   Loop on the main thread polling `crossterm::event::poll(Duration::from_millis(50))`, handling `q`, `Esc`, `Enter`, or `Ctrl+C` (`KeyCode::Char('c')` with `KeyModifiers::CONTROL`) by setting `cancel.store(true, Ordering::Relaxed)`.
            *   Drain progress snapshots into a history buffer `(iter as f64, bbox_area_pct)` and draw a `ratatui` layout containing: (1) top summary block (`grid`, `pages`, `bbox_area_pct`, iteration, elapsed time), (2) `ratatui::widgets::Chart` (`Dataset` using `Marker::Braille`) showing `bbox_area_pct` convergence over iterations, (3) per-page fill summary bars (`Gauge` / list), and (4) bottom keybind legend (`q / Enter / Esc / Ctrl+C: stop early and emit best layout`).
12. **[`./src/puzzle-gen/src/main.rs`](./src/puzzle-gen/src/main.rs) (Modify):**
    *   **Rationale:** Wire `--seed`, `--interactive`, non-TTY fallback handling, and updated `bbox_area_pct` `stdout` reporting into the CLI binary.
    *   **Proposed Changes:**
        *   Declare `mod tui;`.
        *   Add `#[arg(long)] seed: Option<u64>` and `#[arg(long)] interactive: bool` to `Cli`.
        *   Apply `if let Some(seed) = cli.seed { config.seed = seed; }` after `Config::from_yaml`.
        *   If `cli.interactive`, check `io::stderr().is_terminal() && io::stdin().is_terminal()`; if false, push `PipelineNotice { severity: NoticeSeverity::Warning, code: "INTERACTIVE_NON_TTY", ... }` and call `run_pipeline`, otherwise call `tui::run_interactive_pipeline`.
        *   Update `stdout` output: retain `grid=`, `matched_tabs=`, `missing_tabs=`, `slot_mm=`, `pages=`; replace `page_grid=` with `puzzle_area_mm2={:.4}`, `total_bbox_mm2={:.4}`, and `bbox_area_pct={:.2}`.
13. **[`./src/puzzle-gen/tests/integration_pipeline.rs`](./src/puzzle-gen/tests/integration_pipeline.rs) (Modify):**
    *   **Rationale:** Update existing tests for `8.5in x 11in` default page size, rotated Stage 3/4 transform parity, and `bbox_area_pct` CLI metrics, and add integration tests covering irregular nesting compaction, determinism, and non-TTY `--interactive` fallback.
    *   **Proposed Changes:**
        *   Update `parse_translate_mm` into `parse_transform_mm` parsing both `TransformListToken::Translate` and `TransformListToken::Rotate` (applying SVG `rotate(angle)` before `translate(tx, ty)`).
        *   In `synthetic_5x5_stages_1_to_4_and_stage3_vs_stage4_coordinate_parity`, change the inline YAML `page_size` from `"6in x 6in"` to `"3.5in x 3.5in"` so the multi-page `>= 2` assertion exercises multi-page Stage 3 offsets and rotated Stage 3 vs. Stage 4 parity (`<= 1.5e-4 mm`).
        *   In `cli_end_to_end_dry_run_collisions_overwrite_pruning_and_debug_emit`: assert `stdout_dry.contains("bbox_area_pct=")`, `stdout_dry.contains("puzzle_area_mm2=")`, and `stdout_dry.contains("total_bbox_mm2=")` in place of `page_grid=`, and set `small.yaml` `page_size` to `"2in x 2in"` so the pruning test deterministically creates `>= 5` pages.
        *   Add `default_8_5x11_packs_rainy_day_on_single_page_with_compacted_mark`: verifies the `4 in x 4 in` 25-piece example packs onto `1` page under default `Config::default()`, with `bbox_area_pct < 200.0` and `pages[0].bottom_right_anchor_mm.y < cfg.page_height_mm - cfg.margin_mm`.
        *   Add `seeded_nesting_is_deterministic_and_cli_overrides_yaml_seed`: runs `run_pipeline` twice with `seed: 42` verifying identical `PaginatedLayout`, Stage 3, and Stage 4 strings, and verifies CLI `--seed` overrides YAML `seed`.
        *   Add `non_tty_interactive_flag_warns_and_succeeds`: runs the compiled `puzzle-gen` binary with `--interactive --dry-run --no-strict` under captured piped stdio and asserts exit code `0`, `INTERACTIVE_NON_TTY` warning on `stderr`, and `bbox_area_pct=` on `stdout`.
        *   Add `rotation_allows_90_deg_single_piece_fit_on_narrow_strip`: builds a `1x1` `45 mm x 20 mm` piece on `"1.5in x 4in"` (`38.1 mm x 101.6 mm`) sheet and verifies placement succeeds via `Rotation90::Deg90` or `Rotation90::Deg270`.

### Verification Plan

*   **Compilation & Linting:**
    *   `cargo check --workspace --all-targets`
    *   `cargo clippy -p puzzle-gen --all-targets -- -D warnings`
*   **Unit & Integration Tests:**
    *   `cargo test -p puzzle-gen` (verifies all unit tests in `config.rs`, `ingest.rs`, `stage1.rs`, `stage2.rs`, `nest.rs`, `paginate.rs`, `svg_emit.rs` and all E2E tests in `tests/integration_pipeline.rs` including rotated Stage 3/4 parity `<= 1.5e-4 mm` and `--interactive` non-TTY fallback).
*   **Example Dry-Run & Output Inspection:**
    *   `cargo run -p puzzle-gen -- --dry-run --no-strict src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` (verifies `pages=1` and tight `bbox_area_pct` under default `8.5in x 11in` config).
    *   Manual TTY verification (run `cargo run -p puzzle-gen -- --interactive --dry-run --no-strict src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` or a generated `28x36` 1,008-piece test fixture in a real terminal to verify `ratatui` chart rendering, `q`/`Ctrl+C` early stop, and clean terminal restoration, plus release-mode timing `< 60 s` on a `1,008`-piece synthetic SVG).


---

*Drafted with EGM*
