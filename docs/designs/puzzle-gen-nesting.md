# `puzzle-gen`: Stage 3 & 4 Irregular Polygon Nesting and Scrap Compaction

*   **Author:** Tim Sweña (Swast)
*   **Status:** Draft
*   **Date:** 2026-11-13

--------------------------------------------------------------------------------

## Overview

`puzzle-gen` is a local Rust CLI and geometric pipeline in [`src/puzzle-gen`](../../src/puzzle-gen) (originally specified in [`puzzle-gen.md`](./puzzle-gen.md)) that converts an unjoined Stage 0 Inkscape SVG (`Image`, `Pieces` `<rect>` grid, and `Tabs` open `<path>` curves) into physical print-and-cut outputs using an output filename `<prefix>`: Stage 1 (`<prefix>-stage1.svg`, closed spliced contours), Stage 2 (`<prefix>-stage2.svg`, flattened/offset inspection grid), Stage 3 (`<prefix>-stage3.svg`, Inkscape `<inkscape:page>` sticker print sheets with `#puzzle-pattern` fills), and Stage 4 (`<prefix>-stage4-page<N>.svg`, standalone per-page laser-cut paths).

Today, Stages 3 and 4 arrange pieces on a uniform rectangular grid where every piece occupies a worst-case global bounding box and two full-width horizontal bands are reserved across the top and bottom of every sheet. On standard `8.5 in x 11 in` adhesive sticker stock and `12 in x 12 in` chipboard, the `6 in x 6 in` default page size and uniform grid pagination leave over 60% of each sticker sheet unused under default settings.

This design updates the default `page_size` to `8.5in x 11in`, keeps Stage 2 (`<prefix>-stage2.svg`) as an upright `(row, col)` inspection grid, and replaces Stage 3 and Stage 4 uniform grid pagination with 2D irregular polygon bin packing across right-angle rotations (`0°`, `90°`, `180°`, `270°`) and localized corner registration keep-out boxes. The layout engine minimizes the total number of sheets and compacts both the placed pieces and the bottom-right registration mark on each page toward the top-left origin so any unused portion of a sheet stays unprinted and unscored for straight-cut scrap reuse. It runs a fixed deterministic iteration budget by default for reproducible tests and adds an `--interactive` CLI flag that displays a live terminal convergence chart of total page bounding-box area relative to the original puzzle area until completion or early stop (`q`, `Esc`, `Enter`, or `Ctrl+C`).

## The Problem

Fabricating a custom jigsaw puzzle—ranging from a `4 in x 4 in` (`25`-piece) test puzzle up to a `50 cm x 70 cm` (`28x36`, `1,008`-piece) puzzle—on `8.5 in x 11 in` adhesive sticker paper and `12 in x 12 in` chipboard currently suffers from four compounding sources of material waste in Stages 3 and 4:

*   **Default sheet size mismatch with sticker stock:** The current default `page_size` is `6 in x 6 in` (`36 sq in`). Because `6 + 6 = 12 in` exceeds both dimensions of an `8.5 in x 11 in` (`93.5 sq in`) sticker sheet, only one `6 in x 6 in` page can be cut from each sticker sheet, discarding 61.5% of the adhesive label stock before `puzzle-gen` places a single piece.
*   **Full-width horizontal registration bands:** `paginate_pieces` in [`src/puzzle-gen/src/paginate.rs`](../../src/puzzle-gen/src/paginate.rs) subtracts two full-width `6.0 mm` horizontal bands (`12.0 mm` total height) across the entire `203.2 mm` usable width of every page (`2,438.4 mm²` on an `8.5 in x 11 in` sheet), even though the top-left and bottom-right vector page numbers and L-ticks only span `3.40 mm x 5.30 mm` for 1-digit pages and `6.28 mm x 5.30 mm` for 2-digit pages (`4.10 mm x 6.0 mm` and `6.98 mm x 6.0 mm` including a `0.7 mm` safety margin).
*   **Global worst-case rectangular slot inflation:** `compute_slot_bounds` assigns every piece on the sheet a uniform rectangular slot equal to `2 * max_half_w x 2 * max_half_h` around the piece cut bounding-box center (`cut_aabb_mm.center()`) across the entire puzzle. On the included `4 in x 4 in` (`16 sq in`, 25-piece) example where each grid cell is `20.32 mm x 20.32 mm` (`412.90 mm²`), every piece is allocated a `37.15 mm x 35.54 mm` (`1,320.31 mm²`) slot—`~3.2x` (`3.1976x`) the nominal cell area—even for straight-edged border pieces or pieces with inward sockets. Furthermore, empty corner shoulders beside protruding tabs inside an axis-aligned bounding box cannot accept neighboring pieces' tabs under rectangle-only slots. As a result, a `16 sq in` puzzle spills across three `6 in x 6 in` sheets (`108 sq in` total, ~85% waste).
*   **Uncompacted tail pages and fixed bottom-right registration marks:** On partially filled pages, leaving the bottom-right registration mark fixed at `(page_width_mm - margin_mm, page_height_mm - margin_mm)` prints and scores into the far corner of the sheet, preventing the operator from severing the unused portion of the sticker sheet and chipboard with straight guillotine or laser cuts for reuse on smaller runs.

## Goals

*   **`8.5 in x 11 in` Default Sheet Size:** Update the default `page_size` in `Config::default()` and [`src/puzzle-gen/examples/default-config.yaml`](../../src/puzzle-gen/examples/default-config.yaml) to `"8.5in x 11in"` (`215.9 mm x 279.4 mm`) so default runs cover the full printable sticker sheet size with zero pre-layout trim loss and leave contiguous straight-cut offcut strips (`3.5 in x 11 in` or `3.5 in x 12 in` plus a `1 in` end strip) on `12 in x 12 in` chipboard.
*   **Unscrambled Stage 2 Inspection Grid:** Preserve Stage 2 (`<prefix>-stage2.svg`) as an upright, unscrambled `(row, col)` inspection grid with row/column index text labels (`GridLabels`).
*   **Localized Corner Keep-Outs & Compacted Bottom-Right Registration Mark:** Replace the full-width top and bottom `6.0 mm` bands in Stages 3 and 4 with localized top-left and bottom-right corner keep-out boxes, and compact each page's bottom-right registration mark inward to the tightest collision-free bottom-right corner `(br_x, br_y)` enclosing the placed pieces on that page.
*   **Right-Angle Irregular Polygon Nesting (`0°`, `90°`, `180°`, `270°`):** Pack pieces on Stage 3 and Stage 4 sheets using their outer piece footprints—the union of `cut_polygon` (`+kerf / 2`) and `bleed_polygon` (`+print_bleed`), so valid configs with `print_bleed < kerf / 2` remain covered—with `>= piece_spacing_mm` clearance across the four pixel-grid-aligned right-angle rotations (`0°`, `90°`, `180°`, `270°`) rasterized onto a conservative `1 kerf` (`1x` configured `kerf`, `0.18 mm` default) bitmask grid.
*   **Unified Page Bounding-Box Area Minimization & Reporting:** Minimize total page count first and minimize the sum of per-page content bounding-box areas ($A_{\text{bbox}} = \sum_{p=1}^{P} \text{area}(B_p)$, where $B_p$ encloses all placed pieces and compacted registration marks on page $p$). Report `bbox_area_pct` ($\frac{A_{\text{bbox}}}{A_{\text{puzzle}}} \times 100\%$, comparing total page bounding-box area against original Stage 0 puzzle grid area $A_{\text{puzzle}}$) in CLI output.
*   **Stage 3 / Stage 4 Coordinate & Pattern Parity:** Rotate and translate Stage 3 pieces and their `2x2` mirrored raster `#puzzle-pattern` fills via `<g transform="translate(tx, ty) rotate(deg)">` while baking identical right-angle rotations and translations into ungrouped, transform-free Stage 4 `<path d="...">` coordinates within `<= 1.5e-4 mm` physical parity (accounting for SVG decimal serialization precision and unit conversion).
*   **Deterministic Seeded Default & `--interactive` Convergence Mode:** Execute a fixed deterministic iteration budget with `--seed <U64>` (default `0`) in standard non-interactive runs, and support an `--interactive` CLI flag that renders a live `stderr` convergence chart of `bbox_area_pct` over iterations with early key-press stop (`q`, `Esc`, `Enter`, or `Ctrl+C`).
*   **Exact Polygon Invariant Verification:** Validate in `verify_page_invariants` that every placed piece stays within page margins (`<= CONTAINMENT_TOL_MM`), avoids both corner registration keep-out boxes (`>= piece_spacing_mm - CONTAINMENT_TOL_MM`), and maintains exact polygon-to-polygon Euclidean clearance `>= piece_spacing_mm - CONTAINMENT_TOL_MM`.

## Non-Goals

