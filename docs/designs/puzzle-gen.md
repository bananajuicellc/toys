# `puzzle-gen`: Four-Stage Laser Jigsaw Puzzle Generator

*   **Author:** Tim Sweña (Swast)
*   **Status:** Draft
*   **Date:** 2026-11-13

--------------------------------------------------------------------------------

## Overview

`puzzle-gen` is a local Rust CLI and geometric library in `src/puzzle-gen` inside the repository's Cargo workspace. It prepares custom jigsaw puzzle artwork drawn in Inkscape for physical fabrication: printing artwork onto adhesive label paper via Inkscape multi-page PDF export, adhering the sheet to backing board, and cutting individual pieces on a laser cutter.

Starting from an unjoined Stage 0 Inkscape SVG containing the required `Image`, `Pieces`, and `Tabs` layers—embedded raster artwork (`Image`), a rectilinear grid of `<rect>` elements (`Pieces`), and open `<path>` tab curves along internal edges (`Tabs`)—plus an optional YAML configuration file, `puzzle-gen` runs an in-memory geometric pipeline in canonical millimeters (`mm`) and writes four sequential SVG outputs for inspection, printing, and laser cutting:

1.  **Stage 1 (`<prefix>-stage1.svg`):** Matches and splices each shared tab curve into its two adjacent clockwise piece boundaries (removing the replaced straight interior edge across each tab neck), fills every piece with a `2x2` mirrored `<pattern>` built from the single embedded `<image>`, outlines each piece (`#000000`, `0.1 mm` stroke), and renders row and column index labels (`0..rows-1`, `0..cols-1`) in a `GridLabels` layer.
2.  **Stage 2 (`<prefix>-stage2.svg`):** Flattens piece curves into polylines, heals cross-edge Bezier overshoot loops via polygon self-union, offsets each piece outward by `kerf / 2` for laser cut paths and `print_bleed` for print fills, spaces cut pieces apart on a uniform bounding-box grid while locking each piece's image pattern to its original artwork coordinates, and aligns `GridLabels` along the spaced grid.
3.  **Stage 3 (`<prefix>-stage3.svg`):** Reflows pieces in row-major order across Inkscape `<inkscape:page>` sheets inside configured margins and corner registration bands for Inkscape multi-page PDF export. Printed pieces render with `stroke: none` using the outward `print_bleed` contour, paired with stroked vector page numbers and right-angle corner registration ticks (`#000000`, `0.1 mm` stroke) in top-left and bottom-right corner bands.
4.  **Stage 4 (`<prefix>-stage4-page<N>.svg`):** Emits one standalone single-page SVG per page in `page_unit` (default `mm`) with ungrouped, transform-free page-local `kerf / 2` cut paths (`Cut` layer, `#000000`, `0.1 mm` stroke) and identical top-left and bottom-right vector page numbers and corner registration ticks (`PageNumbers` layer, `score_color`, `0.1 mm` stroke), omitting all `<defs>`, `<pattern>`, and `<image>` elements.

## The Problem

Preparing a custom Inkscape jigsaw puzzle for print-and-cut laser fabrication involves four error-prone manual geometry steps:

*   **Manual path splicing:** Authoring a puzzle as a grid of `<rect>` elements plus separate open `<path>` curves for interlocking tabs is fast in Inkscape, but small node-snapping offsets leave gaps between rectangle boundaries and tab endpoints. Turning that sketch into closed pieces manually requires duplicating, reversing, splitting, and node-joining every shared internal curve (for example, 40 internal edges on a 5x5 puzzle). Leaving a straight rectangle segment across a tab neck causes the laser to cut the tab off.
*   **Image bleed and pattern drift:** Shifting pieces apart in Inkscape breaks `userSpaceOnUse` image patterns unless each piece carries a matching `patternTransform`, where minor typos (such as `0.01` vs. `1/96`) shift printed artwork relative to the cut line. Pieces expanded outward for kerf compensation or registration tolerance also need mirrored pixel bleed around border edges so unprinted white label stock stays hidden whenever manual print-to-cut alignment stays within `print_bleed - kerf / 2` (`0.09 mm` under default `0.18 mm` kerf and `0.18 mm` bleed).
*   **Kerf compensation and undercut collisions:** Because a laser beam vaporizes material along its cut path (for example, `0.18 mm` wide), uncompensated pieces fit loosely. Offsetting each piece outward by `kerf / 2` widens tabs and narrows sockets, while pulling pieces apart by only `1 kerf` allows wide tab bulbs to cross adjacent socket necks during cutting. Pieces need full axis-aligned bounding box (AABB) separation plus spacing margin to avoid collisions.
*   **Print-to-cut registration:** Printing adhesive label sheets and cutting them later on a laser bed introduces small alignment offsets. Printing black piece outlines leaves dark ink along cut edges whenever registration drifts, whereas omitting strokes on Stage 3 print fills and placing identical vector page numbers and corner registration ticks within matching margins across Stage 3 and Stage 4 provides clear manual alignment references for the operator.

`puzzle-gen` automates all four stages from one deterministic in-memory pipeline so authors maintain only the unjoined Stage 0 Inkscape SVG and optional YAML parameters while keeping all four stage outputs on disk for inspection.

## Goals

*   **Stage 0 Ingestion & Bounded Early Snapping:** Parse an Inkscape SVG with `Image`, `Pieces`, and `Tabs` layers, resolve document units and viewBox scale into canonical `mm`, enforce per-layer element allowlists, verify an axis-aligned contiguous `rows x cols` grid after `1.0 mm` edge clustering, filter sub-`0.05 mm` stray paths, and match open tab endpoints to internal edges within `2.0 mm` while rejecting ambiguous multi-edge matches.
*   **In-Memory Pipeline with 4 Inspectable Outputs:** Run all geometric stages in canonical `mm` without re-parsing intermediate SVGs, projecting Stages 1–3 into normalized original document units (`doc_unit`) and Stage 4 into `page_unit` (default `mm`).
*   **Single-Image Mirrored Pattern Bleed:** Embed the source raster `<image>` once inside `<defs>` for Stages 1–3 and construct a `2x2` mirrored `<pattern>` via four `<use>` tiles so outward border offsets sample mirrored image pixels without duplicating raster bytes.
*   **Separated Cut Kerf and Print Bleed:** Flatten curves bounded by `segment_length` (default `1 kerf` = `0.18 mm`), heal cross-edge Bezier overshoot loops via `FillRule::Positive` self-union, compute independent `+kerf / 2` (`0.09 mm` default) cut outlines and `+print_bleed` (`0.18 mm` default) strokeless print fills, and apply unified slot + `piece_spacing` separation across Stages 2–4.
*   **Collision-Free Full-AABB Pagination:** Position pieces on Stage 3 and Stage 4 pages inside margin-bounded usable areas (reduced vertically by top and bottom corner registration bands) using symmetric cut-centered AABB slots plus `piece_spacing`, ensuring `page_cols >= 1 && page_rows >= 1`.
*   **Identical Corner Registration Anchors:** Place built-in, font-free single-stroke vector page numbers and right-angle corner registration ticks inside top-left and bottom-right corner bands at identical physical coordinates across Stage 3 (`#000000`) and Stage 4 (`score_color`, default `#0000ff`).
*   **Pattern-Free, Layer-Separated Laser Output:** Emit one standalone `<prefix>-stage4-page<N>.svg` per page with ungrouped, transform-free page-local `<path>` geometry, zero `<defs>`, `<image>`, or `<pattern>` elements, and separated `Cut` and `PageNumbers` layers.
*   **Safe File Overwrites & Stale Page Cleanup:** Refuse to overwrite existing output files when `--overwrite` is omitted; when `--overwrite` is enabled, atomically write each output file and prune leftover `<prefix>-stage4-page<N>.svg` files whose page number exceeds the new page count.

## Non-Goals

*   **Non-rectangular or freeform piece grids in v1:** Input `Pieces` layers must form a rectilinear grid of `<rect>` elements; arbitrary Voronoi or polygon meshes are excluded.
*   **Pairwise edge-overhang or irregular nesting optimizations:** Pieces are spaced uniformly by full bounding boxes (`AABB + piece_spacing`) rather than nesting protruding tabs into adjacent sockets.
*   **Heuristics for malformed manual path joins:** Beyond bounded early edge/tab snapping, transform normalization, and sub-`0.05 mm` stray-path removal, recovery from incomplete or duplicate manual path joins (such as the partial layout proof-of-concept in [rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg](../../src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg)) is out of scope.
*   **Non-Inkscape multi-page Stage 3 splitting:** Stage 3 uses Inkscape `<inkscape:page>` elements for Inkscape PDF export rather than emitting separate per-page Stage 3 print SVGs.
*   **Re-fitting cubic Beziers to offset boundaries:** Stages 2–4 represent offset piece shapes as polylines whose curved segments are bounded by `segment_length`.
*   **External linked raster images or GUI integration:** External non-`data:` `<image>` file references and WASM/UI integration are excluded.

## Open Questions

None.

## Critical User Journeys (CUJs)

### CUJ 1: End-to-End Default Puzzle Generation

*   **Given:** A user has a complete Stage 0 Inkscape SVG (`rainy-day-complete.svg`) with a `4 in x 4 in` embedded raster image in `Image`, a `5x5` grid of `0.8 in x 0.8 in` `<rect>` pieces in `Pieces`, and all 40 open `<path>` curves in `Tabs`, with no YAML config file.
*   **When:** The user runs `puzzle-gen rainy-day-complete.svg`.
*   **Then:**
    *   `puzzle-gen` validates all 40 internal edges, creates `generated/` next to `rainy-day-complete.svg` if needed, prints a layout summary (`grid`, `matched_tabs`, `missing_tabs`, `slot_mm`, `pages`, and output paths), and writes `generated/rainy-day-complete-stage1.svg`, `generated/rainy-day-complete-stage2.svg`, `generated/rainy-day-complete-stage3.svg`, and `generated/rainy-day-complete-stage4-page1.svg` through `generated/rainy-day-complete-stage4-page<N>.svg` using defaults (`page_size: 6in x 6in`, `page_unit: mm`, `margin: 0.25in`, `kerf: 0.18mm`, `segment_length: 1 kerf`, `piece_spacing: 1 kerf`, `print_bleed: 1 kerf`, `score_color: #0000ff`).
    *   The user opens `generated/rainy-day-complete-stage3.svg` in Inkscape, exports a multi-page PDF and prints onto adhesive label paper, adheres each sheet to board, opens each matching `generated/rainy-day-complete-stage4-page<N>.svg`, aligns the laser using the top-left and bottom-right corner numbers and ticks, and cuts interlocking pieces with zero exposed white label stock within the `0.09 mm` (`print_bleed - kerf / 2`) alignment tolerance.

