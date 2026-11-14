# `puzzle-gen`: Four-Stage Laser Jigsaw Puzzle Generator

*   **Author:** Tim Sweña (Swast)
*   **Status:** Draft
*   **Date:** 2026-11-13

--------------------------------------------------------------------------------

## Overview

`puzzle-gen` is a local Rust CLI utility in `src/puzzle-gen` that prepares custom jigsaw puzzle artwork drawn in Inkscape for physical fabrication. The fabrication workflow prints the puzzle artwork onto adhesive label paper using Inkscape's multi-page PDF export, adheres that label sheet to backing board, and cuts the individual pieces on a laser cutter.

Starting from a clean Stage 0 Inkscape SVG containing three layers—an embedded raster artwork (`Image`), a rectangular grid of `<rect>` elements (`Pieces`), and cubic-Bezier `<path>` tabs along internal edges (`Tabs`)—plus an optional YAML configuration file, `puzzle-gen` runs an in-memory geometric pipeline and writes four stage SVG outputs for inspection, printing, and laser cutting:

1. **Stage 1 (`<prefix>-stage1.svg`):** Joins each shared tab curve into its two adjacent piece boundaries (removing straight edges across tab necks), fills every piece with a 2x2 mirrored `<pattern>` built from the `Image` layer, and draws built-in vector row and column index numbers along the outer top and left edges for debugging.
2. **Stage 2 (`<prefix>-stage2.svg`):** Flattens piece curves into polylines controlled by `segment_length`, expands each piece outward by `kerf / 2` to compensate for laser beam width, spaces pieces apart on the grid using post-offset bounding boxes, keeps each piece's image pattern locked to its original artwork coordinates, and aligns built-in vector row and column index numbers along the spaced grid edges for debugging.
3. **Stage 3 (`<prefix>-stage3.svg`):** Reflows the pieces in grid order across Inkscape multi-page `<inkscape:page>` sheets within configured margins for Inkscape PDF export. Pieces render with `stroke: none` and an expanded outward `print_bleed` fill (defaulting to full `kerf`), paired with visible stroked vector page numbers (`fill: none; stroke: #000000`) in opposite corners.
4. **Stage 4 (`<prefix>-stage4-pageN.svg`):** Emits one standalone single-page SVG per page with page-local `kerf / 2` piece cut paths (`fill: none; stroke: #000000`) and identical top-left and bottom-right corner page numbers, removing all `<pattern>` and `<image>` data for laser cutting.

## The Problem

Preparing a custom Inkscape jigsaw puzzle for print-and-cut laser fabrication involves tedious, error-prone manual geometry work:

*   **Manual path splicing:** Authoring a puzzle as a grid of `<rect>` elements plus separate open `<path>` curves for interlocking tabs is fast in Inkscape, but turning that sketch into closed pieces requires duplicating, reversing, splitting, and node-joining every shared internal curve (for example, 40 internal edges on a 5x5 puzzle). Leaving any straight rectangle segment across a tab neck causes the laser to slice the tab off.
*   **Image bleed and pattern drift:** Shifting pieces apart in Inkscape breaks `userSpaceOnUse` image patterns unless each piece carries a precisely matching `patternTransform`, where manual typos (such as `0.01` vs. `1/96`) shift the printed image relative to the cut line. Furthermore, pieces expanded outward for kerf compensation or print registration tolerance require mirrored pixel bleed around border edges so white backing paper never shows.
*   **Kerf compensation and undercut collisions:** Because a laser beam vaporizes material along its cut path (for example, `0.18mm` wide), uncompensated pieces fit loosely. Offsetting each piece outward by `kerf / 2` makes tabs slightly wider and sockets slightly narrower, but pulling pieces apart by only `1 kerf` causes wide tab bulbs to cross adjacent socket necks during cutting. Pieces must be separated far enough for tabs and margins to clear without collision.
*   **Print-to-cut registration:** Printing adhesive label sheets and cutting them afterward on a laser bed introduces small alignment errors. Printing black piece outlines leaves visible dark lines on cut edges whenever registration is slightly off, while using identical margins, full bounding-box spacing, decoupled print bleed, and matching corner page numbers across multi-page print sheets and per-page laser files keeps both steps aligned.

`puzzle-gen` automates all four stages from one deterministic in-memory pipeline so authors only maintain the clean Stage 0 Inkscape SVG and optional YAML parameters, while retaining all four stage files for visual debugging.

## Goals

*   **Clean Stage 0 Ingestion:** Parse a Stage 0 Inkscape SVG containing `Image` (embedded raster `<image>`), `Pieces` (rectilinear `<rect>` grid), and `Tabs` (cubic-Bezier `<path>`) layers, normalizing `viewBox` scale, layer transforms, float noise on grid edges, and zero-span stray clicks with element-level diagnostics.
*   **In-Memory Pipeline with 4 Inspectable Outputs:** Compute all geometry in memory using physical millimeters (`mm`) without inter-stage SVG re-parsing, projecting `-stage1.svg`, `-stage2.svg`, `-stage3.svg`, and `-stage4-pageN.svg` (preserving original document units in Stages 1–3 and configurable `page_unit`, defaulting to `mm`, in Stage 4), and rendering built-in vector row/column numbers along Stage 1 and Stage 2 edges matching diagnostic `(row, col)` indices.
*   **Mirrored Pattern Bleed without Bloat:** Embed the source raster `<image>` at most once in `<defs>` for Stages 1–3, building a 2x2 mirrored `<pattern>` via `<use>` references so outward border offsets sample mirrored image pixels without duplicating raster bytes.
*   **Separated Cut Kerf and Print Bleed:** Flatten boundaries into polylines controlled by `segment_length` (default `1 kerf` = `0.18mm`), apply `kerf / 2` (`0.09mm` default) round-join outward offsetting for cut paths (Stages 2 and 4), and apply decoupled `print_bleed` (default `1 kerf` = `0.18mm`) outward offsetting with `stroke: none` on Stage 3 piece fills so slight print/cut misalignment cuts into bled artwork rather than white stock.
*   **Collision-Free Full-AABB Pagination:** Position pieces on Stage 3 and Stage 4 pages strictly within `[margin, page_size - margin]` using each piece's full post-offset (and `print_bleed`-expanded) bounding box plus `piece_spacing`, ensuring zero tab collisions or margin clipping, and verifying `cols * rows >= 3` per page before emitting files.
*   **Identical Corner Registration Anchors:** Reserve the top-left and bottom-right grid slots on each page for built-in, font-free vector page number paths (`fill: none; stroke: #000000` in Stage 3; separated onto a configurable `PageNumbers` layer/color in Stage 4) placed at identical page-local coordinates in both Stage 3 and Stage 4 for sheet identification and manual laser alignment.
*   **Pattern-Free, Layer-Separated Laser Output:** Emit one standalone `<prefix>-stage4-pageN.svg` per page with flat, transform-free page-local `<path>` geometry, zero `<defs>`, `<image>`, or `<pattern>` elements, and distinct `Cut` (`#000000`) and `PageNumbers` (`score_color`, default `#0000ff`) layers.
*   **Safe File Overwrites & Stale Page Cleanup:** Protect existing output files by default (`--overwrite=false`), and when `--overwrite` is enabled, atomically prune leftover `<prefix>-stage4-page*.svg` files from prior runs with higher page counts.