*   **Arbitrary non-`90°` piece rotations:** Continuous or non-cardinal rotation angles are excluded so printed raster artwork stays aligned with the printer and image pixel grid.
*   **Wall-clock optimization timeout flags:** Non-interactive runs use fixed iteration counts for deterministic testability rather than machine-dependent wall-clock timers.
*   **Heterogeneous multi-sheet-size packing within a single CLI run:** Every page in a single invocation shares `Config.page_size`; packing onto accumulated scrap strips (such as `3.5in x 11in` chipboard offcuts) is performed by passing a custom `page_size` in `--config`.
*   **Changes to Stage 0, Stage 1, or Stage 2 piece geometry:** Tab splicing (`splice_tabs`), curve flattening, self-union healing, and kerf/bleed polygon offsetting remain unchanged.
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
    *   The operator prints Stage 3 onto an `8.5 in x 11 in` sticker sheet, adheres it to `12 in x 12 in` chipboard, aligns Stage 4 using the top-left and compacted bottom-right corner marks, cuts the pieces, and severs the unprinted, unscored lower portion of the `8.5 in x 11 in` sheet as well as the `3.5 in x 11 in` (or `3.5 in x 12 in`) side strip and `1 in` end strip of chipboard with straight cuts for future reuse.

### CUJ 2: Large Puzzle Optimization with `--interactive` Convergence and Early Stop

*   **Given:** A user is fabricating a `50 cm x 70 cm` (`28x36`, `1,008`-piece) puzzle configured in `config.yaml` and wants to monitor layout compaction and stop once the bounding-box area ratio levels off.
*   **When:** The user runs `puzzle-gen large-puzzle.svg --config config.yaml --interactive`.
*   **Then:**
    *   `puzzle-gen` computes the initial constructive multi-pass packing and continuously runs seeded local search iterations, rendering a live terminal chart on `stderr` showing current iteration count, total `pages`, and a convergence plot of `bbox_area_pct` over iterations.
    *   When the user presses `q`, `Esc`, `Enter`, or `Ctrl+C`, the optimizer finishes its current iteration, verifies exact polygon clearance and corner keep-out invariants on the best layout found so far, writes the final Stage 1–4 SVG files, and prints the final summary metrics to `stdout`.

### CUJ 3: Reproducible Seeded Runs and Offcut Strip Packing

*   **Given:** A user has accumulated `3.5 in x 11 in` chipboard offcut strips from previous `12 in x 12 in` boards and creates `offcut-config.yaml` with `page_size: "3.5in x 11in"`.
*   **When:** The user runs `puzzle-gen small-puzzle.svg --config offcut-config.yaml --seed 42 --dry-run` twice on different machines (`--dry-run` validates geometry and prints metrics without creating or writing files).
*   **Then:**
    *   Both runs execute the exact same number of deterministic search iterations under seed `42` and produce identical `pages` and `bbox_area_pct` values on `stdout`.
    *   If any piece cannot fit inside the `3.5 in x 11 in` margin-bounded sheet (with corner keep-outs) under any of the four right-angle rotations (`0°`, `90°`, `180°`, `270°`), `puzzle-gen` exits non-zero before writing files and reports the offending piece `id` and minimum required `page_size`.

--------------------------------------------------------------------------------

## Technical Plan (Architecture)

### Architecture Overview

The pipeline maintains the separation between the I/O-free geometric library ([`src/puzzle-gen/src/lib.rs`](../../src/puzzle-gen/src/lib.rs)) and the CLI/TUI binary driver ([`src/puzzle-gen/src/main.rs`](../../src/puzzle-gen/src/main.rs)). Stages 0, 1, and 2 execute unchanged in canonical millimeters (`mm`), except that `SplicedPuzzle` and `KerfedGrid` forward `puzzle_bounds_mm` (the Stage 0 grid bounding box) so Stage 3/4 pagination can compute `bbox_area_pct` against the true original puzzle area $A_{\text{puzzle}}$.