### CUJ 2: Custom Material, Kerf, and Page Size via YAML Configuration

*   **Given:** A user is cutting `3 mm` plywood on a laser with a `0.24 mm` kerf and printing on `8.5 in x 11 in` label sheets (`7.5 in x 10 in` margin-bounded region inside `0.5 in` margins, reduced vertically by two `6.0 mm` corner bands to obtain the usable placement area), and creates `config.yaml` overriding `page_size: "8.5in x 11in"`, `margin: "0.5in"`, `kerf: "0.24mm"`, `segment_length: "1 kerf"`, `piece_spacing: "1 kerf"`, and `print_bleed: "1 kerf"`.
*   **When:** The user runs `puzzle-gen puzzle.svg --config config.yaml --prefix wood-run`.
*   **Then:**
    *   `puzzle-gen` converts all configured lengths and kerf multipliers (`1 kerf` = `0.24 mm`) to canonical millimeters, computes `+0.12 mm` (`kerf / 2`) cut offsets for Stages 2 and 4 and `+0.24 mm` (`print_bleed`) fill offsets for Stage 3, paginates pieces inside the usable area, creates `generated/` next to `puzzle.svg` if needed, and emits `generated/wood-run-stage1.svg`, `generated/wood-run-stage2.svg`, `generated/wood-run-stage3.svg`, and `generated/wood-run-stage4-page<N>.svg`.

### CUJ 3: Diagnosing Input SVG Authoring Issues

*   **Given:** A user supplies a Stage 0 SVG where one internal grid edge has no tab `<path>`, or where even one piece slot (`slot_w x slot_h`) plus margins and corner bands exceeds `page_size` (`page_cols * page_rows < 1`).
*   **When:** The user runs `puzzle-gen input.svg` (or `puzzle-gen input.svg --no-strict`).
*   **Then:**
    *   In default strict mode, a missing internal tab causes `puzzle-gen` to exit non-zero before writing outputs and print an error identifying the grid edge `(row1, col1)-(row2, col2)`. When `--no-strict` is passed, `puzzle-gen` downgrades missing internal tabs to warnings, keeps those shared edges straight, and completes all four stages.
    *   For unattached non-stray paths, corner-ambiguous or multi-edge tabs, duplicate tabs on one edge, or insufficient page capacity (`page_cols * page_rows < 1`), `puzzle-gen` exits non-zero before writing outputs and prints an error naming the offending SVG element `id` or required minimum `page_size`.

--------------------------------------------------------------------------------

## Technical Plan (Architecture)

### Architecture Overview

`puzzle-gen` separates an I/O-free geometric library (`src/puzzle-gen/src/lib.rs`) from a CLI and filesystem wrapper (`src/puzzle-gen/src/main.rs`). All four stages execute sequentially in memory against immutable domain structs expressed in canonical physical millimeters (`mm`). Downstream stages never re-parse SVG XML emitted by earlier stages; the library returns typed pipeline state (`PipelineArtifacts`), and `main.rs` serializes only the requested output SVGs lazily one stage at a time so `--dry-run` avoids XML/base64 formatting work and multi-megabyte raster payloads are never duplicated simultaneously in memory.

```text
Stage 0 Inkscape SVG ──┐
                       ▼
Optional YAML Config ─▶ [ Config (serde-saphyr) & CoordFrame (mm) ]
                                       │
                                       ▼
                       [ Stage 0 Ingest: IngestedPuzzle ]
                       (Image, 1.0mm-clustered R x C grid, TabCandidates)
                                       │
                                       ▼
                       [ Stage 1 Join & Pattern: SplicedPuzzle ] ──▶ <prefix>-stage1.svg (orig doc_unit)
                       (2.0mm pair snap, clockwise splice, 2x2 <use> pattern)
                                       │
                                       ▼
                       [ Stage 2 Heal & Offset: KerfedGrid ] ──────▶ <prefix>-stage2.svg (orig doc_unit)
                       (Flatten, Positive self-union, +kerf/2, +print_bleed)
                                       │
                                       ▼
                       [ Stage 3/4 Paginate: PaginatedLayout ] ──┬─▶ <prefix>-stage3.svg (orig doc_unit, multi-page)
                       (Corner bands, margin-bounded slots)      └─▶ <prefix>-stage4-page<N>.svg (page_unit, Cut + PageNumbers)
```

`src/puzzle-gen/Cargo.toml` composes pure-Rust crates:
*   **XML & SVG parsing/writing:** `roxmltree`, `svgtypes`, and `xmlwriter` (preserving Inkscape namespaces, layers, `<sodipodi:namedview>`, and `<inkscape:page>` elements).
*   **CLI & YAML configuration:** `clap` (with `derive` feature), `serde` (`derive`), and `serde-saphyr` (directly deserializing typed config with built-in duplicate-key rejection, unknown-field denial via `#[serde(deny_unknown_fields)]`, and parser resource budgets).
*   **Curve geometry & polygon booleans/offsetting:** `kurbo` (exact curve representations, quadratic elevation, curve reversal, and bounded flattening) and `i_overlay` (floating-point `FillRule::Positive` self-union healing and `LineJoin::Round` outward buffering).
*   **Atomic file writes & isolated tests:** `tempfile`.

### CLI Interface & File Lifecycle (`--overwrite`)

*   **Invocation:** `puzzle-gen <INPUT_SVG> [OPTIONS]`
*   **Flags:**
    *   `--config <PATH>`: Optional YAML config file path. If omitted, default configuration applies.
    *   `--output-dir <DIR>`: Output directory for generated SVGs (defaults to `<input_svg_parent>/generated`, or `./generated` if `<INPUT_SVG>` is a bare filename). Created automatically only when writing outputs.
    *   `--prefix <PREFIX>`: Output filename prefix (defaults to `<INPUT_SVG>` file stem). Must not be empty, `.`, or `..`, or contain `/` or `\`.
    *   `--overwrite`: Allow replacing existing target outputs and pruning leftover higher-numbered Stage 4 cut files (defaults to `false`).
    *   `--dry-run`: Run parsing, snapping, offsetting, and pagination checks; print notices and layout summary (`grid`, `matched_tabs`, `missing_tabs`, `slot_mm`, `pages`, `page_cols x page_rows`) without creating `--output-dir` or serializing/writing SVG files.
    *   `--no-strict`: Downgrade missing internal tabs from errors (default) to warnings and keep those edges straight.
    *   `--emit-debug-on-error`: When Stage 2 or Stage 3 fails, still render and write valid completed Stage 1 (and Stage 2 if available) debug SVGs (unless `--dry-run` is set), checking collisions against only the debug file(s) being written and never pruning Stage 4 pages.
*   **File Protection & Stale Page Pruning:**
    *   Existing Stage 4 files match the exact base-10 pattern `<prefix>-stage4-page<K>.svg` (`K >= 1`, no leading zeros) so unrelated files such as `<prefix>-stage4-page-notes.svg` are never touched.
    *   Compute all pipeline geometry and pagination checks before writing files. When probing `--output-dir` prior to directory creation, treat a missing directory (`NotFound`) as zero collisions.
    *   If `--overwrite` is `false` and any target output path already exists, fail listing the colliding path(s) before creating or modifying files.
    *   If `--overwrite` is `true`, create `--output-dir` if absent, write and persist each new stage output via per-file temporary files (`tempfile::NamedTempFile::new_in`) so each individual file replacement is atomic, and only after all writes succeed prune leftover `<prefix>-stage4-page<K>.svg` files where `K > new_page_count`.

### Configuration Schema & Unit Normalization (`CoordFrame`)

The optional YAML configuration file is deserialized via `serde-saphyr` with unknown and duplicate keys rejected:

```yaml
version: 1
page_size: "6in x 6in"
page_unit: "mm"
margin: "0.25in"
kerf: "0.18mm"
segment_length: "1 kerf"
piece_spacing: "1 kerf"
print_bleed: "1 kerf"
score_color: "#0000ff"
```

*   **Schema Validation Rules:**
    *   Empty or comment-only YAML resolves to default `Config`. Multi-document or non-mapping YAML fails with a schema error.
    *   Top-level keys must belong to `{version, page_size, page_unit, margin, kerf, segment_length, piece_spacing, print_bleed, score_color}`; unknown or duplicate keys fail immediately.
    *   If present, `version` must equal integer `1`. All other fields are string scalars. If `score_color` is present with a YAML `null` or empty value (for example when an unquoted `#0000ff` is stripped as a YAML comment), fail with a message advising the author to quote `"#RRGGBB"`.
*   **Dimension Grammar & Parameter Bounds:**
    *   Scalar lengths accept case-insensitive decimal numbers paired with any of the four `LengthUnit` values (`mm`, `cm`, `in`, `px`, using CSS 96 DPI `25.4 / 96.0 mm/px`), ignoring optional whitespace (e.g., `0.18mm` or `0.18 mm`). Unitless numbers, scientific notation, percentages, `NaN`, and infinities fail with a syntax error showing valid examples.
    *   `page_size` accepts `<width> x <height>` (case-insensitive `x`, optional whitespace, each side a scalar length) with positive sheet dimensions (`page_width_mm > 0.0`, `page_height_mm > 0.0`).
    *   `page_unit` accepts `mm`, `cm`, `in`, or `px` (default `mm`) and sets SVG dimensions and `inkscape:document-units` on Stage 4 outputs.
    *   `margin >= 0.0`, `kerf >= 0.0`, `piece_spacing >= 0.0`, and `print_bleed >= 0.0`.
    *   `segment_length`, `piece_spacing`, and `print_bleed` also accept `<multiplier> kerf` (`>= 0.0`), resolved against configured `kerf_mm`. `segment_length_mm` must be `>= 0.02 mm` (preventing runaway curve subdivision); when `kerf_mm == 0.0` and `segment_length` uses a `kerf` multiplier (yielding `0.0 mm`), return a validation error naming both `kerf` and `segment_length` and asking for an explicit positive `segment_length`.
    *   `score_color` accepts a case-insensitive 6-digit hex color (`#RRGGBB`, normalized to lowercase `#rrggbb`, default `#0000ff`) applied only to the Stage 4 `PageNumbers` layer.