## Non-Goals

*   **Non-rectangular or freeform piece grids in v1:** Input `Pieces` layers must form a rectilinear grid of `<rect>` elements; arbitrary Voronoi or polygon meshes are excluded.
*   **Pairwise edge-overhang or irregular nesting optimizations:** Pieces are spaced uniformly by full bounding boxes (`AABB + piece_spacing`) rather than nesting protruding tabs into adjacent sockets.
*   **Heuristics for arbitrary manual-edit SVG corruption:** Beyond normal Inkscape float noise, layer transforms, and zero-span stray clicks, recovery from broken manual path edits (such as the sketch in `rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg`) is out of scope.
*   **Non-Inkscape multi-page Stage 3 splitting:** Stage 3 uses Inkscape `<inkscape:page>` elements intended for Inkscape PDF export rather than emitting separate per-page Stage 3 print SVGs.
*   **Re-fitting cubic Beziers to offset boundaries:** Stages 2–4 represent offset piece shapes as high-resolution polylines bounded by `segment_length`.
*   **External linked raster images or GUI integration:** External non-`data:` `<image>` file references and WASM/UI integration with `src/sticker-size` are excluded.

## Open Questions

None — all design questions were resolved during brainstorming and initial drafting (`--overwrite` defaults to `false`, Stage 4 separates `Cut` vs. `PageNumbers` layers with a configurable `score_color`, and YAML schema details are specified below).

## Critical User Journeys (CUJs)

### CUJ 1: End-to-End Default Puzzle Generation

*   **Given:** A user has a clean Stage 0 Inkscape SVG (`rainy-day.svg`) with a 4in x 4in embedded raster image in `Image`, a 5x5 grid of `0.8in x 0.8in` `<rect>` pieces in `Pieces`, and 40 cubic-Bezier `<path>` curves in `Tabs`, with no YAML config file.
*   **When:** The user runs `puzzle-gen rainy-day.svg`.
*   **Then:**
    *   `puzzle-gen` validates all 40 internal edges, creates the `generated/` directory next to `rainy-day.svg` if needed, prints a concise summary (grid size, matched edges, page count, output paths), and writes `generated/rainy-day-stage1.svg`, `generated/rainy-day-stage2.svg`, `generated/rainy-day-stage3.svg`, and `generated/rainy-day-stage4-page1.svg` through `generated/rainy-day-stage4-pageN.svg` using default settings (`page_size: 6in x 6in`, `page_unit: mm`, `margin: 0.25in`, `kerf: 0.18mm`, `segment_length: 1 kerf`, `piece_spacing: 1 kerf`, `print_bleed: 1 kerf`).
    *   The user opens `generated/rainy-day-stage3.svg` in Inkscape, exports a multi-page PDF to print onto adhesive label paper, adheres each sheet to board, reads the printed corner page number (`1`, `2`, ...) to open the matching `generated/rainy-day-stage4-pageN.svg`, aligns the laser using the corner page numbers, and cuts interlocking pieces without white edge slivers or overlapping cuts.

### CUJ 2: Custom Material, Kerf, and Page Size via YAML Configuration

*   **Given:** A user is cutting thicker wood on a laser with a `0.24mm` kerf and printing on `8.5in x 11in` label sheets with `0.5in` margins, and creates `config.yaml` overriding `page_size`, `margin`, `kerf`, `segment_length`, `piece_spacing`, and `print_bleed`.
*   **When:** The user runs `puzzle-gen puzzle.svg --config config.yaml --prefix wood-run`.
*   **Then:**
    *   `puzzle-gen` converts all configured physical lengths and relative kerf expressions (such as `1 kerf` = `0.24mm`) to canonical millimeters, computes `kerf / 2` (`0.12mm`) cut offsets for Stages 2 and 4 and `print_bleed` (`0.24mm`) fill offsets for Stage 3, repaginates pieces to fit the `8.5in x 11in` printable area, creates `generated/` next to `puzzle.svg` if needed, and emits `generated/wood-run-stage1.svg`, `generated/wood-run-stage2.svg`, `generated/wood-run-stage3.svg`, and `generated/wood-run-stage4-page*.svg`.

### CUJ 3: Diagnosing Input SVG Authoring Issues

*   **Given:** A user supplies a Stage 0 SVG where one internal grid edge is missing a tab `<path>` (leaving two pieces separated by a straight edge), or where `page_size` minus `2 * margin` only fits a `1x2` grid of spaced pieces.
*   **When:** The user runs `puzzle-gen input.svg` (or `puzzle-gen input.svg --no-strict`).
*   **Then:**
    *   In default strict mode, a missing internal tab causes `puzzle-gen` to exit non-zero before writing outputs and print an actionable error identifying the exact grid edge `(row1, col1)-(row2, col2)`. When `--no-strict` is passed, `puzzle-gen` downgrades missing internal tabs to warnings, leaves those shared edges straight, and completes all four stages.
    *   For unattached non-trivial paths, duplicate tabs on one edge, or insufficient page capacity (`cols * rows < 3`), `puzzle-gen` always exits non-zero before writing outputs and prints an actionable error naming the offending SVG element `id` or required minimum `page_size`.


## Technical Plan (Architecture)

### Architecture Overview

`puzzle-gen` follows a strict split between a pure geometric library (`src/puzzle-gen/src/lib.rs`) and a thin CLI/filesystem wrapper (`src/puzzle-gen/src/main.rs`). All four stages execute sequentially in memory against a typed domain model expressed in canonical physical millimeters (`mm`). Downstream stages never re-parse SVG XML emitted by earlier stages; instead, each of the four SVG files is a stateless serializer projection of its corresponding typed domain structure.

```text
Stage 0 Inkscape SVG ──┐
                       ▼
Optional YAML Config ─▶ [ Config & CoordFrame Normalization (mm) ]
                                       │
                                       ▼
                     [ Stage 0 Ingest: IngestedPuzzle ]
                     (Image, R×C <rect> grid, Bezier Tabs)
                                       │
                                       ▼
                     [ Stage 1 Join & Pattern: SplicedPuzzle ] ──▶ <prefix>-stage1.svg (orig units)
                     (Snap endpoints, splice edges, 2×2 <use> pattern)
                                       │
                                       ▼
                     [ Stage 2 Offset & Space: KerfedGrid ]    ──▶ <prefix>-stage2.svg (orig units)
                     (Flatten @ segment_length, +kerf/2, +print_bleed, AABB spacing)
                                       │
                                       ▼
                     [ Stage 3/4 Paginate: PaginatedLayout ]   ──┬─▶ <prefix>-stage3.svg (orig units, multi-page)
                     (Margin-bounded slots, vector digits)       └─▶ <prefix>-stage4-pageN.svg (page_unit, default mm, pattern-free, Cut + PageNumbers)
```

To avoid reimplementing low-level XML, path, curve, and polygon-boolean math from scratch, `src/puzzle-gen/Cargo.toml` composes focused pure-Rust crates scoped locally so `src/sticker-size` WASM targets remain unaffected:
*   **XML & SVG parsing/writing:** `roxmltree`, `svgtypes`, and `xmlwriter` (preserving Inkscape namespaces, layers, and `<inkscape:page>` elements).
*   **CLI & YAML configuration:** `clap` (derive) and `serde` + `serde_norway` (or `serde_yaml_ng`, avoiding archived `serde_yaml`).
*   **Curve geometry & polygon offsetting:** `kurbo` (exact cubic/line segments, reversal, bounded-length flattening) and `i_overlay` (round-join outward polygon buffering and self-intersection resolution via integer-scaled coordinates).

