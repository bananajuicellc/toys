# `puzzle-gen`: Stage 3 & 4 Irregular Polygon Nesting and Scrap Compaction

*   **Author:** Tim Sweña (Swast)
*   **Status:** Draft
*   **Date:** 2026-11-13

--------------------------------------------------------------------------------

## Overview

`puzzle-gen` is a local Rust CLI and geometric pipeline in [`src/puzzle-gen`](../../src/puzzle-gen) (originally specified in [`puzzle-gen.md`](./puzzle-gen.md)) that reads an unjoined Stage 0 Inkscape SVG (`Image`, `Pieces` `<rect>` grid, and `Tabs` open `<path>` curves) and emits SVG files for physical print-and-cut fabrication using output filename prefix `<prefix>`: Stage 1 (`<prefix>-stage1.svg`, closed spliced contours), Stage 2 (`<prefix>-stage2.svg`, flattened/offset inspection grid), Stage 3 (`<prefix>-stage3.svg`, Inkscape `<inkscape:page>` sticker print sheets with `#puzzle-pattern` fills), and Stage 4 (`<prefix>-stage4-page<N>.svg`, standalone per-page laser-cut paths).

Currently, Stages 3 and 4 arrange pieces on a uniform rectangular grid where every piece occupies a worst-case global bounding box and two full-width horizontal bands are reserved across the top and bottom of every sheet. On `8.5 in x 11 in` adhesive sticker stock, the `6 in x 6 in` default page size discards 61.5% of each sheet before layout begins, and worst-case grid pagination amplifies multi-page waste further (leaving ~85% of sheet area unused on the 25-piece example).

This design updates the default `page_size` to `8.5in x 11in`, keeps Stage 2 (`<prefix>-stage2.svg`) as an upright `(row, col)` inspection grid, and replaces Stage 3 and Stage 4 uniform grid pagination with 2D irregular polygon bin packing across right-angle rotations (`0°`, `90°`, `180°`, `270°`) and localized corner registration keep-out boxes. The layout engine minimizes the total number of sheets and compacts both the placed pieces and the bottom-right registration mark on each page toward the top-left origin so the outer right and bottom sheet strips outside the content bounding box stay unprinted and unscored for straight-cut scrap reuse. It runs a fixed deterministic iteration budget by default for reproducible tests and adds an `--interactive` CLI flag that displays a live `stderr` convergence chart of total page bounding-box area relative to the original puzzle area across continuous local-search iterations until early key-press stop (`q`, `Esc`, `Enter`, or `Ctrl+C`) or upstream Stage 0–3 failure.

## Problem Statement

Fabricating a custom jigsaw puzzle—ranging from a `4 in x 4 in` (`25`-piece) test puzzle up to a `50 cm x 70 cm` (`28x36`, `1,008`-piece) puzzle—on `8.5 in x 11 in` adhesive sticker paper and `12 in x 12 in` chipboard currently suffers from four compounding sources of material waste in Stages 3 and 4:

*   **Default sheet size mismatch with sticker stock:** The current default `page_size` is `6 in x 6 in` (`36 sq in`). Although four `6 in x 6 in` squares tile a `12 in x 12 in` chipboard sheet without trim loss, `6 + 6 = 12 in` exceeds both dimensions of an `8.5 in x 11 in` (`93.5 sq in`) sticker sheet, so only one `6 in x 6 in` page fits per sticker sheet—discarding 61.5% of the adhesive label stock as pre-layout trim loss.
*   **Full-width horizontal registration bands:** `paginate_pieces` in [`paginate.rs`](../../src/puzzle-gen/src/paginate.rs) subtracts two full-width `6.0 mm` horizontal bands (`12.0 mm` total height) across the entire `203.2 mm` usable width of every page (`2,438.4 mm²` on an `8.5 in x 11 in` sheet), even though the top-left and bottom-right vector page numbers and L-ticks only span `3.40 mm x 5.30 mm` for 1-digit pages and `6.28 mm x 5.30 mm` for 2-digit pages (`4.10 mm x 6.0 mm` and `6.98 mm x 6.0 mm` including a `0.7 mm` safety margin).
*   **Global worst-case rectangular slot inflation:** `compute_slot_bounds` assigns every piece on the sheet a uniform rectangular slot equal to `2 * max_half_w x 2 * max_half_h` around the piece cut bounding-box center (`cut_aabb_mm.center()`) across the entire puzzle. On the included `4 in x 4 in` (`16 sq in`, 25-piece) example ([`rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg`](../../src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg), run with `--no-strict` because `path19` is omitted) where each grid cell is `20.32 mm x 20.32 mm` (`412.90 mm²`), every piece is allocated a `37.15 mm x 35.54 mm` (`1,320.31 mm²`) slot—`~3.20x` the nominal cell area—even for straight-edged border pieces or pieces with inward sockets. Furthermore, empty corner shoulders beside protruding tabs inside an axis-aligned bounding box cannot accept neighboring pieces' tabs under rectangle-only slots. As a result, a `16 sq in` puzzle spills across three `6 in x 6 in` sheets (`108 sq in` total, ~85% waste).
*   **Uncompacted tail pages and fixed bottom-right registration marks:** On partially filled pages, leaving the bottom-right registration mark fixed at `(page_width_mm - margin_mm, page_height_mm - margin_mm)` prints and scores into the far corner of the sheet, marking and spoiling the unused corner of offcuts when the operator attempts straight guillotine or laser cuts to recover contiguous scrap for smaller runs.

## Goals & Non-Goals

### Goals

*   **`8.5 in x 11 in` Default Sheet Size:** Update the default `page_size` in `Config::default()` and [`default-config.yaml`](../../src/puzzle-gen/examples/default-config.yaml) to `"8.5in x 11in"` (`215.9 mm x 279.4 mm`) so default runs cover the full printable sticker sheet size with zero pre-layout trim loss and leave contiguous straight-cut offcut strips (`3.5 in x 11 in` or `3.5 in x 12 in` plus a `1 in` end strip) on `12 in x 12 in` chipboard.
*   **Unscrambled Stage 2 Inspection Grid:** Preserve Stage 2 (`<prefix>-stage2.svg`) as an upright, unscrambled `(row, col)` inspection grid with row/column index text labels (`GridLabels`).
*   **Localized Corner Keep-Outs & Compacted Bottom-Right Registration Mark:** Replace the full-width top and bottom `6.0 mm` bands in Stages 3 and 4 with localized top-left and bottom-right corner keep-out boxes, and compact each page's bottom-right registration mark inward to the tightest collision-free bottom-right anchor `(br_x, br_y)` enclosing the placed pieces on that page.
*   **Right-Angle Irregular Polygon Nesting (`0°`, `90°`, `180°`, `270°`):** Pack pieces on Stage 3 and Stage 4 sheets using their outer piece footprints (union of `cut_polygon` and `bleed_polygon`, covering valid configs where `print_bleed < kerf / 2`) with `>= piece_spacing_mm` clearance across the four pixel-grid-aligned right-angle rotations (`0°`, `90°`, `180°`, `270°`), accelerated via conservative bitmasks sized from `1 kerf` (subject to a minimum step floor and per-axis grid cap).
*   **Unified Page Bounding-Box Area Minimization & Reporting:** Minimize total page count first and minimize the sum of per-page content bounding-box areas (`A_bbox = sum(area(B_p))` for pages `1..=P`, where `B_p` encloses placed pieces and compacted registration marks on page `p`). Report `bbox_area_pct` (`(A_bbox / A_puzzle) * 100`, comparing total page bounding-box area against original Stage 0 puzzle grid area `A_puzzle`) in CLI output.
*   **Stage 3 / Stage 4 Transform & Coordinate Parity:** Rotate and translate Stage 3 pieces (`bleed_polygon`) and their `2x2` mirrored raster `#puzzle-pattern` fills via `<g transform="translate(tx, ty) rotate(deg)">` while baking identical right-angle rotations and translations into ungrouped, transform-free Stage 4 `cut_polygon` `<path d="...">` elements. Physical coordinates must agree within `<= 2.2e-4 mm` (accounting for SVG decimal serialization precision and unit conversion) unconditionally for `PageNumbers` registration strokes and placement transforms, and for piece vertices whenever `print_bleed == kerf / 2`.
*   **Deterministic Seeded Default & `--interactive` Convergence Mode:** Execute a fixed deterministic iteration budget with `--seed <U64>` (default `0`) in non-interactive runs, and support an `--interactive` CLI flag that renders a live `stderr` convergence chart of `bbox_area_pct` over continuous iterations with early key-press stop (`q`, `Esc`, `Enter`, or `Ctrl+C`).
*   **Exact Polygon Invariant Verification:** Validate in `verify_page_invariants` that every placed piece stays within page margins, avoids both corner registration keep-out boxes, and maintains exact polygon-to-polygon Euclidean clearance `>= piece_spacing_mm` (up to continuous geometric tolerance).

### Non-Goals

*   **Arbitrary non-`90°` piece rotations:** Continuous or non-cardinal rotation angles are excluded so printed raster artwork stays aligned with the printer and image pixel grid.
*   **Wall-clock optimization timeout flags:** Non-interactive runs use fixed iteration counts for deterministic testability rather than machine-dependent wall-clock timers.
*   **Heterogeneous multi-sheet-size packing within a single CLI run:** Every page in a single invocation shares `Config.page_size`; packing onto accumulated scrap strips (such as `3.5in x 11in` chipboard offcuts) is performed by passing a custom `page_size` in `--config`.
*   **Changes to Stage 0, Stage 1, or Stage 2 piece geometry:** Tab splicing (`splice_tabs`), curve flattening, self-union healing, and kerf/bleed polygon offsetting remain unchanged.
*   **GUI or browser-based manual piece arrangement:** Interactive inspection is limited to the terminal convergence chart under `--interactive`.

## Critical User Journeys (CUJs)

### CUJ 1: Default `8.5 in x 11 in` Nested Generation and Scrap Compaction