*   **`CoordFrame` Unit Resolution:**
    *   On Stage 0 ingest, read `inkscape:document-units` from `<sodipodi:namedview>` first, falling back to `<svg inkscape:document-units>`, and otherwise inferring `doc_unit` from `<svg>` `width`/`height` unit (`mm`, `cm`, `in`, `px`, defaulting unitless dimensions to CSS `px`, and rejecting mismatched `width` vs. `height` units when `inkscape:document-units` is omitted). Reject missing, percentage, or non-positive `width`/`height`/`viewBox` dimensions.
    *   Parse `viewBox` (defaulting to `(0, 0)` and CSS `px` dimensions when absent), convert `<svg>` `width` and `height` into `mm`, and compute `user_to_mm = (phys_w_mm / vb_w, phys_h_mm / vb_h)` and `doc_to_mm = doc_unit.to_mm(1.0)`. Reject non-uniform aspect ratios where `|user_to_mm.0 - user_to_mm.1| / max(user_to_mm.0, user_to_mm.1) > 1e-4`.
    *   Normalize all geometry, snapping tolerances, offsetting, and pagination math in canonical `mm` (using SVG `+y`-down coordinates, where clockwise closed polygons have positive signed shoelace area). Stages 1–3 project into normalized `doc_unit` (`1 user unit = 1 doc_unit`) while Stage 4 projects into `Config.page_unit`.

### Core Domain Data Structures

All geometric types share canonical `mm` coordinates in memory; projection to `doc_unit` or `page_unit` happens solely during lazy SVG serialization.

| Type | Pipeline Role | Core Responsibilities & Invariants |
| --- | --- | --- |
| `Config` | Configuration Input | Validated sheet dimensions, `page_unit`, `margin_mm`, `kerf_mm`, `segment_length_mm`, `piece_spacing_mm`, `print_bleed_mm`, and `score_color`. |
| `CoordFrame` | Coordinate Basis | Input `doc_unit`, `viewbox_min_user`, `user_to_mm`, and `doc_to_mm` conversion factors (`mm`). |
| `IngestedPuzzle` | Stage 0 Output | Holds `frame`, validated `data:image/` URI `image_href`, axis-aligned `image_bounds_mm`, `image_preserve_aspect_ratio`, contiguous `(rows, cols)` `GridCell` bounds (`mm`), and normalized `TabCandidate` curves. |
| `SplicedPuzzle` | Stage 1 Output | Holds `frame`, image metadata, `(rows, cols)`, matched/missing edge counts, and closed clockwise `SplicedPiece` contours (`kurbo::BezPath` in `mm`). |
| `KerfedGrid` | Stage 2 Output | Holds `frame`, image metadata, `(rows, cols)`, and per-piece `KerfedPiece` contours (`cut_polygon`, `bleed_polygon`, each a positive-area ring in `mm`). |
| `PaginatedLayout` | Stage 3–4 Layout | Shared symmetric cut-centered `(slot_w_mm, slot_h_mm)`, corner band bounds, `(page_cols, page_rows)` capacity (`>= 1 x 1`), and per-page `PagePlan` slots + corner mark polylines (`mm`). |
| `PipelineArtifacts` | Pipeline Output | Bundles `spliced: SplicedPuzzle`, `kerfed: KerfedGrid`, `layout: PaginatedLayout`, and layout summary metrics without pre-rendering XML strings. |

### Stage-by-Stage Geometric Pipeline

#### Stage 0: Input SVG Ingestion

1.  **Layer Resolution:** Locate `<g inkscape:groupmode="layer">` elements by `inkscape:label` (`Image`, `Pieces`, `Tabs`), failing on missing or duplicate required labels. Allow nested `<g>` groups and `<title>`/`<desc>` metadata inside required layers, allow only `<image>` in `Image`, `<rect>` in `Pieces`, and `<path>` in `Tabs`, and reject unexpected element types citing element `id`. Ignore unrelated guide layers.
2.  **Transform Composition:** Walk each required layer recursively (up to safe nesting depth `64`) and compose ancestor `transform` chains followed by `(p_user - viewbox_min_user) * user_to_mm` into canonical `mm`, rejecting finite coordinates outside safe physical bounds (`<= 10,000 mm`).
3.  **Image Validation:** Extract the single `<image>` in `Image` (supporting both `href` and `xlink:href`, rejecting missing, duplicate, or non-`data:image/(png|jpeg|webp|gif);base64,` URIs). Retain `preserveAspectRatio` (explicitly defaulting omitted values to `"none"` so the image fills its target box cleanly without aspect letterboxing). Allow only translation and positive axis scale on `<image>`; reject rotated, skewed, or negatively scaled transforms citing element `id`. Emit a `Warning` notice if `image_bounds_mm` does not cover the full `Pieces` grid bounds.
4.  **Rectilinear Grid Construction:** Extract `<rect>` elements from `Pieces` (rejecting rounded `rx`/`ry > 1e-4 mm` and checking orthogonality in canonical `mm` on transformed corner vectors so rotations/skews fail scale-invariantly), normalize negative-scale `<rect>` bounds via corner `min`/`max`, group edge coordinates into bounded-diameter `<= 1.0 mm` clusters (mean representative), sort into `(row, col)`, and verify a contiguous non-overlapping `rows x cols` grid where every piece occupies exactly one `[x_c, x_{c+1}] x [y_r, y_{r+1}]` interval.
5.  **Path Normalization & Stray-Path Filtering:** Parse `Tabs` `<path>` `d` strings (`M`/`m`, `L`/`l`, `H`/`h`, `V`/`v`, `C`/`c`, `S`/`s`, `Q`/`q`, `T`/`t`, `A`/`a`, `Z`/`z`) into `LineTo`/`CurveTo` segments in `mm`. Filter sub-`0.05 mm` stray paths (`aabb_diag < 0.05 mm && arc_len < 0.05 mm`, emitting an `Info` notice with element `id`) *before* rejecting non-stray closed (`Z`) or multi-subpath (`> 1` `M`) elements.

#### Stage 1: Tab Splicing & `2x2` Mirrored Pattern

1.  **Canonical Internal Edges & Bounded Pair Matching:** Enumerate all internal grid edges (`(rows - 1) * cols` horizontal edges oriented `L→R` from `x_lines[c]` to `x_lines[c + 1]`; `rows * (cols - 1)` vertical edges oriented `T→B` from `y_lines[r]` to `y_lines[r + 1]`). Match each `TabCandidate` jointly by its `(start, end)` endpoint pair against every internal and outer-border edge:
    *   Compute perpendicular distance `perp_dist` and longitudinal projection `t` along `[0.0, edge_len]` for both endpoints.
    *   Accept a candidate edge when both endpoints have `perp_dist <= 2.0 mm`, `t` inside `[-0.25 mm, edge_len + 0.25 mm]` (or `[-2.0 mm, edge_len + 2.0 mm]` when disambiguated on a single internal edge), and clamped span `|clamp(t_end) - clamp(t_start)| >= 1.0 mm`.
    *   Reject tabs matching `> 1` grid edge (corner or narrow-cell ambiguity), tabs matching an outer border, unmatched tabs, or duplicate tabs on one internal edge, naming element `id` and coordinates.
    *   Project matched endpoints orthogonally onto `[0.0, edge_len]` (`t = clamp(t, 0.0, edge_len)`), translate adjacent cubic control handles by the same delta as their snapped endpoint to preserve tangency, and reverse the normalized curve when `t_end < t_start` so `t_start < t_end` in canonical edge order.
    *   Under strict mode (default), fail naming `(r1, c1)-(r2, c2)` if any internal edge has no tab; under `--no-strict`, emit a `Warning` notice and keep that edge straight.
2.  **Clockwise Contour Assembly:** For every `(row, col)` cell, build a closed clockwise `kurbo::BezPath` across its four edges (`top` L→R, `right` T→B, `bottom` R→L, `left` B→T). Canonical internal tabs splice forward (`t = 0.0 -> edge_len`) on **`top`** and **`right`** cell edges, and in reverse (`t = edge_len -> 0.0`) on **`bottom`** and **`left`** cell edges. When an entry or exit lead length is `<= 1e-4 mm`, elide the `LineTo` segment and snap the tab's start or end coordinate directly onto `slot_start` / `slot_end`.
3.  **Stage 1 SVG Projection (`<prefix>-stage1.svg`):**
    *   Project coordinates into normalized `doc_unit`, emit `<sodipodi:namedview inkscape:document-units="...">`, and expand the root `<svg>` `width`, `height`, and `viewBox` by a fixed `GridLabels` header margin (`8.0 mm`) around the piece bounds so top/left labels and outward tabs are never clipped.
    *   Inside `<defs>`, emit `<image id="puzzle-image" x="0" y="0" ...>` once and `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse" x="img_x" y="img_y" width="2W" height="2H">` with four `<use xlink:href="#puzzle-image">` tiles mirrored across the `2x2` pattern grid.
    *   Wrap piece `<path id="piece-r{row}-c{col}">` elements (`fill: url(#puzzle-pattern); stroke: #000000; stroke-width: <0.1mm in doc_unit>`) in `<g inkscape:groupmode="layer" inkscape:label="Pieces">` and add `<g inkscape:groupmode="layer" inkscape:label="GridLabels">` with centered row (`0..rows-1`) and column (`0..cols-1`) `<text>` indices.

#### Stage 2: Flattening, Self-Union Healing, Kerf/Bleed Offsetting & Grid Spacing

1.  **Curve Flattening:** Keep straight `LineTo` segments as two-point line segments, flatten curved segments to bounded error controlled by `segment_length_mm`, deduplicate consecutive points closer than `0.001 mm`, and normalize each closed ring to positive signed `(x, y)` area (`+1` interior winding in `+y`-down coordinates).
2.  **Self-Union Healing & Outward Polygon Offsetting:**
    *   Run a floating-point `i_overlay` self-union pass (`OverlayRule::Subject`, `FillRule::Positive`) on each piece contour before measuring `unoffset_area`, discarding `-1` exterior cross-edge Bezier overshoot ears while keeping `+1`/`+2` interior areas and verifying a single hole-free ring (`shapes.len() == 1 && shapes[0].len() == 1`).
    *   Short-circuit `0.0` offsets (`kerf_mm == 0.0` or `print_bleed_mm == 0.0`) directly to a clone of the healed unoffset ring. For positive offsets, buffer outward via `i_overlay` floating-point `OutlineOffset` (`LineJoin::Round`) at `+kerf_mm / 2.0` (`cut_polygon`) and `+print_bleed_mm` (`bleed_polygon`), followed by closed-ring Ramer-Douglas-Peucker (`RDP`) simplification.
    *   Validate `cut_polygon` has `shapes.len() == 1 && shapes[0].len() == 1` and grows cleanly relative to `unoffset_area` when `kerf_mm > 0.0`. On `bleed_polygon`, select the positive-area outer boundary ring containing an interior point of the healed unoffset polygon (dropping any inner hole rings formed when bleed bridges across empty sockets) and validate `bleed_area > unoffset_area` when `print_bleed_mm > 0.0` (emitting a `Warning` notice if `print_bleed_mm < kerf_mm / 2.0`). Fail on socket collapse naming piece `(row, col)` and `id`.