Stage 2 SVG emission ([`emit_stage2`](../../src/puzzle-gen/src/svg_emit.rs)) continues calling `paginate::compute_slot_bounds(kerfed)` directly to render the upright, unscrambled `(row, col)` inspection grid with `GridLabels`, without reading `PaginatedLayout`. Stage 3 and Stage 4 pagination ([`paginate.rs`](../../src/puzzle-gen/src/paginate.rs)) is replaced by a deterministic irregular polygon nesting engine ([`nest.rs`](../../src/puzzle-gen/src/nest.rs)) that packs pieces across right-angle rotations (`0°`, `90°`, `180°`, `270°`) onto `8.5 in x 11 in` (default) sheets using conservative grid bitmasks, compacts each page's `BottomRight` registration mark inward toward the placed pieces as a decoupled post-pass, and streams `NestProgress` snapshots to an optional terminal UI in [`tui.rs`](../../src/puzzle-gen/src/tui.rs) when `--interactive` is enabled.

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
  upright (row, col) grid)                       tail-window search, post-pass BR compaction,
                                                 exact polygon clearance verification)
                                                                   │
                                       ┌───────────────────────────┴───────────────────┐
                                       ▼                                               ▼
                        [ <prefix>-stage3.svg ]                        [ <prefix>-stage4-page<N>.svg ]
                        (<g translate rotate> + #puzzle-pattern)       (Transform-free Cut + PageNumbers)
```

New dependencies added to [`src/puzzle-gen/Cargo.toml`](../../src/puzzle-gen/Cargo.toml):

*   **`crossterm` & `ratatui`:** Used exclusively inside the CLI binary layer ([`src/puzzle-gen/src/main.rs`](../../src/puzzle-gen/src/main.rs) and [`src/puzzle-gen/src/tui.rs`](../../src/puzzle-gen/src/tui.rs)) for alternate-screen `stderr` convergence charts and raw-mode key polling (`q`, `Esc`, `Enter`, or `Ctrl+C`) under `--interactive`. The library crate remains terminal-I/O-free.

### CLI Interface & Configuration Schema Updates

*   **YAML Configuration ([`src/puzzle-gen/src/config.rs`](../../src/puzzle-gen/src/config.rs), [`src/puzzle-gen/examples/default-config.yaml`](../../src/puzzle-gen/examples/default-config.yaml)):**
    *   Update `Config::default()` page dimensions to `8.5in x 11in` (`215.9 mm x 279.4 mm`) and update [`default-config.yaml`](../../src/puzzle-gen/examples/default-config.yaml) to match.
    *   Add optional `seed: <u64>` (default `0`), preserving `serde-saphyr` unknown-key and duplicate-key rejection (negatives, floats, and strings fail deserialization).
*   **CLI Flags ([`src/puzzle-gen/src/main.rs`](../../src/puzzle-gen/src/main.rs)):**
    *   `--seed <U64>`: Optional deterministic PRNG seed overriding `config.seed`.
    *   `--interactive`: Optional flag launching the `stderr` convergence TUI while running continuous local search until completion or key press (`q`, `Esc`, `Enter`, or `Ctrl+C`). When `stdin` or `stderr` is not a TTY, emits an `INTERACTIVE_NON_TTY` warning notice and executes the non-interactive fixed-iteration pass. When combined with `--dry-run`, the interactive chart runs and tears down the alternate screen cleanly before printing `stdout` metrics and skipping file output creation.
*   **CLI `stdout` Summary Metrics & Breaking v1 Contract Notice:**
    *   Retain `grid=RxC`, `matched_tabs=M`, `missing_tabs=K`, `slot_mm=WxH` (Stage 2 uniform slot size), and `pages=P`.
    *   Replace `page_grid=CxR` with irregular layout compaction metrics:
        *   `puzzle_area_mm2=<f64>` (Stage 0 `puzzle_bounds_mm` area)
        *   `total_bbox_mm2=<f64>` (sum of per-page `content_bbox_mm` areas across pages `1..=P`)
        *   `bbox_area_pct=<f64>` (`(total_bbox_mm2 / puzzle_area_mm2) * 100.0`)
    *   *Breaking v1 change:* Replacing `PageSlot::{row, col}`, `PaginatedLayout::{page_cols, page_rows}`, and CLI `page_grid=` is an intentional breaking change to the internal library structs and CLI metric output introduced alongside irregular nesting.

### Domain Data Model Updates ([`src/puzzle-gen/src/model.rs`](../../src/puzzle-gen/src/model.rs))

| Type | Changes | Invariants |
| --- | --- | --- |
| `Config` | Default `page_size` changed to `8.5in x 11in`; add `pub seed: u64` (`0` default) | CLI `--seed` overrides YAML `seed` before running the pipeline; runs with equal `Config` are deterministic. |
| `SplicedPuzzle` & `KerfedGrid` | Add `pub puzzle_bounds_mm: kurbo::Rect` | Computed from Stage 0 `x_lines_mm` and `y_lines_mm`; positive area (`> 0.0`). |
| `Rotation90` | New enum `{ Deg0, Deg90, Deg180, Deg270 }` | Exact integer clockwise right-angle rotation around `(0, 0)` in `+y`-down coordinates. |
| `PageSlot` | Replace `row`/`col` with `pub rotation: Rotation90` | `transform_point(p) = rotation.rotate_point(p) + offset_mm`. |
| `PagePlan` | Add `pub content_bbox_mm: kurbo::Rect` and `pub bottom_right_anchor_mm: kurbo::Point` | `content_bbox_mm` spans `[margin_mm, max(pieces_max_x, br_x)] x [margin_mm, max(pieces_max_y, br_y)]`; `corner_strokes_mm` orders `TopLeft` strokes before `BottomRight`. |
| `PaginatedLayout` | Remove `page_cols` and `page_rows`; retain `slot_w_mm` and `slot_h_mm` for CLI `slot_mm=` reporting; add `puzzle_area_mm2`, `total_bbox_area_mm2`, `bbox_area_pct` | `puzzle_area_mm2 > 0.0`; `bbox_area_pct == (total_bbox_area_mm2 / puzzle_area_mm2) * 100.0`. |
| `PaginateOptions` & `NestProgress` | Public solver budget and progress structs (`seed`, `max_iters`, `rows`, `cols`, `iter`, `total_iters`, `pages`, `total_bbox_area_mm2`, `puzzle_area_mm2`, `bbox_area_pct`, `page_fill_ratios`) | Decouples optimization budget and clamped `[0.0, 1.0]` fill telemetry from terminal I/O crates. |

### Right-Angle Irregular Polygon Nesting Architecture ([`src/puzzle-gen/src/nest.rs`](../../src/puzzle-gen/src/nest.rs), [`paginate.rs`](../../src/puzzle-gen/src/paginate.rs))

1.  **Outer Envelope & Conservative Bitmask Precomputation:**
    *   Each piece's outer collision footprint is built by uniting its `cut_polygon` and `bleed_polygon` (`i_overlay` `FillRule::Positive` union, dropping any internal socket-bridging holes while retaining the primary outer contour) so nesting respects both cut paths and bleed borders even when `print_bleed_mm < kerf_mm / 2.0`.
    *   To guard against inscribed chord sagitta loss when buffering by `piece_spacing_mm / 2.0`, dilation adds a fixed conservative margin (`0.005 mm`) whenever `piece_spacing_mm > 0.0`. Each piece's four right-angle rotated footprints are rasterized onto conservative grid bitmasks (marking vertex, supercover edge, and interior scanline cells) sized from `kerf_mm` (`segment_length_mm` when `kerf_mm == 0.0`, floored at `0.10 mm` and capped dynamically so the usable sheet grid stays `<= 2048` cells per axis). Temporary dilated polygons are released immediately after each piece's four variants are rasterized.
    *   Bitmask disjointness acts as a fast collision filter using padded sheet grids; final spacing guarantees rest on exact continuous polygon verification in `verify_page_invariants`.

2.  **Localized Corner Keep-Outs & Multi-Digit Feasibility:**
    *   Full-width horizontal corner bands (`CORNER_BAND_MM`) are replaced by compact corner keep-out boxes of height `CORNER_KEEP_OUT_H_MM` (`6.0 mm`) and digit-dependent width `(1.0 + num_w).max(2.5) + 0.7`, accommodating `layout_corner_page_number` strokes (`3.40 mm x 5.30 mm` for 1-digit pages, `6.28 mm x 5.30 mm` for 2-digit pages, plus `0.7 mm` safety margin).
    *   Each candidate sheet reserves the `TopLeft` keep-out box and a fallback `BottomRight` corner keep-out box pre-buffered by half of `piece_spacing_mm` so placed pieces (already half-buffered in their bitmasks) achieve full `piece_spacing_mm` separation without double-buffering (`1.5x`).
    *   Before search starts, usability bounds (`usable_w > 0.0 && usable_h > 0.0`) and keep-out disjointness are checked, and every piece is tested across all four rotations against worst-case multi-digit page keep-outs; unplaceable pieces fail immediately naming piece `id` and `minimum required page_size`.

3.  **Multi-Start Constructive Placement, Seeded Refinement & Post-Pass Compaction:**
    *   **Constructive Decoder:** Evaluates four deterministic piece orderings (largest dilated bounding box, straight-edge corners/borders first, longest bounding side, and lowest convexity ratio) using sliding-window First-Fit across only the trailing two open sheets (`MAX_OPEN_PAGES = 2`) with Top-gravity placement, yielding $O(N)$ decode complexity per pass instead of $O(N \cdot P)$ full-history scans.
    *   **Tail-Window & Global Refinement (`SplitMix64`):** Applies deterministic seeded mutations (`swap`, `insert`, and `rotation preference shift`) targeting either the trailing tail-page suffix—using an exact frozen-prefix boundary snapshot before the tail window so First-Fit backfilling never invalidates earlier pages—or full global re-decodes, accepting strictly lower `(pages.len(), total_bbox_area_mm2)` solutions.
    *   **Decoupled Final-Page & `BottomRight` Mark Compaction:** Runs strictly once on the winning candidate (never inside the inner search loop): compares Top-gravity vs. Left-gravity re-packing of the final page's pieces, then probes collision-free `BottomRight` mark anchors outward from `(pieces_max_x, pieces_max_y)` within sheet bounds while avoiding `TopLeft` and placed pieces (`piece_spacing_mm`), preferring larger `br_x + br_y` on equal area so interior marks never drift into empty space near `TopLeft`.

4.  **Exact Continuous Verification (`verify_page_invariants`):**
    *   Independently verifies that every vertex of every placed piece (`cut_polygon` and `bleed_polygon`) lies inside sheet margins (`<= CONTAINMENT_TOL_MM`), `BottomRight` anchors stay within margins and remain disjoint from `TopLeft`, piece polygons clear both corner keep-out boxes (`>= piece_spacing_mm - CONTAINMENT_TOL_MM`, checking vertex/edge/rect-corner containment), and all nearby same-page piece pairs satisfy `>= piece_spacing_mm - CONTAINMENT_TOL_MM` Euclidean separation.

### Stage 3 & Stage 4 SVG Emission & Interactive TUI ([`src/puzzle-gen/src/svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs), [`tui.rs`](../../src/puzzle-gen/src/tui.rs), [`main.rs`](../../src/puzzle-gen/src/main.rs))

*   **Rotation Transform & Parity (`Rotation90`):**
    *   Because `Rotation90` rotates around `(0, 0)` with exact integer coefficients `{-1, 0, 1}`, rotation commutes with unit scaling (`mm` to `doc_unit` / `page_unit`).
    *   [`emit_stage3`](../../src/puzzle-gen/src/svg_emit.rs) wraps each unmoved piece (`bleed_polygon` relative to original artwork coordinates) in `<g transform="translate(tx_doc, ty_doc)">` or `<g transform="translate(tx_doc, ty_doc) rotate(90|180|270)">`, transforming both the bleed polygon and its `url(#puzzle-pattern)` fill in lockstep.
    *   [`emit_stage4_page`](../../src/puzzle-gen/src/svg_emit.rs) bakes rotated and translated coordinates directly into ungrouped, transform-free `<path d="...">` elements via a unified `format_closed_polygon` helper, matching Stage 3 physical coordinates within `<= 1.5e-4 mm` (limited only by `LengthUnit::decimal_places` formatting precision and unit round-tripping).
    *   Both Stage 3 and Stage 4 emit identical `corner_strokes_mm` (`TopLeft` followed by `BottomRight` compacted registration marks).
*   **`--interactive` TUI Loop & Lifecycle:**
    *   The CLI binary spawns `run_pipeline_with_options` on a scoped worker thread streaming `NestProgress` updates while holding an RAII terminal guard on `stderr` only when both `stdin` and `stderr` are TTYs.
    *   The UI renders puzzle grid dimensions, page count, current `bbox_area_pct`, iteration counter, convergence plot, clamped per-page fill gauges (showing up to `8` pages plus a compact overflow summary line), and stop-key help (`q`, `Esc`, `Enter`, or `Ctrl+C`).
    *   Upon user stop key or worker channel disconnection (including worker Stage 0–3 errors), the TUI exits raw mode and leaves the alternate screen before notices, errors, `stdout` summary metrics, and Stage 1–4 atomic file writes (skipped under `--dry-run`) execute.

--------------------------------------------------------------------------------

## Alternatives Considered

### Alternative A: Fixed `6 in x 6 in` Page Size vs. Full-Sheet `8.5 in x 11 in` Default with Post-Pass Offcut Compaction

*   **Pros of keeping `6 in x 6 in`:** Divides a `12 in x 12 in` chipboard sheet evenly into four `6 in x 6 in` squares.
*   **Cons:** Because `6 + 6 = 12 in` exceeds both sides of an `8.5 in x 11 in` (`93.5 sq in`) adhesive sticker sheet, only one `6 in x 6 in` (`36 sq in`) page fits per sticker sheet, discarding 61.5% of label stock before packing begins.
*   **Decision:** Default `page_size` to `8.5in x 11in` and reclaim unused partial-page areas via post-pass `BottomRight` mark compaction, achieving zero pre-layout trim loss on letter sticker sheets while yielding reusable `3.5 in x 11 in` and `1 in x 12 in` straight-cut strips on `12 in x 12 in` chipboard.

