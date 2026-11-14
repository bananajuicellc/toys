# `puzzle-gen`: Four-Stage Laser Jigsaw Puzzle Generator

*   **Author:** Tim Sweña (Swast)
*   **Status:** Draft
*   **Date:** 2026-11-13

--------------------------------------------------------------------------------

## Overview

`puzzle-gen` will be implemented as a local Rust CLI and geometric library in `src/puzzle-gen` (currently an empty 3-line `"Hello, world!"` binary stub inside a Cargo workspace alongside the sibling `src/sticker-size` crate, which keeps its own crate-scoped dependencies with no `[workspace.dependencies]`). The tool prepares custom jigsaw puzzle artwork drawn in Inkscape for physical fabrication: printing artwork onto adhesive label paper via Inkscape multi-page PDF export, adhering the sheet to backing board, and cutting individual pieces on a laser cutter.

Starting from an unjoined Stage 0 Inkscape SVG containing the required `Image`, `Pieces`, and `Tabs` layers—an embedded raster artwork (`Image`), a rectilinear grid of `<rect>` elements (`Pieces`), and open `<path>` tab curves along internal edges (`Tabs`)—plus an optional YAML configuration file, `puzzle-gen` runs an in-memory geometric pipeline and writes four sequential SVG outputs for inspection, printing, and laser cutting:

1.  **Stage 1 (`<prefix>-stage1.svg`):** Matches and splices each shared tab curve into its two adjacent piece boundaries (removing the replaced interior portion of the straight edge across each tab neck), fills every piece with a `2x2` mirrored `<pattern>` built from the `Image` layer (`stroke: #000000; stroke-width: 0.1mm` projected to document units), and renders row and column index `<text>` labels (`0..rows-1` and `0..cols-1`) in a `GridLabels` layer for inspection.
2.  **Stage 2 (`<prefix>-stage2.svg`):** Flattens piece curves into polylines controlled by `segment_length`, heals tab-neck self-intersections via `i_overlay` self-union, expands each piece outward by `kerf / 2` to compensate for laser beam width, spaces pieces apart on the grid using post-offset bounding boxes, keeps each piece's image pattern locked to its original artwork coordinates, and aligns row and column `<text>` labels in `GridLabels` along the spaced grid edges.
3.  **Stage 3 (`<prefix>-stage3.svg`):** Reflows pieces in row-major order across Inkscape multi-page `<inkscape:page>` sheets within configured margins for Inkscape PDF export. Pieces render with `stroke: none` and an outward `print_bleed` fill (defaulting to `1 kerf`), paired with stroked vector page numbers and L-shaped corner registration ticks (`fill: none; stroke: #000000; stroke-width: 0.1mm` projected to document units) inside top-left and bottom-right corner bands.
4.  **Stage 4 (`<prefix>-stage4-pageN.svg`):** Emits one standalone single-page SVG per page in `page_unit` (default `mm`) with page-local `kerf / 2` piece cut paths (`fill: none; stroke: #000000; stroke-width: 0.1mm`) and identical top-left and bottom-right corner page numbers (`fill: none; stroke: <score_color>; stroke-width: 0.1mm`) across separate `Cut` and `PageNumbers` layers, omitting all `<defs>`, `<pattern>`, and `<image>` elements.

## The Problem

Preparing a custom Inkscape jigsaw puzzle for print-and-cut laser fabrication involves several error-prone manual geometry steps:

*   **Manual path splicing:** Authoring a puzzle as a grid of `<rect>` elements plus separate open `<path>` curves for interlocking tabs is fast in Inkscape, but imprecise manual node snapping leaves small coordinate mismatches between rectangle boundaries and tab endpoints. Turning that sketch into closed pieces manually requires duplicating, reversing, splitting, and node-joining every shared internal curve (for example, 40 internal edges on a 5x5 puzzle). Leaving a straight rectangle segment across a tab neck causes the laser to cut the tab off.
*   **Image bleed and pattern drift:** Shifting pieces apart in Inkscape breaks `userSpaceOnUse` image patterns unless each piece carries a matching `patternTransform`, where minor typos (such as `0.01` vs. `1/96`) shift printed artwork relative to the cut line. Pieces expanded outward for kerf compensation or registration tolerance also need mirrored pixel bleed around border edges so white backing paper stays hidden as long as manual print-to-cut alignment stays within `print_bleed - kerf / 2` (`0.09mm` default).
*   **Kerf compensation and undercut collisions:** Because a laser beam vaporizes material along its cut path (for example, `0.18mm` wide), uncompensated pieces fit loosely. Offsetting each piece outward by `kerf / 2` widens tabs and narrows sockets, but pulling pieces apart by only `1 kerf` causes wide tab bulbs to cross adjacent socket necks during cutting. Pieces need full bounding-box separation plus spacing margin to avoid collisions.
*   **Print-to-cut registration:** Printing adhesive label sheets and cutting them later on a laser bed introduces small alignment offsets. Printing black piece outlines leaves visible dark ink lines on cut edges whenever registration drifts, whereas omitting strokes on print fills, applying decoupled print bleed, using matching margin bounds across sheets, and placing vector page numbers with corner alignment ticks keeps printing and cutting synchronized.

`puzzle-gen` automates all four stages from one deterministic in-memory pipeline so authors only maintain the unjoined Stage 0 Inkscape SVG and optional YAML parameters while keeping all four stage outputs on disk for visual inspection.

## Goals

*   **Stage 0 Ingestion & Bounded Early Snapping:** Parse an Inkscape SVG containing required `Image`, `Pieces`, and `Tabs` layers, resolving document units from `<sodipodi:namedview>` or `<svg>`, normalizing `viewBox` scale and ancestor transforms, clustering `<rect>` edges within `1.0 mm`, filtering sub-`0.05 mm` stray paths, and matching open tab endpoints to internal edges within `2.0 mm` while rejecting ambiguous 4-way corner matches.
*   **In-Memory Pipeline with 4 Inspectable Outputs:** Compute all geometry in canonical physical millimeters (`mm`) without inter-stage SVG re-parsing, projecting `-stage1.svg`, `-stage2.svg`, and `-stage3.svg` into normalized original document units (`1 user unit = 1 doc_unit`) and `-stage4-pageN.svg` into `page_unit` (default `mm`), with row and column `GridLabels` indices in Stages 1–2.
*   **Single-Image Mirrored Pattern Bleed:** Embed the source raster `<image>` once at `(0, 0)` inside `<defs>` for Stages 1–3, building a `2x2` mirrored `<pattern>` anchored at `(img_x, img_y)` via four `<use>` references so outward border offsets sample mirrored image pixels without duplicating raster bytes.
*   **Separated Cut Kerf and Print Bleed:** Flatten curves into polylines controlled by `segment_length` (default `1 kerf` = `0.18mm`), heal tab-neck self-intersections via `i_overlay` self-union, apply `+kerf / 2` (`0.09mm` default) round-join outward offsetting for cut paths (Stages 2 and 4), and apply decoupled `+print_bleed` (default `1 kerf` = `0.18mm`) outward offsetting with `stroke: none` on Stage 3 piece fills.
*   **Collision-Free Full-AABB Pagination:** Position pieces on Stage 3 and Stage 4 pages inside `[margin, page_width - margin] x [margin + corner_band, page_height - margin - corner_band]` using symmetric cut-centered bounding boxes covering both cut and bleed polygons plus `piece_spacing`, ensuring `page_cols >= 1 && page_rows >= 1`.
*   **Identical Corner Registration Anchors:** Place built-in, font-free single-stroke vector page numbers and L-shaped corner registration ticks (`fill: none; stroke: #000000` in Stage 3; `fill: none; stroke: <score_color>` on `PageNumbers` in Stage 4) inside reserved top-left and bottom-right corner bands at identical physical page-local coordinates across Stage 3 and Stage 4.
*   **Pattern-Free, Layer-Separated Laser Output:** Emit one standalone `<prefix>-stage4-pageN.svg` per page with flat, transform-free page-local `<path>` geometry, zero `<defs>`, `<image>`, or `<pattern>` elements, and separated `Cut` (`#000000`) and `PageNumbers` (`score_color`, default `#0000ff`) layers.
*   **Safe File Overwrites & Stale Page Cleanup:** Refuse to overwrite existing output files when `--overwrite` is omitted; when `--overwrite` is enabled, write and rename updated files via per-file temporary files before pruning leftover `<prefix>-stage4-page<K>.svg` (`K > new_page_count`) files.