3.  **Stage 2 SVG Projection (`<prefix>-stage2.svg`):**
    *   Reuse the shared `(slot_w_mm, slot_h_mm)` + `piece_spacing_mm` pitch from `PaginatedLayout` on a `(label_margin_mm, label_margin_mm)` grid origin, centering piece `(row, col)` in its grid cell via `<g transform="translate(dx, dy)">` while keeping `<path id="piece-r{row}-c{col}">` coordinates locked to original puzzle space so `url(#puzzle-pattern)` tracks each piece's original artwork slice.
    *   Wrap pieces in `<g inkscape:groupmode="layer" inkscape:label="Pieces">`, align `GridLabels` `<text>` indices to spaced row/column centers, and size root `width`, `height`, and `viewBox` in `doc_unit`.

#### Stage 3 & Stage 4: Full-AABB Pagination, Vector Digits & Layer Separation

1.  **Shared Slot Sizing & Page Capacity:**
    *   Compute unified symmetric cut-centered slot dimensions across all `cut_polygon` and `bleed_polygon` vertices (`slot_w_mm`, `slot_h_mm`), reserving `6.0 mm` top and bottom corner registration bands inside page margins (`usable_w = page_width_mm - 2.0 * margin_mm`, `usable_h = page_height_mm - 2.0 * margin_mm - 2.0 * 6.0`).
    *   Compute `page_cols` and `page_rows` with `piece_spacing_mm` gaps and fail before writing outputs if `page_cols < 1 || page_rows < 1`, reporting `slot_w x slot_h`, `usable_w x usable_h`, and the minimum `page_size` needed for one piece.
2.  **Slot Allocation & Corner Registration Anchors:**
    *   Assign pieces in row-major puzzle order into `(margin_mm, margin_mm + 6.0)`-origin sheet grids, translating each piece's `cut_aabb.center()` onto its slot center.
    *   Place 1-based single-stroke vector page numbers (`1`, `2`, ...) with baseline bars and right-angle L-shaped corner registration ticks strictly inside the top-left (`[margin_mm, margin_mm + 6.0]`) and bottom-right (`[page_height_mm - margin_mm - 6.0, page_height_mm - margin_mm]`) corner bands with non-overlapping clearance from both the L-tick arms and outer margins (even when `margin_mm == 0.0`).
    *   Verify at pagination time that all translated `bleed_polygon` and `cut_polygon` vertices remain inside `[margin_mm, page_width_mm - margin_mm] x [margin_mm + 6.0, page_height_mm - margin_mm - 6.0]` within tolerance, maintain `>= piece_spacing_mm` clearance, and preserve total piece count.
3.  **Stage 3 Multi-Page Inkscape Output (`<prefix>-stage3.svg`):**
    *   Project into `doc_unit` with `<sodipodi:namedview inkscape:document-units="...">` containing one `<inkscape:page>` per sheet placed side-by-side (`x = (p - 1) * page_w_doc`), root `<svg>` sized to page 1, and shared `<defs>` (`#puzzle-image` + `#puzzle-pattern`).
    *   Place translated `bleed_polygon` `<path id="piece-r{row}-c{col}">` elements (`fill: url(#puzzle-pattern); stroke: none;`) inside `<g inkscape:groupmode="layer" inkscape:label="PrintPieces">` and horizontally offset page numbers + L-ticks (`fill: none; stroke: #000000; stroke-width: <0.1mm in doc_unit>`) inside `<g inkscape:groupmode="layer" inkscape:label="PageNumbers">`.
4.  **Stage 4 Standalone Cut Outputs (`<prefix>-stage4-page<N>.svg`):**
    *   Project into `Config.page_unit` (default `mm`) with `<sodipodi:namedview inkscape:document-units="...">` and root `<svg>` sized to `page_w_out x page_h_out`, omitting `<defs>`, `<image>`, `<pattern>`, and all `transform` attributes.
    *   Bake page-local translations directly into ungrouped `<path d="...">` coordinates across two separated layers: `Cut` (`cut_polygon`, `fill: none; stroke: #000000; stroke-width: <0.1mm in page_unit>`) and `PageNumbers` (identical top-left and bottom-right vector page numbers and L-ticks, `fill: none; stroke: <score_color>; stroke-width: <0.1mm in page_unit>`).

--------------------------------------------------------------------------------

## Alternatives Considered

### Alternative A: Arbitrary Non-Rectangular or Freeform Piece Polygons in `v1`

*   **Pros:** Supports irregular Voronoi, spiral, or hand-drawn puzzle layouts beyond rectilinear grids.
*   **Cons:** Requires planar graph face extraction, arbitrary-valence half-edge healing, and irregular bin packing instead of row-major slot grids.
*   **Reason for Rejection:** Restricting `v1` inputs to rectilinear `<rect>` grids bounds the geometry problem to predictable edge-to-edge tab matching and grid pagination.

### Alternative B: Spacing Stage 2 Pieces by Nominal Grid Pitch + `1 kerf` (`0.18 mm`)

*   **Pros:** Packs pieces closer together horizontally and vertically than full bounding-box separation.
*   **Cons:** Protruding undercut tab bulbs exceed `0.18 mm` overhang depth relative to adjacent sockets.
*   **Reason for Rejection:** Spacing by full post-offset bounding boxes prevents neighboring cuts from intersecting undercut tab necks.

### Alternative C: Pairwise Edge-Overhang or Irregular S-Curve Nesting in Stages 2–4

*   **Pros:** Reduces sheet waste slightly by nesting tabs into neighboring sockets.
*   **Cons:** Adds non-convex pairwise clearance checks and asymmetric margin bounds for small per-sheet savings on custom runs.
*   **Reason for Rejection:** Uniform symmetric AABB slots guarantee collision-free margins and keep piece spacing predictable.

### Alternative D: Omitting Intermediate Stage 1 and Stage 2 SVG Files

*   **Pros:** Writes fewer output files on disk.
*   **Cons:** Hides whether a visual or geometry issue originated during tab splicing, flattening, or kerf offsetting.
*   **Reason for Rejection:** Writing all four inspectable stage SVGs from the in-memory model exposes intermediate geometry directly for visual inspection.

### Alternative E: Coupling Stage 3 Print Fills to Stage 2/4 `kerf / 2` Cut Offsets

*   **Pros:** Reuses a single buffered polygon contour for both print fills and laser cut paths.
*   **Cons:** `kerf / 2` (`0.09 mm`) gives zero extra margin beyond the cut kerf when manually aligning printed label sheets on a laser bed.
*   **Reason for Rejection:** Keeping `print_bleed` independent (default `1 kerf` = `0.18 mm`) gives `print_bleed - kerf / 2` (`0.09 mm` default) print-to-cut registration headroom.

### Alternative F: Splitting Stage 3 Into Separate Per-Page Print SVGs

*   **Pros:** Lets single-page viewers display individual print sheets directly.
*   **Cons:** Requires printing each sheet individually instead of exporting one multi-page PDF.
*   **Reason for Rejection:** Inkscape `<inkscape:page>` natively exports one multi-page PDF for batch label printing.

### Alternative G: Naming the Curve-Flattening Parameter `polyline_density` or Defaulting It to `0.1 mm`

*   **Pros:** Uses a slightly tighter default chord step (`0.1 mm`).
*   **Cons:** `polyline_density` is ambiguous regarding direction (larger = finer or coarser), and smaller chords increase output path point count proportionally to `1 / segment_length`.
*   **Reason for Rejection:** `segment_length` makes units explicit (`mm` or `kerf`), and defaulting to `"1 kerf"` (`0.18 mm`) scales polyline density with laser kerf.

### Alternative H: Retaining `<defs>`, `<image>`, or `<pattern>` Elements in Stage 4 Laser Cut Files

*   **Pros:** Keeps Stage 4 SVG structure parallel to Stages 1–3.
*   **Cons:** Retains large raster payloads and transform trees that some laser CAM importers mis-scale, reject, or interpret as raster engraving jobs.
*   **Reason for Rejection:** Stripping patterns, images, and `transform` attributes leaves only ungrouped cut paths and scoreable vector digits for CAM import.

### Alternative I: Fitting Cubic Beziers to Offset Contours or Using System Fonts for Page Numbers

*   **Pros:** Emits fewer SVG path segments on offset boundaries and uses `<text>` elements for page numbers.
*   **Cons:** Post-offset Bezier refitting risks curvature overshoot near sharp socket necks, and `<text>` page numbers depend on installed fonts or manual text-to-path conversion before laser cutting.
*   **Reason for Rejection:** Polyline-flattened contours and built-in vector glyphs avoid refitting errors and font dependencies on Stage 4 sheets.

### Alternative K: Reserving Two Full Piece Slots per Page for Corner Page Numbers

*   **Pros:** Treats page numbers as regular slot occupants inside the piece grid.
*   **Cons:** Wastes two full piece-sized cells on every sheet and blocks printing on sheets where only 1 or 2 pieces fit (`page_cols * page_rows < 3`).
*   **Reason for Rejection:** Fixed full-width `6.0 mm` top-left and bottom-right corner bands trade a uniform `12.0 mm` vertical margin reduction for simple row-major grid pagination that works down to `page_cols * page_rows >= 1`.

--------------------------------------------------------------------------------

## Non-Functional Requirements

*   **Raster Footprint Efficiency:** Keep the base64 `<image>` byte payload inside `<defs>` once per Stage 1–3 document via `<use>` references, omit `<defs>`, `<image>`, and `<pattern>` from Stage 4 cut sheets, and emit SVG XML lazily per stage so raster payloads are not duplicated simultaneously in memory.
*   **Numerical Determinism & Precision:** Execute all geometry in canonical `mm` and serialize coordinates at fixed precision (`6` decimal places for `in`/`cm`, `4` decimal places for `mm`/`px`) so Stage 3 (`<g transform>` + `<path d>`) and Stage 4 (baked `<path d>`) physical page coordinates agree within `<= 1.5e-4 mm`.
*   **Fail-Fast Validation & Atomic Per-File Writes:** Validate inputs, bounds, and pagination before writing outputs; refuse collisions unless `--overwrite` is enabled; write each output through a temporary file in `--output-dir` before persisting; and prune stale Stage 4 pages only after all new writes succeed (noting multi-file batch updates are per-file atomic rather than cross-file transactional).