### Alternative B: Arbitrary Continuous-Angle Piece Rotations (`0°..360°`)

*   **Pros:** Allows tighter nesting along diagonal tab necks and angled border tabs.
*   **Cons:** Rotates raster artwork at non-cardinal angles relative to the printer and image pixel grid, introducing interpolation blur across piece seams and requiring non-integer trigonometric pattern transforms.
*   **Decision:** Restrict placement orientations to right angles (`0°`, `90°`, `180°`, `270°`), keeping raster samples grid-aligned with exact integer coordinate maps.

### Alternative C: Row-Major Order or Spatial-Band Locality Constraints on Sheets

*   **Pros:** Preserves spatial `(row, col)` or local-cluster assembly order on the laser bed so nearby puzzle pieces stay grouped together after cutting.
*   **Cons:** Prohibits pairing small border pieces or inward-socket interior pieces from across the puzzle into narrow sheet gaps and corner shoulders, increasing page counts on large puzzles.
*   **Decision:** Allow unconstrained global piece permutations and right-angle orientations on Stages 3 and 4 to minimize sheet consumption, while preserving [`emit_stage2`](../../src/puzzle-gen/src/svg_emit.rs) (`<prefix>-stage2.svg`) as an upright `(row, col)` visual lookup map for post-cut assembly.

### Alternative D: Fixed Corner Registration vs. In-Loop or Post-Pass Mark Relocation

*   **Pros of fixed bottom-right mark or omitting compaction:** Places the registration mark at identical coordinates on every sheet; or computing `(br_x, br_y)` inside every candidate decode evaluates mark area directly during search.
*   **Cons:** Fixed marks print and score into the unused corner of tail pages, spoiling blank offcuts; conversely, probing continuous mark anchors inside the inner local search loop adds polygon-to-rectangle distance checks to every candidate evaluation.
*   **Decision:** Pre-reserve the fallback corner mark during search and run one deterministic outward `BottomRight` mark probe per page strictly as a post-pass on the winning layout.

### Alternative E: Wall-Clock Optimization Timeout Flag (`--optimize-time <SECONDS>`)

*   **Pros:** Bounds non-interactive optimization runs to a predictable wall-clock duration on slower hardware.
*   **Cons:** Produces different iteration counts and different physical layouts across machines or background load, breaking reproducible test suites.
*   **Decision:** Use fixed deterministic iteration budgets (`DEFAULT_OPTIMIZE_ITERS = 64`) in standard runs while providing `--interactive` for operator-controlled early stopping.

### Alternative F: Internal Fixed Heuristic vs. Configurable Deterministic Seed (`seed` in YAML & CLI)

*   **Pros of hiding `seed`:** Keeps `config.yaml` limited strictly to physical laser and sheet dimensions.
*   **Cons:** Prevents operators from re-running or reproducing a specific non-default layout seed from versioned project configuration or CLI automation.
*   **Decision:** Expose optional `seed` (default `0`) in `config.yaml` and `--seed <U64>` on the CLI alongside `--interactive`.

### Alternative G: Dependency-Free `std` Line-Buffered Output vs. `crossterm` + `ratatui` TUI

*   **Pros of line-buffered `std`:** Avoids adding two external TUI crates to `Cargo.toml`.
*   **Cons:** Standard cooked `stdin` blocks until `Enter` (ignoring single-key `q` / `Esc`), and line-buffered `stderr` cannot update a convergence plot in place or intercept raw `Ctrl+C` before graceful post-verification file output.
*   **Decision:** Add `crossterm` and `ratatui` isolated inside the binary crate (`tui.rs` / `main.rs`) while keeping the library crate free of terminal dependencies.

### Alternative H: Tight AABB Rectangle Packing Without Polygon Nesting

*   **Pros:** Simple rectangle skyline or guillotine packing with zero polygon rasterization.
*   **Cons:** Empty corner shoulders beside protruding tabs inside every piece's axis-aligned bounding box are treated as solid obstacles, blocking tab-to-shoulder and tab-to-socket interleaving.
*   **Decision:** Rasterize true outer piece contours (`cut_polygon ∪ bleed_polygon`) so protruding tabs pack into neighboring empty shoulders and concave pockets.

### Alternative I: All-Pairs Continuous NFPs ($O((4N)^2)$) or Coarse-to-Fine Bitmask Pyramids

*   **Pros:** Continuous No-Fit Polygons (NFPs) compute exact sliding contact boundaries; multi-resolution bitmask pyramids speed up very fine-grid scans.
*   **Cons:** For `1,008` pieces (`4,032` oriented polygons of ~80–120 vertices), computing $\sim 8.1 \times 10^6$ non-convex pairwise NFPs scales quadratically ($O((4N)^2)$) in precomputation time and faces degenerate tangent cases on narrow undercut sockets. Conversely, maintaining multi-level pyramids adds memory and cache complexity when a single `1 kerf` mask with per-row free-run skipping and a 2-page sliding window (`MAX_OPEN_PAGES = 2`) keeps precomputation $O(4N)$ and each decode $O(N)$.
*   **Decision:** Precompute single-resolution conservative bitmasks with per-row free-run pruning and `MAX_OPEN_PAGES = 2` sliding-window First-Fit, backed by continuous `verify_page_invariants` checks.

### Alternative J: Stage 3 `<clipPath>` / Pre-Rotated `<pattern>` vs. `<g transform="translate(...) rotate(...)">`

*   **Pros of pre-rotating artwork tiles or `<clipPath>`:** Bakes piece rotations directly into Stage 3 polygon coordinates, avoiding nested `<g transform="rotate(...)">` elements around `url(#puzzle-pattern)` fills that some older Cairo PDF versions mis-render.
*   **Cons:** Generating rotated `<pattern>` definitions per orientation quadruples pattern definitions or requires per-piece `patternTransform` matrix math on `use` tiles, duplicating Stage 4 transform logic without demonstrated Inkscape breakage.
*   **Decision:** Use `<g transform="translate(tx, ty) rotate(deg)">` in [`emit_stage3`](../../src/puzzle-gen/src/svg_emit.rs) matching existing Stage 2/3 `<pattern>` sharing, verify empirically with `inkscape --export-type=pdf`, and retain pre-rotated pattern generation strictly as a documented fallback if Cairo export fails.

--------------------------------------------------------------------------------

## Non-Functional Requirements

*   **1,008-Piece Scaling & Memory Footprint:** Precomputing `4N` bitmasks (`4,032` oriented variants for a `28x36`, `1,008`-piece `50 cm x 70 cm` puzzle at default `1 kerf` = `0.18 mm` step) must keep retained variant bitmasks `< 25 MB` (dropping each piece's temporary `dilated` polygon immediately after rasterizing its four orientations, while bounding each sheet's usable bitmask grid to `<= 512 KiB` plus a small padding border; at the `0.10 mm` minimum step floor on maximum-size pieces, `4N` variant masks scale up to `~45 MB`). Constructive multi-start plus `64` deterministic local search iterations must complete within `< 60 s` in `--release` builds on a `28x36` puzzle (`< 250 ms` on a `5x5`, `25`-piece puzzle).
*   **Cross-Platform Determinism:** Non-interactive runs with the same input SVG, `Config`, and `seed` must produce identical page counts, piece placements, rotations, compacted registration mark coordinates, and `bbox_area_pct` values across Windows, macOS, and Linux.
*   **Stage 3 / Stage 4 Coordinate Parity (`<= 1.5e-4 mm`):** Every piece vertex in Stage 3 (`<g transform="translate(...) rotate(...)">` + `<path d>`) and Stage 4 (baked transform-free `<path d>`), as well as every `PageNumbers` registration stroke (`TopLeft` and `BottomRight`), must agree in physical page coordinates within `<= 1.5e-4 mm`, bounded by SVG decimal formatting precision in `LengthUnit::decimal_places` (`4` decimal places for `mm`/`px`, `6` for `in`/`cm`) and unit conversion round-tripping.
*   **Terminal State Restoration & Non-TTY Safety:** `--interactive` must restore raw mode and leave the alternate screen via an RAII `Drop` guard on normal completion, early user stop (`q`, `Esc`, `Enter`, or `Ctrl+C`), worker error return (including `mpsc` disconnection before keypress), or panic unwind before printing notices or errors, and must fall back to non-interactive execution with an `INTERACTIVE_NON_TTY` `Warning` notice when `stdin` or `stderr` is not a TTY.

## Risks & Limits