### CLI Interface & File Lifecycle (`--overwrite`)

*   **Invocation:** `puzzle-gen <INPUT_SVG> [OPTIONS]`
*   **Flags:**
    *   `--config <PATH>`: Optional YAML config file path. If omitted, defaults apply.
    *   `--output-dir <DIR>`: Directory for generated SVGs (defaults to `<input_svg_parent>/generated`, or `./generated` if `<INPUT_SVG>` is a bare filename). Created automatically via `create_dir_all` if missing when writing outputs (skipped under `--dry-run`).
    *   `--prefix <PREFIX>`: Output filename prefix (defaults to `<INPUT_SVG>` filename stem).
    *   `--overwrite`: Boolean flag defaulting to `false`.
    *   `--dry-run`: Run parsing, snapping, offsetting, and pagination checks; print diagnostics and layout metrics without writing files.
    *   `--no-strict`: Allow missing internal tabs by downgrading them from errors (the default) to warnings and leaving those edges straight.
*   **File Protection & Stale Page Pruning:**
    *   Before touching disk, compute all outputs in memory and validate all geometric invariants.
    *   If `--overwrite` is `false` (default) and any target file (`<prefix>-stage1.svg`, `<prefix>-stage2.svg`, `<prefix>-stage3.svg`, or any matching `<prefix>-stage4-page*.svg`) already exists in `--output-dir`, abort with an explicit error listing the colliding path(s) and instructing the user to pass `--overwrite` or pick another `--prefix`/`--output-dir`.
    *   If `--overwrite` is `true`, delete any existing `<prefix>-stage4-page*.svg` files in `--output-dir` once in-memory validation succeeds, then write all stage files atomically via temporary files and renames so reducing the page count from `N` to `M < N` never leaves stale higher-numbered cut files on disk.

### YAML Configuration Schema & Unit Normalization (`CoordFrame`)

The optional YAML configuration file validates strictly via `#[serde(deny_unknown_fields)]` and accepts an optional `version: 1` header:

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

*   **Dimension Grammar:**
    *   Scalar lengths accept decimal numbers paired with `mm`, `cm`, `in`, or `px` (96 DPI CSS pixel conversion `25.4 / 96 mm`), ignoring optional whitespace (e.g., `0.18mm` or `0.18 mm`). Bare unitless numbers are rejected with a syntax error showing valid examples.
    *   `page_size` accepts `<width> x <height>` (case-insensitive `x`, optional spaces, each side parsed as a scalar length, e.g., `6in x 6in` or `8.5in x 11in`).
    *   `page_unit` accepts `mm`, `cm`, `in`, or `px` (defaulting to `mm`) and sets the SVG coordinate, dimension, and `inkscape:document-units` output units for Stage 4 laser-cut files independently of `page_size` or the input SVG's units.
    *   `segment_length`, `piece_spacing`, and `print_bleed` also accept `<multiplier> kerf` (e.g., `1 kerf` or `1.5 kerf`), resolved dynamically against the configured `kerf` value in `mm`.
    *   `score_color` accepts a CSS hex color string (`#RRGGBB`, default `#0000ff`) applied to the `PageNumbers` layer in Stage 4.
*   **`CoordFrame` Unit Resolution:**
    *   At ingest, `CoordFrame` parses the input `<svg>` `width`, `height`, `viewBox`, and `inkscape:document-units` attributes to compute exact `user_to_mm_x` and `user_to_mm_y` scale factors (failing loudly if `width`/`height` are incompatible with `viewBox` aspect ratio or lack physical units when no unit hint is present).
    *   All geometry, tolerances, offsets, and pagination math run in canonical `mm`. Stages 1, 2, and 3 project coordinates back into the original SVG's physical document units (`in`, `cm`, `mm`, etc., keeping the shared 2x2 `<pattern>` in original document units across all print stages), while Stage 4 projects coordinates into the configured `page_unit` (defaulting to `mm`).

### Core Domain Data Structures

```text
CoordFrame:
  doc_unit: LengthUnit          # e.g., Inches, Millimeters, Centimeters, Pixels
  user_to_mm: (f64, f64)        # scale from input viewBox units to physical mm
  doc_to_mm: f64                # scale from input document unit to physical mm
  page_unit: LengthUnit         # configured output unit for Stage 4 cut files (default mm)

Config (canonical mm):
  page_width_mm: f64
  page_height_mm: f64
  page_unit: LengthUnit
  margin_mm: f64
  kerf_mm: f64
  segment_length_mm: f64
  piece_spacing_mm: f64
  print_bleed_mm: f64
  score_color: String

GridCell:
  row: usize
  col: usize
  id: String                    # source <rect> id
  bounds_mm: Rect               # exact clustered rectangle in mm

InternalEdge:
  orientation: Horizontal | Vertical
  cell_a: (usize, usize)        # top or left neighbor
  cell_b: (usize, usize)        # bottom or right neighbor
  start_mm: Point
  end_mm: Point
  tab_curve: Option<Vec<CubicSeg>> # normalized from start_mm -> end_mm

IngestedPuzzle:
  frame: CoordFrame
  image_href: String            # embedded data: URI
  image_bounds_mm: Rect         # transformed <image> bounds in puzzle space
  rows: usize
  cols: usize
  cells: Vec<GridCell>
  raw_tabs: Vec<TabCandidate>   # transformed curves with source element ids

SplicedPiece:
  row: usize
  col: usize
  id: String
  contour: BezPath              # closed clockwise boundary in mm

KerfedPiece:
  row: usize
  col: usize
  id: String
  cut_polygon: PolygonRing      # offset +kerf/2 in original puzzle mm
  bleed_polygon: PolygonRing    # offset +print_bleed in original puzzle mm
  stage2_offset_mm: Vec2        # translation on Stage 2 spaced grid
  stage2_aabb_mm: Rect          # cut_polygon bounding box at Stage 2 position

PageSlot:
  piece_index: Option<usize>    # None for reserved corner page-number slots
  page_local_center_mm: Point   # slot center within page_size bounds
  piece_offset_mm: Option<Vec2> # translation from original puzzle coords to page coords

PagePlan:
  page_number: usize            # 1-based page index
  slots: Vec<PageSlot>
  top_left_digits: Vec<Polyline>
  bottom_right_digits: Vec<Polyline>

PaginatedLayout:
  page_cols: usize
  page_rows: usize
  slot_size_mm: Size            # max(cut_polygon, bleed_polygon) AABB across all pieces
  pages: Vec<PagePlan>

Diagnostic:
  severity: Warning | Error
  stage: StageId
  element_id: Option<String>
  grid_edge: Option<((usize, usize), (usize, usize))>
  message: String
```

### Stage-by-Stage Geometric Pipeline