## Risks & Limits

*   **Local-Trust Input Scope & Security Boundaries:** `puzzle-gen` is a single-user local CLI designed to process author-supplied Inkscape SVGs and local YAML configs rather than untrusted remote uploads. To bound resource use and prevent non-raster injection on local inputs, `puzzle-gen` enforces `data:image/(png|jpeg|webp|gif);base64,` URI prefixes on `<image>`, rejects finite SVG coordinates exceeding `10,000 mm`, enforces `<g>` recursion depth `<= 64`, enforces `segment_length_mm >= 0.02 mm`, relies on `serde-saphyr`'s built-in YAML parsing budgets, validates `--prefix` against path separators or traversal (`""`, `.`, `..`, `/`, `\`), and writes atomic temp files via `tempfile::NamedTempFile::new_in`.
*   **Socket-Mouth Bridging Under Extreme Kerf or Bleed:** Because outward buffering widens protrusions and narrows concave openings, extreme `kerf` or `print_bleed` values on narrow concave socket mouths can bridge across the socket entrance and create an internal enclosed hole (`shapes[0].len() > 1`) or collapse the boundary. Stage 2 rejects any hole or split on `cut_polygon` naming `(row, col)` and `id`, while retaining the outer boundary contour containing the piece interior on `bleed_polygon` so large print bleed can bridge empty sockets cleanly without failing.
*   **Imprecise Node Snapping & Handle Overshoot in Input SVGs:** Hand-drawn pieces and tabs can have slight misalignments or Bezier handles crossing straight edges. Bounded early snapping (`1.0 mm` `<rect>` clustering and `2.0 mm` joint endpoint-pair matching with tangency-preserving handle translation) combined with multi-edge ambiguity rejection and Stage 2 `FillRule::Positive` self-union healing repairs small authoring misalignments within those tolerances while failing early on ambiguous geometry.
*   **Zero-Offset Edge Cases:** Setting `kerf: "0mm"` or `print_bleed: "0mm"` yields zero offset delta. Short-circuiting `0.0` offsets directly to the healed unoffset contour avoids fragile zero-radius polygon offsetting and false area-growth errors.
*   **Overwrites & Laser CAM Compatibility:** Broad wildcard deletion can touch unintended user files, and `<defs>`, `<image>`, `<pattern>`, or nested `transform` attributes can cause scaling or import errors in laser CAM software. Matching Stage 4 pages via exact base-10 page suffixes, writing via `NamedTempFile` before pruning stale pages, and emitting ungrouped, transform-free Stage 4 paths mitigates file clobbering and known SVG CAM import hazards.

--------------------------------------------------------------------------------

## Additional Resources

*   [rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg](../../src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg): Inkscape Stage 0 reference file (`4 in x 4 in` grid, 25 `<rect>` pieces, 39 open tab `<path>` elements numbered `path1`–`path40` with `path19` absent from XML, leaving internal edge `((2, 0), (3, 0))` along `y = 2.4in, x in [0, 0.8in]` without a tab, plus an embedded PNG on `Image`).
*   [rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg](../../src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg): Partial proof-of-concept showing `6 in x 6 in` `<inkscape:page>` layout, a pattern-filled piece (`rect1`, alongside unsplit background `rect17` in `Layer 1`), and incomplete manual path joins across the remaining `fill: none` pieces.

## Future Improvements

*   **Pairwise Edge-Overhang Nesting:** Replace uniform full-AABB slots with alternating row/column nesting while keeping clearance invariants intact.
*   **Non-Rectangular Piece Topologies:** Support Voronoi and arbitrary closed piece polygons beyond rectilinear `<rect>` grids.
*   **External Image Links & Offset Bezier Refitting:** Support file-linked raster images in addition to `data:` URIs and add tolerance-bounded cubic Bezier refitting for offset cut contours.

--------------------------------------------------------------------------------

## Detailed Implementation Plan

### Target Files to Create/Modify

Files are organized into three incremental phases so `cargo check` and `cargo test` run and pass after every phase.

#### Phase 1: Crate Manifest, Domain Model, Configuration & Library Root

##### 1. `src/puzzle-gen/Cargo.toml` (Modify)
*   **Type / Scope:** Crate Manifest (Cargo Configuration)
*   **Proposed Changes:**
    *   Add dependencies:
        *   `clap = { version = "4.5", features = ["derive"] }`
        *   `roxmltree = "0.20"`
        *   `svgtypes = "0.15"`
        *   `xmlwriter = "0.1"`
        *   `serde = { version = "1.0", features = ["derive"] }`
        *   `serde-saphyr = "2.1"`
        *   `kurbo = "0.11"`
        *   `i_overlay = "9"`
        *   `tempfile = "3.10"` (used in `[dependencies]` for atomic `NamedTempFile` writes as well as integration tests).

##### 2. `Cargo.lock` (Modify)
*   **Type / Scope:** Workspace Lockfile
*   **Proposed Changes:**
    *   Update root `Cargo.lock` when resolving `src/puzzle-gen/Cargo.toml` dependencies.

##### 3. `src/puzzle-gen/src/model.rs` (Create)
*   **Type / Scope:** Rust Domain Model & Error Types
*   **Proposed Changes:**
    *   Define `LengthUnit { Mm, Cm, In, Px }` with `to_mm(self, val: f64) -> f64`, `from_mm(self, mm: f64) -> f64` (`25.4 / 96.0` mm/px), `as_str(self) -> &'static str`, and `decimal_places(self) -> usize` (`6` for `In`/`Cm`, `4` for `Mm`/`Px`).
    *   Define `CoordFrame { pub doc_unit: LengthUnit, pub viewbox_min_user: (f64, f64), pub user_to_mm: (f64, f64), pub doc_to_mm: f64 }`.
    *   Define `Config { pub page_width_mm: f64, pub page_height_mm: f64, pub page_unit: LengthUnit, pub margin_mm: f64, pub kerf_mm: f64, pub segment_length_mm: f64, pub piece_spacing_mm: f64, pub print_bleed_mm: f64, pub score_color: String }` implementing `Default` (`6in x 6in`, `Mm`, `0.25in`, `0.18mm`, `0.18mm`, `0.18mm`, `0.18mm`, `"#0000ff"`).
    *   Define Stage 0–4 structs using `kurbo` primitives:
        *   `GridCell { pub row: usize, pub col: usize, pub id: String, pub bounds_mm: kurbo::Rect }`
        *   `Orientation { Horizontal, Vertical }`
        *   `InternalEdge { pub orientation: Orientation, pub coord_mm: f64, pub start_mm: f64, pub end_mm: f64, pub cell_a: (usize, usize), pub cell_b: (usize, usize) }`
        *   `TabCandidate { pub id: String, pub path_mm: kurbo::BezPath }`
        *   `IngestedPuzzle { pub frame: CoordFrame, pub image_href: String, pub image_bounds_mm: kurbo::Rect, pub image_preserve_aspect_ratio: String, pub rows: usize, pub cols: usize, pub x_lines_mm: Vec<f64>, pub y_lines_mm: Vec<f64>, pub cells: Vec<GridCell>, pub tabs: Vec<TabCandidate> }`
        *   `SplicedPiece { pub row: usize, pub col: usize, pub id: String, pub path_mm: kurbo::BezPath }`
        *   `SplicedPuzzle { pub frame: CoordFrame, pub image_href: String, pub image_bounds_mm: kurbo::Rect, pub image_preserve_aspect_ratio: String, pub rows: usize, pub cols: usize, pub matched_tabs: usize, pub missing_tabs: usize, pub pieces: Vec<SplicedPiece> }`
        *   `KerfedPiece { pub row: usize, pub col: usize, pub id: String, pub cut_aabb_mm: kurbo::Rect, pub cut_polygon: Vec<kurbo::Point>, pub bleed_polygon: Vec<kurbo::Point> }`
        *   `KerfedGrid { pub frame: CoordFrame, pub image_href: String, pub image_bounds_mm: kurbo::Rect, pub image_preserve_aspect_ratio: String, pub rows: usize, pub cols: usize, pub matched_tabs: usize, pub missing_tabs: usize, pub pieces: Vec<KerfedPiece> }`
        *   `CornerAnchor { TopLeft, BottomRight }`, `PageSlot { pub piece_index: usize, pub row: usize, pub col: usize, pub offset_mm: kurbo::Vec2 }`, `PagePlan { pub page_number: usize, pub slots: Vec<PageSlot>, pub corner_strokes_mm: Vec<Vec<kurbo::Point>> }`, `PaginatedLayout { pub slot_w_mm: f64, pub slot_h_mm: f64, pub page_cols: usize, pub page_rows: usize, pub pages: Vec<PagePlan> }`
        *   `PipelineArtifacts { pub spliced: SplicedPuzzle, pub kerfed: KerfedGrid, pub layout: PaginatedLayout }`
        *   `NoticeSeverity { Info, Warning }`, `PipelineNotice { pub severity: NoticeSeverity, pub code: &'static str, pub message: String }`
        *   `PipelineError { pub stage: &'static str, pub message: String, pub debug_spliced: Option<SplicedPuzzle>, pub debug_kerfed: Option<KerfedGrid> }` implementing `Display` and `std::error::Error`.