*   **Inkscape Cairo PDF Export of Rotated `userSpaceOnUse` Patterns:** Stage 3 nests `<path style="fill:url(#puzzle-pattern)">` inside `<g transform="translate(...) rotate(90|180|270)">`. Certain Inkscape/Cairo PDF export versions mishandle `patternUnits="userSpaceOnUse"` inside nested rotated groups. Empirical verification using `inkscape --export-type=pdf` is included in the Verification Plan; if Inkscape mis-aligns rotated artwork in exported PDFs, [`emit_stage3`](../../src/puzzle-gen/src/svg_emit.rs) will switch to pre-rotated `<pattern>` transforms (`Alternative J` fallback).
*   **Zero or Ultra-Small `kerf` Bitmask Resolution & `0.10 mm` Memory Growth:** Setting `kerf: "0mm"` or very small `kerf` on large sheets would cause division by zero or excessive bitmask memory if `step_mm` matched `kerf_mm`. Flooring `step_mm` at `0.10 mm` (using `segment_length_mm` when `kerf_mm == 0.0`) and capping usable grid dimensions at `<= 2048` cells/axis bounds each usable sheet bitmask to `<= 512 KiB` (plus `2 * pad_cells` border rows/cols), while eager per-piece disposal of temporary dilated polygons avoids holding `4N` polygons alongside bitmasks (keeping `1,008`-piece variant bitmasks `~15 MB` at `0.18 mm` and `~45 MB` at `0.10 mm`).
*   **Bottom-Right Registration Mark Contention on Full Pages:** Packing pieces into the bottom-right corner could block placement of a collision-free `BottomRight` mark inside sheet margins. Pre-reserving the fallback corner keep-out box during piece placement guarantees at least one valid mark location always remains for the post-pass probe.
*   **Interactive Key Cancellation & Per-File Output Atomicity:** Raw-mode event polling intercepts `q`, `Esc`, `Enter`, and `Ctrl+C` (`KeyModifiers::CONTROL`) via `crossterm` so optimization stops between iterations and runs full `verify_page_invariants` checks before writing files. External OS `SIGINT`/`SIGTERM` signals are not trapped independently and terminate immediately; written SVG outputs are replaced per-file atomically via `NamedTempFile::persist` (matching [`puzzle-gen.md`](./puzzle-gen.md)), not wrapped in a multi-file filesystem transaction across Stages 1–4.
*   **Local-Trust Security Boundary & Terminal Input Surface:** Like [`puzzle-gen.md`](./puzzle-gen.md), `puzzle-gen` operates under a local-trust execution model on operator-supplied Stage 0 SVGs and YAML configs (`serde-saphyr` resource limits, explicit `usable_w > 0.0 && usable_h > 0.0` dimension checks, and `2048`-cell axis caps prevent unbounded allocation). Under `--interactive`, `crossterm` and `ratatui` read single keystrokes locally from the attached TTY and render bounded telemetry strings (`rows x cols`, numeric metrics, clamped `[0.0, 1.0]` fill gauges) without executing untrusted SVG text in terminal escape sequences.

--------------------------------------------------------------------------------

## Additional Resources

*   [`docs/designs/puzzle-gen.md`](./puzzle-gen.md): Initial four-stage `puzzle-gen` architecture and Stage 0–2 geometric specifications.

## Future Improvements

*   **Stage 2 Diagnostic Problem-Stroke Highlighting:** Color and bold specific edges or pieces in `<prefix>-stage2.svg` where non-fatal warnings (`MISSING_INTERNAL_TAB` under `--no-strict`, self-union overshoot healing, or bleed socket bridging) or `--emit-debug-on-error` Stage 2 offsetting failures occurred (deferred to a separate design).

--------------------------------------------------------------------------------

## Detailed Implementation Plan

All Rust code changes sit within the `puzzle-gen` crate ([`src/puzzle-gen`](../../src/puzzle-gen)) plus Cargo's workspace lockfile ([`Cargo.lock`](../../Cargo.lock)). No other workspace member crate (`sticker-size`) is modified. Implementation proceeds across three sequential phases so `cargo check --workspace --all-targets` and `cargo test -p puzzle-gen` remain green after every phase.

### Phase 1: Dependencies, Configuration Schema & Stage 0–2 Bounds

1.  **[`src/puzzle-gen/Cargo.toml`](../../src/puzzle-gen/Cargo.toml) & [`Cargo.lock`](../../Cargo.lock) (Modify):**
    *   **Rationale:** Add `crossterm` and `ratatui` to support the `--interactive` terminal convergence chart and raw-mode key-polling loop in the CLI binary while keeping the library crate free of terminal dependencies.
    *   **Proposed Changes:** Add `crossterm = "0.29"` and `ratatui = "0.30"` under `[dependencies]` in `src/puzzle-gen/Cargo.toml` and update workspace `Cargo.lock`.
2.  **[`src/puzzle-gen/examples/default-config.yaml`](../../src/puzzle-gen/examples/default-config.yaml) (Modify):**
    *   **Rationale:** Align the documented default config file with the new `8.5in x 11in` default page size and explicit `seed: 0` parameter so `Config::from_yaml` equality tests pass.
    *   **Proposed Changes:** Change `page_size: "6in x 6in"` to `page_size: "8.5in x 11in"`, update the `piece_spacing` comment to describe minimum clearance between neighboring pieces across Stages 2–4, and add `seed: 0` documenting deterministic seeded layout search.
3.  **[`src/puzzle-gen/src/model.rs`](../../src/puzzle-gen/src/model.rs) (Modify — Additive Phase 1 Types):**
    *   **Rationale:** Introduce `8.5in x 11in` defaults, YAML/CLI layout `seed`, `puzzle_bounds_mm`, `Rotation90`, and solver control/progress types before the Phase 2 pagination cutover.
    *   **Proposed Changes:**
        *   Add `pub seed: u64` to `Config`; in `Config::default()`, set `page_width_mm: LengthUnit::In.to_mm(8.5)`, `page_height_mm: LengthUnit::In.to_mm(11.0)`, and `seed: 0`.
        *   Add `pub puzzle_bounds_mm: kurbo::Rect` to `SplicedPuzzle` and `KerfedGrid`.
        *   Define `pub enum Rotation90 { Deg0, Deg90, Deg180, Deg270 }` (`#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`) with `pub const ALL: [Rotation90; 4]`, `pub fn degrees(self) -> u16`, `pub fn next_cw(self) -> Self`, `pub fn prev_cw(self) -> Self`, and `pub fn rotate_point(self, p: kurbo::Point) -> kurbo::Point` mapping `(x, y)` to `(x, y)`, `(-y, x)`, `(-x, -y)`, and `(y, -x)`.
        *   Define `pub const DEFAULT_OPTIMIZE_ITERS: usize = 64;`, `pub struct PaginateOptions { pub seed: u64, pub max_iters: Option<usize> }` (`PaginateOptions::from_config(config: &Config)` returns `max_iters: Some(DEFAULT_OPTIMIZE_ITERS)`), and `pub struct NestProgress { pub rows: usize, pub cols: usize, pub iter: usize, pub total_iters: Option<usize>, pub pages: usize, pub total_bbox_area_mm2: f64, pub puzzle_area_mm2: f64, pub bbox_area_pct: f64, pub page_fill_ratios: Vec<f64> }`.
4.  **[`src/puzzle-gen/src/config.rs`](../../src/puzzle-gen/src/config.rs) (Modify):**
    *   **Rationale:** Parse and validate optional `seed` in `config.yaml` while preserving strict unknown-key and duplicate-key rejection.
    *   **Proposed Changes:**
        *   Add `seed: Option<u64>` to `RawConfig` (so `serde_saphyr` rejects negative numbers, floats, and strings during deserialization) and apply `if let Some(seed) = raw.seed { cfg.seed = seed; }` in `Config::from_yaml`.
        *   Update unit tests in `config.rs` (`yaml_defaults_and_kerf_multipliers`, verifying default `seed == 0`, explicit `seed: 42`, and rejection of `seed: -1` and `seed: 1.5`).
5.  **[`src/puzzle-gen/src/stage1.rs`](../../src/puzzle-gen/src/stage1.rs) (Modify):**
    *   **Rationale:** Forward Stage 0 rectilinear grid bounds into `SplicedPuzzle` so `bbox_area_pct` measures against the true Stage 0 puzzle area rather than image bounds or offset contours.
    *   **Proposed Changes:** In `splice_tabs`, compute `puzzle_bounds_mm = Rect::new(puzzle.x_lines_mm[0], puzzle.y_lines_mm[0], puzzle.x_lines_mm[puzzle.cols], puzzle.y_lines_mm[puzzle.rows])` and populate `SplicedPuzzle::puzzle_bounds_mm`.
6.  **[`src/puzzle-gen/src/stage2.rs`](../../src/puzzle-gen/src/stage2.rs) (Modify):**
    *   **Rationale:** Propagate `puzzle_bounds_mm` into `KerfedGrid` and expose existing `i_overlay` / polygon helpers for crate reuse.
    *   **Proposed Changes:**
        *   In `heal_and_offset`, set `puzzle_bounds_mm: spliced.puzzle_bounds_mm` on `KerfedGrid`.
        *   Promote `outline_options_i64`, `ring_from_io`, `perp_dist_point_segment`, and `find_interior_point` visibility to `pub(crate)` (joining already-`pub(crate)` `signed_area`, `polygon_aabb`, and `point_in_polygon`) so `nest.rs` and `paginate.rs` reuse `stage2.rs` utilities without duplication.