## Non-Goals

*   **Non-rectangular or freeform piece grids in v1:** Input `Pieces` layers must form a rectilinear grid of `<rect>` elements; arbitrary Voronoi or polygon meshes are excluded.
*   **Pairwise edge-overhang or irregular nesting optimizations:** Pieces are spaced uniformly by full bounding boxes (`AABB + piece_spacing`) rather than nesting protruding tabs into adjacent sockets.
*   **Heuristics for arbitrary manual-edit SVG corruption:** Beyond bounded early edge/tab snapping, layer transform normalization, and sub-`0.05 mm` stray-path removal, recovery from incomplete or duplicate manual path joins (such as the partial layout proof-of-concept in [rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg](../../src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg)) is out of scope.
*   **Non-Inkscape multi-page Stage 3 splitting:** Stage 3 uses Inkscape `<inkscape:page>` elements for Inkscape PDF export rather than emitting separate per-page Stage 3 print SVGs.
*   **Re-fitting cubic Beziers to offset boundaries:** Stages 2–4 represent offset piece shapes as polylines bounded by `segment_length`.
*   **External linked raster images or GUI integration:** External non-`data:` `<image>` file references and WASM/UI integration are excluded.

## Open Questions

None.

## Critical User Journeys (CUJs)

### CUJ 1: End-to-End Default Puzzle Generation

*   **Given:** A user has a Stage 0 Inkscape SVG (`rainy-day.svg`) with a 4in x 4in embedded raster image in `Image`, a 5x5 grid of `0.8in x 0.8in` `<rect>` pieces in `Pieces`, and 40 open `<path>` curves in `Tabs`, with no YAML config file.
*   **When:** The user runs `puzzle-gen rainy-day.svg`.
*   **Then:**
    *   `puzzle-gen` validates all 40 internal edges, creates `generated/` next to `rainy-day.svg` if needed, prints a summary (`grid`, `matched_tabs`, `missing_tabs`, `slot_mm`, `pages`, and output paths), and writes `generated/rainy-day-stage1.svg`, `generated/rainy-day-stage2.svg`, `generated/rainy-day-stage3.svg`, and `generated/rainy-day-stage4-page1.svg` through `generated/rainy-day-stage4-pageN.svg` using default settings (`page_size: 6in x 6in`, `page_unit: mm`, `margin: 0.25in`, `kerf: 0.18mm`, `segment_length: 1 kerf`, `piece_spacing: 1 kerf`, `print_bleed: 1 kerf`).
    *   The user opens `generated/rainy-day-stage3.svg` in Inkscape, exports a multi-page PDF to adhesive label paper, adheres each sheet to board, opens the matching `generated/rainy-day-stage4-pageN.svg`, aligns the laser using the top-left and bottom-right corner numbers and ticks, and cuts interlocking pieces with zero exposed white stock within the `0.09mm` (`print_bleed - kerf / 2`) alignment tolerance.

### CUJ 2: Custom Material, Kerf, and Page Size via YAML Configuration

*   **Given:** A user is cutting `3mm` plywood on a laser with a `0.24mm` kerf and printing on `8.5in x 11in` label sheets (`7.5in x 10in` usable area inside `0.5in` margins, minus reserved corner bands), and creates `config.yaml` overriding `page_size: "8.5in x 11in"`, `margin: "0.5in"`, `kerf: "0.24mm"`, `segment_length: "1 kerf"`, `piece_spacing: "1 kerf"`, and `print_bleed: "1 kerf"`.
*   **When:** The user runs `puzzle-gen puzzle.svg --config config.yaml --prefix wood-run`.
*   **Then:**
    *   `puzzle-gen` converts all configured lengths and kerf multipliers (`1 kerf` = `0.24mm`) to canonical millimeters, computes `+0.12mm` (`kerf / 2`) cut offsets for Stages 2 and 4 and `+0.24mm` (`print_bleed`) fill offsets for Stage 3, paginates pieces across `7.5in x 10in` margin-bounded usable sheets (less corner bands), creates `generated/` next to `puzzle.svg` if needed, and emits `generated/wood-run-stage1.svg`, `generated/wood-run-stage2.svg`, `generated/wood-run-stage3.svg`, and `generated/wood-run-stage4-page<N>.svg`.

### CUJ 3: Diagnosing Input SVG Authoring Issues

*   **Given:** A user supplies a Stage 0 SVG where one internal grid edge has no tab `<path>`, or where even one spaced piece slot plus margins and corner bands exceeds `page_size` (`page_cols * page_rows < 1`).
*   **When:** The user runs `puzzle-gen input.svg` (or `puzzle-gen input.svg --no-strict`).
*   **Then:**
    *   In default strict mode, a missing internal tab causes `puzzle-gen` to exit non-zero before writing outputs and print an error identifying the grid edge `(row1, col1)-(row2, col2)`. When `--no-strict` is passed, `puzzle-gen` downgrades missing internal tabs to warnings, keeps those shared edges straight, and completes all four stages.
    *   For unattached non-trivial paths, endpoints ambiguous within `2.0 mm` of a 4-way corner intersection, duplicate tabs on one edge, or insufficient page capacity (`page_cols * page_rows < 1`), `puzzle-gen` exits non-zero before writing outputs and prints an error naming the offending SVG element `id` or required minimum `page_size`.

--------------------------------------------------------------------------------

## Technical Plan (Architecture)

### Architecture Overview

`puzzle-gen` separates a pure geometric library (`src/puzzle-gen/src/lib.rs`) from a CLI and filesystem wrapper (`src/puzzle-gen/src/main.rs`). All four stages execute sequentially in memory against immutable domain structures expressed in canonical physical millimeters (`mm`). Downstream stages never re-parse SVG XML emitted by earlier stages; each output SVG is a stateless projection of the typed pipeline state.

```text
Stage 0 Inkscape SVG ──┐
                       ▼
Optional YAML Config ─▶ [ Config (saphyr) & CoordFrame Normalization (mm) ]
                                       │
                                       ▼
                      [ Stage 0 Ingest: IngestedPuzzle ]
                      (Image, 1.0mm-clustered R×C <rect> grid, TabCandidates)
                                       │
                                       ▼
                      [ Stage 1 Join & Pattern: SplicedPuzzle ] ──▶ <prefix>-stage1.svg (orig doc_unit)
                      (2.0mm pair snap, 3-part splice, 2×2 <use> pattern)
                                       │
                                       ▼
                      [ Stage 2 Heal, Offset & Space: KerfedGrid ] ──▶ <prefix>-stage2.svg (orig doc_unit)
                      (Flatten, self-union, +kerf/2, +print_bleed, AABB spacing)
                                       │
                                       ▼
                      [ Stage 3/4 Paginate: PaginatedLayout ] ──┬─▶ <prefix>-stage3.svg (orig doc_unit, multi-page)
                      (Corner bands, margin-bounded slots)       └─▶ <prefix>-stage4-pageN.svg (page_unit, pattern-free, Cut + PageNumbers)
```

`src/puzzle-gen/Cargo.toml` composes focused pure-Rust crates:
*   **XML & SVG parsing/writing:** `roxmltree`, `svgtypes`, and `xmlwriter` (preserving Inkscape namespaces, layers, `<sodipodi:namedview>`, and `<inkscape:page>` elements).
*   **CLI & YAML configuration:** `clap` (with `derive` feature) and `saphyr` (DOM YAML parsing with explicit top-level mapping key validation, avoiding `serde` and archived `serde_yaml`).
*   **Curve geometry & polygon booleans/offsetting:** `kurbo` (exact line, quadratic, arc, and cubic primitives, curve reversal, and bounded subdivision) and `i_overlay` (deterministic integer-scaled self-union healing and `LineJoin::Round` outward buffering).

### CLI Interface & File Lifecycle (`--overwrite`)