##### 4. `src/puzzle-gen/src/config.rs` (Create)
*   **Type / Scope:** Configuration (`serde-saphyr`) & `CoordFrame` Module
*   **Proposed Changes:**
    *   Define `#[derive(Debug, Default, Deserialize)] #[serde(deny_unknown_fields, default)] struct RawConfig` with `version: Option<i64>`, `page_size: Option<String>`, `page_unit: Option<String>`, `margin: Option<String>`, `kerf: Option<String>`, `segment_length: Option<String>`, `piece_spacing: Option<String>`, `print_bleed: Option<String>`, and `score_color: Option<Option<String>>` using `#[serde(deserialize_with = "deserialize_present_nullable_string")]` so explicit YAML `null` (`Some(None)`) is distinguished from an omitted key (`None`).
    *   Implement `Config::from_yaml(yaml: Option<&str>) -> Result<Config, PipelineError>` using `serde_saphyr::from_str::<Option<RawConfig>>(s)`:
        *   Return `Config::default()` when `yaml` is `None` or deserializes to `None` (empty/comment-only input).
        *   `serde-saphyr` rejects duplicate keys and multi-document streams by default, and `#[serde(deny_unknown_fields)]` rejects unknown top-level keys.
        *   Validate `version == 1` if supplied; if `score_color` is `Some(None)` or `Some(Some(s))` with `s.trim().is_empty()`, return an error advising the author to quote `"#RRGGBB"`.
    *   Implement `parse_scalar_length`, `parse_kerf_relative_length`, `parse_page_size`, `parse_page_unit`, and `parse_score_color`:
        *   Reject unitless numbers, `%`, scientific notation (`e`/`E`), `NaN`, and infinities. Match units case-insensitively (`mm`, `cm`, `in`, `px`) and allow optional whitespace (`0.18mm`, `0.18 mm`, `1kerf`, `1 kerf`).
        *   Validate `page_width_mm > 0.0`, `page_height_mm > 0.0`, `margin_mm >= 0.0`, `kerf_mm >= 0.0`, `piece_spacing_mm >= 0.0`, `print_bleed_mm >= 0.0`, and `segment_length_mm >= 0.02`. Reject `kerf_mm == 0.0` paired with a `kerf`-relative `segment_length` (yielding `0.0 mm`) with an error naming both `kerf` and `segment_length`.
    *   Implement `CoordFrame::from_svg_root(doc: &roxmltree::Document) -> Result<CoordFrame, PipelineError>`:
        *   Resolve `doc_unit` from `<sodipodi:namedview inkscape:document-units>`, falling back to `<svg inkscape:document-units>`, then `<svg>` `width`/`height` units (defaulting unitless dimensions to CSS `px` at `96 DPI`, and failing if `inkscape:document-units` is absent and `width` vs. `height` declare conflicting explicit units).
        *   Reject missing, `%`, or `<= 0.0` `<svg>` `width`, `height`, or `viewBox` dimensions; parse `viewBox` (defaulting to `(0.0, 0.0)` and CSS `px` dimensions); compute `user_to_mm` and `doc_to_mm`; reject aspect mismatch `> 1e-4`.
    *   Add Phase 1 unit tests (`#[cfg(test)]`) covering length grammar (case-insensitivity, whitespace, scientific notation/`NaN` rejection), `kerf` multipliers, `kerf: "0mm"` + `"1 kerf"` `segment_length`, `segment_length < 0.02 mm` rejection, duplicate/unknown/comment-stripped YAML errors, and `CoordFrame` unit resolution & aspect checks.

##### 5. `src/puzzle-gen/src/lib.rs` (Create)
*   **Type / Scope:** Rust Library Root (Phase 1 Incremental Exports)
*   **Proposed Changes:**
    *   Export `pub mod config; pub mod model;` so `cargo check` and `cargo test -p puzzle-gen --lib` compile and execute Phase 1 tests cleanly.

#### Phase 2: Stage 0–2 Ingestion, Bounded Early Snapping & Buffering

##### 6. `src/puzzle-gen/src/ingest.rs` (Create)
*   **Type / Scope:** Rust Stage 0 SVG Ingestion Module
*   **Proposed Changes:**
    *   Implement `ingest_svg(svg_xml: &str, notices: &mut Vec<PipelineNotice>) -> Result<IngestedPuzzle, PipelineError>` using `roxmltree` and `svgtypes`:
        *   Locate `<g inkscape:groupmode="layer">` groups by `inkscape:label` (`Image`, `Pieces`, `Tabs`); fail on missing or duplicate labels; ignore other guide layers.
        *   Walk each required layer recursively up to nesting depth `64` (failing if depth exceeds `64`), composing ancestor `transform` chains followed by `(p_user - viewbox_min_user) * user_to_mm` into `mm`. Allow nested `<g>` and `<title>`/`<desc>`, and enforce per-layer tag allowlists (`<image>` in `Image`, `<rect>` in `Pieces`, `<path>` in `Tabs`), rejecting any other element type citing tag name and `id`. Reject any transformed finite coordinate exceeding `|coord| > 10_000.0 mm`.
        *   Validate the single `<image>` (`href` or `xlink:href`, requiring `data:image/(png|jpeg|webp|gif);base64,` prefix), retain `preserveAspectRatio` (default `"none"` when omitted), reject rotated/skewed/negatively-scaled `<image>` transforms citing `id`, and emit a `Warning` notice if `image_bounds_mm` does not cover the full `Pieces` grid bounds.
        *   Extract `<rect>` elements from `Pieces` (supporting `LengthUnit` attributes, defaulting `x`/`y` to `0.0`), reject rounded corners (`rx > 1e-4 mm || ry > 1e-4 mm`), verify orthogonality in canonical `mm` on transformed axis edge vectors `v_x, v_y` (`|v_x.y| / ||v_x|| <= 1e-4 && |v_y.x| / ||v_y|| <= 1e-4`), and compute `bounds_mm` via corner `min`/`max` (supporting negative scales uniformly).
        *   Cluster `<rect>` edge coordinates using bounded-diameter `<= 1.0 mm` clustering (sort unique coordinates, open a new cluster whenever `val - cluster_min > 1.0 mm`, replace cluster coordinates with their arithmetic mean). Sort into `(row, col)`, confirm contiguous `rows x cols` coverage (`rows >= 1 && cols >= 1 && cells.len() == rows * cols`), and verify every cell occupies exactly one unique `[x_lines[col], x_lines[col + 1]] x [y_lines[row], y_lines[row + 1]]` slot.
        *   Parse `Tabs` `<path>` `d` strings via `svgtypes::SimplifyingPathParser` (which handles `M`/`m`, `L`/`l`, `H`/`h`, `V`/`v`, `C`/`c`, `S`/`s`, `Q`/`q`, `T`/`t`, `A`/`a`, `Z`/`z` and lowers elliptical arcs directly into `CurveTo` cubics). Transform coordinates to `mm`, elevate `SimplePathSegment::Quadratic` via **`kurbo::QuadBez::to_cubic()`**, and filter sub-`0.05 mm` stray paths (`aabb_diag < 0.05 mm && arc_len < 0.05 mm`, emitting an `Info` notice with `id`) *before* rejecting retained closed (`Z`) or multi-subpath (`> 1` `M`) elements.

##### 7. `src/puzzle-gen/src/stage1.rs` (Create)
*   **Type / Scope:** Rust Stage 1 Edge Matching & Splicing Module
*   **Proposed Changes:**
    *   Implement `splice_tabs(puzzle: &IngestedPuzzle, strict: bool, notices: &mut Vec<PipelineNotice>) -> Result<SplicedPuzzle, PipelineError>`:
        *   Build canonical horizontal edges (`((row, col), (row + 1, col))`, `L→R`, `t in [0.0, x_lines[col + 1] - x_lines[col]]`, `cell_a = (row, col)`, `cell_b = (row + 1, col)`) and vertical edges (`((row, col), (row, col + 1))`, `T→B`, `t in [0.0, y_lines[row + 1] - y_lines[row]]`, `cell_a = (row, col)`, `cell_b = (row, col + 1)`), plus outer-border reference edges for border-attachment rejection.
        *   Evaluate each `TabCandidate` jointly by its `(start, end)` endpoint pair against all internal and border edges: candidate qualifies when both endpoints satisfy `perp_dist <= 2.0 mm`, `t` inside `[-2.0 mm, edge_len + 2.0 mm]`, and clamped span `|clamp(t_end, 0.0, edge_len) - clamp(t_start, 0.0, edge_len)| >= 1.0 mm`.
        *   Reject tabs qualifying on `> 1` grid edge (resolving 4-way corner and narrow-cell parallel ambiguity without banning valid corner-adjacent tabs), tabs matching outer border edges, unmatched tabs, duplicate matches on one edge, or tabs whose endpoint overshoots past `0.25 mm` beyond `[0.0, edge_len]` when adjacent to another edge.
        *   Project matched endpoints orthogonally onto `[0.0, edge_len]` (`t = clamp(t, 0.0, edge_len)`), update `start`/`end` to the snapped coordinates while translating adjacent cubic control handles (`c1` / `c2`) by the same snap vector to preserve endpoint tangency, and reverse the `kurbo::BezPath` when `t_end < t_start` so canonical `t_start < t_end`.
        *   Fail when `strict == true` if any internal edge has no tab; emit a `Warning` notice and leave a straight line when `strict == false`.
        *   Assemble clockwise `kurbo::BezPath` contours (`top` L→R, `right` T→B, `bottom` R→L, `left` B→T): emit `top` and `right` internal edges **forward** (`t = 0.0 -> edge_len`) and `bottom` and `left` internal edges **reversed** (`t = edge_len -> 0.0`). On each slotted tab edge (`slot_start -> slot_end` with directed tab `(tab_entry, tab_exit)`), emit `LineTo(tab_entry)` if entry lead length `> 1e-4 mm` (else snap the first tab segment start directly to `slot_start`), emit the normalized tab segments (`LineTo` and `CurveTo`), and emit `LineTo(slot_end)` if exit lead length `> 1e-4 mm` (else snap the last tab segment end directly to `slot_end`).

##### 8. `src/puzzle-gen/src/stage2.rs` (Create)
*   **Type / Scope:** Rust Stage 2 Flattening, Healing & Polygon Offsetting Module
*   **Proposed Changes:**
    *   Implement `heal_and_offset(spliced: &SplicedPuzzle, config: &Config, notices: &mut Vec<PipelineNotice>) -> Result<KerfedGrid, PipelineError>`:
        *   Preserve straight `LineTo` segments as two-point lines; flatten curved segments using `kurbo::flatten` with tolerance `(config.segment_length_mm * 0.1).min(0.05)` and uniformly subdivide any curved chord exceeding `config.segment_length_mm`. Deduplicate consecutive points closer than `0.001 mm` and normalize closed rings to positive signed `(x, y)` area (`+1` winding in SVG `+y`-down coordinates).
        *   Run `i_overlay` floating-point self-union (`SingleFloatOverlay::overlay` with `OverlayRule::Subject`, `FillRule::Positive`) on each ring before measuring `unoffset_area`, discarding `-1` exterior cross-edge overshoot ears while verifying `shapes.len() == 1 && shapes[0].len() == 1`.
        *   Short-circuit `0.0` offsets (`kerf_mm == 0.0` for `cut_polygon`; `print_bleed_mm == 0.0` for `bleed_polygon`) directly to a clone of the healed unoffset ring.
        *   For positive offsets, buffer outward in `mm` via `i_overlay` floating-point `OutlineOffset::outline(&OutlineStyle::new(offset_mm).line_join(LineJoin::Round(0.1)))`, then run closed-ring Ramer-Douglas-Peucker (`RDP`) simplification (splitting at the farthest antipodal vertex pair before running standard `RDP` with tolerance `min(0.005, config.segment_length_mm * 0.05) mm`).
        *   Validate `cut_polygon` has `shapes.len() == 1 && shapes[0].len() == 1` and `area > unoffset_area` when `kerf_mm > 0.0`. On `bleed_polygon`, keep the outer contour (`shape[0]`) of the positive-area shape containing a sample interior point of the healed unoffset polygon (allowing socket-bridging internal holes `shape[1..]` to drop cleanly) and verify `area > unoffset_area` when `print_bleed_mm > 0.0`; emit a `Warning` notice if `config.print_bleed_mm < config.kerf_mm / 2.0`. Fail on socket collapse naming piece `(row, col)` and `id`.
    *   Add Phase 2 unit tests (`#[cfg(test)]`) covering `ingest`, `stage1`, and `stage2`: `1.0 mm` bounded-diameter clustering, `mm`-space `<rect>` rotation rejection & `1 x 1` cell overlap checks, `QuadBez::to_cubic()`, `2.0 mm` endpoint snapping with handle translation, multi-edge corner ambiguity rejection, clockwise horizontal/vertical edge splicing with elided `<= 1e-4 mm` leads, `FillRule::Positive` healing of cross-edge overshoot ears, `OutlineOffset` `LineJoin::Round`, zero-offset short-circuiting, `print_bleed < kerf / 2` warning, and `kurbo::flatten` curve bounds.