#### Stage 0: Input SVG Ingestion
1.  **Layer Resolution:** Locate `<g inkscape:groupmode="layer">` groups by `inkscape:label` (`Image`, `Pieces`, `Tabs`). Fail with a schema error citing missing layer labels if any required layer is absent. Ignore empty or helper guide layers.
2.  **Transform Composition:** Walk each required layer recursively and multiply ancestor `transform` matrices (plus any root `viewBox` offset/scale via `CoordFrame`) down into each `<image>`, `<rect>`, and `<path>` element.
3.  **Image Validation:** Extract the single `<image>` from `Image`, supporting both `xlink:href` and `href`. Reject missing `<image>` elements, multiple `<image>` elements, or external non-`data:` URIs with an actionable error.
4.  **Rectilinear Grid Construction:** Extract `<rect>` shapes from `Pieces`, cluster edge coordinates within `0.01 mm` epsilon to absorb Inkscape floating-point noise (such as `1e-8` shifts), assign sorted `(row, col)` indices independent of XML element order, and verify a complete `R x C` grid with no overlapping or missing cells.
5.  **Path Normalization & Stray Filtering:** Parse `Tabs` `<path>` `d` strings (supporting `M`/`m`, `L`/`l`, `H`/`h`, `V`/`v`, `C`/`c`, `S`/`s`, `Q`/`q`, `A`/`a`, `Z`/`z`) via `svgtypes` + `kurbo` into continuous cubic/line segment sequences in `mm`. Discard zero-span stray clicks whose bounding-box diagonal and arc length are both `< 0.05 mm`, emitting an informational diagnostic with element `id`.

#### Stage 1: Tab Splicing & 2x2 Mirrored Pattern
1.  **Tab-to-Edge Matching:** Match each remaining `Tabs` curve against the internal shared edges of the `R x C` grid using an endpoint snap tolerance of `0.5 mm`:
    *   Determine whether the curve runs `start -> end` or `end -> start` relative to the canonical edge direction; reverse the segment sequence when needed and snap exact endpoints onto the clustered grid vertices so adjacent corners meet at identical coordinates.
    *   Reject tabs whose endpoints snap to an outer puzzle border, fail to match any internal edge within `0.5 mm`, or map to an edge that already owns a tab, reporting element `id` and coordinates.
    *   By default (strict mode), fail with an `Error` naming the grid edge `(r1, c1)-(r2, c2)` if any internal edge has no tab; when `--no-strict` is passed, emit a `Warning` instead and keep that edge a straight line segment.
2.  **Clockwise Contour Assembly:** For every cell `(row, col)`, build a closed clockwise `BezPath` traversing its four edges (`top` L→R, `right` T→B, `bottom` R→L, `left` B→T). When an internal edge has a tab curve, splice the forward or reversed curve directly in place of the straight rectangle segment so straight cut lines across tab necks are omitted.
3.  **2x2 Mirrored Pattern & Debug Grid Labels (`<prefix>-stage1.svg`):** Emit the `<image>` element exactly once inside `<defs>` (at transformed `(img_x, img_y)` with size `(W, H)`). Define one shared `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse" x="img_x" y="img_y" width="2W" height="2H">` containing four `<use>` references to that `<image>`: normal at `(0, 0)`, horizontally mirrored `translate(2W, 0) scale(-1, 1)` at `(W, 0)`, vertically mirrored `translate(0, 2H) scale(1, -1)` at `(0, H)`, and mirrored on both axes `translate(2W, 2H) scale(-1, -1)` at `(W, H)`. Render all `SplicedPiece` contours with `fill: url(#puzzle-pattern); stroke: #000000`. Add a `<g inkscape:groupmode="layer" inkscape:label="GridLabels">` layer emitting built-in single-stroke vector digits (`0..rows-1` along the outer left edge and `0..cols-1` along the outer top edge, matching 0-based diagnostic `(row, col)` indices, styled with `fill: none; stroke: <score_color>`) inside a top/left gutter outside piece contours, adjusting the Stage 1 SVG `width`, `height`, and `viewBox` (negative `min-x`/`min-y` if needed) to frame both gutter labels and artwork without shifting piece or `userSpaceOnUse` `<pattern>` coordinates.

#### Stage 2: Flattening, Kerf/Bleed Offsetting & Grid Spacing
1.  **Curve Flattening:** Keep straight grid-edge segments as single 2-point lines (avoiding vertex inflation on flat piece edges), and flatten cubic Bezier segments via `kurbo` recursive subdivision until every chord length `<= segment_length_mm` (default `1 kerf` = `0.18 mm`) and midpoint chordal deviation `<= segment_length_mm * 0.1`. Deduplicate consecutive points closer than `0.001 mm`.
2.  **Outward Polygon Offsetting (`i_overlay`):** Scale each closed polygon ring to deterministic integer grid coordinates (`1000` units per `mm`, i.e., `1 micrometer` resolution) and run round-join (`JoinType::Round`, mimicking a circular laser spot) outward polygon buffering twice per piece:
    *   `cut_polygon`: offset outward by `+kerf_mm / 2.0` (`+0.09 mm` default).
    *   `bleed_polygon`: offset outward by `+print_bleed_mm` (`+0.18 mm` default when `1 kerf`).
    *   Run collinear vertex simplification (`<= 0.005 mm` perpendicular error) on both output rings to trim redundant micro-segments while staying well below `kerf / 10`.
    *   Validate that every resulting ring is a single non-self-intersecting closed polygon with positive area larger than the un-offset piece; emit an error naming piece `(row, col)` and `id` if a tight tab/socket neck collapses into disconnected islands or inverted loops.
3.  **Stage 2 Grid Spacing, Pattern Lock & Debug Grid Labels:** Compute a uniform grid pitch across all `cut_polygon` bounding boxes (`slot_w = max(cut_aabb.width)`, `slot_h = max(cut_aabb.height)`) separated by `piece_spacing_mm`. Translate each piece's `cut_polygon` in `<prefix>-stage2.svg` using `<g transform="translate(dx, dy)">` with path coordinates kept in original puzzle space so the single shared `url(#puzzle-pattern)` remains locked to each piece's original artwork slice without cloning `<pattern>` definitions. Emit a `<g inkscape:groupmode="layer" inkscape:label="GridLabels">` layer with built-in single-stroke vector digits (`0..rows-1` aligned to each spaced row along the outer left gutter and `0..cols-1` aligned to each spaced column along the outer top gutter, styled with `fill: none; stroke: <score_color>`), and expand the Stage 2 SVG `width`, `height`, and `viewBox` (in original document units) to encompass both the gutter labels and the spaced grid without shifting piece or `<pattern>` coordinates.

#### Stage 3 & Stage 4: Full-AABB Pagination, Vector Digits & Layer Separation
1.  **Uniform Full-AABB Slot & Page Capacity:** Across all pieces, compute the unified slot dimensions covering the outer envelope of both `cut_polygon` and `bleed_polygon`:
    *   `slot_w = max_pieces(max(cut_aabb.width, bleed_aabb.width))`
    *   `slot_h = max_pieces(max(cut_aabb.height, bleed_aabb.height))`
    *   `usable_w = page_width_mm - 2.0 * margin_mm`
    *   `usable_h = page_height_mm - 2.0 * margin_mm`
    *   `page_cols = floor((usable_w + piece_spacing_mm) / (slot_w + piece_spacing_mm))`
    *   `page_rows = floor((usable_h + piece_spacing_mm) / (slot_h + piece_spacing_mm))`
    *   Validate `page_cols >= 1`, `page_rows >= 1`, and `page_cols * page_rows >= 3`. If fewer than 3 slots fit on a page, fail immediately with an error stating the computed `slot_w x slot_h`, `usable_w x usable_h`, and minimum `page_size` needed to fit 3 slots.