*   **Invocation:** `puzzle-gen <INPUT_SVG> [OPTIONS]`
*   **Flags:**
    *   `--config <PATH>`: Optional YAML config file path. If omitted, default configuration applies.
    *   `--output-dir <DIR>`: Output directory for generated SVGs (defaults to `<input_svg_parent>/generated`, or `./generated` if `<INPUT_SVG>` is a bare filename). Created automatically via `create_dir_all` only when writing outputs.
    *   `--prefix <PREFIX>`: Output filename prefix (defaults to `<INPUT_SVG>` file stem). Validated on startup to be non-empty and free of path separators (`/`, `\`) or `.` / `..` path traversal segments.
    *   `--overwrite`: Allow replacing existing target outputs and pruning leftover higher-numbered Stage 4 cut files (defaults to `false`).
    *   `--dry-run`: Run parsing, snapping, offsetting, and pagination checks; print notices and layout summary (`grid`, `matched_tabs`, `missing_tabs`, `slot_mm`, `pages`, `page_cols x page_rows`) to `stdout`/`stderr` without creating `--output-dir` or writing files.
    *   `--no-strict`: Downgrade missing internal tabs from errors (default) to warnings and keep those edges straight.
    *   `--emit-debug-on-error`: When Stage 2 or Stage 3 fails, still write valid completed Stage 1 (and Stage 2 if available) debug SVGs subject to `--overwrite` collision rules and `--dry-run`.
*   **File Protection & Stale Page Pruning:**
    *   Existing Stage 4 files are matched via `^<prefix>-stage4-page[1-9][0-9]*\.svg$` so unrelated files such as `<prefix>-stage4-page-notes.svg` are never touched.
    *   Compute outputs and run layout checks in memory before writing files. When probing `--output-dir` prior to `create_dir_all`, treat `ErrorKind::NotFound` as zero collisions.
    *   If `--overwrite` is `false` and `<prefix>-stage1.svg`, `<prefix>-stage2.svg`, `<prefix>-stage3.svg`, or any matching `<prefix>-stage4-page<K>.svg` already exists, abort with an error listing the colliding path(s) before creating or modifying files.
    *   If `--overwrite` is `true`, create `--output-dir` if absent, write and rename every new stage output via per-file temporary files (`<output_dir>/.<prefix>.tmp-<pid>-...`, ignored by Stage 4 filename matching), and only after all writes succeed prune leftover `<prefix>-stage4-page<K>.svg` files where `K > new_page_count`.

### YAML Configuration Schema & Unit Normalization (`CoordFrame`)

The optional YAML configuration file is parsed via `saphyr::Yaml::load_from_str` and schema-validated (rejecting unknown or duplicate keys) without `serde`:

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

*   **`saphyr` Mapping Validation Rules:**
    *   Empty or comment-only YAML (`docs.is_empty()`) resolves to default `Config`. Multi-document YAML (`docs.len() > 1`) or non-mapping roots return an error.
    *   Top-level mapping keys must belong to `{version, page_size, page_unit, margin, kerf, segment_length, piece_spacing, print_bleed, score_color}`; unknown or duplicate keys fail immediately naming the offending key.
    *   If present, `version` must be integer `1`. All other fields must be YAML string scalars. If `score_color` parses as null or empty (for example when an unquoted `#0000ff` is stripped as a YAML comment), fail with a message advising the author to quote `"#RRGGBB"`.
*   **Dimension Grammar & Parameter Bounds:**
    *   Scalar lengths accept decimal numbers paired with any of the four closed `LengthUnit` values (`mm`, `cm`, `in`, `px`, using CSS 96 DPI `25.4 / 96.0 mm/px`), ignoring optional whitespace (e.g., `0.18mm` or `0.18 mm`). Unitless numbers fail with a syntax error showing valid examples.
    *   `page_size` accepts `<width> x <height>` (case-insensitive `x`, optional whitespace, each side a scalar length) with `page_width_mm > 0.0` and `page_height_mm > 0.0`.
    *   `page_unit` accepts `mm`, `cm`, `in`, or `px` (default `mm`) and sets SVG width, height, viewBox coordinates, and `inkscape:document-units` on Stage 4 outputs.
    *   `margin >= 0.0`, `kerf >= 0.0`, `piece_spacing >= 0.0`, and `print_bleed >= 0.0`.
    *   `segment_length`, `piece_spacing`, and `print_bleed` also accept `<multiplier> kerf` (`>= 0.0`), resolved against configured `kerf_mm`. `segment_length_mm` must be `> 0.0`; setting `kerf: "0mm"` while leaving `segment_length: "1 kerf"` returns a validation error naming both `kerf` and `segment_length` and asking for an explicit positive `segment_length`.
    *   `score_color` accepts a case-insensitive 6-digit hex color (`#RRGGBB`, normalized to lowercase `#rrggbb`, default `#0000ff`) applied only to the Stage 4 `PageNumbers` layer.
*   **`CoordFrame` Unit Resolution:**
    *   On Stage 0 ingest, read `inkscape:document-units` from `<sodipodi:namedview>` first, falling back to `<svg inkscape:document-units>`, and otherwise inferring `doc_unit` from `<svg>` `width`/`height` unit (`mm`, `cm`, `in`, `px`, defaulting to `px` for unitless dimensions). Reject missing `width` or `height` attributes on `<svg>`.
    *   Parse `viewBox` (`viewbox_min_user`, `(vb_w, vb_h)`, defaulting to `(0, 0)` and physical pixel dimensions when `viewBox` is absent), convert physical `<svg>` `width` and `height` (treating unitless SVG lengths as `96 DPI px`) into `mm`, and compute `user_to_mm = (phys_w_mm / vb_w, phys_h_mm / vb_h)`. Reject non-uniform aspect ratios where `|user_to_mm.0 - user_to_mm.1| / max(user_to_mm.0, user_to_mm.1) > 1e-4`.
    *   Normalize all geometry, snapping tolerances, offsetting, and pagination math in canonical `mm`. Stages 1–3 project coordinates into normalized original document units (`1 user unit = 1 doc_unit`) and emit `<sodipodi:namedview inkscape:document-units="...">`, while Stage 4 projects into `Config.page_unit` and emits `<sodipodi:namedview inkscape:document-units="...">`.

### Core Domain Data Structures

All geometric types share canonical `mm` coordinates in memory; projection to SVG document units happens solely during serialization.

| Type | Stage | Core Responsibilities & Invariants |
| --- | --- | --- |
| `CoordFrame` & `Config` | Config / Pre-Stage 0 | Holds input SVG `doc_unit`, `viewbox_min_user`, `user_to_mm`, `doc_to_mm`, plus validated `Config` sheet, kerf, flattening, spacing, bleed, and color parameters (`mm`). |
| `IngestedPuzzle` | Stage 0 Output | Stores `frame`, single `data:` URI `image_href`, axis-aligned `image_bounds_mm`, `image_preserve_aspect_ratio` (default `"none"`), row-major clustered `R x C` `GridCell` rectangles (`bounds_mm`), and normalized `TabCandidate` `kurbo::BezPath` curves. |
| `SplicedPuzzle` | Stage 1 Output | Holds `frame`, `image_href`, `image_bounds_mm`, `image_preserve_aspect_ratio`, grid size `(rows, cols)`, matched edge counts, and closed clockwise `SplicedPiece` contours (`kurbo::BezPath` in `mm`). |
| `KerfedGrid` | Stage 2 Output | Holds `frame`, `image_href`, `image_bounds_mm`, `image_preserve_aspect_ratio`, `(rows, cols)`, and per-piece `KerfedPiece` contours (`cut_polygon`, `bleed_polygon`, each a closed positive signed-area `Vec<kurbo::Point>` ring in `mm`). |
| `PaginatedLayout` | Stage 3–4 Input | Computes symmetric cut-centered `slot_size_mm`, corner registration band bounds, `(page_cols, page_rows)` capacity (`>= 1 x 1`), and per-page `PagePlan` slots + corner number/tick polylines in canonical `mm`. |
| `StageOutputs` & `PipelineResult` | Pipeline Output | Bundles rendered UTF-8 SVG XML strings (`stage1_svg`, `stage2_svg`, `stage3_svg`, `stage4_pages: Vec<(usize, String)>`) alongside `Vec<PipelineNotice>` warnings/notices and layout summary metrics. |

### Stage-by-Stage Geometric Pipeline

#### Stage 0: Input SVG Ingestion

1.  **Layer Resolution:** Locate `<g inkscape:groupmode="layer">` groups by `inkscape:label` (`Image`, `Pieces`, `Tabs`). Fail with a schema error citing missing or duplicate layer labels. Ignore helper guide layers and metadata `<title>`/`<desc>` elements; reject unexpected non-shape elements inside required layers.
2.  **Transform Composition:** Walk each required layer recursively and apply ancestor `transform` chains followed by `(p_user - viewbox_min_user) * user_to_mm` into canonical `mm`.
3.  **Image Validation:** Extract the single `<image>` in `Image` (supporting both `href` and `xlink:href`, rejecting missing, duplicate, or non-`data:` URIs). Retain `preserveAspectRatio` (default `"none"` when omitted). Allow only translation and positive axis scale on `<image>`; reject rotated, skewed, or negatively scaled `<image>` transforms citing element `id`. Emit a `Warning` notice if `image_bounds_mm` does not cover the full `Pieces` grid bounds.
4.  **Rectilinear Grid Construction:** Extract `<rect>` elements from `Pieces` (rejecting rotated/skewed `<rect>` matrices whose off-diagonal terms exceed `1e-4`), cluster unique edge coordinates within `1.0 mm` to snap slight Inkscape node offsets onto shared canonical grid lines, assign `(row, col)` indices independent of XML order, and verify a contiguous non-overlapping `rows x cols` grid (`rows >= 1 && cols >= 1 && cells.len() == rows * cols`, failing if any cell collapses or overlaps).
5.  **Path Normalization & Stray-Path Filtering:** Parse `Tabs` `<path>` `d` strings (`M`/`m`, `L`/`l`, `H`/`h`, `V`/`v`, `C`/`c`, `S`/`s`, `Q`/`q`, `A`/`a`, `Z`/`z`) via `svgtypes::SimplifyingPathParser` into cubic/line `kurbo` segments in `mm` (elevating quadratic segments via `kurbo::QuadBez::raise()` and converting arcs via `kurbo::Arc::to_cubic_beziers()`). Filter sub-`0.05 mm` stray paths (`aabb_diag < 0.05 mm && arc_len < 0.05 mm`, emitting an `Info` notice with element `id`) *before* rejecting non-trivial closed (`Z`) or multi-subpath (`> 1` `M`) elements.

#### Stage 1: Tab Splicing & `2x2` Mirrored Pattern

1.  **Bounded Endpoint-Pair Tab Matching:** Enumerate all internal grid edges (`(rows * (cols - 1))` vertical and `((rows - 1) * cols)` horizontal edges from clustered Stage 0 lines). Match each `TabCandidate` jointly by its `(start, end)` endpoint pair against every internal edge:
    *   Compute perpendicular distance `perp_dist` and longitudinal projection `t` along `[0.0, edge_len]` for both endpoints.
    *   Accept a candidate edge when both endpoints have `perp_dist <= 2.0 mm`, `t` inside `[-2.0 mm, edge_len + 2.0 mm]`, and nonzero longitudinal span `|t_end - t_start| >= 1.0 mm`.
    *   Reject tabs whose endpoint falls within `2.0 mm` of both a horizontal and a vertical internal edge at a 4-way corner intersection, tabs matching an outer border, unmatched tabs, or duplicate tabs mapped to the same internal edge, naming element `id` and coordinates.
    *   Project matched endpoints orthogonally onto the internal edge line (`t = clamp(t, 0.0, edge_len)`), update first/last endpoints to `snapped_start`/`snapped_end`, and reverse the curve when `t_end < t_start`.
    *   Under default strict mode, fail with an error naming `(r1, c1)-(r2, c2)` if any internal edge has no tab; under `--no-strict`, emit a `Warning` notice and keep that edge a straight line.
2.  **Clockwise Contour Assembly:** For every `(row, col)` cell, build a closed clockwise `kurbo::BezPath` across its four edges (`top` L→R, `right` T→B, `bottom` R→L, `left` B→T). On any internal edge with a tab, splice three parts in canonical edge traversal order (`cell_a` forward, `cell_b` reversed): `LineTo(snapped_entry)` if the entry lead exceeds `1e-4 mm`, the cubic tab chain, and `LineTo(edge_end)` if the exit lead exceeds `1e-4 mm`.
3.  **Stage 1 SVG Projection (`<prefix>-stage1.svg`):**
    *   Project coordinates into normalized document units (`1 user unit = 1 doc_unit`) and emit `<sodipodi:namedview inkscape:document-units="...">`.
    *   Emit `<image id="puzzle-image" x="0" y="0" width="W" height="H" preserveAspectRatio="..." xlink:href="..." />` inside `<defs>`.
    *   Emit `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse" x="img_x" y="img_y" width="2W" height="2H">` with four `<use href="#puzzle-image">` nodes carrying no `x`/`y` attributes on transformed quadrants (`<use href="#puzzle-image" />`, `transform="translate(2W, 0) scale(-1, 1)"`, `transform="translate(0, 2H) scale(1, -1)"`, `transform="translate(2W, 2H) scale(-1, -1)"`).
    *   Render each piece `<path id="piece-r{row}-c{col}">` styled with `fill: url(#puzzle-pattern); stroke: #000000; stroke-width: <stroke_w>` (`0.1 mm` projected to `doc_unit`).
    *   Add a `<g inkscape:groupmode="layer" inkscape:label="GridLabels">` layer placing `<text>` row labels (`0..rows-1`) along the left edge and column labels (`0..cols-1`) along the top edge within a fixed header margin above/left of the artwork.

#### Stage 2: Flattening, Self-Union Healing, Kerf/Bleed Offsetting & Grid Spacing

1.  **Curve Flattening:** Keep straight grid-edge `LineTo` segments as single 2-point lines, and flatten cubic Bezier segments recursively until max control-point-to-chord perpendicular distance `<= segment_length_mm * 0.1` (`max(dist(c1, chord), dist(c2, chord))`) and chord length `<= segment_length_mm`, capped at recursion depth `16`. Deduplicate consecutive points closer than `0.001 mm` and normalize each closed ring to positive signed `(x, y)` area.
2.  **Self-Union Healing & Outward Polygon Offsetting (`i_overlay`):** Scale each closed contour to integer coordinates at `1000` units/mm (`1 um` resolution):
    *   Run an `i_overlay` self-union pass (`OverlayRule::Subject`, `FillRule::NonZero`) on the flattened contour before measuring `unoffset_area`, healing bowtie loops from Bezier handle overshoots or endpoint snapping. Verify the healed shape is one positive-area ring with zero holes (`shapes.len() == 1 && shapes[0].len() == 1`).
    *   Buffer twice via `i_overlay` `OutlineOffset` / `IntOutlineOffset` with `OutlineStyle` and `LineJoin::Round(0.02)`: `cut_polygon` at `+kerf_mm / 2.0` (`+0.09 mm` default) and `bleed_polygon` at `+print_bleed_mm` (`+0.18 mm` default).
    *   Run collinear simplification with tolerance `min(0.005, segment_length_mm * 0.05)`.
    *   Validate `cut_polygon` has `shapes.len() == 1 && shapes[0].len() == 1` and `area > unoffset_area` when `kerf_mm > 0.0` (`area >= unoffset_area - 1e-3 && area > 0.0` when `kerf_mm == 0.0`). On `bleed_polygon`, keep the largest positive-area outer boundary ring containing `cut_polygon` (allowing merged inner socket cavities to drop without failing) and validate area positivity (`area > unoffset_area` when `print_bleed_mm > 0.0` or `>= unoffset_area - 1e-3 && area > 0.0` when `0.0`), failing on neck collapse with piece `(row, col)` and `id`.
3.  **Stage 2 SVG Projection (`<prefix>-stage2.svg`):**
    *   Compute Stage 2 slot pitch (`stage2_slot_w = max(cut_aabb.width)`, `stage2_slot_h = max(cut_aabb.height)`) and place piece `(row, col)` centered in its grid cell via `<g transform="translate(dx, dy)">` where `stage2_offset = cell_center - cut_aabb.center()`.
    *   Keep `<path id="piece-r{row}-c{col}">` coordinates in original puzzle space (`fill: url(#puzzle-pattern); stroke: #000000; stroke-width: <stroke_w>` for `0.1 mm`) so `url(#puzzle-pattern)` locks to each piece's original artwork slice.
    *   Emit `<sodipodi:namedview inkscape:document-units="...">` and `<g inkscape:groupmode="layer" inkscape:label="GridLabels">` `<text>` indices aligned to spaced row/column centers, expanding root `width`, `height`, and `viewBox` in `doc_unit`.

#### Stage 3 & Stage 4: Full-AABB Pagination, Vector Digits & Layer Separation

1.  **Symmetric Slot Sizing & Page Capacity:**
    *   Compute unified slot bounds symmetric around each piece's `cut_aabb.center()` across all `cut_polygon` and `bleed_polygon` vertices:
        *   `slot_w = max_pieces(2.0 * max(|min_x - cut_cx|, |max_x - cut_cx|))`
        *   `slot_h = max_pieces(2.0 * max(|min_y - cut_cy|, |max_y - cut_cy|))`
    *   Reserve top and bottom corner registration bands (`corner_band_h_mm = 6.0 mm`) inside page margins:
        *   `usable_w = page_width_mm - 2.0 * margin_mm`
        *   `usable_h = page_height_mm - 2.0 * margin_mm - 2.0 * corner_band_h_mm`
        *   `page_cols = floor((usable_w + piece_spacing_mm) / (slot_w + piece_spacing_mm))`
        *   `page_rows = floor((usable_h + piece_spacing_mm) / (slot_h + piece_spacing_mm))`
    *   Validate `page_cols >= 1 && page_rows >= 1`. Otherwise fail before writing files, reporting `slot_w x slot_h`, `usable_w x usable_h`, and the minimum `page_size` (`slot_w + 2 * margin_mm` by `slot_h + 2 * margin_mm + 2 * corner_band_h_mm`) needed to fit one piece.
2.  **Slot Allocation & Corner Registration Anchors:**
    *   Assign pieces in row-major puzzle order into `[margin_mm, margin_mm + corner_band_h_mm]`-origin sheet grids (`slot_center_mm = (margin_mm + col * (slot_w + piece_spacing_mm) + slot_w / 2.0, margin_mm + corner_band_h_mm + row * (slot_h + piece_spacing_mm) + slot_h / 2.0)`, `piece_offset_mm = slot_center_mm - cut_aabb.center()`).
    *   Render 1-based page numbers (`1`, `2`, ...) in the top-left corner band `(margin_mm, margin_mm)` and bottom-right corner band `(page_width_mm - margin_mm - num_w, page_height_mm - margin_mm - 4.0)` using a built-in static table of `[0, 1] x [0, 1]` stroke glyphs (`0`–`9`, with top flag on `1`, scaled to `4.0 mm` height, `0.6` aspect ratio, `0.2 * digit_w` gap, a horizontal baseline bar `0.15 * digit_h` below, and a sharp L-shaped corner registration tick inside the band).
    *   Assert at pagination time that all translated `bleed_polygon` and `cut_polygon` vertices fit within `[margin_mm - 1e-4, page_w - margin_mm + 1e-4] x [margin_mm + corner_band_h_mm - 1e-4, page_h - margin_mm - corner_band_h_mm + 1e-4]`, maintain `>= piece_spacing_mm - 1e-4` clearance, and preserve total piece count.
3.  **Stage 3 Multi-Page Inkscape Output (`<prefix>-stage3.svg`):**
    *   Project into `doc_unit` (`1 user unit = 1 doc_unit`) with `<sodipodi:namedview inkscape:document-units="...">` containing `<inkscape:page x="(p-1)*page_w" y="0" width="page_w" height="page_h" id="page<p>" />` for each page `p` and root `<svg>` dimensions sized to page 1.
    *   Wrap printed pieces in `<g inkscape:groupmode="layer" inkscape:label="PrintPieces">`, translating each `bleed_polygon` inside `<g transform="translate((p-1)*page_w + piece_dx, piece_dy)">` styled with `fill: url(#puzzle-pattern); stroke: none;`.
    *   Wrap corner page numbers and L-ticks in `<g inkscape:groupmode="layer" inkscape:label="PageNumbers">` styled with `fill: none; stroke: #000000; stroke-width: <stroke_w>` (`0.1 mm` projected to `doc_unit`).
4.  **Stage 4 Standalone Cut Outputs (`<prefix>-stage4-pageN.svg`):**
    *   Project into `Config.page_unit` (default `mm`) with `<sodipodi:namedview inkscape:document-units="...">` and root `<svg>` sized to `page_w x page_h`, omitting `<defs>`, `<image>`, and `<pattern>`.
    *   Bake all page-local translations directly into flat `<path d="...">` coordinates (zero `transform` attributes, serialized at `>= 6` decimal places for `in`/`cm` and `>= 4` for `mm`/`px` so Stage 3 vs. Stage 4 physical coordinates agree within `1e-4 mm`):
        *   `Cut` (`inkscape:label="Cut"`): `cut_polygon` outlines styled `fill: none; stroke: #000000; stroke-width: <stroke_w>`.
        *   `PageNumbers` (`inkscape:label="PageNumbers"`): identical top-left and bottom-right vector page numbers and L-ticks styled `fill: none; stroke: <score_color>; stroke-width: <stroke_w>` (`0.1 mm` projected to `page_unit`).

--------------------------------------------------------------------------------

## Alternatives Considered

### Alternative A: Arbitrary Non-Rectangular or Freeform Piece Polygons in `v1`

*   **Pros:** Supports irregular Voronoi, spiral, or hand-drawn puzzle layouts beyond rectilinear grids.
*   **Cons:** Requires planar graph face extraction, arbitrary-valence half-edge healing, and irregular bin packing instead of row-major slot grids.
*   **Reason for Rejection:** Restricting `v1` inputs to rectilinear `<rect>` grids bounds the geometry problem to predictable edge-to-edge tab matching and grid pagination.

### Alternative B: Spacing Stage 2 Pieces by Nominal Grid Pitch + `1 kerf` (`0.18mm`)

*   **Pros:** Packs pieces closer together horizontally and vertically than full bounding-box separation.
*   **Cons:** Protruding undercut tab bulbs exceed `0.18mm` overhang depth relative to adjacent sockets.
*   **Reason for Rejection:** Spacing by full post-offset bounding boxes prevents neighboring cuts from intersecting undercut tab necks.

### Alternative C: Pairwise Edge-Overhang or Irregular S-Curve Nesting in Stages 2–4

*   **Pros:** Reduces sheet waste slightly by nesting tabs into neighboring sockets.
*   **Cons:** Adds non-convex pairwise clearance checks and asymmetric margin bounds for little sheet savings on small batch runs.
*   **Reason for Rejection:** Uniform symmetric AABB slots guarantee collision-free margins and keep piece spacing predictable.

### Alternative D: Omitting Intermediate Stage 1 and Stage 2 SVG Files

*   **Pros:** Writes fewer output files on disk.
*   **Cons:** Hides whether a visual or geometry issue originated during tab splicing, flattening, or kerf offsetting.
*   **Reason for Rejection:** Writing all four inspectable stage SVGs from the in-memory model speeds up diagnosing SVG authoring issues.

### Alternative E: Coupling Stage 3 Print Fills to Stage 2/4 `kerf / 2` Cut Offsets

*   **Pros:** Reuses a single buffered polygon contour for both print fills and laser cut paths.
*   **Cons:** `kerf / 2` (`0.09mm`) gives zero extra margin beyond the cut kerf when manually aligning printed label sheets on a laser bed.
*   **Reason for Rejection:** Keeping `print_bleed` independent (default `1 kerf` = `0.18mm`) gives `print_bleed - kerf / 2` (`0.09mm` default) print-to-cut registration headroom.

### Alternative F: Splitting Stage 3 Into Separate Per-Page Print SVGs

*   **Pros:** Lets single-page viewers display individual print sheets directly.
*   **Cons:** Requires printing each sheet individually instead of exporting one multi-page PDF.
*   **Reason for Rejection:** Inkscape `<inkscape:page>` natively exports one multi-page PDF for batch label printing.

### Alternative G: Naming the Curve-Flattening Parameter `polyline_density` or Defaulting It to `0.1mm`

*   **Pros:** Uses a slightly tighter default chord step (`0.1mm`).
*   **Cons:** `polyline_density` is ambiguous regarding direction (larger = finer or coarser), and smaller chords increase output path point count proportionally to `1 / segment_length`.
*   **Reason for Rejection:** `segment_length` makes units explicit (`mm` or `kerf`), and defaulting to `"1 kerf"` (`0.18mm`) scales polyline density with laser kerf while bounding control-point flatness to `<= segment_length * 0.1` (`0.018mm` default).

### Alternative H: Retaining `<defs>`, `<image>`, or `<pattern>` Elements in Stage 4 Laser Cut Files

*   **Pros:** Keeps Stage 4 SVG structure parallel to Stages 1–3.
*   **Cons:** Adds large raster payloads and transform trees that laser CAM importers frequently mis-scale, reject, or interpret as raster engraving jobs.
*   **Reason for Rejection:** Stripping patterns, images, and transforms leaves only flat cut paths and scoreable vector digits for CAM import.

### Alternative I: Fitting Cubic Beziers to Offset Contours or Using System Fonts for Page Numbers

*   **Pros:** Emits fewer SVG path segments on offset boundaries and uses `<text>` elements for page numbers.
*   **Cons:** Post-offset Bezier refitting risks curvature overshoot near sharp socket necks, and `<text>` page numbers depend on installed fonts or manual text-to-path conversion before laser cutting.
*   **Reason for Rejection:** Flat `segment_length` polylines and built-in vector glyphs avoid refitting errors and font dependencies on Stage 4 sheets.

### Alternative J: Applying `stroke: none` to Stage 3 Corner Page Numbers

*   **Pros:** Uses one styling rule for every path in Stage 3.
*   **Cons:** Unfilled single-stroke vector digits vanish when `stroke: none` is applied.
*   **Reason for Rejection:** Only print-bleed piece fills omit strokes; corner numbers and ticks remain stroked (`#000000`) so operators can read sheet numbers and align the laser.

### Alternative K: Reserving Two Full Piece Slots per Page for Corner Page Numbers

*   **Pros:** Treats page numbers as regular slot occupants inside the piece grid.
*   **Cons:** Wastes two full piece-sized cells on every sheet and blocks printing on sheets where only 1 or 2 pieces fit (`page_cols * page_rows < 3`).
*   **Reason for Rejection:** Fixed `6.0 mm` top-left and bottom-right corner bands keep corner registration marks outside piece grids and allow sheets with `page_cols * page_rows >= 1`.

--------------------------------------------------------------------------------

## Non-Functional Requirements

*   **Raster Footprint Efficiency:** Keep the base64 `<image>` byte payload inside `<defs>` once per Stage 1–3 document via `<use>` references, and omit `<defs>`, `<image>`, and `<pattern>` from Stage 4 cut sheets.
*   **Numerical Determinism & Precision:** Execute all geometry in canonical `mm`, scale contours deterministically at `1000` integer units per `mm` (`1 um`) for `i_overlay`, and serialize coordinates with `>= 6` decimal places (`in`/`cm`) or `>= 4` decimal places (`mm`/`px`) so Stage 3 and Stage 4 coordinates agree within `1e-4 mm`.
*   **Fail-Fast Validation & Crash-Safe File Writes:** Validate inputs, bounds, and pagination before writing outputs; refuse collisions unless `--overwrite` is enabled; and write new files through per-file temp files before pruning stale Stage 4 pages.

## Risks & Limits

*   **Integer Grid Overflow (`i_overlay`):** At `1000` units per `mm` (`1 um`), `i32` coordinates handle puzzles and multi-page layouts up to roughly `+-2,000,000 mm` before overflowing. Ingest validates finite coordinates well inside safe bounds and fails with a dimension error if exceeded.
*   **Neck Collapse Under Extreme Kerf:** Large `kerf` settings on very thin puzzle tab necks can pinch offset contours into disconnected islands. Stage 2 validates healed single-ring topology with zero holes on `cut_polygon` and aborts naming `(row, col)` and `id` if a piece splits.
*   **Imprecise Node Snapping & Handle Overshoot in Input SVGs:** Hand-drawn pieces and tabs can have slight misalignments or Bezier handles crossing straight edges. Bounded early snapping (`1.0 mm` `<rect>` clustering and `2.0 mm` joint endpoint-pair segment matching) combined with 4-way corner rejection and Stage 2 `i_overlay` self-union healing repairs normal authoring slop while failing early on ambiguous geometry.
*   **Large Print Bleed or Zero Offsets:** Large `print_bleed` values can bridge across narrow empty sockets and create internal holes, while `kerf: "0mm"` reduces offset area deltas to zero. Retaining the outer positive-area boundary on `bleed_polygon` and checking zero-offset area tolerances avoids spurious failures.
*   **Overwrites & Laser CAM Compatibility:** Broad wildcard deletion can touch unintended user files, and nested `<transform>` or `<defs>` tags break laser CAM importers. Matching Stage 4 pages via exact base-10 page suffixes, writing via per-file temporary files before pruning stale pages, and emitting transform-free Stage 4 paths prevents data loss and CAM import errors.

--------------------------------------------------------------------------------

## Additional Resources

*   [rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg](../../src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg): Inkscape Stage 0 reference file (`4in x 4in` grid, 25 `<rect>` pieces, 39 open tab `<path>` elements numbered `path1`–`path40` with `path19` absent from XML, leaving internal edge `((2, 0), (3, 0))` along `y = 2.4in, x in [0, 0.8in]` without a tab, plus an embedded PNG on `Image`).
*   [rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg](../../src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg): Partial proof-of-concept showing `6in x 6in` `<inkscape:page>` layout and pattern-filled pieces with incomplete manual path joins.

## Future Improvements

*   **Pairwise Edge-Overhang Nesting:** Replace uniform full-AABB slots with alternating row/column nesting while keeping clearance invariants intact.
*   **Non-Rectangular Piece Topologies:** Support Voronoi and arbitrary closed piece polygons beyond rectilinear `<rect>` grids.
*   **External Image Links & Offset Bezier Refitting:** Support file-linked raster images in addition to `data:` URIs and add tolerance-bounded cubic Bezier refitting for offset cut contours.

--------------------------------------------------------------------------------

## Detailed Implementation Plan

### Target Files to Create/Modify

Files are organized into three incremental phases so `cargo check` and `cargo test` run after every phase.

#### Phase 1: Crate Manifest, Configuration & Domain Model

##### 1. `src/puzzle-gen/Cargo.toml` (Modify)
*   **Type / Scope:** Crate Manifest (Cargo Configuration)
*   **Proposed Changes:**
    *   Add dependencies pinned to compatible versions: `clap = { version = "4.5", features = ["derive"] }`, `roxmltree = "0.20"`, `svgtypes = "0.15"`, `xmlwriter = "0.1"`, `saphyr = "0.0.6"`, `kurbo = "0.11"`, `i_overlay = "1.10"`.
    *   Add `[dev-dependencies]` `tempfile = "3.10"` for isolated CLI and file-lifecycle integration tests.

##### 2. `Cargo.lock` (Modify)
*   **Type / Scope:** Workspace Lockfile
*   **Proposed Changes:**
    *   Update root `Cargo.lock` when resolving `src/puzzle-gen/Cargo.toml` dependencies.

##### 3. `src/puzzle-gen/src/model.rs` (Create)
*   **Type / Scope:** Rust Domain Model & Error Types
*   **Proposed Changes:**
    *   Define `LengthUnit` (`Mm`, `Cm`, `In`, `Px`) with `to_mm(self, val)` and `from_mm(self, mm)` (`25.4 / 96.0` mm/px).
    *   Define `CoordFrame` (`doc_unit: LengthUnit`, `viewbox_min_user: (f64, f64)`, `user_to_mm: (f64, f64)`, `doc_to_mm: f64`).
    *   Define `Config` (`page_width_mm`, `page_height_mm`, `page_unit`, `margin_mm`, `kerf_mm`, `segment_length_mm`, `piece_spacing_mm`, `print_bleed_mm`, `score_color: String`).
    *   Define Stage 0–4 data structures using `kurbo` primitives directly: `GridCell`, `Orientation`, `InternalEdge`, `TabCandidate`, `IngestedPuzzle`, `SplicedPiece`, `SplicedPuzzle`, `KerfedPiece` (`row`, `col`, `id`, `cut_polygon: Vec<kurbo::Point>`, `bleed_polygon: Vec<kurbo::Point>`), `KerfedGrid`, `PageSlot`, `PagePlan`, `PaginatedLayout`, `StageOutputs`, `NoticeSeverity` (`Info`, `Warning`), `PipelineNotice`, `PipelineResult`, and `PipelineError`.

##### 4. `src/puzzle-gen/src/config.rs` (Create)
*   **Type / Scope:** Configuration (`saphyr`) & `CoordFrame` Module
*   **Proposed Changes:**
    *   Implement `Config::from_yaml(yaml: Option<&str>) -> Result<Config, PipelineError>` using `saphyr::Yaml::load_from_str`:
        *   Return default `Config` when `yaml` is `None` or `docs.is_empty()` (empty/comment-only input); reject `docs.len() > 1` or non-mapping roots.
        *   Iterate top-level mapping entries, accepting only `{version, page_size, page_unit, margin, kerf, segment_length, piece_spacing, print_bleed, score_color}` and failing on unknown or duplicate keys.
        *   Validate `version == 1` integer if supplied and require string scalars for all other fields (with a quote-string hint if `score_color` parses as null due to unquoted `#` comment stripping).
    *   Implement `parse_scalar_length`, `parse_kerf_relative_length`, `parse_page_size`, and `parse_score_color` validating finite values (`page_width_mm > 0.0`, `page_height_mm > 0.0`, `margin_mm >= 0.0`, `kerf_mm >= 0.0`, `piece_spacing_mm >= 0.0`, `print_bleed_mm >= 0.0`, `segment_length_mm > 0.0`, rejecting `kerf: "0mm"` paired with `<N> kerf` for `segment_length`).
    *   Implement `CoordFrame::from_svg_root(doc: &roxmltree::Document) -> Result<CoordFrame, PipelineError>` reading `inkscape:document-units` from `<sodipodi:namedview>` first (fallback `<svg>`, then `width`/`height` unit), handling unitless CSS `px`, storing `viewbox_min_user`, and rejecting aspect mismatch `> 1e-4`.
    *   Add Phase 1 unit tests (`#[cfg(test)]`) covering grammar, `kerf` multipliers, `saphyr` unknown/duplicate/comment-stripped color errors, and `CoordFrame`.

#### Phase 2: Stage 0–2 Ingestion, Bounded Early Snapping & Buffering

##### 5. `src/puzzle-gen/src/ingest.rs` (Create)
*   **Type / Scope:** Rust Stage 0 SVG Ingestion Module
*   **Proposed Changes:**
    *   Implement `ingest_svg(svg_xml: &str, notices: &mut Vec<PipelineNotice>) -> Result<IngestedPuzzle, PipelineError>` using `roxmltree` and `svgtypes`:
        *   Resolve required `Image`, `Pieces`, and `Tabs` `<g inkscape:groupmode="layer">` layers; fail on missing or duplicate labels or unexpected non-shape elements; ignore helper layers and `<title>`/`<desc>`.
        *   Compose ancestor `transform` chains followed by `viewbox_min_user` and `user_to_mm` down to every `<image>`, `<rect>`, and `<path>`.
        *   Validate one `<image>` (`href` or `xlink:href`, `data:` URI only), retain `preserveAspectRatio` (default `"none"`), reject rotated/skewed/flipped `<image>` transforms, and emit a `Warning` notice if `image_bounds_mm` does not cover `Pieces` grid bounds.
        *   Reject rotated/skewed `<rect>` elements (`|off_diag| > 1e-4`), cluster `<rect>` edges within `1.0 mm` onto canonical shared coordinates, sort into `(row, col)`, and validate non-overlapping contiguous `rows x cols` coverage (`cells.len() == rows * cols`).
        *   Normalize `Tabs` `<path>` strings via `svgtypes::SimplifyingPathParser` into `kurbo::BezPath` chains (`QuadBez::raise()`, `Arc::to_cubic_beziers()`), filtering sub-`0.05 mm` stray paths (`aabb_diag < 0.05 mm && arc_len < 0.05 mm` with `Info` notice) *before* rejecting non-trivial closed (`Z`) or multi-subpath (`> 1` `M`) elements.

##### 6. `src/puzzle-gen/src/stage1.rs` (Create)
*   **Type / Scope:** Rust Stage 1 Edge Matching & Splicing Module
*   **Proposed Changes:**
    *   Implement `splice_tabs(puzzle: &IngestedPuzzle, strict: bool, notices: &mut Vec<PipelineNotice>) -> Result<SplicedPuzzle, PipelineError>`:
        *   Build canonical horizontal `((row, col)-(row+1, col))` and vertical `((row, col)-(row, col+1))` internal edges from Stage 0's clustered grid lines.
        *   Match each `TabCandidate` jointly by `(start, end)` endpoint pair: require both endpoints within `perp_dist <= 2.0 mm` of the same internal edge, longitudinal projections in `[-2.0 mm, edge_len + 2.0 mm]`, and `|t_end - t_start| >= 1.0 mm`; reject tabs whose endpoint falls within `2.0 mm` of two perpendicular edges at a 4-way corner, border-edge tabs, unmatched tabs, or duplicate edge matches.
        *   Project matched endpoints orthogonally onto the internal edge line (`t = clamp(t, 0.0, edge_len)`), rewrite endpoints to snapped edge coordinates, and reverse curve orientation when `t_end < t_start`.
        *   Fail when `strict == true` if any internal edge has no tab; emit a `Warning` notice and leave a straight line when `strict == false`.
        *   Assemble closed clockwise `kurbo::BezPath` contours (`top` L→R, `right` T→B, `bottom` R→L, `left` B→T), splicing `[LineTo(snapped_entry) if > 1e-4 mm, tab_curve, LineTo(edge_end) if > 1e-4 mm]`.

##### 7. `src/puzzle-gen/src/stage2.rs` (Create)
*   **Type / Scope:** Rust Stage 2 Flattening, Healing & Polygon Offsetting Module
*   **Proposed Changes:**
    *   Implement `offset_and_space(spliced: &SplicedPuzzle, config: &Config) -> Result<KerfedGrid, PipelineError>`:
        *   Preserve straight `LineTo` segments as 2-point lines and flatten cubic curves recursively up to chord length `<= config.segment_length_mm` and max control-point flatness `<= config.segment_length_mm * 0.1` (recursion cap `16`), deduplicating vertices closer than `0.001 mm`.
        *   Normalize rings to positive signed `(x, y)` area, scale to `1000` units/mm (`1 um` `i32`), and run `i_overlay` self-union (`OverlayRule::Subject`, `FillRule::NonZero`) to heal bowtie loops before measuring `unoffset_area` (verifying `shapes.len() == 1 && shapes[0].len() == 1`).
        *   Buffer `cut_polygon` at `+config.kerf_mm / 2.0` and `bleed_polygon` at `+config.print_bleed_mm` using `i_overlay` `OutlineOffset` / `IntOutlineOffset` (`OutlineStyle`, `LineJoin::Round(0.02)`), then run collinear simplification at `min(0.005, config.segment_length_mm * 0.05)`.
        *   Validate `cut_polygon` (`shapes.len() == 1 && shapes[0].len() == 1`, zero holes) and `bleed_polygon` (largest positive-area outer ring containing `cut_polygon`), using `area > unoffset_area` for positive offsets and `area >= unoffset_area - 1e-3 && area > 0.0` when offset is `0.0`.
    *   Add Phase 2 unit tests validating `1.0 mm` clustering, `2.0 mm` early tab snapping, 4-way corner rejection, self-union healing, `LineJoin::Round`, zero-offset handling, and S-curve flatness.

#### Phase 3: Pagination, SVG Serialization, CLI & Integration Suite

##### 8. `src/puzzle-gen/src/paginate.rs` (Create)
*   **Type / Scope:** Rust Pagination & Vector Registration Digit Module
*   **Proposed Changes:**
    *   Define static `[0, 1] x [0, 1]` stroke polylines for digits `'0'..='9'` (with top flag on `'1'`) and implement `layout_corner_page_number(page_number: usize, origin_mm: kurbo::Point, height_mm: f64, corner: CornerAnchor) -> Vec<Vec<kurbo::Point>>` (`0.6` aspect ratio, `0.2 * digit_w` gap, baseline bar `0.15 * digit_h` below, plus sharp L-shaped corner tick).
    *   Implement `paginate_pieces(kerfed: &KerfedGrid, config: &Config) -> Result<PaginatedLayout, PipelineError>`:
        *   Compute symmetric cut-centered `slot_w` and `slot_h` (`2.0 * max(|min - cut_center|, |max - cut_center|)` across `cut_polygon` and `bleed_polygon`), reserve `6.0 mm` top/bottom corner bands, and calculate `page_cols` and `page_rows`.
        *   Require `page_cols >= 1 && page_rows >= 1`; otherwise fail stating `slot_w x slot_h`, `usable_w x usable_h`, and minimum `page_size`.
        *   Assign slots in row-major order on `(margin_mm, margin_mm + 6.0)` grids (`piece_offset_mm = slot_center_mm - cut_aabb.center()`) and verify piece counts, margin containment (`1e-4 mm` epsilon), and `>= piece_spacing_mm - 1e-4` clearance directly at the end of `paginate_pieces`.

##### 9. `src/puzzle-gen/src/svg_emit.rs` (Create)
*   **Type / Scope:** Rust Stage SVG Projection Module
*   **Proposed Changes:**
    *   Implement deterministic `xmlwriter` functions (`fmt_coord` using `6` decimals for `In`/`Cm` and `4` decimals for `Mm`/`Px`):
        *   `emit_stage1(spliced: &SplicedPuzzle) -> String`: `doc_unit` projection (`1 user unit = 1 doc_unit`), `<sodipodi:namedview inkscape:document-units="...">`, `<defs>` `<image x="0" y="0">` + `<pattern x="img_x" y="img_y">` + four `<use>` tiles (no `x`/`y` on transformed quadrants), piece `<path>` outlines (`stroke: #000000; stroke-width: <0.1mm in doc_unit>`), and `GridLabels` `<text>` layer.
        *   `emit_stage2(kerfed: &KerfedGrid, config: &Config) -> String`: `doc_unit` projection, shared pattern, `stage2_slot_w x stage2_slot_h` centered `<g transform="translate(dx, dy)">` groups keeping `<path>` coords locked to original puzzle space (`stroke-width: <0.1mm in doc_unit>`), `<sodipodi:namedview>`, and aligned `GridLabels` `<text>` layer.
        *   `emit_stage3(kerfed: &KerfedGrid, layout: &PaginatedLayout, config: &Config) -> String`: `doc_unit` projection, `<sodipodi:namedview inkscape:document-units="...">` with `<inkscape:page>` nodes and root `<svg>` sized to page 1, `PrintPieces` layer (`bleed_polygon`, `fill: url(#puzzle-pattern); stroke: none;`), and `PageNumbers` layer (`fill: none; stroke: #000000; stroke-width: <0.1mm in doc_unit>`).
        *   `emit_stage4_page(kerfed: &KerfedGrid, page: &PagePlan, config: &Config) -> String`: `config.page_unit` projection with `<sodipodi:namedview>`, zero `<defs>`/`<image>`/`<pattern>`/`transform`, flat `Cut` (`#000000`) and `PageNumbers` (`config.score_color`) layers with `0.1 mm` stroke width.

##### 10. `src/puzzle-gen/src/lib.rs` (Create)
*   **Type / Scope:** Rust Library Root
*   **Proposed Changes:**
    *   Export `pub mod config; pub mod ingest; pub mod model; pub mod paginate; pub mod stage1; pub mod stage2; pub mod svg_emit;`.
    *   Provide `pub fn run_pipeline(svg_xml: &str, config: &Config, strict: bool) -> Result<PipelineResult, PipelineError>` returning `PipelineResult` (and preserving valid Stage 1 / Stage 2 partial SVG strings in `PipelineError` when Stage 2 or Stage 3 fails so `--emit-debug-on-error` can inspect intermediate stages).

##### 11. `src/puzzle-gen/src/main.rs` (Modify)
*   **Type / Scope:** Rust CLI Entrypoint & File Lifecycle Manager
*   **Proposed Changes:**
    *   Parse CLI args via `clap`; validate `--prefix` as a single path-free stem; read `--config` if passed and call `Config::from_yaml`.
    *   Call `puzzle_gen::run_pipeline`, emit `notices` to `stderr`, and on downstream error honor `--emit-debug-on-error` (if `--dry-run` is false and collision rules permit).
    *   Under `--dry-run`, print layout summary metrics without writing files; otherwise check collisions (ignoring `ErrorKind::NotFound` on missing `--output-dir` and matching Stage 4 files via `^<prefix>-stage4-page[1-9][0-9]*\.svg$`), create `--output-dir`, write outputs via per-file temporary files (`<output_dir>/.<prefix>.tmp-<pid>-...`) before pruning leftover `K > pages.len()` files under `--overwrite`, and print output paths to `stdout`.

##### 12. `src/puzzle-gen/examples/default-config.yaml` (Create)
*   **Type / Scope:** Example YAML Configuration
*   **Proposed Changes:**
    *   Add a documented reference config file containing default values (`version: 1`, `page_size: "6in x 6in"`, `page_unit: "mm"`, `margin: "0.25in"`, `kerf: "0.18mm"`, `segment_length: "1 kerf"`, `piece_spacing: "1 kerf"`, `print_bleed: "1 kerf"`, `score_color: "#0000ff"`).

##### 13. `src/puzzle-gen/tests/integration_pipeline.rs` (Create)
*   **Type / Scope:** Rust Integration & CLI Regression Suite
*   **Proposed Changes:**
    *   Test `rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg`: fail in strict mode citing missing internal edge `((2, 0), (3, 0))` along `y = 2.4in, x in [0, 0.8in]`, and pass under `--no-strict` emitting 1 missing-edge warning and 25 closed healed contours.
    *   Test synthetic complete 5x5 SVG across Stages 1–4: verify `<sodipodi:namedview>` units, `<image x="0" y="0">` + 4 `<use>` tiles without transformed `x`/`y`, `preserveAspectRatio="none"`, `GridLabels`, `PrintPieces` vs. `PageNumbers`, zero `<defs>`/`<image>`/`<pattern>`/`transform` on Stage 4, `Cut` vs. `PageNumbers` colors, and serialized Stage 3 vs. Stage 4 `<path>` parity within `1e-4 mm`.
    *   Test bounded early snapping and edge cases: `1.0 mm` `<rect>` perturbation, `2.0 mm` joint tab endpoint misalignment, 4-way corner intersection rejection, `Quadratic`/`Arc` elevation, sub-`0.05 mm` closed stray clicks, rotated `<rect>` / `<image>` rejection, `bleed_polygon` socket bridge-over, zero `kerf`, `saphyr` unknown/duplicate key and unquoted `#RRGGBB` errors.
    *   Test CLI binary invocations in isolated `tempfile::tempdir()` workspaces: `--dry-run`, `--output-dir`, `--prefix`, `--emit-debug-on-error`, collision failure without `--overwrite`, write-before-prune stale page deletion with `--overwrite`, and `page_cols * page_rows < 1` error messages.

### Verification Plan

*   **Phase Checkpoints:** Run `cargo test -p puzzle-gen --lib` after Phase 1 (`config`, `model`), Phase 2 (`ingest`, `stage1`, `stage2`), and Phase 3 (`paginate`, `svg_emit`).
*   **Compilation/Build:** Run `cargo build -p puzzle-gen` and `cargo check --workspace` to confirm zero build errors across workspace members.
*   **Static Analysis & Formatting:** Run `cargo clippy -p puzzle-gen --all-targets -- -D warnings` and `cargo fmt -p puzzle-gen -- --check`.
*   **Integration Suite:** Run `cargo test -p puzzle-gen --test integration_pipeline` covering all four unit inputs (`mm`, `cm`, `in`, `px`), `<N> kerf` expressions, strict vs. `--no-strict`, collision vs. `--overwrite` stale-page pruning, `saphyr` error branches, early snapping bounds, and serialized Stage 3 vs. Stage 4 coordinate parity.
*   **CLI Smoke Run:** Execute `cargo run -p puzzle-gen -- src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg --no-strict --output-dir target/smoke-out` and assert exit code `0` with generated Stage 1–4 SVGs inside gitignored `target/smoke-out/`.
*   **Manual Inkscape Verification:** Open `target/smoke-out/rainy-day-puzzle-4x4-25piece-step0-piece-shape-stage3.svg` and `-stage4-page*.svg` in Inkscape and confirm:
    *   Stage 3 displays exact matching page count, aligned artwork inside bleed contours, zero clipped margin edges, and visible `#000000` corner numbers + L-ticks.
    *   Stage 4 opens in `mm` document units with flat paths split into `Cut` (`#000000`) and `PageNumbers` (`#0000ff`).

--------------------------------------------------------------------------------

*Drafted with Elephant-Goldfish Model (EGM)*