##### 9. `src/puzzle-gen/src/lib.rs` (Modify)
*   **Type / Scope:** Rust Library Root (Phase 2 Incremental Exports)
*   **Proposed Changes:**
    *   Append `pub mod ingest; pub mod stage1; pub mod stage2;` so `cargo check` and `cargo test -p puzzle-gen --lib` compile and run Phase 1 and Phase 2 unit tests.

#### Phase 3: Pagination, SVG Serialization, CLI & Integration Suite

##### 10. `src/puzzle-gen/src/paginate.rs` (Create)
*   **Type / Scope:** Rust Pagination & Vector Registration Digit Module
*   **Proposed Changes:**
    *   Define static `[0, 1] x [0, 1]` single-stroke polyline glyph tables for `'0'..='9'`:
        *   `'0'`: `&[&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)]]`
        *   `'1'`: `&[&[(0.2, 0.25), (0.5, 0.0), (0.5, 1.0)], &[(0.15, 1.0), (0.85, 1.0)]]`
        *   `'2'`: `&[&[(0.0, 0.2), (0.2, 0.0), (0.8, 0.0), (1.0, 0.2), (1.0, 0.4), (0.0, 1.0), (1.0, 1.0)]]`
        *   `'3'`: `&[&[(0.0, 0.15), (0.2, 0.0), (0.8, 0.0), (1.0, 0.2), (0.8, 0.45), (0.35, 0.45)], &[(0.8, 0.45), (1.0, 0.7), (0.8, 1.0), (0.2, 1.0), (0.0, 0.85)]]`
        *   `'4'`: `&[&[(0.8, 1.0), (0.8, 0.0), (0.0, 0.65), (1.0, 0.65)]]`
        *   `'5'`: `&[&[(1.0, 0.0), (0.0, 0.0), (0.0, 0.45), (0.8, 0.45), (1.0, 0.65), (1.0, 0.8), (0.8, 1.0), (0.2, 1.0), (0.0, 0.85)]]`
        *   `'6'`: `&[&[(0.9, 0.0), (0.2, 0.0), (0.0, 0.2), (0.0, 0.85), (0.2, 1.0), (0.8, 1.0), (1.0, 0.85), (1.0, 0.55), (0.8, 0.45), (0.0, 0.45)]]`
        *   `'7'`: `&[&[(0.0, 0.0), (1.0, 0.0), (0.35, 1.0)]]`
        *   `'8'`: `&[&[(0.2, 0.45), (0.0, 0.25), (0.0, 0.15), (0.2, 0.0), (0.8, 0.0), (1.0, 0.15), (1.0, 0.25), (0.8, 0.45), (0.2, 0.45), (0.0, 0.65), (0.0, 0.85), (0.2, 1.0), (0.8, 1.0), (1.0, 0.85), (1.0, 0.65), (0.8, 0.45)]]`
        *   `'9'`: `&[&[(0.1, 1.0), (0.8, 1.0), (1.0, 0.8), (1.0, 0.15), (0.8, 0.0), (0.2, 0.0), (0.0, 0.15), (0.0, 0.45), (0.2, 0.55), (1.0, 0.55)]]`
    *   Implement `layout_corner_page_number(page_number: usize, page_width_mm: f64, page_height_mm: f64, margin_mm: f64, corner: CornerAnchor) -> Vec<Vec<kurbo::Point>>`:
        *   Use `digit_h = 4.0 mm`, `digit_w = 2.4 mm` (`0.6 * digit_h`), `digit_gap = 0.48 mm` (`0.2 * digit_w`), `num_w = D * digit_w + (D - 1) * digit_gap`, and a horizontal baseline bar at `y = origin_y + digit_h + 0.6` spanning `[origin_x, origin_x + num_w]`.
        *   `CornerAnchor::TopLeft`: right-angle L-tick `[(margin_mm, margin_mm + 2.5), (margin_mm, margin_mm), (margin_mm + 2.5, margin_mm)]`; digits anchored at `origin_x = margin_mm + 1.0`, `origin_y = margin_mm + 0.7` (occupying `y in [margin_mm + 0.7, margin_mm + 5.3]`, inside `[margin_mm, margin_mm + 6.0]` and clear of the L-tick).
        *   `CornerAnchor::BottomRight`: right-angle L-tick `[(page_width_mm - margin_mm - 2.5, page_height_mm - margin_mm), (page_width_mm - margin_mm, page_height_mm - margin_mm), (page_width_mm - margin_mm, page_height_mm - margin_mm - 2.5)]`; digits anchored at `origin_x = page_width_mm - margin_mm - 1.0 - num_w`, `origin_y = page_height_mm - margin_mm - 5.3` (occupying `y in [page_height_mm - margin_mm - 5.3, page_height_mm - margin_mm - 0.7]`, strictly inside `[page_height_mm - margin_mm - 6.0, page_height_mm - margin_mm]`, clear of the L-tick, and safe when `margin_mm == 0.0`).
    *   Implement shared helper `pub fn compute_slot_bounds(kerfed: &KerfedGrid) -> (f64, f64)` computing symmetric cut-centered `slot_w_mm` and `slot_h_mm` (`2.0 * max(|v - cut_center|)` across all `cut_polygon` and `bleed_polygon` vertices).
    *   Implement `paginate_pieces(kerfed: &KerfedGrid, config: &Config) -> Result<PaginatedLayout, PipelineError>`:
        *   Call `compute_slot_bounds(kerfed)`, reserve `6.0 mm` top/bottom corner bands (`usable_w = page_width_mm - 2.0 * margin_mm`, `usable_h = page_height_mm - 2.0 * margin_mm - 12.0`), and compute `page_cols` and `page_rows`.
        *   Require `page_cols >= 1 && page_rows >= 1`; otherwise fail stating `slot_w x slot_h`, `usable_w x usable_h`, and minimum `page_size` (`slot_w + 2 * margin_mm` by `slot_h + 2 * margin_mm + 12.0`).
        *   Assign slots in row-major order on `(margin_mm, margin_mm + 6.0)`-origin grids (`slot_center_mm = (margin_mm + col * (slot_w_mm + piece_spacing_mm) + slot_w_mm / 2.0, margin_mm + 6.0 + row * (slot_h_mm + piece_spacing_mm) + slot_h_mm / 2.0)`, `offset_mm = slot_center_mm - cut_aabb_mm.center()`) and verify piece count preservation, margin containment (`1e-4 mm` tolerance), and `>= piece_spacing_mm - 1e-4 mm` clearance.
    *   Add Phase 3 unit tests (`#[cfg(test)]`) verifying L-tick vs. digit non-overlap, corner-band containment under `margin: "0mm"`, multi-digit `num_w` scaling, `page_cols * page_rows < 1` diagnostics, and `compute_slot_bounds` symmetry.