2.  **Slot Allocation & Corner Page Numbers:** On each page, reserve slot `(0, 0)` (top-left) and slot `(page_rows - 1, page_cols - 1)` (bottom-right) for the page number (`1`, `2`, ...), assigning the remaining `page_cols * page_rows - 2` slots to pieces in row-major puzzle order. Generate the page number using a built-in static table of single-stroke vector paths for digits `0`–`9` (incorporating a top flag/serif on `1` and a horizontal baseline bar spanning beneath the number as an orientation cue against 180-degree rotation), uniformly scaled and centered inside the reserved `slot_w x slot_h` box with `10%` inner padding.
3.  **Stage 3 Multi-Page Inkscape Output (`<prefix>-stage3.svg`):**
    *   Use the original SVG's document units (`doc_unit`) for root `width`, `height`, `viewBox` (sized to page 1), `inkscape:document-units`, and each `<inkscape:page x="(p-1)*page_w" y="0" width="page_w" height="page_h" id="pageP" />` inside `<sodipodi:namedview>`.
    *   For each piece on page `p`, place `<path>` using `bleed_polygon` coordinates inside `<g transform="translate(page_origin_x + slot_dx, slot_dy)">` styled with `fill: url(#puzzle-pattern); stroke: none;` (referencing the single shared `2x2` mirrored `<pattern>` in `<defs>`).
    *   Emit the top-left and bottom-right vector page number paths on page `p` styled with `fill: none; stroke: #000000; stroke-width: <stroke_w>` so printed sheets display both the matching Stage 4 page index and corner laser-alignment anchors.
4.  **Stage 4 Standalone Cut Outputs (`<prefix>-stage4-pageN.svg`):**
    *   Emit one standalone SVG per page using `page_unit` (defaulting to `mm`) for `width`, `height`, `viewBox`, `inkscape:document-units`, and baked path coordinates (matching a single `page_w x page_h` sheet) with **no `<defs>`, `<image>`, or `<pattern>` elements**.
    *   Bake all page-local slot translations directly into flat `<path d="...">` coordinates (zero `transform` attributes anywhere in the document) inside two `<g inkscape:groupmode="layer">` groups:
        *   `Cut` (`inkscape:label="Cut"`): `cut_polygon` piece outlines styled with `fill: none; stroke: #000000; stroke-width: <stroke_w>`.
        *   `PageNumbers` (`inkscape:label="PageNumbers"`): the exact same top-left and bottom-right vector digit paths from Stage 3 at identical physical page-local coordinates, styled with `fill: none; stroke: <score_color>; stroke-width: <stroke_w>` so laser CAM software can score or dry-run align numbers without cutting through the sheet.
5.  **Pre-Write Cross-Stage Verification:** Assert before writing any files that:
    *   Piece counts match across Stages 0, 1, 2, 3, and the sum of Stage 4 pages.
    *   Every Stage 3 piece `bleed_polygon` and Stage 4 piece `cut_polygon` fits inside `[margin_mm, page_size_mm - margin_mm]` and maintains `>= piece_spacing_mm` clearance from adjacent piece slots and page-number boxes.
    *   Stage 3 piece `cut_polygon` slot centers and corner page-number vertices match Stage 4 baked page-local coordinates in canonical `mm` (before unit projection) within `1e-4 mm`.

--------------------------------------------------------------------------------

## Alternatives Considered

### Alternative A: Arbitrary Non-Rectangular or Freeform Piece Polygons in `v1`

*   **Pros**: Supports irregular Voronoi, spiral, or hand-drawn puzzle layouts without restricting pieces to a rectilinear grid.
*   **Cons**: Requires general planar graph face extraction, half-edge topology healing across arbitrary vertex valences, and irregular bin packing instead of row-major slot pagination.
*   **Reason for Rejection**: Rejected by author: Assume a rectangular grid of `<rect>` elements for `v1` to keep edge matching and grid pagination predictable.

### Alternative B: Spacing Stage 2 Pieces Strictly by Nominal Grid Pitch + `1 kerf` (`0.18mm`) Without Clearing Tab Depth

*   **Pros**: Keeps pieces closer together horizontally and vertically than full bounding-box separation.
*   **Cons**: Moving mating pieces apart along a grid axis by only `0.18mm` does not clear protruding undercut tab bulbs, causing neighboring piece bulbs to intersect adjacent socket necks after kerf expansion.
*   **Reason for Rejection**: Rejected by author: Pulling mating undercut tabs apart by `0.18mm` causes wide tab bulbs to cross adjacent socket necks.

### Alternative C: Pairwise Edge-Overhang or Irregular S-Curve Nesting in Stages 2–4

*   **Pros**: Packs interlocking tabs slightly tighter across sheets and reduces wasted material area on large puzzles.
*   **Cons**: Adds complex pairwise tab-clearance geometry checks, asymmetric edge-margin offsets, and non-uniform slot pitch for corner page numbers.
*   **Reason for Rejection**: Rejected by author: Simple full post-offset bounding-box (`AABB`) spacing inside `[margin, page_size - margin]` avoids collision edge cases without nesting complexity for `v1`.

### Alternative D: Omitting Intermediate Stage 1 and Stage 2 SVG Files

*   **Pros**: Produces fewer files on disk and skips writing pre-paginated debug SVGs.
*   **Cons**: Makes it harder to isolate whether a geometry or visual defect originated in tab splicing (Stage 1), curve flattening and kerf offsetting (Stage 2), or page layout and bleed (Stage 3).
*   **Reason for Rejection**: Rejected by author: Emitting all four stage SVGs from an in-memory pipeline makes intermediate geometry and pattern transformations inspectable for debugging.

### Alternative E: Coupling Stage 3 Print Fills to Stage 2/4 `kerf / 2` Cut Offsets

*   **Pros**: Reuses a single offset polyline contour across print and cut outputs.
*   **Cons**: `kerf / 2` (`0.09mm`) is narrower than typical manual registration tolerance when aligning printed adhesive sheets to laser cuts, risking exposed white borders around pieces.
*   **Reason for Rejection**: Rejected by author: `kerf / 2` (`0.09mm`) is too narrow for manual print/cut registration; Stage 3 uses a decoupled `print_bleed` offset (defaulting to full `kerf` = `0.18mm`).

### Alternative F: Splitting Stage 3 Into Separate Per-Page Print SVGs

*   **Pros**: Lets basic browsers or single-page viewers open individual print sheets without multi-page SVG support.
*   **Cons**: Forces the user to open and print each page manually instead of exporting a single multi-page PDF.
*   **Reason for Rejection**: Rejected by author: Inkscape's PDF export handles multi-page `<inkscape:page>` printing directly.

### Alternative G: Naming the Curve-Flattening Parameter `polyline_density` or Defaulting It to `0.1mm`