*   **Phase 1 Checkpoint:** Run `cargo check --workspace --all-targets` and `cargo test -p puzzle-gen`.

### Phase 2: Irregular Nesting Engine, Model Cutover, Stage 3/4 Emission & CLI Metrics

7.  **[`src/puzzle-gen/src/model.rs`](../../src/puzzle-gen/src/model.rs) (Modify — Phase 2 Domain Cutover):**
    *   **Rationale:** Switch `PageSlot`, `PagePlan`, and `PaginatedLayout` together with their downstream callers so every file builds cleanly in one phase.
    *   **Proposed Changes:**
        *   In `PageSlot`, replace `row: usize` and `col: usize` with `pub rotation: Rotation90`, and add `pub fn transform_point(self, p: kurbo::Point) -> kurbo::Point { self.rotation.rotate_point(p) + self.offset_mm }`.
        *   In `PagePlan`, add `pub content_bbox_mm: kurbo::Rect` and `pub bottom_right_anchor_mm: kurbo::Point`.
        *   In `PaginatedLayout`, remove `page_cols: usize` and `page_rows: usize`, retain `slot_w_mm: f64` and `slot_h_mm: f64` (used by `main.rs` for CLI `slot_mm=` summary output), and add `pub puzzle_area_mm2: f64`, `pub total_bbox_area_mm2: f64`, and `pub bbox_area_pct: f64`.