##### 11. `src/puzzle-gen/src/svg_emit.rs` (Create)
*   **Type / Scope:** Rust Stage SVG Projection Module
*   **Proposed Changes:**
    *   Define fixed root `<svg>` namespaces (`xmlns="http://www.w3.org/2000/svg"`, `xmlns:svg="http://www.w3.org/2000/svg"`, `xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"`, `xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"`, and `xmlns:xlink="http://www.w3.org/1999/xlink"` on Stages 1–3; omit `xmlns:xlink` on Stage 4) and `fmt_coord(val, unit)` (`6` decimals for `In`/`Cm`, `4` for `Mm`/`Px`). Always emit explicit unit suffixes (`mm`, `cm`, `in`, `px`) on root `<svg>` `width` and `height` attributes alongside unitless `viewBox` coordinates in `doc_unit` (Stages 1–3) or `page_unit` (Stage 4).
    *   Implement `emit_stage1(spliced: &SplicedPuzzle) -> String`:
        *   Compute piece bounding box plus `label_margin_mm = 8.0 mm` padding on all sides for `viewBox`, `width`, and `height` in `doc_unit`; emit `<sodipodi:namedview inkscape:document-units="...">`.
        *   Inside `<defs>`, emit `<image id="puzzle-image" x="0" y="0" width="<W_doc>" height="<H_doc>" preserveAspectRatio="..." xlink:href="..." />` and `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse" x="<img_x_doc>" y="<img_y_doc>" width="<2W_doc>" height="<2H_doc>">` with four `<use xlink:href="#puzzle-image" />` tiles (`none`, `translate(2W, 0) scale(-1, 1)`, `translate(0, 2H) scale(1, -1)`, `translate(2W, 2H) scale(-1, -1)`).
        *   Emit `<g inkscape:groupmode="layer" id="layer-pieces" inkscape:label="Pieces">` containing `<path id="piece-r{row}-c{col}" style="fill:url(#puzzle-pattern);stroke:#000000;stroke-width:<0.1mm_doc>;fill-rule:nonzero" d="..." />`.
        *   Emit `<g inkscape:groupmode="layer" id="layer-grid-labels" inkscape:label="GridLabels">` placing `<text>` row labels (`0..rows-1`, `x = min_x_doc - 4.0mm_doc`, `y = row_center_doc`) and column labels (`0..cols-1`, `x = col_center_doc`, `y = min_y_doc - 4.0mm_doc`) styled `font-family:sans-serif;font-size:<3.5mm_doc>;fill:#000000;text-anchor:middle;dominant-baseline:central`.
    *   Implement `emit_stage2(kerfed: &KerfedGrid, config: &Config) -> String`:
        *   Call `paginate::compute_slot_bounds(kerfed)` to obtain `(slot_w_mm, slot_h_mm)`, space grid cells at pitch `(slot_w_mm + config.piece_spacing_mm, slot_h_mm + config.piece_spacing_mm)` starting from grid origin `(label_margin_mm, label_margin_mm)` (`cell_center_mm = (8.0 + col * pitch_w + slot_w_mm / 2.0, 8.0 + row * pitch_h + slot_h_mm / 2.0)`, `translate_mm = cell_center_mm - cut_aabb_mm.center()`), expand `width`/`height`/`viewBox` in `doc_unit` with `8.0 mm` bottom/right padding, emit shared `<defs>` pattern, wrap `<g transform="translate(dx_doc, dy_doc)"><path id="piece-r{row}-c{col}" ... /></g>` inside `Pieces` (`0.1 mm` `#000000` stroke, `url(#puzzle-pattern)` fill), and emit aligned `GridLabels`.
    *   Implement `emit_stage3(kerfed: &KerfedGrid, layout: &PaginatedLayout, config: &Config) -> String`:
        *   Project into `doc_unit`, emit `<sodipodi:namedview inkscape:document-units="...">` with `<inkscape:page x="(p-1)*page_w_doc" y="0" width="page_w_doc" height="page_h_doc" id="page<p>" />` and root `<svg>` sized to page 1, and emit `<defs>` (`#puzzle-image` + `#puzzle-pattern`).
        *   Emit `<g inkscape:groupmode="layer" id="layer-print-pieces" inkscape:label="PrintPieces">` with `<g transform="translate((p-1)*page_w_doc + offset_x_doc, offset_y_doc)"><path id="piece-r{row}-c{col}" style="fill:url(#puzzle-pattern);stroke:none" d="<bleed_polygon>" /></g>`.
        *   Emit `<g inkscape:groupmode="layer" id="layer-page-numbers" inkscape:label="PageNumbers">` with corner stroke `<path>`s translated horizontally by `(p - 1) * page_w_doc` (`fill:none;stroke:#000000;stroke-width:<0.1mm_doc>`).
    *   Implement `emit_stage4_page(kerfed: &KerfedGrid, page: &PagePlan, config: &Config) -> String`:
        *   Project into `config.page_unit`, emit `<sodipodi:namedview inkscape:document-units="...">`, size root `<svg>` to `page_w_out x page_h_out`, omit `<defs>`, `<image>`, `<pattern>`, and all `transform` attributes, and emit baked page-local `<path>` elements across `<g inkscape:groupmode="layer" id="layer-cut" inkscape:label="Cut">` (`id="piece-r{row}-c{col}"`, `fill:none;stroke:#000000;stroke-width:<0.1mm_page_unit>`) and `<g inkscape:groupmode="layer" id="layer-page-numbers" inkscape:label="PageNumbers">` (`fill:none;stroke:<config.score_color>;stroke-width:<0.1mm_page_unit>`).
    *   Add Phase 3 unit tests (`#[cfg(test)]`) verifying `<sodipodi:namedview>`, `<defs>` `xlink:href` structure, `Pieces`/`GridLabels`/`PrintPieces`/`Cut`/`PageNumbers` layers, Stage 3 `PageNumbers` `(p - 1) * page_w_doc` shifts, and absence of `<defs>`/`<image>`/`<pattern>`/`transform` in Stage 4.

##### 12. `src/puzzle-gen/src/lib.rs` (Modify)
*   **Type / Scope:** Rust Library Root (Final Phase 3 Exports & Pipeline Entrypoint)
*   **Proposed Changes:**
    *   Add `pub mod paginate; pub mod svg_emit;`.
    *   Provide `pub fn run_pipeline(svg_xml: &str, config: &Config, strict: bool, notices: &mut Vec<PipelineNotice>) -> Result<PipelineArtifacts, PipelineError>` running Stage 0 -> Stage 1 -> Stage 2 -> Pagination in memory, preserving any valid `SplicedPuzzle` / `KerfedGrid` inside `PipelineError` on downstream failure without eagerly serializing XML strings.

##### 13. `src/puzzle-gen/src/main.rs` (Modify)
*   **Type / Scope:** Rust CLI Entrypoint & File Lifecycle Manager
*   **Proposed Changes:**
    *   Parse CLI args via `clap`; validate `--prefix` (`!is_empty() && prefix != "." && prefix != ".." && !contains(['/', '\\'])`); read `--config` if passed and call `Config::from_yaml`.
    *   Call `puzzle_gen::run_pipeline(&svg_xml, &config, !cli.no_strict, &mut notices)` and print all accumulated `notices` to `stderr` on both success and failure.
    *   On error, if `--emit-debug-on-error` is enabled and `--dry-run` is false, serialize available `debug_spliced` (`stage1`) and `debug_kerfed` (`stage2`) payloads lazily, probe collisions only for those debug output paths (ignoring `ErrorKind::NotFound`), and write them without pruning Stage 4 files.
    *   Under `--dry-run`, print structured `key=value` summary lines (`grid=RxC`, `matched_tabs=M`, `missing_tabs=K`, `slot_mm=WxH`, `pages=P`, `page_grid=CxR`) to `stdout` without creating `--output-dir` or calling `svg_emit`.
    *   Otherwise scan `--output-dir` (ignoring `ErrorKind::NotFound`), detect existing Stage 4 files via pure `std` string prefix/suffix + base-10 non-zero-leading-digit matching (`<prefix>-stage4-page<K>.svg`), abort on collisions when `--overwrite` is false, create `--output-dir`, lazily call `svg_emit::emit_stage1..4` one stage at a time and persist each via `tempfile::NamedTempFile::new_in`, prune leftover `K > pages.len()` files under `--overwrite`, and print summary + written paths to `stdout`.

##### 14. `src/puzzle-gen/examples/default-config.yaml` (Create)
*   **Type / Scope:** Example YAML Configuration
*   **Proposed Changes:**
    *   Add a commented default configuration file (`version: 1`, `page_size: "6in x 6in"`, `page_unit: "mm"`, `margin: "0.25in"`, `kerf: "0.18mm"`, `segment_length: "1 kerf"`, `piece_spacing: "1 kerf"`, `print_bleed: "1 kerf"`, `score_color: "#0000ff"`).

##### 15. `src/puzzle-gen/tests/integration_pipeline.rs` (Create)
*   **Type / Scope:** Rust Integration & CLI Regression Suite
*   **Proposed Changes:**
    *   Test `rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg`: fail in strict mode citing missing internal edge `((2, 0), (3, 0))` along `y = 2.4in, x in [0, 0.8in]`, and pass under `--no-strict` emitting 1 missing-edge warning and 25 closed healed contours (confirming `FillRule::Positive` drops `path2`/`path10` exterior overshoot ears cleanly).
    *   Test synthetic complete 5x5 SVG across Stages 1–4: verify `<sodipodi:namedview>` units, `<image x="0" y="0">` + 4 `<use>` tiles with `xlink:href`, `preserveAspectRatio="none"`, `Pieces` and `GridLabels` layers, `PrintPieces` vs. `PageNumbers`, zero `<defs>`/`<image>`/`<pattern>`/`transform` on Stage 4, `Cut` vs. `PageNumbers` colors, unconditional `PageNumbers` `<path>` parity within `1.5e-4 mm` between Stage 3 and Stage 4, and piece `<path>` parity within `1.5e-4 mm` when `print_bleed == kerf / 2`.
    *   Test unit conversions (`mm`, `cm`, `in`, `px`) and `<N> kerf` expressions end to end through `run_pipeline` and Stage 4 outputs.
    *   Test bounded early snapping and edge cases: `1.0 mm` bounded-diameter `<rect>` perturbation, `2.0 mm` joint tab endpoint misalignment with handle translation, multi-edge ambiguity rejection vs. valid corner-adjacent full-edge tab, `Quadratic` (`to_cubic`) & `Arc` parsing, sub-`0.05 mm` stray paths, rotated `<rect>` / `<image>` rejection, `bleed_polygon` socket bridging, `print_bleed < kerf / 2` warning, zero `kerf` short-circuiting, and `serde-saphyr` duplicate-key, unknown-key, and unquoted `#RRGGBB` errors.
    *   Test CLI binary invocations in isolated `tempfile::tempdir()` temporary directories: `--dry-run`, `--output-dir`, `--prefix`, `--emit-debug-on-error` (including notice emission on failure and non-pruning of Stage 4 files), collision failure without `--overwrite`, write-before-prune stale page deletion with `--overwrite`, and `page_cols * page_rows < 1` error messages.

### Verification Plan

*   **Phase Checkpoints:** Run `cargo check -p puzzle-gen` and `cargo test -p puzzle-gen --lib` after Phase 1 (`config`, `model`), Phase 2 (`ingest`, `stage1`, `stage2`), and Phase 3 (`paginate`, `svg_emit`).
*   **Compilation/Build:** Run `cargo build -p puzzle-gen` and `cargo check --workspace` to confirm zero build errors across workspace members.
*   **Static Analysis & Formatting:** Run `cargo clippy -p puzzle-gen --all-targets -- -D warnings` and `cargo fmt -p puzzle-gen -- --check`.
*   **Integration Suite:** Run `cargo test -p puzzle-gen --test integration_pipeline` covering all four unit inputs (`mm`, `cm`, `in`, `px`), `<N> kerf` expressions, strict vs. `--no-strict`, collision vs. `--overwrite` stale-page pruning, `serde-saphyr` error branches, early snapping bounds, `FillRule::Positive` overshoot healing, and serialized Stage 3 vs. Stage 4 coordinate parity (`<= 1.5e-4 mm`).
*   **CLI Smoke Run:** Execute `cargo run -p puzzle-gen -- src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg --no-strict --overwrite --output-dir target/smoke-out` and assert exit code `0` with generated Stage 1–4 SVGs inside gitignored `target/smoke-out/`.
*   **Manual Inkscape Verification:** Open `target/smoke-out/rainy-day-puzzle-4x4-25piece-step0-piece-shape-stage3.svg` and `-stage4-page*.svg` in Inkscape and confirm:
    *   Stage 3 `<inkscape:page>` count equals the CLI summary `pages` count and the number of generated `-stage4-page*.svg` files; printed artwork fills align inside `PrintPieces` bleed contours without white gaps; and no `PrintPieces` bleed geometry extends past sheet margins or overlaps `#000000` corner numbers or L-ticks.
    *   Stage 4 opens in `mm` document units with transform-free paths split across `Cut` (`#000000`) and `PageNumbers` (`#0000ff`).

--------------------------------------------------------------------------------

*Drafted with Elephant-Goldfish Model (EGM)*