*   **Pros**: Keeps a density-based name or slightly finer default chord spacing (`0.1mm`).
*   **Cons**: `polyline_density` is ambiguous as to whether larger values increase or decrease vertex count, and a `0.1mm` default produces nearly twice as many path vertices on large puzzles compared to `1 kerf` (`0.18mm`).
*   **Reason for Rejection**: Rejected by author: Renamed to `segment_length` so smaller values indicate higher curve resolution, and defaulted to `"1 kerf"` (`0.18mm`) to scale with laser kerf while keeping chord deviation near `0.002mm` on tight curves.

### Alternative H: Retaining `<defs>`, `<image>`, or `<pattern>` Elements in Stage 4 Laser Cut Files

*   **Pros**: Keeps Stage 4 SVG structure similar to Stages 1–3.
*   **Cons**: Adds large raster payloads and transform trees that some laser CAM importers reject, mis-scale, or attempt to engrave.
*   **Reason for Rejection**: Rejected by author: Stage 4 strips all patterns and raster images completely, leaving only flat, transform-free vector cut and page-number paths split across `Cut` and `PageNumbers` layers.

### Alternative I: Fitting Cubic Beziers to Offset Contours or Using System Fonts for Page Numbers

*   **Pros**: Produces fewer SVG path commands on offset curves and standard `<text>` elements for page numbers.
*   **Cons**: Bezier refitting after polygon offsetting risks curvature overshoot near sharp socket necks, and `<text>` elements depend on installed system fonts and manual text-to-path conversion before laser cutting.
*   **Reason for Rejection**: Rejected by author: Polylines controlled by `segment_length` and built-in font-free vector digits avoid curve-fitting instability and external font dependencies.

### Alternative J: Applying `stroke: none` to Stage 3 Corner Page Numbers

*   **Pros**: Uniform styling rule across every path in Stage 3.
*   **Cons**: Setting `stroke: none` on un-filled single-stroke vector digits makes page numbers invisible on printed sheets.
*   **Reason for Rejection**: Rejected by author: `stroke: none` applies only to Stage 3 piece fills; corner page numbers stay visibly stroked (`fill: none; stroke: #000000`) in both Stage 3 and Stage 4 for sheet identification and manual laser alignment.

--------------------------------------------------------------------------------

## Non-Functional Requirements

*   **Performance & File Footprint**:
    *   Emit `<image>` once inside `<defs>` and construct the `2x2` mirrored `<pattern>` using four transformed `<use>` nodes so Stage 1–3 files do not duplicate base64 image payloads.
    *   Strip `<defs>`, `<image>`, and `<pattern>` completely from Stage 4 laser-cut outputs.
    *   Keep straight grid segments as 2-point lines, subdivide Bezier curves up to `segment_length` (default `"1 kerf"` = `0.18mm`), and run `0.005mm` collinear simplification to control polyline vertex counts while bounding tight-curve sagitta deviation near `0.002mm`.
*   **Numerical Determinism & Precision**:
    *   Normalize all parsed inputs into canonical internal `mm` values and project Stage 4 outputs into `page_unit` (default `mm`) only during SVG serialization.
    *   Scale closed contours to deterministic integer coordinates (`1000` units per `mm`, i.e., `1 micrometer`) for `i_overlay` outward buffering.
*   **Operational Safety & Validation**:
    *   Fail fast on missing internal tabs under default strict mode (`--no-strict` downgrades missing tabs to warnings and leaves those edges straight).
    *   Default `--overwrite` to `false` to prevent accidental file overwrites; when `--overwrite` is set, remove stale `<prefix>-stage4-page*.svg` files before writing updated outputs.
    *   Execute piece-conservation, margin/spacing clearance, `page_cols * page_rows >= 3`, and Stage 3-to-Stage 4 coordinate parity (`<= 1e-4 mm`) checks in memory prior to writing outputs atomically.

--------------------------------------------------------------------------------

### Additional Resources

*   [rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg](src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg): Inkscape Stage 0 reference file (`4in x 4in` grid, 25 `<rect>` pieces, 39 tab `<path>` elements with `path19` omitted, embedded PNG on `Image` layer).
*   [rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg](src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg): Manual proof-of-concept demonstrating multi-page `6in x 6in` `<inkscape:page>` layout and pattern-filled pieces.

### Future Improvements

*   **Pairwise Edge-Overhang Nesting**: Replace uniform full-AABB slots with alternating row/column nesting while keeping clearance invariants intact.
*   **Non-Rectangular Piece Topologies**: Support Voronoi and arbitrary closed piece polygons beyond rectilinear `<rect>` grids.
*   **External Image Links & Offset Bezier Refitting**: Support file-linked raster images in addition to `data:` URIs and add tolerance-bounded cubic Bezier refitting for offset cut contours.

--------------------------------------------------------------------------------

## Detailed Implementation Plan

### Target Files to Create/Modify

#### 1. `src/puzzle-gen/Cargo.toml` (Modify)

*   **Type / Scope**: Crate Manifest (Cargo Configuration)
*   **Proposed Changes**: Add local dependencies scoped strictly to `puzzle-gen` (leaving the sibling `src/sticker-size` WASM crate untouched): `clap` (with `derive` feature) for CLI parsing; `roxmltree`, `svgtypes`, and `xmlwriter` for SVG DOM parsing, path/transform normalization, and namespace-aware XML writing; `serde` (with `derive` feature) and `serde_norway` (or `serde_yaml_ng`) for strict YAML config deserialization; `kurbo` for cubic Bezier/line geometry, reversal, and subdivision; and `i_overlay` for deterministic integer-grid outward polygon offsetting and self-intersection resolution.

#### 2. `src/puzzle-gen/src/lib.rs` (Create)

*   **Type / Scope**: Rust Library Root
*   **Proposed Changes**: Expose the pipeline submodules (`config`, `diagnostics`, `digits`, `ingest`, `model`, `stage1`, `stage2`, `stage3_4`, `svg_emit`, `verify`) and provide the top-level orchestration API `run_pipeline(svg_xml: &str, config: &Config, strict: bool) -> Result<PipelineResult, PipelineError>` returning typed intermediate models, `Diagnostic` warnings/info messages, and serialized `StageOutputs` (`stage1_svg`, `stage2_svg`, `stage3_svg`, and `stage4_pages: Vec<(usize, String)>`) without touching the filesystem.

#### 3. `src/puzzle-gen/src/diagnostics.rs` (Create)

*   **Type / Scope**: Rust Core Diagnostics Module
*   **Proposed Changes**: Define `Severity` (`Info`, `Warning`, `Error`), `StageId` (`Config`, `Ingest`, `Stage1`, `Stage2`, `Stage3_4`, `Verify`), `Diagnostic` (carrying optional `element_id` and `grid_edge: Option<((usize, usize), (usize, usize))>`), `DiagnosticBag`, and structured `PipelineError` formatting actionable CLI messages with source IDs, coordinates, and remediation hints.

#### 4. `src/puzzle-gen/src/config.rs` (Create)