*   **Given:** A user has a Stage 0 Inkscape SVG (`src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg`, `4 in x 4 in`, 25 pieces with one intentionally omitted internal tab `path19`) and runs with default configuration (`page_size: 8.5in x 11in`, `margin: 0.25in`, `kerf: 0.18mm`, `piece_spacing: 1 kerf`, `print_bleed: 1 kerf`).
*   **When:** The user runs `puzzle-gen --no-strict src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg`.
*   **Then:**
    *   `puzzle-gen` keeps `generated/rainy-day-puzzle-4x4-25piece-step0-piece-shape-stage2.svg` as an upright `5x5` inspection grid, packs all 25 pieces onto a single `8.5 in x 11 in` page in Stages 3 and 4 using right-angle rotations (`0°`, `90°`, `180°`, `270°`) and bitmask nesting, and pulls the page's bottom-right registration mark up and left to the tightest collision-free anchor `(br_x, br_y)` enclosing the packed pieces.
    *   `puzzle-gen` prints deterministic summary metrics including `pages=1` and `bbox_area_pct=<percentage>` comparing the page content bounding-box area against the `16 sq in` original puzzle area.
    *   The operator prints Stage 3 onto an `8.5 in x 11 in` sticker sheet, adheres it to `12 in x 12 in` chipboard, aligns Stage 4 using the top-left and compacted bottom-right corner marks, cuts the pieces, and severs the unprinted, unscored outer bottom/right portion of the `8.5 in x 11 in` sheet outside the content bounding box—as well as the `3.5 in x 11 in` (or `3.5 in x 12 in`) side strip and `1 in` end strip of chipboard—with straight cuts for future reuse.

### CUJ 2: Large Puzzle Optimization with `--interactive` Convergence and Early Stop

*   **Given:** A user is fabricating a `50 cm x 70 cm` (`28x36`, `1,008`-piece) puzzle configured in `config.yaml` and wants to monitor layout compaction and stop once the bounding-box area ratio levels off.
*   **When:** The user runs `puzzle-gen large-puzzle.svg --config config.yaml --interactive`.
*   **Then:**
    *   `puzzle-gen` computes the initial constructive multi-pass packing and continuously runs seeded local search iterations, rendering a live terminal chart on `stderr` showing current iteration count, total `pages`, and a convergence plot of `bbox_area_pct` over iterations.
    *   When the user presses `q`, `Esc`, `Enter`, or `Ctrl+C`, the optimizer finishes its current iteration, runs final post-pass compaction and exact polygon clearance/keep-out verification on the best layout found so far, writes the final Stage 1–4 SVG files, and prints the final summary metrics to `stdout`.

### CUJ 3: Reproducible Seeded Runs and Offcut Strip Packing

*   **Given:** A user has accumulated `3.5 in x 11 in` chipboard offcut strips from previous `12 in x 12 in` boards and creates `offcut-config.yaml` with `page_size: "3.5in x 11in"`.
*   **When:** The user runs `puzzle-gen small-puzzle.svg --config offcut-config.yaml --seed 42 --dry-run` twice on different machines (`--dry-run` validates geometry and prints metrics without creating or writing files).
*   **Then:**
    *   Both runs execute the exact same number of deterministic search iterations under seed `42` and produce identical `pages` and `bbox_area_pct` values on `stdout`.
    *   If any piece cannot fit inside the `3.5 in x 11 in` margin-bounded sheet (with corner keep-outs) under any of the four right-angle rotations (`0°`, `90°`, `180°`, `270°`), `puzzle-gen` exits non-zero before writing files and reports the offending piece `id` and minimum required `page_size`.

--------------------------------------------------------------------------------

## Proposed Architecture

### Architecture Overview

The pipeline maintains separation between the pure geometric library modules ([`lib.rs`](../../src/puzzle-gen/src/lib.rs)) and the CLI/TUI binary driver ([`main.rs`](../../src/puzzle-gen/src/main.rs)). Stages 0, 1, and 2 execute unchanged in canonical millimeters (`mm`), except that `SplicedPuzzle` and `KerfedGrid` forward `puzzle_bounds_mm` (the Stage 0 grid bounding box) so Stage 3/4 pagination can compute `bbox_area_pct` against the original puzzle area `A_puzzle`.