8.  **[`src/puzzle-gen/src/nest.rs`](../../src/puzzle-gen/src/nest.rs) (Create):**
    *   **Rationale:** Isolate conservative bitmask precomputation, sliding-window constructive placement, `SplitMix64` tail-window local search, and post-pass `BottomRight` mark compaction into focused helpers invoked by `paginate.rs`.
    *   **Proposed Changes:**
        *   Declare entry point:
            `pub(crate) fn pack_irregular_pages<F>(kerfed: &KerfedGrid, config: &Config, options: &PaginateOptions, cancel: &AtomicBool, on_progress: &mut F) -> Result<Vec<PagePlan>, PipelineError> where F: FnMut(NestProgress)`
        *   Define `SplitMix64` (`next_u64` with `0x9e3779b97f4a7c15`, `0xbf58476d1ce4e5b9`, `0x94d049bb133111eb`, and unbiased `gen_range(upper: usize) -> usize`).
        *   Validate `usable_w = config.page_width_mm - 2.0 * config.margin_mm > 0.0` and `usable_h = config.page_height_mm - 2.0 * config.margin_mm > 0.0` (else return `PipelineError::new("paginate", ...)` with minimum required `page_size`) and `kerfed.puzzle_bounds_mm.area() > 0.0`.
        *   Compute `grid_step_mm(usable_w, usable_h, config)`: `let raw = if config.kerf_mm > 0.0 { config.kerf_mm } else { config.segment_length_mm }; raw.max(0.10).max(usable_w.max(usable_h) / 2048.0)`. Derive `usable_cols = ((usable_w / step_mm).floor() as usize).clamp(1, 2048)` and `usable_rows = ((usable_h / step_mm).floor() as usize).clamp(1, 2048)`.
        *   Define `const DILATION_EPS_MM: f64 = 0.005;` (guarding `LineJoin::Round(0.1)` chord sagitta) and `pad_mm = if config.piece_spacing_mm > 0.0 { 0.5 * config.piece_spacing_mm + DILATION_EPS_MM } else { 0.0 }`, with `pad_cells = (pad_mm / step_mm).ceil() as usize + 1`.
        *   For each piece `i`: marshal `cut_polygon` and `bleed_polygon` (`&[Point]`) into `Vec<[f64; 2]>`, call `cut_io.overlay(&bleed_io, OverlayRule::Union, FillRule::Positive)` (3-argument `SingleFloatOverlay::overlay`), and extract the largest positive-area outer ring containing `stage2::find_interior_point(&piece.cut_polygon)` using `stage2::ring_from_io`, `stage2::signed_area`, and `stage2::point_in_polygon` (dropping any socket-bridging holes). Compute temporary `dilated_poly` via `outer_env_io.outline_custom_as::<i64>(&OutlineStyle::new(pad_mm).line_join(LineJoin::Round(0.1)), stage2::outline_options_i64())` when `config.piece_spacing_mm > 0.0` (or cloning `outer_env` when `0.0`), extracting its outer ring identically.
        *   For each `(piece_i, rot)` in `Rotation90::ALL`, rotate `outer_env` and temporary `dilated_poly`, shift local coordinates by `(-env_min_x + (pad_cells as f64) * step_mm, -env_min_y + (pad_cells as f64) * step_mm)`, and conservatively rasterize `dilated_poly` onto a row-major `Vec<u64>` bitmask (`LSB = col 0`, bit `lx & 63`) covering vertices, supercover edge traversals, and horizontal scanlines at `(row + 0.5) * step_mm`. Drop temporary `dilated_poly` vectors right after rasterizing piece `i`'s four variants.
        *   Implement `SheetBitmask` with dimensions `total_cols = usable_cols + 2 * pad_cells` and `total_rows = usable_rows + 2 * pad_cells`, row stride `words_per_row = total_cols.div_ceil(64)`, `words: Vec<u64>`, and `row_max_free_run: Vec<u16>` (precomputing widest zero-bit span per usable row to skip saturated rows in $O(1)$). Placing variant local `(0, 0)` at padded sheet `(gx, gy)` checks `(sheet_word >> sh) | (next_word << (64 - sh))` (branching on `sh == 0` to avoid shift overflow) against `variant_words`.
        *   Pre-mark localized corner keep-outs for `page_number` using `paginate::corner_keepout_size_mm(page_number) -> (kw, kh)` expanded only by `0.5 * config.piece_spacing_mm` (pairing with each piece's `0.5 * piece_spacing_mm` dilation for exact `1.0x` total buffer): `TopLeft` usable box `[0.0, kw + 0.5 * spacing] x [0.0, kh + 0.5 * spacing]` and fallback `BottomRight` usable box `[usable_w - kw - 0.5 * spacing, usable_w] x [usable_h - kh - 0.5 * spacing, usable_h]`.
        *   Validate initial feasibility against `max_possible_pages = kerfed.pieces.len().max(1)` (ensuring multi-digit page `10+` keep-out growth remains placeable): confirm `TopLeft` and fallback `BottomRight` are disjoint on the empty sheet, and test each piece across `Rotation90::ALL` with valid placement bounds `env_w <= usable_w + CONTAINMENT_TOL_MM`, `env_h <= usable_h + CONTAINMENT_TOL_MM`, `max_gx = ((usable_w - env_w).max(0.0) / step_mm).floor().min((usable_cols - 1) as f64) as usize`, and identical `max_gy`. On failure, return `PipelineError::new("paginate", ...)` naming `piece.id`, minimum rotated dimensions `(w_r, h_r)`, and deterministic `minimum required page_size` (smallest area between `(w_r + 2*margin + 2*(kw_max + spacing)) x (h_r + 2*margin)` and `(w_r + 2*margin) x (h_r + 2*margin + 2*(kh + spacing))` across `Rotation90::ALL`). Map placed `(gx, gy)` to continuous `offset_mm = Vec2::new(margin + ((gx as f64) * step_mm).min((usable_w - env_w).max(0.0)) - env_min_x, margin + ((gy as f64) * step_mm).min((usable_h - env_h).max(0.0)) - env_min_y)`.
        *   Implement `decode_layout` (`const MAX_OPEN_PAGES: usize = 2;`): for each piece in `order`, test `[pref, pref.next_cw(), pref.next_cw().next_cw(), pref.prev_cw()]` across the trailing `<= MAX_OPEN_PAGES` open sheets (`Top-gravity`: ascending `gy` then `gx`) picking the candidate minimizing `(new_extent_area_mm2, gy, gx, rot_rank)`, opening a new sheet (sized with current `page_number` keep-outs) only if none fits.
        *   Evaluate four initial deterministic orderings: (1) decreasing `dilated` AABB area (`piece_index` asc tie-break), (2) border/corner pieces first (`grid_border_edges desc`, `outer_env_area desc`, preferring `Deg0` for top/left borders and `Deg180`/`Deg90`/`Deg270` aligning borders flush to margins), (3) decreasing `max(env_w, env_h)` (`Deg0`), and (4) increasing `convexity_ratio = outer_env_area / (env_w * env_h)`.
        *   Implement `SplitMix64` refinement (`const TAIL_PAGES: usize = 2;`): loop for `options.max_iters` (`Some(64)` or until `cancel.load(Ordering::Relaxed)`). When `best.pages.len() > TAIL_PAGES`, apply tail-window mutations `80%` of the time (re-decoding strictly the suffix pieces onto fresh pages after the frozen `0..(pages.len() - TAIL_PAGES)` prefix snapshot) and global mutations `20%` of the time (full decode). Within the active slice, choose `50%` swap two indices, `30%` move/insert one element, or `20%` rotate one element's `rot_prefs[piece_index]`. Accept strictly better `(cand_pages, cand_area_sum) < (best_pages, best_area_sum - 1e-6)` and invoke `on_progress` after baseline `iter = 0`, on each accepted improvement, when `iter > 0 && iter % 8 == 0`, and on completion (with `page_fill_ratios[p] = (placed_outer_env_area_sum / (usable_w * usable_h)).clamp(0.0, 1.0)`).
        *   Run post-pass compaction once on the winning layout: (a) `compact_final_page` re-packs the final page under Left-gravity (`gx` ascending, then `gy` ascending) vs. Top-gravity and keeps the tighter piece bounding box, and (b) for every page `p`, run outward `BottomRight` mark probing from `base = (pieces_max_x.max(margin + kw).min(fallback.x), pieces_max_y.max(margin + kh).min(fallback.y))` plus deterministic `(aabb.x1 + kw + spacing, base.y)` / `(base.x, aabb.y1 + kh + spacing)` shifts past blocking pieces and `fallback = (page_w - margin, page_h - margin)`, filtering via `paginate::polygon_rect_clearance_ok` and `TopLeft`/`BottomRight` disjointness, minimizing `content_bbox_mm.area()`, and breaking area ties by largest `br_x + br_y`. Assemble `PagePlan` with `TopLeft` strokes ordered first and `BottomRight` strokes second.
        *   Add unit tests in `nest.rs`: conservative rasterizer covers vertices/edges/interiors, `SplitMix64` determinism, single-piece `90°` rotation fits when `0°` exceeds sheet width, and post-pass `BottomRight` mark compaction chooses tightest anchor with `br_x + br_y` tie-breaking.
9.  **[`src/puzzle-gen/src/paginate.rs`](../../src/puzzle-gen/src/paginate.rs) (Modify):**
    *   **Rationale:** Replace `CORNER_BAND_MM` uniform bands with localized keep-out helpers, anchor-parameterized corner strokes, shared polygon clearance checks, and exact continuous verification while preserving `compute_slot_bounds`.
    *   **Proposed Changes:**
        *   Replace `CORNER_BAND_MM` with `pub const CORNER_KEEP_OUT_H_MM: f64 = 6.0;`.
        *   Implement `pub fn corner_keepout_size_mm(page_number: usize) -> (f64, f64)` (`keepout_w = (1.0 + num_w).max(2.5) + 0.7`, `CORNER_KEEP_OUT_H_MM`) and `pub fn corner_keepout_rects_mm(page_number: usize, config: &Config, br_anchor: Point) -> [Rect; 2]` returning `[TopLeft, BottomRight]`.
        *   Implement `pub fn layout_corner_page_number_at_anchor(page_number: usize, anchor_x: f64, anchor_y: f64, corner: CornerAnchor) -> Vec<Vec<Point>>` anchored at the outer L-tick corner vertex, and delegate `layout_corner_page_number` to it.
        *   Keep `pub fn compute_slot_bounds(kerfed: &KerfedGrid) -> (f64, f64)` intact.
        *   Add `pub(crate) fn polygon_rect_clearance_ok(poly: &[Point], rect: Rect, min_spacing: f64, tol: f64) -> bool` (checking vertex-in-rect, rect-4-corners-in-polygon via `stage2::point_in_polygon`, and segment-to-rect-edge distance via `stage2::perp_dist_point_segment`) reused by both `nest.rs` and `verify_page_invariants`.
        *   Add `pub fn paginate_pieces_with_options<F>(kerfed: &KerfedGrid, config: &Config, options: &PaginateOptions, cancel: &AtomicBool, mut on_progress: F) -> Result<PaginatedLayout, PipelineError> where F: FnMut(NestProgress)` validating `puzzle_area_mm2 > 0.0`, invoking `nest::pack_irregular_pages`, checking every page with `verify_page_invariants`, validating `assigned_count == kerfed.pieces.len()`, and computing `total_bbox_area_mm2` and `bbox_area_pct`. Delegate `paginate_pieces(kerfed, config)` to it.
        *   Update `verify_page_invariants(kerfed: &KerfedGrid, page_number: usize, slots: &[PageSlot], br_anchor: Point, config: &Config) -> Result<(), PipelineError>` to check: (a) all transformed `cut_polygon` and `bleed_polygon` vertices inside sheet margins within `CONTAINMENT_TOL_MM`, (b) `br_anchor` inside valid margin/keep-out bounds and `tl_rect` / `br_rect` disjoint, (c) `polygon_rect_clearance_ok` for every piece's `cut_polygon` and `bleed_polygon` against both keep-out boxes at `config.piece_spacing_mm - CONTAINMENT_TOL_MM`, and (d) pairwise continuous separation `>= config.piece_spacing_mm - CONTAINMENT_TOL_MM` across `cut_polygon` and `bleed_polygon` contours (testing containment with `stage2::point_in_polygon` and exact segment-to-segment distance).
        *   Update `compute_slot_bounds_is_symmetric_and_rejects_oversized_piece` for the updated `PaginatedLayout` and keep-out single-piece error contract, and add unit tests for `corner_keepout_rects_mm`, `layout_corner_page_number_at_anchor`, and `verify_page_invariants` catching piece overlap, `BottomRight` keep-out violation, and overlapping keep-out boxes.
10. **[`src/puzzle-gen/src/lib.rs`](../../src/puzzle-gen/src/lib.rs) (Modify):**
    *   **Rationale:** Register `nest.rs` and expose `run_pipeline_with_options` for progress streaming and cancellation while preserving debug artifact attachment on pagination failure.
    *   **Proposed Changes:**
        *   Add `mod nest;`.
        *   Add `pub fn run_pipeline_with_options<F>(svg_xml: &str, config: &Config, strict: bool, notices: &mut Vec<PipelineNotice>, options: &PaginateOptions, cancel: &AtomicBool, on_progress: F) -> Result<PipelineArtifacts, PipelineError> where F: FnMut(NestProgress)`, forwarding into `paginate::paginate_pieces_with_options` and retaining `.map_err(|err| err.with_debug_spliced(spliced.clone()).with_debug_kerfed(kerfed.clone()))?`.
        *   Delegate `run_pipeline` to `run_pipeline_with_options` with `PaginateOptions::from_config(config)`, `&AtomicBool::new(false)`, and `|_| {}`.
11. **[`src/puzzle-gen/src/svg_emit.rs`](../../src/puzzle-gen/src/svg_emit.rs) (Modify):**
    *   **Rationale:** Emit matching `Rotation90` transforms across Stage 3 (`<g transform="...">`) and Stage 4 (baked transform-free paths) via one polygon formatting helper.
    *   **Proposed Changes:**
        *   Extend `format_closed_polygon(pts: &[Point], rot: Rotation90, offset_mm: Vec2, unit: LengthUnit) -> String` to apply `rot.rotate_point(p) + offset_mm` directly (passing `Rotation90::Deg0` in `emit_stage2` and `emit_stage3`, and `slot.rotation` in `emit_stage4_page`).
        *   In `emit_stage3`, emit `transform="translate(tx, ty)"` for `Deg0` or `transform="translate(tx, ty) rotate(90|180|270)"` for rotated pieces.
        *   Preserve `emits_valid_stage1_to_stage4_xml_structure` testing pipeline output (`TopLeft`-first `corner_strokes_mm` preserves `p2_mark.starts_with("M 3.100000")`), and add a dedicated unit test `rotates_stage3_and_stage4_consistently_for_deg90` constructing `PageSlot { rotation: Rotation90::Deg90, ... }`, `puzzle_bounds_mm`, and `content_bbox_mm` to verify exact Stage 3 `rotate(90)` syntax and Stage 4 coordinate baking.
12. **[`src/puzzle-gen/src/main.rs`](../../src/puzzle-gen/src/main.rs) (Modify — Phase 2 CLI Seed & Metrics):**
    *   **Rationale:** Expose `--seed` override and update `stdout` layout metrics before adding `--interactive` TUI plumbing in Phase 3.
    *   **Proposed Changes:**
        *   Add `#[arg(long)] seed: Option<u64>` to `Cli`, applying `if let Some(seed) = cli.seed { config.seed = seed; }` after loading YAML.
        *   Update `stdout` reporting: retain `grid=`, `matched_tabs=`, `missing_tabs=`, `slot_mm=`, and `pages=`; replace `page_grid=` with `puzzle_area_mm2={:.4}`, `total_bbox_mm2={:.4}`, and `bbox_area_pct={:.2}`.
13. **[`src/puzzle-gen/tests/integration_pipeline.rs`](../../src/puzzle-gen/tests/integration_pipeline.rs) (Modify — Phase 2 Existing Test Migration):**
    *   **Rationale:** Update coordinate-parity helpers for `Rotation90` and migrate existing `6in x 6in` / `page_grid=` test expectations.
    *   **Proposed Changes:**
        *   Replace `parse_translate_mm` with `parse_transform_mm(transform: Option<&str>, unit: LengthUnit) -> (f64, f64, f64)` returning `(tx_mm, ty_mm, rot_deg)`, and update `parse_path_points_mm(d: &str, tx_mm: f64, ty_mm: f64, rot_deg: f64, unit: LengthUnit)` to apply SVG clockwise `rotate(rot_deg)` before adding `(tx_mm, ty_mm)`.
        *   In `synthetic_5x5_stages_1_to_4_and_stage3_vs_stage4_coordinate_parity`, change inline YAML `page_size` from `"6in x 6in"` to `"3.5in x 3.5in"` so `pages.len() >= 2` exercises multi-page Stage 3 offsets and rotated Stage 3 vs. Stage 4 parity (`<= 1.5e-4 mm`), and verify any non-zero `Rotation90` piece transforms.
        *   In `cli_end_to_end_dry_run_collisions_overwrite_pruning_and_debug_emit`: replace `stdout_dry.contains("page_grid=")` with assertions checking `bbox_area_pct=`, `puzzle_area_mm2=`, and `total_bbox_mm2=`, and set `small.yaml` `page_size` to `"2in x 2in"` so pruning deterministically creates `>= 5` pages.

*   **Phase 2 Checkpoint:** Run `cargo check --workspace --all-targets` and `cargo test -p puzzle-gen`.

### Phase 3: Interactive `ratatui`/`crossterm` TUI, CLI Wiring & Nesting Integration Tests

14. **[`src/puzzle-gen/src/tui.rs`](../../src/puzzle-gen/src/tui.rs) (Create):**
    *   **Rationale:** Isolate `ratatui` + `crossterm` terminal lifecycle, RAII alternate-screen/raw-mode cleanup, input event polling, and convergence chart rendering inside the CLI binary crate.
    *   **Proposed Changes:**
        *   Define `TerminalGuard` RAII struct entering alternate screen + raw mode on `io::stderr()` during construction and unconditionally calling `disable_raw_mode()` + `LeaveAlternateScreen` in `Drop`.
        *   Implement `pub(crate) fn run_interactive_pipeline(svg_xml: &str, config: &Config, strict: bool, notices: &mut Vec<PipelineNotice>) -> Result<PipelineArtifacts, PipelineError>`:
            *   Allocate `Arc<AtomicBool>` (`cancel`) and `mpsc::channel::<NestProgress>()`; execute `run_pipeline_with_options` with `PaginateOptions { seed: config.seed, max_iters: None }` on a `std::thread::scope` worker thread, forwarding worker notices back onto `notices` and dropping the sender when finished.
            *   Loop on the main thread polling `crossterm::event::poll(Duration::from_millis(50))`, setting `cancel.store(true, Ordering::Relaxed)` on `q`, `Esc`, `Enter`, or `KeyCode::Char('c')` with `KeyModifiers::CONTROL`.
            *   Drain progress snapshots (`TryRecvError::Ok` / `Empty`) into `(iter as f64, bbox_area_pct)` history; exit the render loop immediately on `TryRecvError::Disconnected` (handling both normal completion and early Stage 0–3 worker errors before joining the worker thread and dropping `TerminalGuard`).
            *   Render a `ratatui` layout with: (1) top summary block (`grid={rows}x{cols}`, `pages`, `bbox_area_pct`, `iter`, elapsed time), (2) `ratatui::widgets::Chart` (`Dataset` using `Marker::Braille`) plotting `bbox_area_pct` over iterations, (3) up to `8` per-page `Gauge` bars with `ratio(r.clamp(0.0, 1.0))` plus a single `"... +K more pages (avg fill X%)"` overflow summary line when `pages > 8`, and (4) bottom keybind legend (`q / Enter / Esc / Ctrl+C: stop early and emit best layout`).
15. **[`src/puzzle-gen/src/main.rs`](../../src/puzzle-gen/src/main.rs) (Modify — Phase 3 Interactive CLI Flag):**
    *   **Rationale:** Dispatch `--interactive` to `tui.rs` on TTY sessions with `INTERACTIVE_NON_TTY` fallback while preserving notice emission, debug artifact output, `stdout` summary metrics, and `--dry-run`.
    *   **Proposed Changes:**
        *   Declare `mod tui;` and add `#[arg(long)] interactive: bool` to `Cli`.
        *   When `cli.interactive` is set, check `io::stderr().is_terminal() && io::stdin().is_terminal()`; if false, push `PipelineNotice { severity: NoticeSeverity::Warning, code: "INTERACTIVE_NON_TTY", ... }` and call `run_pipeline`; if true, call `tui::run_interactive_pipeline`. Subsequent notice printing, `--emit-debug-on-error` (skipped under `--dry-run`), `stdout` metrics, and Stage 1–4 atomic writes (skipped under `--dry-run`) run identically after TUI exit.
16. **[`src/puzzle-gen/tests/integration_pipeline.rs`](../../src/puzzle-gen/tests/integration_pipeline.rs) (Modify — Phase 3 Nesting, TUI Fallback & Benchmark Tests):**
    *   **Rationale:** Cover default `8.5in x 11in` single-page compaction, determinism, `90°` narrow-strip fitting, piped `--interactive` fallback, and release-mode `1,008`-piece throughput.
    *   **Proposed Changes:**
        *   Extract `build_complete_grid_svg(rows: usize, cols: usize, unit: LengthUnit) -> String` and delegate `build_complete_5x5_svg(unit)` to `(5, 5, unit)`.
        *   Add `default_8_5x11_packs_rainy_day_on_single_page_with_compacted_mark`: verifies the `4 in x 4 in` 25-piece example packs onto `1` page under `Config::default()`, with `bbox_area_pct < 200.0` and `pages[0].bottom_right_anchor_mm.y < cfg.page_height_mm - cfg.margin_mm`.
        *   Add `seeded_nesting_is_deterministic_and_cli_overrides_yaml_seed`: runs `run_pipeline` twice with `seed: 42` verifying identical `PaginatedLayout`, Stage 3, and Stage 4 strings, and checks CLI `--seed` overrides YAML `seed`.
        *   Add `non_tty_interactive_flag_warns_and_succeeds`: runs the compiled `puzzle-gen` binary with `--interactive --dry-run --no-strict` under piped non-TTY stdio asserting exit code `0`, `INTERACTIVE_NON_TTY` warning on `stderr`, and `bbox_area_pct=` on `stdout`.
        *   Add `rotation_allows_90_deg_single_piece_fit_on_narrow_strip`: builds a `1x1` `45 mm x 20 mm` piece on `"1.5in x 4in"` (`38.1 mm x 101.6 mm`) sheet and verifies placement succeeds via `Rotation90::Deg90` or `Rotation90::Deg270`.
        *   Add `#[ignore]` integration benchmark `large_1008_piece_release_benchmark`: runs `build_complete_grid_svg(28, 36, LengthUnit::Mm)` through `run_pipeline` with default `64` iterations and asserts elapsed time `< 60 s`.

*   **Phase 3 Checkpoint:** Run `cargo check --workspace --all-targets`, `cargo clippy -p puzzle-gen --all-targets -- -D warnings`, and `cargo test -p puzzle-gen`.

### Verification Plan

*   **Compilation & Linting:**
    *   `cargo check --workspace --all-targets`
    *   `cargo clippy -p puzzle-gen --all-targets -- -D warnings`
*   **Unit & Integration Tests:**
    *   `cargo test -p puzzle-gen` (executes all unit tests across `config.rs`, `ingest.rs`, `stage1.rs`, `stage2.rs`, `nest.rs`, `paginate.rs`, `svg_emit.rs` and integration tests in `tests/integration_pipeline.rs`, including rotated Stage 3/4 parity `<= 1.5e-4 mm` and `--interactive` non-TTY fallback).
    *   `cargo test -p puzzle-gen --release -- --ignored large_1008_piece_release_benchmark --nocapture` (verifies `28x36` `1,008`-piece release build runtime `< 60 s`).
*   **Example Dry-Run & Inkscape PDF Verification:**
    *   `cargo run -p puzzle-gen -- --dry-run --no-strict src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` (verifies `pages=1` and `bbox_area_pct < 200.0` under default `8.5in x 11in` config).
    *   Write Stage 3 output for a rotated fixture (e.g., `rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` to a temp directory) and run `inkscape --export-type=pdf` to verify rotated `<g transform="translate(...) rotate(...)">` `#puzzle-pattern` tiles align visually inside clipped `bleed_polygon` shapes without Cairo transform drift.
*   **Manual TTY Verification:**
    *   Run `cargo run -p puzzle-gen -- --interactive --dry-run --no-strict src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` in an interactive terminal and confirm `ratatui` chart rendering, early stop response across `q`, `Esc`, `Enter`, and `Ctrl+C`, and restored shell cursor/echo state after exit.

---

*Drafted with EGM*