*   **Type / Scope**: Rust Configuration & Unit Normalization Module
*   **Proposed Changes**: Implement `RawYamlConfig` using `#[serde(deny_unknown_fields)]` (supporting optional `version: 1`, `page_size`, `page_unit`, `margin`, `kerf`, `segment_length`, `piece_spacing`, `print_bleed`, and `score_color`), `LengthUnit` (`Mm`, `Cm`, `In`, `Px` with `25.4 / 96.0` mm/px), dimension grammar parsers rejecting bare unitless numbers (`parse_scalar_length`, `parse_kerf_relative_length`, `parse_page_size`, `parse_score_color`), positive/finite bounds validation, and `Config::from_yaml(Option<&str>) -> Result<Config, PipelineError>` normalizing all values into canonical `mm`. Also implement `CoordFrame::from_svg_root(...)` resolving input `<svg>` `width`, `height`, `viewBox`, and `inkscape:document-units` into exact `user_to_mm` and `doc_to_mm` factors.

#### 5. `src/puzzle-gen/src/model.rs` (Create)

*   **Type / Scope**: Rust Domain Model
*   **Proposed Changes**: Define the immutable stage transfer types in canonical `mm`: `GridCell`, `Orientation`, `CubicSeg` (line or cubic Bezier segment), `InternalEdge`, `TabCandidate`, `IngestedPuzzle`, `SplicedPiece`, `SplicedPuzzle`, `PolygonRing` (closed outer ring of `(f64, f64)` points), `KerfedPiece`, `KerfedGrid`, `PageSlot`, `PagePlan`, `PaginatedLayout`, and `StageOutputs`. Isolate unit conversion projection helpers (`mm_to_unit(mm, unit)`) on `LengthUnit` so geometry logic only operates in millimeters.

#### 6. `src/puzzle-gen/src/ingest.rs` (Create)

*   **Type / Scope**: Rust Stage 0 SVG Ingestion Module
*   **Proposed Changes**: Implement `ingest_svg(svg_xml: &str, bag: &mut DiagnosticBag) -> Result<IngestedPuzzle, PipelineError>` using `roxmltree` and `svgtypes`: locate `<g inkscape:groupmode="layer">` groups by `inkscape:label` (`Image`, `Pieces`, `Tabs`), ignoring helper/empty layers and failing on missing required layers; compose ancestor `transform` chains and root `viewBox` mapping down into elements; validate a single `data:` URI `<image>` (accepting both `href` and `xlink:href`, rejecting missing, duplicate, or external links); cluster `<rect>` boundaries within `0.01 mm` epsilon to absorb Inkscape float noise, reject rotated `<rect>` grid cells, sort cells into a complete `R x C` grid independent of XML order; and normalize `Tabs` `<path>` `d` strings via `svgtypes::SimplifyingPathParser` into `CubicSeg` chains, filtering zero-span stray clicks (`diagonal < 0.05 mm` and `arc_length < 0.05 mm`) with `Info` diagnostics and rejecting closed or disconnected multi-subpath tab elements.

#### 7. `src/puzzle-gen/src/stage1.rs` (Create)

*   **Type / Scope**: Rust Stage 1 Edge Matching & Contour Splicing Module
*   **Proposed Changes**: Implement `splice_tabs(puzzle: &IngestedPuzzle, strict: bool, bag: &mut DiagnosticBag) -> Result<SplicedPuzzle, PipelineError>`: enumerate all internal grid edges (`(rows * (cols - 1))` vertical and `((rows - 1) * cols)` horizontal), match `TabCandidate` endpoints against edge endpoints within `0.5 mm`, reverse curve orientation when needed, snap exact endpoints onto clustered grid vertices, reject outer-border tabs, unmatched tabs, or double-assigned edges naming element `id` and coordinates, fail under strict mode (`!no_strict`) on missing internal tabs while downgrading them to `Warning` diagnostics (leaving straight edges) when `no_strict` is active, and assemble a closed clockwise `kurbo::BezPath` (`top` L→R, `right` T→B, `bottom` R→L, `left` B→T) per `(row, col)` cell.

#### 8. `src/puzzle-gen/src/stage2.rs` (Create)

*   **Type / Scope**: Rust Stage 2 Flattening, Offsetting & Grid Spacing Module
*   **Proposed Changes**: Implement `offset_and_space(spliced: &SplicedPuzzle, config: &Config) -> Result<KerfedGrid, PipelineError>`: keep straight grid-edge segments as single 2-point lines, subdivide `CubicSeg` Beziers recursively until chord length `<= config.segment_length_mm` and midpoint chordal deviation `<= config.segment_length_mm * 0.1`, deduplicate vertices closer than `0.001 mm`, scale closed rings to `1000` integer units per `mm` (`1 micrometer`) for deterministic round-join (`JoinType::Round`) `i_overlay` outward buffering by `+config.kerf_mm / 2.0` (`cut_polygon`) and `+config.print_bleed_mm` (`bleed_polygon`), simplify collinear points within `0.005 mm` perpendicular error, verify each output yields a single positive-area non-self-intersecting ring larger than the un-offset piece (failing with piece `(row, col)` and `id` otherwise), and compute uniform Stage 2 grid offsets (`max(cut_aabb)` separated by `config.piece_spacing_mm`).

#### 9. `src/puzzle-gen/src/digits.rs` (Create)

*   **Type / Scope**: Rust Font-Free Vector Digit Renderer
*   **Proposed Changes**: Define a static lookup table of normalized `[0, 1] x [0, 1]` single-stroke polyline glyphs for digits `'0'..='9'` (including a top flag/serif on `'1'`). Provide `layout_corner_page_number(page_number: usize, slot_rect_mm: Rect) -> Vec<Vec<Point>>` (adding a horizontal baseline bar spanning beneath the digits as a 180-degree orientation cue, centered inside the slot with `10%` padding) for Stage 3/4 page corners, and `layout_gutter_index(index: usize, anchor_box_mm: Rect) -> Vec<Vec<Point>>` for Stage 1/2 top/left row and column debug labels (`0..rows-1` and `0..cols-1`).

#### 10. `src/puzzle-gen/src/stage3_4.rs` (Create)

*   **Type / Scope**: Rust Stage 3 & Stage 4 Pagination Module
*   **Proposed Changes**: Implement `paginate_pieces(kerfed: &KerfedGrid, config: &Config) -> Result<PaginatedLayout, PipelineError>`: compute unified `slot_w` and `slot_h` across `max_pieces(max(cut_aabb, bleed_aabb))`, `usable_w = page_width_mm - 2.0 * margin_mm`, `usable_h = page_height_mm - 2.0 * margin_mm`, `page_cols`, and `page_rows`; enforce `page_cols >= 1`, `page_rows >= 1`, and `page_cols * page_rows >= 3` (failing with computed `slot_w x slot_h`, `usable_w x usable_h`, and minimum `page_size` needed for 3 slots); reserve slot `(0, 0)` and slot `(page_rows - 1, page_cols - 1)` per page for `digits::layout_corner_page_number`; assign remaining slots in row-major order; compute `piece_offset_mm = slot_center_mm - cut_aabb.center()` so Stage 3 `bleed_polygon` and Stage 4 `cut_polygon` share exact cut-slot centers; and verify every `bleed_polygon` also clears margins cleanly.

#### 11. `src/puzzle-gen/src/verify.rs` (Create)