Stage 2 SVG emission (`emit_stage2` in [`svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs)) and CLI summary reporting in [`main.rs`](../../src/puzzle-gen/src/main.rs) call `compute_slot_bounds` directly to render the upright `(row, col)` inspection grid and print `slot_mm=`. In Stage 3 and Stage 4 pagination, [`paginate.rs`](../../src/puzzle-gen/src/paginate.rs) remains the public entry module—owning localized corner keep-outs, vector stroke assembly, `verify_page_invariants`, and `PaginatedLayout` metrics—while delegating irregular piece nesting and bottom-right mark anchor probing to `src/puzzle-gen/src/nest.rs` (new). When `--interactive` is enabled, the solver streams `NestProgress` snapshots to the terminal UI in `src/puzzle-gen/src/tui.rs` (new).

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
 (compute_slot_bounds:                          (nest.rs: bitmask nesting, 0/90/180/270 rot,
  upright (row, col) grid)                       tail-window search, post-pass BR compaction;
                                                 paginate.rs: stroke layout + exact verification)
                                                                   │
                                       ┌───────────────────────────┴───────────────────┐
                                       ▼                                               ▼
                        [ <prefix>-stage3.svg ]                        [ <prefix>-stage4-page<N>.svg ]
                        (<g translate rotate> + #puzzle-pattern)       (Transform-free Cut + PageNumbers)
```

New dependencies added to [`Cargo.toml`](../../src/puzzle-gen/Cargo.toml):

*   **`crossterm` & `ratatui`:** Used exclusively inside the CLI binary modules ([`main.rs`](../../src/puzzle-gen/src/main.rs) and `src/puzzle-gen/src/tui.rs` (new)) for alternate-screen `stderr` convergence charts and raw-mode key polling (`q`, `Esc`, `Enter`, or `Ctrl+C`) under `--interactive`, keeping library modules ([`lib.rs`](../../src/puzzle-gen/src/lib.rs) and geometric stages) free of terminal-I/O imports.

### CLI Interface & Configuration Schema Updates

*   **YAML Configuration ([`config.rs`](../../src/puzzle-gen/src/config.rs), [`default-config.yaml`](../../src/puzzle-gen/examples/default-config.yaml)):**
    *   Update `Config::default()` page dimensions to `8.5in x 11in` (`215.9 mm x 279.4 mm`) and update [`default-config.yaml`](../../src/puzzle-gen/examples/default-config.yaml) to match.
    *   Add optional `seed` (default `0`, unsigned integer), preserving `serde-saphyr` unknown-key and duplicate-key rejection (negatives, floats, and strings fail deserialization).
*   **CLI Flags ([`main.rs`](../../src/puzzle-gen/src/main.rs)):**
    *   `--seed <U64>`: Optional deterministic PRNG seed overriding `config.seed`.
    *   `--interactive`: Optional flag launching the `stderr` convergence TUI while running continuous local search until user key press (`q`, `Esc`, `Enter`, or `Ctrl+C`) or upstream Stage 0–3 failure. When `stdin` or `stderr` is not a TTY, emits an `INTERACTIVE_NON_TTY` warning notice and executes the non-interactive fixed-iteration pass. When combined with `--dry-run`, the interactive chart runs and exits alternate screen before printing `stdout` metrics and skipping file writes.
*   **CLI `stdout` Summary Metrics & Breaking v1 Contract Notice:**
    *   Retain `grid=RxC`, `matched_tabs=M`, `missing_tabs=K`, `slot_mm=WxH` (Stage 2 uniform slot size), and `pages=P`.
    *   Replace `page_grid=CxR` with irregular layout compaction metrics:
        *   `puzzle_area_mm2=<f64>` (Stage 0 `puzzle_bounds_mm` area)
        *   `total_bbox_mm2=<f64>` (sum of per-page `content_bbox_mm` areas across pages `1..=P`)
        *   `bbox_area_pct=<f64>` (`(total_bbox_mm2 / puzzle_area_mm2) * 100.0`)
    *   *Breaking v1 change:* Replacing `PageSlot::{row, col}`, `PaginatedLayout::{page_cols, page_rows, slot_w_mm, slot_h_mm}`, and CLI `page_grid=` is an intentional breaking change to the public library structs in [`model.rs`](../../src/puzzle-gen/src/model.rs) and CLI metric output introduced alongside irregular nesting.

### Domain Data Model Updates

Changes in [`model.rs`](../../src/puzzle-gen/src/model.rs) extend configuration, stage bounds, placement orientation, compaction geometry, and progress reporting:

| Type | Changes | Invariants |
| --- | --- | --- |
| `Config` | Default `page_size` changed to `8.5in x 11in`; add deterministic layout `seed` (`0` default) | CLI `--seed` overrides YAML `seed` before running the pipeline; runs with equal `Config` are deterministic. |
| `SplicedPuzzle` & `KerfedGrid` | Add Stage 0 `puzzle_bounds_mm` rectangle | Computed from Stage 0 `x_lines_mm` and `y_lines_mm`; positive area (`> 0.0`). |
| `Rotation90` | New right-angle rotation enum (`0°`, `90°`, `180°`, `270°`) | Exact integer clockwise rotation around `(0, 0)` in `+y`-down coordinates. |
| `PageSlot` | Replace grid `row`/`col` with `rotation` (`Rotation90`) | Maps piece point `p` via `rotate(p) + offset_mm`. |
| `PagePlan` | Add per-page `content_bbox_mm` rectangle and compacted `bottom_right_anchor_mm` point | `content_bbox_mm` spans from top-left margin to `(max(pieces_max_x, br_x), max(pieces_max_y, br_y))`; `corner_strokes_mm` orders `TopLeft` strokes before `BottomRight`. |
| `PaginatedLayout` | Remove uniform-grid `page_cols`, `page_rows`, `slot_w_mm`, `slot_h_mm`; add `puzzle_area_mm2`, `total_bbox_area_mm2`, `bbox_area_pct` | `puzzle_area_mm2 > 0.0`; `bbox_area_pct == (total_bbox_area_mm2 / puzzle_area_mm2) * 100.0`. |
| `PaginateOptions` & `NestProgress` | Add solver budget (`seed`, optional `max_iters`) and progress snapshot (`rows`, `cols`, `iter`, `total_iters`, `pages`, `total_bbox_area_mm2`, `puzzle_area_mm2`, `bbox_area_pct`, `page_fill_ratios`) | Decouples optimization budget, cancellation, and clamped `[0.0, 1.0]` per-page fill telemetry from terminal I/O crates. |

### Right-Angle Irregular Polygon Nesting Architecture

Nesting logic is split between `src/puzzle-gen/src/nest.rs` (new) for bitmask placement, local search, and mark-anchor probing, and [`paginate.rs`](../../src/puzzle-gen/src/paginate.rs) for corner keep-outs, stroke assembly, and continuous invariant verification:

1.  **Outer Envelope & Conservative Bitmask Precomputation:**
    *   Each piece's outer collision footprint is built by uniting its `cut_polygon` and `bleed_polygon` and dropping any internal socket-bridging holes (enclosed loops formed when outward bleed offsetting bridges across narrow socket necks), so nesting respects both cut paths and bleed borders even when `print_bleed < kerf / 2`.
    *   When `piece_spacing > 0`, the outer envelope is dilated by half of `piece_spacing` plus a radius-scaled chord-sagitta safety margin so polygonal arc approximation never undercuts required clearance. Each piece's four right-angle rotated footprints are rasterized conservatively onto grid bitmasks (marking vertices, edges, and scanline interiors) sized from `kerf` (falling back to `segment_length` when `kerf == 0`, floored and dynamically capped per sheet axis so sheet bitmasks stay compact). Temporary dilated polygons are released after rasterizing each piece's four orientations.
    *   Bitmask disjointness acts as a fast collision filter using margin-padded sheet grids; final spacing guarantees rest on exact continuous polygon verification in `verify_page_invariants`.

2.  **Localized Corner Keep-Outs & Feasibility Validation:**
    *   Full-width horizontal corner bands are replaced by compact corner keep-out boxes sized dynamically to the page number's digit width plus safety margin, expanded by half of `piece_spacing` on the sheet bitmask so half-dilated pieces achieve full `1.0x` separation without double-buffering (`1.5x`).
    *   Before search starts, usable sheet bounds and page-1 `TopLeft` vs. fallback `BottomRight` keep-out disjointness are checked, and every piece is checked across all four rotations against page-1 keep-outs; unplaceable pieces fail immediately naming piece `id` and minimum required `page_size`. When opening multi-digit pages (`10+`), sheet bitmasks use that page's wider keep-out box and reject placements that violate multi-digit bounds.

3.  **Multi-Start Constructive Placement, Seeded Refinement & Post-Pass Compaction:**
    *   **Constructive Decoder:** Evaluates four deterministic piece orderings (decreasing dilated bounding area, straight-edge corners/borders first, decreasing longest bounding side, and increasing convexity ratio) using First-Fit priority across the trailing two open sheets (checking the earlier open sheet first before testing the newer sheet, minimizing pre-compaction piece extent area) with Top-gravity placement, yielding `O(N)` decode complexity per pass instead of `O(N * P)` full-history scans.
    *   **Tail-Window & Global Refinement:** Canonicalizes the best solution's piece ordering by page order so earlier prefix pages form a contiguous frozen prefix, then runs deterministic seeded mutations (`swap`, `insert`, and `rotation preference shift`) across either the trailing tail-page suffix (80% of moves) or the full ordering (20% of moves), accepting strictly lower `(page_count, extent_area_sum)`. During search, `NestProgress` reports pre-compaction page extent area sum; completion reports post-pass `content_bbox_mm` area.
    *   **Decoupled Final-Page & `BottomRight` Mark Compaction:** Runs strictly once on the winning candidate: re-packs final-page pieces under Left-gravity vs. Top-gravity, probes collision-free `BottomRight` mark anchors outward from placed piece extents across 1D and 2D blocking-piece shifts plus the far corner fallback while avoiding `TopLeft` and placed pieces (`piece_spacing`), and selects the gravity + anchor combination yielding the smallest `content_bbox_mm` area (breaking area ties by largest `br_x + br_y` so marks stay toward the outer corner of empty margins).

4.  **Exact Continuous Verification (`verify_page_invariants`):**
    *   Independently verifies that every placed piece's `cut_polygon` and `bleed_polygon` vertices lie inside sheet margins within tolerance, `BottomRight` anchors stay within margins and disjoint from `TopLeft`, piece polygons clear both corner keep-out boxes by `>= piece_spacing` (within tolerance), and all overlapping-AABB same-page piece pairs maintain exact continuous polygon-to-polygon Euclidean clearance using shared segment-to-segment distance and point-in-polygon checks.

### Stage 3 & Stage 4 SVG Emission & Interactive TUI

Coordinate emission and interactive progress display sit in [`svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs), `src/puzzle-gen/src/tui.rs` (new), and [`main.rs`](../../src/puzzle-gen/src/main.rs):

*   **Rotation Transform & Parity (`Rotation90`):**
    *   Because right-angle rotation around `(0, 0)` uses exact integer coefficients `{-1, 0, 1}`, rotation commutes with unit scaling (`mm` to `doc_unit` / `page_unit`).
    *   `emit_stage3` in [`svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs) wraps each unmoved piece (`bleed_polygon` in original artwork coordinates) in `<g transform="translate(tx_doc, ty_doc)">` or `<g transform="translate(tx_doc, ty_doc) rotate(90|180|270)">`, transforming both the bleed polygon and its `url(#puzzle-pattern)` fill together.
    *   `emit_stage4_page` in [`svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs) bakes rotated and translated coordinates directly into ungrouped, transform-free `<path d="...">` elements via a unified polygon formatting helper, keeping `PageNumbers` strokes, placement transforms, and `print_bleed == kerf / 2` vertex coordinates aligned within `<= 2.2e-4 mm` without altering `LengthUnit::decimal_places`.
    *   Both Stage 3 and Stage 4 emit identical `corner_strokes_mm` (`TopLeft` followed by `BottomRight` compacted registration marks).
*   **`--interactive` TUI Loop & Lifecycle:**
    *   The CLI binary spawns the pipeline on a scoped worker thread streaming `NestProgress` updates while holding an RAII terminal guard on `stderr` only when both `stdin` and `stderr` are TTYs.
    *   The UI renders puzzle grid dimensions, page count, current `bbox_area_pct`, iteration counter, convergence plot, clamped per-page fill gauges (showing up to `8` pages plus a compact overflow summary line), and stop-key help (`q`, `Esc`, `Enter`, or `Ctrl+C`).
    *   Upon user stop key or worker channel disconnection (including worker Stage 0–3 errors), the TUI restores terminal mode and exits the alternate screen before notices, errors, `stdout` summary metrics, and Stage 1–4 atomic file writes (skipped under `--dry-run`) execute.

--------------------------------------------------------------------------------

## Alternatives Considered

### Alternative A: Fixed `6 in x 6 in` Page Size vs. Full-Sheet `8.5 in x 11 in` Default with Post-Pass Offcut Compaction

*   **Pros:** Keeping `6 in x 6 in` divides a `12 in x 12 in` chipboard sheet evenly into four squares.
*   **Cons:** Because `6 + 6 = 12 in` exceeds both sides of an `8.5 in x 11 in` (`93.5 sq in`) adhesive sticker sheet, only one `6 in x 6 in` (`36 sq in`) page fits per sticker sheet, discarding 61.5% of label stock before packing begins.
*   **Decision:** Default `page_size` to `8.5in x 11in` and reclaim unused partial-page areas via post-pass `BottomRight` mark compaction, achieving zero pre-layout trim loss on letter sticker sheets while yielding reusable `3.5 in x 11 in` and `1 in x 12 in` straight-cut strips on `12 in x 12 in` chipboard.

### Alternative B: Arbitrary Continuous-Angle Piece Rotations (`0°..360°`)

*   **Pros:** Allows tighter nesting along diagonal tab necks and angled border tabs.
*   **Cons:** Rotates raster artwork at non-cardinal angles relative to the printer and image pixel grid, introducing interpolation blur across piece seams and requiring non-integer trigonometric pattern transforms.
*   **Decision:** Restrict placement orientations to right angles (`0°`, `90°`, `180°`, `270°`), keeping raster samples grid-aligned with exact integer coordinate maps.

### Alternative C: Row-Major Order or Spatial-Band Locality Constraints on Sheets

*   **Pros:** Preserves spatial `(row, col)` or local-cluster assembly order on the laser bed so nearby puzzle pieces stay grouped together after cutting.
*   **Cons:** Prohibits pairing small border pieces or inward-socket interior pieces from across the puzzle into narrow sheet gaps and corner shoulders, increasing page counts on large puzzles.
*   **Decision:** Allow unconstrained global piece permutations and right-angle orientations on Stages 3 and 4 to minimize sheet consumption, while preserving `emit_stage2` in [`svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs) (`<prefix>-stage2.svg`) as an upright `(row, col)` visual lookup map for post-cut assembly.

### Alternative D1: Fixed Corner Registration Mark vs. Post-Pass Mark Compaction

*   **Pros:** Places the registration mark at identical coordinates on every sheet with no collision-probing pass.
*   **Cons:** Prints and scores into the unused corner of partially filled tail pages, spoiling blank offcut areas for straight-cut reuse.
*   **Decision:** Pre-reserve the fallback corner mark during search and compact each page's `BottomRight` mark inward toward placed pieces via a post-pass probe.

### Alternative D2: In-Loop Dynamic Mark Probing vs. Post-Pass Compaction

*   **Pros:** Evaluates exact compacted mark area directly inside every candidate score during local search.
*   **Cons:** Adding polygon-to-rectangle distance checks or continuous `(br_x, br_y)` probing inside every decode candidate increases inner-loop runtime across thousands of placement tests.
*   **Decision:** Score pre-compaction piece extents during inner search and run one deterministic outward `BottomRight` mark probe per page strictly as a post-pass on the winning layout.

### Alternative E: Wall-Clock Optimization Timeout Flag (`--optimize-time <SECONDS>`)

*   **Pros:** Bounds non-interactive optimization runs to a predictable wall-clock duration on slower hardware.
*   **Cons:** Produces different iteration counts and different physical layouts across machines or background load, breaking reproducible test suites.
*   **Decision:** Use fixed deterministic iteration budgets (`64` iterations by default) in standard runs while providing `--interactive` for operator-controlled early stopping.

### Alternative F: Internal Fixed Heuristic vs. Configurable Deterministic Seed (`seed` in YAML & CLI)

*   **Pros:** Hiding `seed` keeps `config.yaml` limited strictly to physical laser and sheet dimensions.
*   **Cons:** Prevents operators from re-running or reproducing a specific non-default layout seed from versioned project configuration or CLI automation.
*   **Decision:** Expose optional `seed` (default `0`) in `config.yaml` and `--seed <U64>` on the CLI alongside `--interactive`.

### Alternative G: Dependency-Free `std` Line-Buffered Output vs. `crossterm` + `ratatui` TUI

*   **Pros:** Standard `std` line output avoids adding two external TUI crates to [`Cargo.toml`](../../src/puzzle-gen/Cargo.toml).
*   **Cons:** Standard cooked `stdin` blocks until `Enter` (ignoring single-key `q` / `Esc`), and line-buffered `stderr` cannot update a convergence plot in place or intercept raw `Ctrl+C` before graceful post-verification file output.
*   **Decision:** Add `crossterm` and `ratatui` isolated inside the binary CLI modules (`tui.rs` / [`main.rs`](../../src/puzzle-gen/src/main.rs)) while keeping library modules ([`lib.rs`](../../src/puzzle-gen/src/lib.rs)) free of terminal-I/O imports.

### Alternative H: Tight AABB Rectangle Packing Without Polygon Nesting

*   **Pros:** Simple rectangle skyline or guillotine packing with zero polygon rasterization.
*   **Cons:** Empty corner shoulders beside protruding tabs inside every piece's axis-aligned bounding box are treated as solid obstacles, blocking tab-to-shoulder and tab-to-socket interleaving.
*   **Decision:** Rasterize outer piece contours (`cut_polygon ∪ bleed_polygon`) so protruding tabs pack into neighboring empty shoulders and concave pockets.

### Alternative I1: All-Pairs Continuous No-Fit Polygons (NFPs)

*   **Pros:** Continuous NFPs compute exact sliding contact boundaries without raster grid quantization.
*   **Cons:** For `1,008` pieces (`4,032` oriented polygons of ~80–120 vertices), computing `~8.1 x 10^6` non-convex pairwise NFPs scales quadratically (`O((4N)^2)`) in precomputation time and requires degenerate tangent handling on narrow undercut sockets.
*   **Decision:** Reject quadratic NFP precomputation in favor of `O(4N)` bitmask precomputation backed by continuous `verify_page_invariants` checks.

### Alternative I2: Multi-Resolution Bitmask Pyramids & Unbounded Open-Page Scans vs. Single-Grid Row-Run Pruning & 2-Page Window

*   **Pros:** Multi-resolution pyramids prune dense solid regions at coarse levels, and scanning all prior open pages `0..P` lets trailing small pieces backfill early-page gaps directly.
*   **Cons:** Multi-level pyramids increase precomputation storage and bit-pyramid synchronization complexity; scanning all `P` open sheets per piece turns decode into `O(N * P)` rather than `O(N)`.
*   **Decision:** Precompute single-resolution conservative bitmasks with per-row free-run pruning and a 2-page trailing window (`MAX_OPEN_PAGES = 2`) for `O(N)` decodes, mitigating multi-page suboptimality via diverse constructive orderings and 20% global re-decodes.

### Alternative J: Stage 3 `<clipPath>` / Pre-Rotated `<pattern>` vs. `<g transform="translate(...) rotate(...)">`

*   **Pros:** Pre-rotating artwork tiles or `<clipPath>` bakes piece rotations directly into Stage 3 polygon coordinates, avoiding nested `<g transform="rotate(...)">` elements around `url(#puzzle-pattern)` fills that older Cairo PDF exporters may mis-render.
*   **Cons:** Generating rotated `<pattern>` definitions per orientation quadruples pattern definitions or requires per-piece `patternTransform` matrix math on `use` tiles, duplicating Stage 4 transform logic without demonstrated Inkscape breakage.
*   **Decision:** Use `<g transform="translate(tx, ty) rotate(deg)">` in `emit_stage3` in [`svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs) matching existing Stage 2/3 `<pattern>` sharing, verify empirically with `inkscape --export-type=pdf`, and retain pre-rotated pattern generation as a fallback if Cairo export fails.

--------------------------------------------------------------------------------

## Non-Functional Requirements

*   **1,008-Piece Scaling & Memory Footprint:** Precomputing `4N` bitmasks (`4,032` oriented variants for a `28x36`, `1,008`-piece `50 cm x 70 cm` puzzle at default `1 kerf` = `0.18 mm` step) must keep retained variant bitmasks `< 25 MB` (analytical estimate: `~15 MB` at `0.18 mm` step assuming `~25 mm x 25 mm` dilated footprints, scaling to `~45 MB` at the `0.10 mm` step floor on sheets whose usable dimensions are `<= 204.8 mm` or `~0.1302 mm` capped step on `8.5 in x 11 in` sheets; temporary dilated polygons are dropped immediately per piece, and each usable sheet grid is capped at `<= 512 KiB` plus `2 * pad_cells` margin padding). Constructive multi-start plus `64` deterministic local search iterations must complete in `< 60 s` in `--release` builds on a `28x36` puzzle (`< 250 ms` on a `5x5`, `25`-piece puzzle).
*   **Cross-Platform Determinism:** Non-interactive runs with the same input SVG, `Config`, and `seed` must produce identical page counts, piece placements, rotations, compacted registration mark coordinates, and `bbox_area_pct` values across Windows, macOS, and Linux.
*   **Stage 3 / Stage 4 Coordinate Parity (`<= 2.2e-4 mm`):** Every `PageNumbers` registration stroke (`TopLeft` and `BottomRight`), every piece placement transform (`Rotation90 + offset_mm`), and every piece vertex when `print_bleed == kerf / 2` must agree between Stage 3 and Stage 4 physical page coordinates within `<= 2.2e-4 mm`, reflecting existing SVG decimal formatting precision (`4` decimal places for `mm`/`px`, `6` for `in`/`cm`) and cross-unit conversion round-tripping without altering `LengthUnit::decimal_places`. (Under default `print_bleed = 0.18 mm != kerf / 2 = 0.09 mm`, Stage 3 intentionally emits `bleed_polygon` while Stage 4 emits `cut_polygon`.)
*   **Terminal State Restoration & Non-TTY Safety:** `--interactive` must restore raw mode and leave the alternate screen via an RAII `Drop` guard on early user stop (`q`, `Esc`, `Enter`, or `Ctrl+C`), worker error or channel disconnection before keypress, or panic unwind before printing notices or errors, and must fall back to non-interactive execution with an `INTERACTIVE_NON_TTY` `Warning` notice when `stdin` or `stderr` is not a TTY.

## Risks & Mitigations

*   **Inkscape Cairo PDF Export of Rotated `userSpaceOnUse` Patterns:** Stage 3 nests `<path style="fill:url(#puzzle-pattern)">` inside `<g transform="translate(...) rotate(90|180|270)">`. Certain Inkscape/Cairo PDF export versions may mishandle `patternUnits="userSpaceOnUse"` inside nested rotated groups. Empirical verification using `inkscape --export-type=pdf` is included in the [Verification Plan](#verification-plan); if Inkscape mis-aligns rotated artwork in exported PDFs, `emit_stage3` in [`svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs) will switch to pre-rotated `<pattern>` transforms (`Alternative J` fallback).
*   **Bitmask Grid Coarsening on Small `kerf` or Large Sheets (`> 368.64 mm`):** Flooring `step_mm` at `0.10 mm` (using `segment_length_mm` when `kerf_mm == 0.0`) and capping usable grid dimensions at `2048` cells per axis coarsens the raster filter beyond `1 kerf` whenever `kerf_mm < 0.10 mm` or `max(usable_w, usable_h) > 2048 * kerf_mm` (`> 368.64 mm` / `~14.5 in` at default `0.18 mm` kerf; e.g., `~0.44 mm` step on a `24 in x 36 in` laser bed). Larger grid steps inflate conservative bitmask envelopes by up to one extra step per edge, modestly reducing packing density on oversized laser beds without violating continuous spacing verified by `verify_page_invariants`.
*   **Sliding-Window (`MAX_OPEN_PAGES = 2`) & Tail-Window (`TAIL_PAGES = 2`) Multi-Page Suboptimality:** Limiting `decode_layout` to the trailing two open sheets prevents small trailing pieces from backfilling gaps on closed pages `1..=P-2`, and spending `80%` of local search mutations on the last two pages concentrates refinement on tail compaction rather than earlier sheets. Evaluating four diverse deterministic constructive orderings alongside `20%` full-permutation global mutations mitigates early-page gap entrapment while preserving `O(N)` decode speed.
*   **Interactive Key Cancellation & Per-File Output Atomicity:** Raw-mode event polling intercepts `q`, `Esc`, `Enter`, and `Ctrl+C` (`KeyModifiers::CONTROL`) via `crossterm` so optimization stops between iterations and runs post-pass compaction plus full `verify_page_invariants` checks before writing files. External OS `SIGINT`/`SIGTERM` signals are not trapped independently and terminate immediately without running `Drop` cleanup; written SVG outputs are replaced per-file atomically via `NamedTempFile::persist` (matching [`puzzle-gen.md`](./puzzle-gen.md)), not wrapped in a multi-file filesystem transaction across Stages 1–4.
*   **Local-Trust Security Boundary & Resource Caps:** Like [`puzzle-gen.md`](./puzzle-gen.md), `puzzle-gen` operates under a local-trust execution model on operator-supplied Stage 0 SVGs and YAML configs. `serde-saphyr` resource limits, explicit positive usable-sheet dimension checks, and the `2048`-cell axis cap bound each per-sheet bitmask allocation to `<= 512 KiB` plus padding, while per-piece variant bitmasks scale linearly (`4N`) with input piece count `N`.

--------------------------------------------------------------------------------

## Additional Resources

*   [`docs/designs/puzzle-gen.md`](./puzzle-gen.md): Initial four-stage `puzzle-gen` architecture and Stage 0–4 geometric and SVG specifications.

## Future Improvements

*   **Stage 2 Diagnostic Problem-Stroke Highlighting:** Color and bold specific edges or pieces in `<prefix>-stage2.svg` where non-fatal warnings (`MISSING_INTERNAL_TAB` under `--no-strict`, `BLEED_LESS_THAN_HALF_KERF`), silent Stage 2 geometric repairs (self-union overshoot healing or bleed socket bridging), or `--emit-debug-on-error` Stage 2 offsetting failures occurred (deferred to a separate design).

--------------------------------------------------------------------------------

## Detailed Implementation Plan

All Rust code changes are confined to the `puzzle-gen` crate ([`src/puzzle-gen`](../../src/puzzle-gen)) plus Cargo's workspace lockfile ([`Cargo.lock`](../../Cargo.lock)). No other workspace member crate (`sticker-size`) is modified. Implementation proceeds across three sequential phases so `cargo check --workspace --all-targets` and `cargo test -p puzzle-gen` compile and pass after every phase.

### Phase 1: Dependencies, Configuration Schema & Stage 0–2 Bounds

1.  **[`src/puzzle-gen/Cargo.toml`](../../src/puzzle-gen/Cargo.toml) & [`Cargo.lock`](../../Cargo.lock) (Modify):**
    *   **Rationale:** Add `crossterm` and `ratatui` to support the `--interactive` terminal convergence chart and raw-mode key-polling loop in the CLI binary while keeping library modules (`lib.rs`) free of terminal-I/O imports.
    *   **Proposed Changes:** Add `crossterm = "0.29"` and `ratatui = "0.30"` under `[dependencies]` in `src/puzzle-gen/Cargo.toml` and update workspace `Cargo.lock`.
2.  **[`src/puzzle-gen/examples/default-config.yaml`](../../src/puzzle-gen/examples/default-config.yaml) (Modify):**
    *   **Rationale:** Align the documented default config file with the new `8.5in x 11in` default page size and explicit `seed: 0` parameter so `Config::from_yaml` equality tests pass.
    *   **Proposed Changes:** Change `page_size: "6in x 6in"` to `page_size: "8.5in x 11in"`, update the `piece_spacing` comment to describe minimum clearance between neighboring piece contours across Stages 2–4, and add `seed: 0` documenting deterministic seeded layout search.
3.  **[`src/puzzle-gen/src/model.rs`](../../src/puzzle-gen/src/model.rs) (Modify — Additive Phase 1 Types):**
    *   **Rationale:** Introduce `8.5in x 11in` defaults, YAML/CLI layout `seed`, `puzzle_bounds_mm`, `Rotation90`, and solver control/progress types before the Phase 2 pagination cutover.
    *   **Proposed Changes:**
        *   Add `pub seed: u64` to `Config`; in `Config::default()`, set `page_width_mm: LengthUnit::In.to_mm(8.5)`, `page_height_mm: LengthUnit::In.to_mm(11.0)`, and `seed: 0`.
        *   Add `pub puzzle_bounds_mm: kurbo::Rect` to `SplicedPuzzle` and `KerfedGrid`.
        *   Define `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum Rotation90 { Deg0, Deg90, Deg180, Deg270 }` with `pub const ALL: [Rotation90; 4]`, `pub fn degrees(self) -> u16`, `pub fn next_cw(self) -> Self`, `pub fn prev_cw(self) -> Self`, and `pub fn rotate_point(self, p: kurbo::Point) -> kurbo::Point` mapping `(x, y)` to `(x, y)`, `(-y, x)`, `(-x, -y)`, and `(y, -x)`.
        *   Define `pub const DEFAULT_OPTIMIZE_ITERS: usize = 64;`, `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct PaginateOptions { pub seed: u64, pub max_iters: Option<usize> }` (`PaginateOptions::from_config(config: &Config)` returns `max_iters: Some(DEFAULT_OPTIMIZE_ITERS)`), and `#[derive(Debug, Clone, PartialEq)] pub struct NestProgress { pub rows: usize, pub cols: usize, pub iter: usize, pub total_iters: Option<usize>, pub pages: usize, pub total_bbox_area_mm2: f64, pub puzzle_area_mm2: f64, pub bbox_area_pct: f64, pub page_fill_ratios: Vec<f64> }`.
4.  **[`src/puzzle-gen/src/config.rs`](../../src/puzzle-gen/src/config.rs) (Modify):**
    *   **Rationale:** Parse and validate optional `seed` in `config.yaml` while preserving strict unknown-key and duplicate-key rejection.
    *   **Proposed Changes:**
        *   Add `seed: Option<u64>` to `RawConfig` (so `serde_saphyr` rejects negative numbers, floats, and strings during deserialization) and apply `if let Some(seed) = raw.seed { cfg.seed = seed; }` in `Config::from_yaml`.
        *   Update unit tests in `config.rs` (`yaml_defaults_and_kerf_multipliers`, verifying default `seed == 0`, explicit `seed: 42`, and rejection of `seed: -1` and `seed: 1.5`).
5.  **[`src/puzzle-gen/src/stage1.rs`](../../src/puzzle-gen/src/stage1.rs) (Modify):**
    *   **Rationale:** Forward Stage 0 rectilinear grid bounds into `SplicedPuzzle` so `bbox_area_pct` measures against the Stage 0 puzzle grid area rather than image bounds or offset contours.
    *   **Proposed Changes:** In `splice_tabs`, compute `puzzle_bounds_mm = Rect::new(puzzle.x_lines_mm[0], puzzle.y_lines_mm[0], puzzle.x_lines_mm[puzzle.cols], puzzle.y_lines_mm[puzzle.rows])` and populate `SplicedPuzzle::puzzle_bounds_mm`.
6.  **[`src/puzzle-gen/src/stage2.rs`](../../src/puzzle-gen/src/stage2.rs) (Modify):**
    *   **Rationale:** Propagate `puzzle_bounds_mm` into `KerfedGrid` and share existing `i_overlay` outer-ring selection and geometry helpers for crate reuse.
    *   **Proposed Changes:**
        *   In `heal_and_offset`, set `puzzle_bounds_mm: spliced.puzzle_bounds_mm` on `KerfedGrid`.
        *   Extract `pub(crate) fn select_outer_ring(shapes: Vec<Vec<Vec<[f64; 2]>>>, interior_pt: Point) -> Option<Vec<Point>>` from lines `120..=133` in `process_piece` (selecting the largest positive-area outer ring containing `interior_pt` while dropping internal socket-bridging holes) and call it from `process_piece`.
        *   Promote `outline_options_i64`, `perp_dist_point_segment`, and `find_interior_point` visibility to `pub(crate)` (joining already-`pub(crate)` `signed_area`, `polygon_aabb`, and `point_in_polygon`).
7.  **[`src/puzzle-gen/src/paginate.rs`](../../src/puzzle-gen/src/paginate.rs) (Modify — Phase 1 Test Literal Update):**
    *   **Rationale:** Keep `cargo check --workspace --all-targets` and unit tests compiling cleanly at the Phase 1 Checkpoint after adding `puzzle_bounds_mm` to `KerfedGrid`.
    *   **Proposed Changes:** In `#[cfg(test)] mod tests::compute_slot_bounds_is_symmetric_and_rejects_oversized_piece`, add `puzzle_bounds_mm: Rect::new(0.0, 0.0, 20.0, 20.0)` (positive area) to the `KerfedGrid` initializer.

*   **Phase 1 Checkpoint:** Run `cargo check --workspace --all-targets` and `cargo test -p puzzle-gen`.

### Phase 2: Irregular Nesting Engine, Model Cutover, Stage 3/4 Emission & CLI Metrics

8.  **[`src/puzzle-gen/src/model.rs`](../../src/puzzle-gen/src/model.rs) (Modify — Phase 2 Domain Cutover):**
    *   **Rationale:** Switch `PageSlot`, `PagePlan`, and `PaginatedLayout` together with their downstream callers so every file builds in one phase.
    *   **Proposed Changes:**
        *   In `PageSlot`, replace `row: usize` and `col: usize` with `pub rotation: Rotation90`, and add `pub fn transform_point(self, p: kurbo::Point) -> kurbo::Point { self.rotation.rotate_point(p) + self.offset_mm }`.
        *   In `PagePlan`, add `pub content_bbox_mm: kurbo::Rect` and `pub bottom_right_anchor_mm: kurbo::Point`.
        *   In `PaginatedLayout`, remove `page_cols: usize`, `page_rows: usize`, `slot_w_mm: f64`, and `slot_h_mm: f64` (`main.rs` and `emit_stage2` call `paginate::compute_slot_bounds` directly), and add `pub puzzle_area_mm2: f64`, `pub total_bbox_area_mm2: f64`, and `pub bbox_area_pct: f64`.
9.  **`src/puzzle-gen/src/nest.rs` (Create):**
    *   **Rationale:** Isolate conservative bitmask precomputation, sliding-window constructive placement, `SplitMix64` tail-window local search, and post-pass `BottomRight` mark anchor probing into focused helper functions invoked by `paginate.rs`, keeping vector glyph layout and `PagePlan` assembly inside `paginate.rs`.
    *   **Proposed Changes:**
        *   **Public Crate Interface:**
            *   Define `pub(crate) struct RawPagePlacement { pub slots: Vec<PageSlot>, pub br_anchor_mm: kurbo::Point, pub content_bbox_mm: kurbo::Rect }`.
            *   Declare `pub(crate) fn pack_irregular_pages<F>(kerfed: &KerfedGrid, config: &Config, options: &PaginateOptions, cancel: &AtomicBool, on_progress: &mut F) -> Result<Vec<RawPagePlacement>, PipelineError> where F: FnMut(NestProgress)`.
        *   **PRNG (`SplitMix64`):**
            *   Implement `next_u64` (`0x9e3779b97f4a7c15`, `0xbf58476d1ce4e5b9`, `0x94d049bb133111eb`) and unbiased `gen_range(upper: usize) -> usize`.
        *   **Sheet Usability & Page-1 Keep-Out Validation:**
            *   Validate `kerfed.puzzle_bounds_mm.area() > 0.0`.
            *   Compute `usable_w = config.page_width_mm - 2.0 * config.margin_mm` and `usable_h = config.page_height_mm - 2.0 * config.margin_mm`.
            *   Query page-1 keep-out dimensions `(kw_1, kh) = paginate::corner_keepout_size_mm(1)` and compute minimum valid disjoint-keepout sheet size `min_sheet = (2.0 * config.margin_mm + 2.0 * kw_1 + config.piece_spacing_mm) x (2.0 * config.margin_mm + 2.0 * kh + config.piece_spacing_mm)`.
            *   If `usable_w <= 0.0 || usable_h <= 0.0`, return `PipelineError::new("paginate", ...)` reporting `minimum required page_size is {min_w:.2}mm x {min_h:.2}mm`.
            *   Check page-1 `TopLeft` and fallback `BottomRight` keep-out disjointness (`usable_w >= 2.0 * kw_1 + config.piece_spacing_mm || usable_h >= 2.0 * kh + config.piece_spacing_mm`); if both overlap, return `PipelineError::new("paginate", ...)` stating corner keep-outs overlap and reporting `minimum required page_size is {min_w:.2}mm x {min_h:.2}mm` (without citing a `piece.id`).
        *   **Grid Resolution & Scaled Sagitta Dilation:**
            *   Compute `raw_step = if config.kerf_mm > 0.0 { config.kerf_mm } else { config.segment_length_mm }`, `step_mm = raw_step.max(0.10).max(usable_w.max(usable_h) / 2048.0)`.
            *   Compute `usable_cols = (usable_w / step_mm).floor().clamp(1.0, 2048.0) as usize` and `usable_rows = (usable_h / step_mm).floor().clamp(1.0, 2048.0) as usize`.
            *   Compute `dilation_eps_mm = 0.005 + 0.002 * config.piece_spacing_mm` (covering `LineJoin::Round(0.1)` chord sagitta proportional to radius `0.5 * piece_spacing_mm`), `pad_mm = if config.piece_spacing_mm > 0.0 { 0.5 * config.piece_spacing_mm + dilation_eps_mm } else { 0.0 }`, and `pad_cells = (pad_mm / step_mm).ceil() as usize + 1`.
        *   **Outer Envelope & Conservative Bitmask Precomputation:**
            *   For each piece `i`, convert `cut_polygon` and `bleed_polygon` (`&[Point]`) to `Vec<[f64; 2]>`, call `cut_io.overlay(&bleed_io, OverlayRule::Union, FillRule::Positive)` (3-argument `SingleFloatOverlay::overlay`), and extract `outer_env` via `stage2::select_outer_ring(shapes, stage2::find_interior_point(&piece.cut_polygon))` (dropping internal socket-bridging holes).
            *   When `config.piece_spacing_mm > 0.0`, compute temporary `dilated_poly` via `outer_env_io.outline_custom_as::<i64>(&OutlineStyle::new(pad_mm).line_join(LineJoin::Round(0.1)), stage2::outline_options_i64())` and extract its outer ring via `stage2::select_outer_ring`; otherwise clone `outer_env`.
            *   For each `(piece_i, rot)` in `Rotation90::ALL`, rotate `outer_env` and temporary `dilated_poly`, compute `outer_env` bounds `(env_min_x, env_min_y, env_w, env_h)` and `outer_env_area`, shift local coordinates by `(-env_min_x + (pad_cells as f64) * step_mm, -env_min_y + (pad_cells as f64) * step_mm)`, and conservatively rasterize `dilated_poly` onto row-major `Vec<u64>` words (`LSB = col 0`, bit `lx & 63`, clamping indices to variant bounds) covering vertices, supercover edge traversals, and horizontal scanlines at `(row + 0.5) * step_mm`.
            *   Precompute `row_max_one_run: Vec<u16>` (longest contiguous run of `1`-bits in each variant row `vy`, or `0` if empty) and `variant_w` / `variant_h` (clamped to `<= total_cols` / `<= total_rows` where needed). Drop temporary `dilated_poly` vectors immediately after piece `i`'s four variants are rasterized.
        *   **`SheetBitmask` & Keep-Out Rasterization:**
            *   Construct sheets with `total_cols = usable_cols + 2 * pad_cells`, `total_rows = usable_rows + 2 * pad_cells`, `words_per_row = total_cols.div_ceil(64)`, `words: Vec<u64>`, and `row_max_free_run: Vec<u16>` sized to `total_rows` (recomputing row `r`'s widest zero-bit span across `0..total_cols` in `O(words_per_row)` per row whenever bits are written).
            *   Reserve keep-outs for `page_number` using `(kw, kh) = paginate::corner_keepout_size_mm(page_number)` expanded by `0.5 * config.piece_spacing_mm`: usable boxes `[0.0, kw + 0.5 * spacing] x [0.0, kh + 0.5 * spacing]` (`TopLeft`) and `[usable_w - kw - 0.5 * spacing, usable_w] x [usable_h - kh - 0.5 * spacing, usable_h]` (`BottomRight`). Shift coordinates by `+pad_cells * step_mm` into padded sheet coordinates and mark grid cell ranges `[floor(x0 / step_mm)..ceil(x1 / step_mm)] x [floor(y0 / step_mm)..ceil(y1 / step_mm)]` clamped to `0..total_cols` and `0..total_rows` (leaving the outer `pad_cells` margin ring unmarked `0` so dilated piece buffers can overhang margins up to `CONTAINMENT_TOL_MM` without false collision).
            *   When testing variant placement at padded `(gx, gy)` (requiring `gx + variant_cols <= total_cols` and `gy + variant_rows <= total_rows`), read `(sheet_word >> sh)` and conditionally OR `(next_word << (64 - sh))` only when `sh > 0 && word_col + 1 < words_per_row`.
        *   **Single-Piece Pre-Flight Feasibility & Multi-Digit Page Opening:**
            *   Before search, test every piece `i` across `Rotation90::ALL` on an empty page-1 `SheetBitmask`: require `env_w <= usable_w + paginate::CONTAINMENT_TOL_MM`, `env_h <= usable_h + paginate::CONTAINMENT_TOL_MM`, `max_gx = ((usable_w - env_w).max(0.0) / step_mm).floor().min((usable_cols - 1) as f64) as usize` (and matching `max_gy`), and at least one collision-free `(gx, gy)` against page-1 keep-outs.
            *   If no rotation fits on page 1, return `PipelineError::new("paginate", ...)` naming `piece.id`, the `(w_r, h_r)` outer-envelope dimensions of the rotation yielding the minimum required `page_size` area (choosing the smaller area between `(w_r + 2*margin + 2*(kw_1 + spacing)) x (h_r + 2*margin)` and `(w_r + 2*margin) x (h_r + 2*margin + 2*(kh + spacing))`), and that `minimum required page_size`.
            *   Map placed `(gx, gy)` to continuous `offset_mm = Vec2::new(config.margin_mm + ((gx as f64) * step_mm).min((usable_w - env_w).max(0.0)) - env_min_x, config.margin_mm + ((gy as f64) * step_mm).min((usable_h - env_h).max(0.0)) - env_min_y)`.
            *   If `decode_layout` opens a multi-digit sheet (`page_number >= 10`) and the active piece cannot fit on that freshly opened empty sheet under `corner_keepout_size_mm(page_number)`, fail with that piece's required `page_size` for `page_number` keep-outs (if encountered during initial constructive passes) or reject the local-search candidate cleanly (keeping `best`).
        *   **`decode_layout` (`MAX_OPEN_PAGES = 2`, First-Fit Sheet Priority, Exact Row-Scan Pruning):**
            *   Define a page's pre-compaction extent rectangle from placed `outer_env` bounds `(pieces_max_x, pieces_max_y)` and keep-out `(kw, kh)`: `extent_w = (pieces_max_x - margin_mm).max(kw)`, `extent_h = (pieces_max_y - margin_mm).max(kh)`, `extent_area_mm2 = extent_w * extent_h` (excluding fallback corner marks).
            *   For each piece in `order`, test open sheets in the trailing window `pages.len().saturating_sub(MAX_OPEN_PAGES)..pages.len()` strictly from older to newer (**First-Fit sheet priority**: if at least one rotation fits on the older open sheet, select the best placement on that older sheet and skip newer sheets).
            *   On a candidate sheet, test rotations `[pref, pref.next_cw(), pref.next_cw().next_cw(), pref.prev_cw()]`. For each `(rot_rank, rot)`, scan `gy` in `0..=max_gy`:
                *   Skip row `gy` immediately if any variant row `vy` has `sheet.row_max_free_run[gy + vy] < var.row_max_one_run[vy]`.
                *   Compute `row_min_h = cur_extent_h.max((gy as f64) * step_mm + env_h)` and `min_possible_area = cur_extent_w.max(env_w) * row_min_h`; if a valid candidate on this sheet is already found with `best_area_on_sheet` and `min_possible_area >= best_area_on_sheet`, break out of `gy` (lossless early exit since later rows have larger `gy` and cannot improve `new_extent_area_mm2` or `gy`).
                *   Scan `gx` in `0..=max_gx` (jumping ahead over occupied words via `trailing_ones()`) and accept only the **first** collision-free `gx` on row `gy` (smallest `gx` minimizes both `new_extent_area_mm2` and the `gx` tie-breaker), minimizing `(new_extent_area_mm2, gy, gx, rot_rank)` across rotations on that sheet.
            *   Open a new sheet (with `page_number = pages.len() + 1` keep-outs) only if no trailing open sheet fits.
        *   **Four Initial Orderings & Exact `rot_prefs`:**
            *   Define `grid_border_edges(piece) = (row == 0) as u8 + (col == 0) as u8 + (row + 1 == rows) as u8 + (col + 1 == cols) as u8`.
            *   Evaluate 4 deterministic constructive passes:
                1. Decreasing `dilated` AABB area (`piece_index` ascending tie-break), all `rot_prefs = Rotation90::Deg0`.
                2. Border/corner pieces first (`grid_border_edges` desc, `outer_env_area` desc, `piece_index` asc), setting `rot_prefs` to align straight boundary edges flush to top/left margins: `(row == 0, col == 0) -> Deg0`, `(row == 0, col + 1 == cols && cols > 1) -> Deg270`, `(row + 1 == rows && col + 1 == cols && rows > 1 && cols > 1) -> Deg180`, `(row + 1 == rows && col == 0 && rows > 1) -> Deg90`, single border `col + 1 == cols -> Deg180` / `row + 1 == rows -> Deg180`, else `Deg0`.
                3. Decreasing `max(env_w, env_h)` (`piece_index` asc), all `rot_prefs = Rotation90::Deg0`.
                4. Increasing `convexity_ratio = outer_env_area / (env_w * env_h)` (`piece_index` asc), all `rot_prefs = Rotation90::Deg0`.
        *   **`SplitMix64` Local Search & Canonical `order`:**
            *   Define `const TAIL_PAGES: usize = 2;`. Whenever `best` updates (from constructive passes or search), canonicalize `best.order` by page order (`pages[0]` slots' `piece_index`es, followed by `pages[1]`, etc.) and update `best.rot_prefs` to the placed rotations so frozen prefix `0..(best.pages.len() - TAIL_PAGES)` maps strictly to a contiguous prefix slice `order[0..prefix_len]` and the last `TAIL_PAGES` pages map to `order[prefix_len..N]`.
            *   Loop `iter` from `1` up to `options.max_iters` (or indefinitely while `!cancel.load(Ordering::Relaxed)`). Emit baseline `on_progress` after constructive passes at `iter = 0` (using pre-compaction `cand_area_sum = sum(extent_area_mm2)`).
            *   When `best.pages.len() > TAIL_PAGES`, apply tail-window mutations `80%` of iterations (cloning frozen prefix `0..(best.pages.len() - TAIL_PAGES)` `SheetBitmask`s and mutating/re-decoding only `order[prefix_len..N]`) and global mutations `20%` of iterations (full decode). Within the active slice, if `slice.len() >= 2`, pick `50%` swap two indices, `30%` move/insert one element, or `20%` rotate one element's `rot_prefs[piece_index]`; if `slice.len() < 2`, apply only the rotation mutation.
            *   Accept candidate when `(cand_pages, cand_area_sum)` is strictly better than `(best_pages, best_area_sum - 1e-6)` (or fewer pages), and emit `on_progress` on each accepted improvement and when `iter > 0 && iter % 8 == 0 && Some(iter) != options.max_iters`.
        *   **Post-Pass Compaction (`compact_final_page` & 1D/2D `BottomRight` Mark Probing):**
            *   Run strictly once on the winning candidate: keep the final page's winning Top-gravity placement (`pieces_order` = winning slot `piece_index`es, `rot_prefs` = winning slot rotations), also attempt re-packing those same pieces onto a single fresh sheet under Left-gravity (`gx` ascending, then `gy` ascending), and discard the Left-gravity attempt if it overflows onto `> 1` page.
            *   For every page `p` (and for both final-page Top-gravity vs. valid 1-page Left-gravity candidates before selecting the lower post-mark `content_bbox_mm.area()`): compute `(pieces_max_x, pieces_max_y)` across placed `outer_env` contours, `(kw, kh) = paginate::corner_keepout_size_mm(p)`, `fallback = (config.page_width_mm - config.margin_mm, config.page_height_mm - config.margin_mm)`, and `base = (pieces_max_x.max(config.margin_mm + kw).min(fallback.x), pieces_max_y.max(config.margin_mm + kh).min(fallback.y))`.
            *   Build candidate `(br_x, br_y)` anchors within `[config.margin_mm + kw, fallback.x] x [config.margin_mm + kh, fallback.y]`: `base`, 1D X shifts `(aabb_i.x1 + kw + spacing, base.y)`, 1D Y shifts `(base.x, aabb_i.y1 + kh + spacing)`, 2D pair shifts `(aabb_i.x1 + kw + spacing, aabb_j.y1 + kh + spacing)` over `outer_env` AABBs on page `p`, and `fallback`.
            *   Filter non-fallback candidates using `paginate::polygon_rect_clearance_ok(poly, br_rect, config.piece_spacing_mm, 0.0)` across placed `cut_polygon` and `bleed_polygon` contours plus `TopLeft`/`BottomRight` disjointness (with `fallback` checked at `paginate::CONTAINMENT_TOL_MM` and always retained as guaranteed fallback), minimizing `content_bbox_mm.area()` where `content_bbox_mm = Rect::new(margin_mm, margin_mm, pieces_max_x.max(br_x), pieces_max_y.max(br_y))` and breaking area ties by largest `br_x + br_y`.
            *   After post-pass compaction, invoke `on_progress` once with final post-compaction `total_bbox_area_mm2` and `bbox_area_pct` (setting `page_fill_ratios[p] = (placed_outer_env_area_sum / (usable_w * usable_h)).clamp(0.0, 1.0)`).
        *   **Unit Tests in `nest.rs`:** Verify conservative rasterizer covers vertices/edges/interiors, `SplitMix64` determinism, single-piece `90°` rotation fits when `0°` exceeds sheet width, and post-pass `BottomRight` mark compaction chooses tightest anchor (including 2D shift and `br_x + br_y` tie-breaking).
10. **[`src/puzzle-gen/src/paginate.rs`](../../src/puzzle-gen/src/paginate.rs) (Modify):**
    *   **Rationale:** Replace `CORNER_BAND_MM` uniform bands with localized keep-out helpers, anchor-parameterized corner strokes, shared exact polygon clearance checks, `PagePlan` assembly, and exact continuous verification while preserving `compute_slot_bounds`.
    *   **Proposed Changes:**
        *   Promote `pub(crate) const CONTAINMENT_TOL_MM: f64 = 1e-4;` and replace `CORNER_BAND_MM` with `pub const CORNER_KEEP_OUT_H_MM: f64 = 6.0;`.
        *   Implement `pub fn corner_keepout_size_mm(page_number: usize) -> (f64, f64)` (`num_w = d_count * 2.4 + (d_count - 1.0).max(0.0) * 0.48`, `keepout_w = (1.0 + num_w).max(2.5) + 0.7`, `CORNER_KEEP_OUT_H_MM`) and `pub fn corner_keepout_rects_mm(page_number: usize, config: &Config, br_anchor: Point) -> [Rect; 2]` returning `[TopLeft, BottomRight]`.
        *   Implement `pub fn layout_corner_page_number_at_anchor(page_number: usize, anchor_x: f64, anchor_y: f64, corner: CornerAnchor) -> Vec<Vec<Point>>` anchored at the outer L-tick corner vertex, and delegate `layout_corner_page_number` to it.
        *   Keep `pub fn compute_slot_bounds(kerfed: &KerfedGrid) -> (f64, f64)` intact.
        *   Implement shared exact geometric clearance helpers:
            *   `pub(crate) fn segment_to_segment_distance(a: Point, b: Point, c: Point, d: Point) -> f64` (returning `0.0` when segments `[a, b]` and `[c, d]` intersect, otherwise minimum of the four endpoint `stage2::perp_dist_point_segment` distances).
            *   `pub(crate) fn polygon_ring_clearance_ok(poly_a: &[Point], poly_b: &[Point], min_spacing: f64, tol: f64) -> bool` (unconditionally returning `false` if any vertex of `poly_a` is inside `poly_b` or vice versa via `stage2::point_in_polygon`, or if any edges intersect at distance `<= 1e-12`, even when `min_spacing == 0.0`; otherwise verifying `segment_to_segment_distance >= (min_spacing - tol).max(0.0)` across all edge pairs).
            *   `pub(crate) fn polygon_rect_clearance_ok(poly: &[Point], rect: Rect, min_spacing: f64, tol: f64) -> bool` (converting `rect` to its 4-corner ring and delegating to `polygon_ring_clearance_ok`).
        *   Add `pub fn paginate_pieces_with_options<F>(kerfed: &KerfedGrid, config: &Config, options: &PaginateOptions, cancel: &AtomicBool, mut on_progress: F) -> Result<PaginatedLayout, PipelineError> where F: FnMut(NestProgress)` validating `puzzle_area_mm2 > 0.0`, invoking `nest::pack_irregular_pages`, assembling `PagePlan` (`layout_corner_page_number_at_anchor` for `TopLeft` followed by `BottomRight`), verifying every page with `verify_page_invariants`, validating `assigned_count == kerfed.pieces.len()`, and computing `total_bbox_area_mm2` and `bbox_area_pct`. Delegate `paginate_pieces(kerfed, config)` to it.
        *   Update `verify_page_invariants(kerfed: &KerfedGrid, page_number: usize, slots: &[PageSlot], br_anchor: Point, config: &Config) -> Result<(), PipelineError>` to check: (a) all transformed `cut_polygon` and `bleed_polygon` vertices inside sheet margins within `CONTAINMENT_TOL_MM`, (b) `br_anchor` inside valid margin/keep-out bounds and `tl_rect` / `br_rect` disjoint, (c) `polygon_rect_clearance_ok(poly, keepout_rect, config.piece_spacing_mm, CONTAINMENT_TOL_MM)` for every piece's `cut_polygon` and `bleed_polygon` against both keep-out boxes, and (d) pairwise `polygon_ring_clearance_ok(poly_a, poly_b, config.piece_spacing_mm, CONTAINMENT_TOL_MM)` for all overlapping-AABB same-page piece pairs across `cut_polygon` and `bleed_polygon`.
        *   Update `compute_slot_bounds_is_symmetric_and_rejects_oversized_piece` error assertion from `"30.00 x 21.00"` to `"26.00 x 21.00"` (the test piece's actual rotated outer-envelope width/height) and `"p0"`, and add unit tests for `corner_keepout_rects_mm`, `layout_corner_page_number_at_anchor`, crossing-segment keep-out detection, and `verify_page_invariants` catching piece overlap, `BottomRight` keep-out violation, and overlapping keep-out boxes.
11. **[`src/puzzle-gen/src/lib.rs`](../../src/puzzle-gen/src/lib.rs) (Modify):**
    *   **Rationale:** Expose `run_pipeline_with_options` for progress streaming and cancellation while keeping `nest` crate-private and preserving debug artifact attachment on pagination failure.
    *   **Proposed Changes:**
        *   Add `mod nest;`.
        *   Add `pub fn run_pipeline_with_options<F>(svg_xml: &str, config: &Config, strict: bool, notices: &mut Vec<PipelineNotice>, options: &PaginateOptions, cancel: &AtomicBool, on_progress: F) -> Result<PipelineArtifacts, PipelineError> where F: FnMut(NestProgress)`, forwarding into `paginate::paginate_pieces_with_options` and retaining `.map_err(|err| err.with_debug_spliced(spliced.clone()).with_debug_kerfed(kerfed.clone()))?`.
        *   Delegate `run_pipeline` to `run_pipeline_with_options` with `PaginateOptions::from_config(config)`, `&AtomicBool::new(false)`, and `|_| {}`.
12. **[`src/puzzle-gen/src/svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs) (Modify):**
    *   **Rationale:** Emit matching `Rotation90` transforms across Stage 3 (`<g transform="...">`) and Stage 4 (baked transform-free paths) via one polygon formatting helper.
    *   **Proposed Changes:**
        *   Extend `format_closed_polygon(pts: &[Point], rot: Rotation90, offset_mm: Vec2, unit: LengthUnit) -> String` to apply `rot.rotate_point(p) + offset_mm` directly (passing `Rotation90::Deg0` in `emit_stage2` and `emit_stage3`, and `slot.rotation` in `emit_stage4_page`).
        *   In `emit_stage3`, emit `transform="translate(tx, ty)"` for `Deg0` or `transform="translate(tx, ty) rotate(90|180|270)"` for rotated pieces.
        *   Preserve `emits_valid_stage1_to_stage4_xml_structure` testing pipeline output (`TopLeft`-first `corner_strokes_mm` preserves `p2_mark.starts_with("M 3.100000")`), and add unit test `rotates_stage3_and_stage4_consistently_for_deg90` constructing `PageSlot { rotation: Rotation90::Deg90, ... }`, `puzzle_bounds_mm`, and `content_bbox_mm` to verify exact Stage 3 `rotate(90)` syntax and Stage 4 coordinate parity `<= 2.2e-4 mm`.
13. **[`src/puzzle-gen/src/main.rs`](../../src/puzzle-gen/src/main.rs) (Modify — Phase 2 CLI Seed & Metrics):**
    *   **Rationale:** Expose `--seed` override and update `stdout` layout metrics before adding `--interactive` TUI wiring in Phase 3.
    *   **Proposed Changes:**
        *   Add `#[arg(long)] seed: Option<u64>` to `Cli`, applying `if let Some(seed) = cli.seed { config.seed = seed; }` after loading YAML.
        *   Compute `(slot_w_mm, slot_h_mm) = puzzle_gen::paginate::compute_slot_bounds(&artifacts.kerfed);` for `slot_mm=` reporting.
        *   Update `stdout` reporting: retain `grid=`, `matched_tabs=`, `missing_tabs=`, `slot_mm=`, and `pages=`; replace `page_grid=` with `puzzle_area_mm2={:.4}`, `total_bbox_mm2={:.4}`, and `bbox_area_pct={:.2}`.
14. **[`src/puzzle-gen/tests/integration_pipeline.rs`](../../src/puzzle-gen/tests/integration_pipeline.rs) (Modify — Phase 2 Existing Test Migration):**
    *   **Rationale:** Update coordinate-parity helpers for `Rotation90` and migrate existing `6in x 6in` / `page_grid=` test expectations.
    *   **Proposed Changes:**
        *   Replace `parse_translate_mm` with `parse_transform_mm(transform: Option<&str>, unit: LengthUnit) -> (f64, f64, f64)` returning `(tx_mm, ty_mm, rot_deg)`, and update `parse_path_points_mm(d: &str, tx_mm: f64, ty_mm: f64, rot_deg: f64, unit: LengthUnit)` to apply SVG clockwise `rotate(rot_deg)` before adding `(tx_mm, ty_mm)`.
        *   In `synthetic_5x5_stages_1_to_4_and_stage3_vs_stage4_coordinate_parity`, change inline YAML `page_size` from `"6in x 6in"` to `"3.5in x 3.5in"` so `pages.len() >= 2` exercises multi-page Stage 3 offsets, update the coordinate parity threshold from `1.5e-4` to `2.2e-4`, and verify rotated parity on any non-zero `Rotation90` transforms.
        *   In `cli_end_to_end_dry_run_collisions_overwrite_pruning_and_debug_emit`: replace `stdout_dry.contains("page_grid=")` with assertions checking `bbox_area_pct=`, `puzzle_area_mm2=`, and `total_bbox_mm2=`, and set `small.yaml` `page_size` to `"2in x 2in"` so the small-page run (`run1`) deterministically creates `>= 5` pages before `run2` verifies stale-page pruning.

*   **Phase 2 Checkpoint:** Run `cargo check --workspace --all-targets` and `cargo test -p puzzle-gen`.

### Phase 3: Interactive `ratatui`/`crossterm` TUI, CLI Wiring & Nesting Integration Tests

15. **`src/puzzle-gen/src/tui.rs` (Create):**
    *   **Rationale:** Isolate `ratatui` + `crossterm` terminal lifecycle, RAII alternate-screen/raw-mode cleanup, input event polling, and convergence chart rendering inside the CLI binary crate.
    *   **Proposed Changes:**
        *   Define `TerminalGuard` RAII struct entering alternate screen + raw mode on `io::stderr()` during construction and unconditionally calling `disable_raw_mode()` + `LeaveAlternateScreen` in `Drop`.
        *   Implement `pub(crate) fn run_interactive_pipeline(svg_xml: &str, config: &Config, strict: bool, notices: &mut Vec<PipelineNotice>) -> Result<PipelineArtifacts, PipelineError>`:
            *   Allocate `Arc<AtomicBool>` (`cancel`) and `mpsc::channel::<NestProgress>()`; execute `run_pipeline_with_options` with `PaginateOptions { seed: config.seed, max_iters: None }` on a `std::thread::scope` worker thread, forwarding worker notices back onto `notices` and dropping the sender when finished.
            *   Loop on the main thread polling `crossterm::event::poll(Duration::from_millis(50))`, filtering `Event::Key(key)` with `key.kind == KeyEventKind::Press` and setting `cancel.store(true, Ordering::Relaxed)` on `q`, `Esc`, `Enter`, or `KeyCode::Char('c')` with `KeyModifiers::CONTROL`.
            *   Drain progress snapshots (`Ok(progress)` / `Err(TryRecvError::Empty)`) into `(iter as f64, bbox_area_pct)` history; exit the render loop on `Err(TryRecvError::Disconnected)` without waiting for a keypress (handling both early Stage 0–3 worker errors and worker completion before joining the worker thread and dropping `TerminalGuard`).
            *   Render a `ratatui` layout with: (1) top summary block (`grid={rows}x{cols}`, `pages`, `bbox_area_pct`, `iter`, elapsed time), (2) `ratatui::widgets::Chart` (`Dataset` using `Marker::Braille` with axis bounds clamped so `max_x >= min_x + 1.0` and `max_y >= min_y + 1.0` for empty or single-point histories) plotting `bbox_area_pct` over iterations, (3) up to `8` per-page `Gauge` bars with `ratio(r.clamp(0.0, 1.0))` plus a single `"... +K more pages (avg fill X%)"` overflow summary line when `pages > 8`, and (4) bottom keybind legend (`q / Enter / Esc / Ctrl+C: stop early and emit best layout`).
16. **[`src/puzzle-gen/src/main.rs`](../../src/puzzle-gen/src/main.rs) (Modify — Phase 3 Interactive CLI Flag):**
    *   **Rationale:** Dispatch `--interactive` to `tui.rs` on TTY sessions with `INTERACTIVE_NON_TTY` fallback while preserving notice emission, debug artifact output, `stdout` summary metrics, and `--dry-run`.
    *   **Proposed Changes:**
        *   Declare `mod tui;` and add `#[arg(long)] interactive: bool` to `Cli`.
        *   When `cli.interactive` is set, check `io::stderr().is_terminal() && io::stdin().is_terminal()`; if false, push `PipelineNotice { severity: NoticeSeverity::Warning, code: "INTERACTIVE_NON_TTY", ... }` and call `run_pipeline`; if true, call `tui::run_interactive_pipeline`. Both `--interactive` and default runs share the subsequent post-pipeline notice printing, `--emit-debug-on-error` (skipped under `--dry-run`), `stdout` metrics, and Stage 1–4 atomic writes (skipped under `--dry-run`).
17. **[`src/puzzle-gen/tests/integration_pipeline.rs`](../../src/puzzle-gen/tests/integration_pipeline.rs) (Modify — Phase 3 Nesting, TUI Fallback & Benchmark Tests):**
    *   **Rationale:** Cover default `8.5in x 11in` single-page compaction, determinism, `90°` narrow-strip fitting with rotated parity, piped `--interactive` fallback, and release-mode `1,008`-piece throughput.
    *   **Proposed Changes:**
        *   Extract `build_complete_grid_svg(rows: usize, cols: usize, unit: LengthUnit) -> String` computing separate `total_w = unit.from_mm((cols as f64) * 20.0)` and `total_h = unit.from_mm((rows as f64) * 20.0)` (handling non-square `28x36` grids) and delegate `build_complete_5x5_svg(unit)` to `(5, 5, unit)`.
        *   Add `default_8_5x11_packs_rainy_day_on_single_page_with_compacted_mark`: calls `run_pipeline(..., false, ...)` (`strict = false`, because `path19` is omitted in the example SVG) and verifies the `4 in x 4 in` 25-piece example packs onto `1` page under `Config::default()`, with `bbox_area_pct < 200.0` and `pages[0].bottom_right_anchor_mm.y < cfg.page_height_mm - cfg.margin_mm`.
        *   Add `seeded_nesting_is_deterministic_and_cli_overrides_yaml_seed`: runs `run_pipeline` twice with `seed: 42` verifying identical `PaginatedLayout`, Stage 3, and Stage 4 strings, and checks CLI `--seed` overrides YAML `seed`.
        *   Add `non_tty_interactive_flag_warns_and_succeeds`: runs the compiled `puzzle-gen` binary with `.stdin(Stdio::null())` and `--interactive --dry-run --no-strict` under piped stdio asserting exit code `0`, `INTERACTIVE_NON_TTY` warning on `stderr`, and `bbox_area_pct=` on `stdout`.
        *   Add `rotation_allows_90_deg_single_piece_fit_on_narrow_strip`: builds a `1x1` `45 mm x 20 mm` piece on `"1.5in x 4in"` (`38.1 mm x 101.6 mm`) sheet with `print_bleed == kerf / 2` (`0.5 kerf`), verifies placement succeeds via `Rotation90::Deg90` or `Rotation90::Deg270`, and asserts Stage 3 vs. Stage 4 rotated vertex parity `<= 2.2e-4 mm`.
        *   Add `#[ignore]` integration benchmark `large_1008_piece_release_benchmark`: runs `build_complete_grid_svg(28, 36, LengthUnit::Mm)` through `run_pipeline` with default `64` iterations and asserts elapsed time `< 60 s`.

*   **Phase 3 Checkpoint:** Run `cargo check --workspace --all-targets`, `cargo clippy -p puzzle-gen --all-targets -- -D warnings`, and `cargo test -p puzzle-gen`.

### Verification Plan

*   **Compilation & Linting:**
    *   `cargo check --workspace --all-targets`
    *   `cargo clippy -p puzzle-gen --all-targets -- -D warnings`
*   **Unit & Integration Tests:**
    *   `cargo test -p puzzle-gen` (executes all unit tests across `config.rs`, `ingest.rs`, `stage1.rs`, `stage2.rs`, `nest.rs`, `paginate.rs`, `svg_emit.rs` and integration tests in `tests/integration_pipeline.rs`, including rotated Stage 3/4 parity `<= 2.2e-4 mm` and `--interactive` non-TTY fallback).
    *   `cargo test -p puzzle-gen --release -- --ignored large_1008_piece_release_benchmark --nocapture` (verifies `28x36` `1,008`-piece release build runtime `< 60 s`).
*   **Example Dry-Run & Inkscape PDF Verification:**
    *   `cargo run -p puzzle-gen -- --dry-run --no-strict src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` (verifies `pages=1` and `bbox_area_pct < 200.0` under default `8.5in x 11in` config).
    *   Emit Stage 3 output to a temporary directory for a narrow-strip or small-sheet config confirmed to emit `<g transform="... rotate(...)">` groups (for example, `rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` with `--no-strict` and a `3.5in x 3.5in` config, checking `rotate(` appears in Stage 3), run `inkscape --export-type=pdf <prefix>-stage3.svg`, and visually inspect the exported PDF to confirm rotated `#puzzle-pattern` tiles align inside `bleed_polygon` shapes without Cairo transform drift.
*   **Manual TTY Verification:**
    *   Run `cargo run -p puzzle-gen -- --interactive --dry-run --no-strict src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` in an interactive terminal once for each stop key (`q`, `Esc`, `Enter`, and `Ctrl+C`), confirming `ratatui` chart rendering, early stop response, and restored shell cursor/echo state after exit.

---

*Drafted with EGM*