*   **Type / Scope**: Rust Pre-Write Cross-Stage Verification Module
*   **Proposed Changes**: Implement `verify_invariants(ingested: &IngestedPuzzle, spliced: &SplicedPuzzle, kerfed: &KerfedGrid, layout: &PaginatedLayout, config: &Config) -> Result<(), PipelineError>` asserting before any disk writes that: total piece counts match across Stage 0, Stage 1, Stage 2, Stage 3, and Stage 4 page slots; every Stage 3 `bleed_polygon` and Stage 4 `cut_polygon` remains strictly inside `[margin_mm, page_size_mm - margin_mm]` and maintains `>= piece_spacing_mm - 1e-4` clearance between neighboring piece/corner-number AABBs; and Stage 3 piece `cut_polygon` slot centers and corner page-number vertices match Stage 4 canonical `mm` baked coordinates within `1e-4 mm`.

#### 12. `src/puzzle-gen/src/svg_emit.rs` (Create)

*   **Type / Scope**: Rust Stage SVG Projection Module
*   **Proposed Changes**: Implement deterministic `xmlwriter` serializers generating `StageOutputs`:
    *   `emit_stage1`: project into `IngestedPuzzle.frame.doc_unit`, emit single `<image>` in `<defs>` + 2x2 mirrored `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse">` using four `<use>` nodes, render `SplicedPiece` contours (`fill: url(#puzzle-pattern); stroke: #000000`), and emit `<g inkscape:groupmode="layer" inkscape:label="GridLabels">` (`0..rows-1` left, `0..cols-1` top, `fill: none; stroke: <score_color>`) expanding `viewBox` into negative top/left gutter space without shifting piece or pattern coordinates.
    *   `emit_stage2`: project into `doc_unit`, reuse the single `<image>` + 2x2 `<use>` `<pattern>`, wrap each `cut_polygon` kept in original puzzle coordinates inside `<g transform="translate(dx, dy)">`, emit `GridLabels` aligned to spaced row/col gutter coordinates, and expand root `width`, `height`, and `viewBox`.
    *   `emit_stage3`: project into `doc_unit`, include `<sodipodi:namedview>` with `<inkscape:page>` elements (`x="(p-1)*page_w"`, `y="0"`, `width="page_w"`, `height="page_h"`) sized to page 1 on the root `<svg>`, place `bleed_polygon` paths inside translated groups (`fill: url(#puzzle-pattern); stroke: none;`), emit visible stroked top-left/bottom-right vector digits (`fill: none; stroke: #000000`), and embed a run-metadata XML comment tying printed pages to Stage 4 outputs.
    *   `emit_stage4_page`: project into `config.page_unit` (default `mm`), omit `<defs>`, `<image>`, and `<pattern>` completely, bake all translations into flat `d="..."` coordinates (zero `transform` attributes), split `<g inkscape:groupmode="layer" inkscape:label="Cut">` (`fill: none; stroke: #000000`) from `<g inkscape:groupmode="layer" inkscape:label="PageNumbers">` (`fill: none; stroke: <score_color>`), and embed matching run-metadata XML comment.

#### 13. `src/puzzle-gen/src/main.rs` (Modify)

*   **Type / Scope**: Rust CLI Entrypoint & File Lifecycle Manager
*   **Proposed Changes**: Parse `<INPUT_SVG>`, `--config`, `--output-dir` (defaulting to `<input_svg_parent>/generated` or `./generated`), `--prefix` (defaulting to input file stem), `--overwrite` (default `false`), `--dry-run`, and `--no-strict` via `clap`; read inputs, call `puzzle_gen::run_pipeline`, and print diagnostics to `stderr`. Unless `--dry-run` is set: check `--overwrite == false` collisions against `-stage1.svg`, `-stage2.svg`, `-stage3.svg`, and any existing `<prefix>-stage4-page*.svg` before creating/writing files; create `--output-dir` via `create_dir_all` if needed; delete stale `<prefix>-stage4-page*.svg` files when `--overwrite` is enabled; write outputs atomically via same-directory temporary files + renames; and print a concise stdout summary (grid dimensions, matched edge count, page count, and file paths).

#### 14. `src/puzzle-gen/examples/default-config.yaml` (Create)

*   **Type / Scope**: Example YAML Configuration
*   **Proposed Changes**: Provide a documented reference YAML config file containing all default parameters (`version: 1`, `page_size: "6in x 6in"`, `page_unit: "mm"`, `margin: "0.25in"`, `kerf: "0.18mm"`, `segment_length: "1 kerf"`, `piece_spacing: "1 kerf"`, `print_bleed: "1 kerf"`, `score_color: "#0000ff"`) for CUJ 2 discovery.

#### 15. `src/puzzle-gen/tests/integration_pipeline.rs` (Create)

*   **Type / Scope**: Rust Integration & Regression Test Suite
*   **Proposed Changes**: Add end-to-end tests covering: ingesting `src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` under strict mode (asserting failure citing missing internal edge `path19`'s `(row, col)` coordinates) and under `--no-strict` (asserting 1 warning, 25 closed pieces, and straight edge fallback); a complete 40-tab synthetic 5x5 grid run asserting single-`<image>` + 4-`<use>` 2x2 pattern emission, Stage 1/2 `GridLabels` presence, Stage 3 `stroke: none` piece fills vs. visible stroked corner digits and `<inkscape:page>` pagination, Stage 4 `mm` projection without `<defs>`/`<image>`/`<pattern>`/`transform` attributes, and `Cut` vs. `PageNumbers` layer colors; `--overwrite=false` collision failures; `--overwrite=true` stale `-stage4-page*.svg` deletion when increasing `page_size` reduces page count; `cols * rows < 3` capacity errors; and `deny_unknown_fields` / unitless-number config rejections.

### Verification Plan

*   **Compilation/Build**: Run `cargo build -p puzzle-gen` and `cargo check --workspace` to confirm zero build errors and verify the sibling `src/sticker-size` workspace member remains unaffected.
*   **Unit Tests**: Run `cargo test -p puzzle-gen --lib` validating dimension grammar and kerf-multiplier math, `CoordFrame` aspect/unit checks, `0.01 mm` float-noise clustering, zero-span stray path filtering, tab snapping/reversal, clockwise edge splicing, chord-length and sagitta subdivision bounds, `i_overlay` offset positivity and neck-collapse failure handling, single-stroke vector digit placement, `cols * rows >= 3` pagination limits, and `<= 1e-4 mm` Stage 3/4 parity checks.
*   **Static Analysis & Linting**: Run `cargo clippy -p puzzle-gen --all-targets -- -D warnings` and `cargo fmt -p puzzle-gen -- --check`.
*   **Test Coverage Requirement**: Exercise every stage transition, unit type (`mm`, `cm`, `in`, `px`, `<N> kerf`), strict vs. `--no-strict` branch, `--overwrite` vs. stale-page pruning path, and verification error path across `--lib` and `--test` suites.
*   **E2E/Integration**: Run `cargo test -p puzzle-gen --test integration_pipeline` and execute an end-to-end CLI smoke run on `src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` with `--no-strict`, opening the generated `-stage3.svg` in Inkscape to verify `<inkscape:page>` PDF export and inspecting `-stage4-page*.svg` in XML/Inkscape to verify `mm` page units and `Cut`/`PageNumbers` layer separation.

--------------------------------------------------------------------------------

<!-- Provenance: Indicates initial structural drafting via EGM. -->

*Drafted with EGM*
