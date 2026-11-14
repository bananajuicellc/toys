# `puzzle-gen`: Four-Stage Laser Jigsaw Puzzle Generator

* **Author:** Tim Sweña (Swast)
* **Date:** 2026-11-13

## Overview

`puzzle-gen` is a local Rust CLI tool in `src/puzzle-gen` that prepares Inkscape jigsaw puzzle artwork for a two-step physical workflow: printing the puzzle image onto adhesive label paper via Inkscape's multi-page PDF export, adhering it to backing board, and cutting the pieces on a laser cutter.

Starting from a clean Inkscape SVG with three layers (`Image`, a rectangular grid of `<rect>` elements in `Pieces`, and cubic-Bezier `<path>` tabs in `Tabs`) plus an optional YAML configuration file, the tool computes an in-memory pipeline and writes four stage SVG outputs so each stage can be inspected for debugging:
1. **Stage 1 (`<prefix>-stage1.svg`):** Joins shared tab curves into closed piece `<path>` outlines (removing straight edges across tab necks) and fills them with a 2x2 mirrored `<pattern>` generated from the `Image` layer.
2. **Stage 2 (`<prefix>-stage2.svg`):** Flattens piece outlines to polylines using `segment_length`, expands each piece outward by `kerf / 2` for laser beam compensation, spaces pieces apart using post-offset bounding boxes, and locks each piece's pattern to its original image coordinates.
3. **Stage 3 (`<prefix>-stage3.svg`):** Reflows pieces in grid order across Inkscape multi-page `<inkscape:page>` sheets within configured margins for Inkscape PDF export, renders piece fills with `stroke: none` using a decoupled outward `print_bleed` (defaulting to full `kerf`), and draws visible stroked vector page numbers (`fill: none; stroke: #000000`) in top-left and bottom-right corners.
4. **Stage 4 (`<prefix>-stage4-pageN.svg`):** Writes one standalone single-page SVG per sheet containing page-local `kerf / 2` piece cut paths (`fill: none; stroke: #000000`) and identical corner page numbers with no `<pattern>` or raster `<image>`, ready for laser cutting.

## Analysis

Seven parallel Goldfish reviews audited the concept against the repository scaffolding ([Cargo.toml](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/src/puzzle-gen/Cargo.toml)) and the two example SVGs ([rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg) and [rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg)). Across the lenses and author clarifications, the key findings and resolutions cover architecture, input assumptions, and physical/geometric trade-offs.

### 1. In-Memory Pipeline with Four Debuggable Stage Outputs
* **Finding:** Writing Stage 1 to disk and having Stage 2 re-parse it—repeating for Stages 3 and 4—couples every stage to XML encoding details (`<pattern>` IDs, `<g transform>`, `<inkscape:page>`), loses grid `(row, col)` metadata, and tangles disk I/O with geometry testing. At the same time, emitting all four stage SVGs remains valuable for visually debugging each transformation step, while rolling custom SVG path parsers and non-convex polygon offsetters from scratch in an empty crate adds unnecessary maintenance work.
* **Resolution:** Keep a strict split between `lib.rs` (a pure in-memory pipeline `IngestedPuzzle -> SplicedPuzzle -> KerfedGrid -> PaginatedLayout` returning typed stage models and structured `Vec<Diagnostic>` warnings) and `main.rs` (CLI argument parsing, YAML config loading, and atomic file serialization). Emit all four stage SVGs as stateless projections of the in-memory pipeline so operators retain full stage-by-stage debuggability without inter-stage XML parsing. Normalize internal geometry to canonical millimeters (`mm`) behind a `CoordFrame` boundary, and reuse focused pure-Rust crates (`roxmltree`, `svgtypes`, `xmlwriter`, `kurbo`, and a polygon offset kernel such as `i_overlay` or `clipper2-rust`) scoped to `src/puzzle-gen/Cargo.toml` so `src/sticker-size` WASM builds remain unaffected.

### 2. Clean Stage 0 Input Contract & Practical Snap Handling
* **Finding:** While [rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg) shows manual Inkscape editing experiments (duplicate layers, `scale(1/96)` vs. `0.01` typos, broken paths), it was a rough sketch of intent rather than a specification for dirty inputs. Even clean Stage 0 files (like [step0](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg)), however, naturally contain minor Inkscape floating-point noise (`1e-8` coordinates), mixed relative/absolute `m`/`c`/`C` path verbs, reversed path directions, small endpoint gaps, or an omitted internal tab (`path19` missing among 40 internal edges).
* **Resolution:** Assume a clean Stage 0 input SVG (`Image`, `Pieces`, `Tabs` layers in consistent document units) without building special recovery heuristics for manual-edit artifacts. Compose root `viewBox` scale and any layer `<g transform>`, cluster `<rect>` coordinates within a small float tolerance to construct the grid, snap nearby tab endpoints onto internal edges, ignore stray zero-span paths, and emit actionable element-level diagnostics (citing element `id` and `(row, col)` edge) that fail by default on missing tabs (with `--no-strict` to allow straight edges with a warning), duplicate edge tabs, or unattached non-trivial paths, alongside `--dry-run`.

### 3. Contradiction A: Decoupled Print Bleed Tolerance vs. Cut Dimension Fidelity
* **Conflict:** Stage 3 removes piece strokes (`stroke: none`) so slight laser misregistration does not leave black lines on finished pieces. If Stage 3 uses the exact `kerf / 2` (`0.09mm`) Stage 2/4 cut boundary, any registration error beyond `0.09mm` (~0.0035 in) slices into unprinted white label paper. Expanding the laser cut path further would oversize the puzzle pieces.
* **Catalog Patterns Applied:** *Extraction / Taking Out* + *Beforehand Cushioning*.
* **Resolution Selected:** Decouple Stage 3 print geometry from Stage 2/Stage 4 cut geometry. Stage 2 and Stage 4 offset piece outlines outward by `kerf / 2` (`0.09mm`) to match the laser beam centerline, while Stage 3 offsets each piece's pattern-filled mask outward by a configurable `print_bleed` parameter (defaulting to full `kerf` = `0.18mm`). Because Stage 3 and Stage 4 intentionally use different outward offsets around the same slot centers, slight manual registration errors cut into bled image pixels without altering physical piece fit.
* **Trade-offs & Ideal Final Result (IFR):** Piece path outlines differ in offset radius between Stage 3 and Stage 4, requiring separate polygon offset calls per piece and slightly larger Stage 3/4 bounding envelopes. In the IFR, adhesive prints provide continuous image bleed past the laser kerf while cut pieces retain exact mechanical dimensions.

### 4. Contradiction B: Mating Fit Symmetry & Smooth Laser Motion vs. Polyline Offsetting
* **Conflict:** Offsetting each piece independently outward by `kerf / 2` widens outward tabs while narrowing inward sockets. At tight tab necks or small Bezier hooks near endpoints, self-intersection cleanup can pinch or clip a socket neck without clipping its mating tab. Meanwhile, naming a maximum segment length `polyline_density` is inverted (increasing the number lowers resolution), and subdividing straight edges every `0.1mm` inflates vertex counts and risks laser motion-planner stutter.
* **Catalog Patterns Applied:** *Beforehand Cushioning* + *Dynamics* + *Intermediary*.
* **Resolution Selected:** Rename `polyline_density` to `segment_length` (default `1 kerf` = `0.18mm`), keep straight edge segments as 2-point lines while sampling cubic Bezier curves at or below `segment_length` (plus chord-error bounds), run round-join polygon offsetting (`JoinType::Round`, matching circular laser kerf), simplify redundant collinear vertices strictly within sub-kerf tolerance (`<= 0.005mm`), and validate post-offset single-ring topology and tab-vs-socket neck clearance so pinched mates fail loudly.
* **Trade-offs & IFR:** Validating neck clearance couples adjacent edge mates during Stage 2 rather than treating pieces purely independently. In the IFR, mating tabs and sockets fit cleanly while cut files remain compact.

### 5. Contradiction C: Margin Safety, Zero Collisions & Visible Registration Anchors vs. Tight Spacing & Page Numbers
* **Conflict:**
  1. **Margin overhangs & wrapped-page collisions:** On wrapped Stage 3 pages, non-adjacent neighbors can point outward tabs toward each other, and tabs in `step0` protrude `> 0.25in`, overflowing `[margin, page_size - margin]` if positioned by nominal `<rect>` origins. While complex per-edge overhang packing could squeeze pieces closer, maintaining S-curve/pairwise nesting rules adds implementation complexity.
  2. **Page-number capacity & visibility:** Reserving two full piece-sized grid slots (top-left and bottom-right) leaves `0` piece slots if a usable page fits `<= 2` slots, and removing strokes globally in Stage 3 would erase the page numbers needed both to pick the right Stage 4 SVG and to align the sheet on the laser cutter.
* **Catalog Patterns Applied:** *Extraction / Taking Out* + *Segmentation*.
* **Resolution Selected:**
  - Keep spacing uniform and simple without pairwise edge-overhang nesting: compute each piece's **full post-offset axis-aligned bounding box (AABB)** including all four sides' protruding tabs (plus `print_bleed` clearance for Stages 3–4), and lay out grid rows/columns separated by `piece_spacing` strictly inside `[margin, page_size - margin]`.
  - Validate upfront that each page grid fits `cols * rows >= 3` slots so reserving top-left and bottom-right page-number slots always leaves at least 1 piece slot, failing fast with an actionable error otherwise.
  - Keep `stroke: none` strictly on Stage 3 puzzle pieces; keep visible stroked font-free vector page numbers (`fill: none; stroke: #000000`) in **both** Stage 3 and Stage 4 at identical page-local coordinates, scaled to fit inside the reserved corner slot with asymmetric orientation cues (such as a serifed `1` or baseline underline). Stage 4 may organize pieces and page numbers on distinct layers (`Cut` vs. `PageNumbers`) so operators can score or test-align numbers separately from piece cuts.
* **Trade-offs & IFR:** Full-AABB spacing on a uniform page grid uses slightly more sheet area than irregular nesting, trading peak material density for straightforward math, zero margin or tab-to-tab collisions, and visible alignment anchors in both stages. In the IFR, sheets stay inside margins with zero cut intersections and clear corner registration markers.

### 6. Contradiction D: Pattern Image Lock & Payload Size vs. Clean Pattern-Free Laser Output
* **Conflict:** Repeating base64 `<image>` bytes 4 times inside the 2x2 mirrored `<pattern>`—or cloning `<pattern>` definitions per piece—multiplies high-DPI SVG sizes by 4x–4Nx and risks SVG 1.1 `patternTransform` override bugs, while laser cutters expect Stage 4 files with zero patterns, zero raster images, and zero nested transforms.
* **Catalog Patterns Applied:** *Preliminary Action* + *Segmentation*.
* **Resolution Selected:** Emit `<image>` once in `<defs>` and construct the 2x2 mirrored `<pattern>` using four transformed `<use>` nodes. In Stages 1–3, keep image coordinates locked cleanly to each moved piece (via `<g transform="translate(dx, dy)">` over a single shared `<pattern patternUnits="userSpaceOnUse">`, verified against Inkscape PDF export) without cloning `<image>` bytes. In Stage 4, strip all `<defs>`, `<image>`, and `<pattern>` elements completely and emit flat, transform-free page-local `<path d="...">` outlines.
* **Trade-offs & IFR:** Separating print (transform-grouped) and cut (transform-flattened) emitters requires automated parity checks to ensure Stage 3 slot centers and Stage 4 cut coordinates match exactly. In the IFR, print SVGs store one image payload while Stage 4 laser SVGs contain pure vector paths.

### 7. Inkscape PDF Printing & Operational Safeguards
* **Inkscape PDF export:** Stage 3 targets Inkscape's multi-page `<inkscape:page>` structure inside `<sodipodi:namedview>` directly for printing via Inkscape's PDF export workflow.
* **CLI, config, and stale page cleanup:** Standardize CLI arguments (`--config`, `--output-dir`, `--prefix`, `--overwrite` defaulting to `false`, `--dry-run`, `--no-strict`), reject unknown YAML keys (`deny_unknown_fields`), resolve `"1 kerf"` multiplier syntax relative to configured `kerf`, write outputs atomically after all checks pass, prune leftover `<prefix>-stage4-page*.svg` files from prior runs when `--overwrite` is set, and embed run metadata to tie Stage 3 prints to Stage 4 cut pages.

## Goals vs. Non-Goals

* **In-Scope Goals:**
  * Read a clean Inkscape SVG with `Image` (`<image>`), `Pieces` (rectangular `<rect>` grid), and `Tabs` (cubic-Bezier `<path>`) layers plus optional YAML configuration (`page_size`, `page_unit` defaulting to `mm`, `margin`, `kerf`, `segment_length`, `piece_spacing`, `print_bleed`).
  * Normalize `viewBox` scaling, layer transforms, float noise on grid edges, and stray zero-span paths with clear element-level diagnostics.
  * Emit all four stage SVGs (`-stage1.svg`, `-stage2.svg`, `-stage3.svg`, and `-stage4-pageN.svg`) from an in-memory pipeline for debuggability, keeping original document units in Stages 1–3, rendering 0-based edge row/column vector labels (`GridLabels` layer, `0..rows-1` / `0..cols-1`) along the outer top/left borders of Stages 1 and 2, and using configured `page_unit` (default `mm`) in Stage 4.
  * Apply `kerf / 2` outward polygon offsetting with round joins for Stages 2 and 4 cut geometry, decoupled `print_bleed` (defaulting to full `kerf`) for Stage 3 unstroked fills, uniform full-AABB spacing inside margins, single-instance `<image>` 2x2 mirrored pattern bleed, Inkscape `<inkscape:page>` export support in Stage 3, pattern-free transform-free paths in Stage 4, and identical visible stroked vector corner page numbers (`fill: none; stroke: #000000`) in Stages 3 and 4.
  * Enforce piece-conservation, clearance, minimum page-capacity (`cols * rows >= 3`), and Stage 3-to-Stage 4 alignment parity checks before writing outputs.
* **Out-of-Scope Non-Goals:**
  * Non-rectangular or arbitrary Voronoi/freeform piece grids in `v1`.
  * Pairwise edge-overhang or irregular shape nesting optimizations in `v1`.
  * Handling arbitrary manual-edit SVG corruption beyond standard Inkscape float noise, stray zero-span clicks, and layer transforms.
  * Per-page Stage 3 SVG splitting or non-Inkscape multi-page print renderers (Inkscape PDF export is used for printing Stage 3).
  * Fitting smooth cubic Beziers back onto offset polylines in `v1`.
  * Interactive GUI or WASM integration with `src/sticker-size`, or external linked `<image>` files outside embedded `data:` URIs.

## Alternatives Weighed

* **Arbitrary non-rectangular piece polygons in v1:** *Rejected by author:* Assume a rectangular grid of `<rect>` elements for `v1` to keep edge matching and grid pagination predictable.
* **Spacing Stage 2 pieces strictly by `1 kerf` (`0.18mm`) without clearing tab depth:** *Rejected by author:* Pulling mating undercut tabs apart by `0.18mm` causes wide tab bulbs to cross adjacent socket necks.
* **Pairwise edge-overhang / S-curve nesting in Stage 2/3:** *Rejected by author:* Simple full post-offset bounding-box (`AABB`) spacing inside margins avoids collision edge cases without nesting complexity for `v1`.
* **Omitting Stage 1 and Stage 2 output files:** *Rejected by author:* Emitting all four stage SVGs (projected from an in-memory pipeline) makes intermediate geometry and pattern transformations inspectable for debugging.
* **Keeping Stage 3 fills coupled to Stage 2/4 `kerf / 2` cut offsets:** *Rejected by author:* `kerf / 2` (`0.09mm`) is too narrow for manual print/cut registration; Stage 3 uses a decoupled `print_bleed` offset (defaulting to full `kerf` = `0.18mm`).
* **Splitting Stage 3 into per-page SVGs for non-Inkscape viewers:** *Rejected by author:* Inkscape's PDF export handles multi-page `<inkscape:page>` printing directly.
* **Naming the curve-flattening parameter `polyline_density`:** *Rejected by author:* Renamed to `segment_length` so smaller values unambiguously indicate higher curve resolution.
* **Retaining `<defs>` / `<pattern>` in Stage 4 laser outputs:** *Rejected by author:* Stage 4 strips all patterns and raster images completely, leaving only vector cut and page-number paths.
* **Fitting cubic Beziers to offset curves or depending on external font files:** *Rejected by author:* Polylines controlled by `segment_length` and built-in font-free vector digits avoid curve-fitting instability and external font dependencies.
* **Vanishing Stage 3 page numbers under `stroke: none`:** *Rejected by author:* Corner page numbers stay visibly stroked (`fill: none; stroke: #000000`) in both Stage 3 and Stage 4 for sheet identification and manual laser alignment.

## Key Decisions & Staging for Next Steps

Resolved decisions carried forward into `/egm-draft`:

1. **In-Memory Pipeline, 4 Debuggable Outputs:** Keep pure in-memory geometry stages (`lib.rs`) projecting all four stage SVGs without SVG-to-SVG DOM re-parsing, including 0-based outer top/left vector row/column labels (`GridLabels` layer, `0..rows-1` and `0..cols-1`) in Stages 1 and 2 for visual debugging.
2. **Simple Full-AABB Spacing & Margin Compliance:** Space pieces using full post-offset bounding boxes (`AABB`) + `piece_spacing` strictly inside `[margin, page_size - margin]` (no pairwise overhang nesting), and enforce `cols * rows >= 3` per page.
3. **Decoupled Stage 3 Bleed (`print_bleed`) vs. Stage 4 Cut (`kerf / 2`):** Stage 3 fills piece masks offset outward by `print_bleed` (default `1 kerf`, `stroke: none`); Stages 2 and 4 emit cut paths offset outward by `kerf / 2`.
4. **Visible Stroked Corner Page Numbers & Inkscape PDF Print:** Top-left and bottom-right font-free vector page numbers keep `fill: none; stroke: #000000` at identical page-local coordinates in Stage 3 and Stage 4; Stage 3 uses Inkscape `<inkscape:page>` for PDF export; Stage 4 strips all patterns/images.
5. **Config Naming & Clean Stage 0 Scope:** Rename `polyline_density` to `segment_length` (default `1 kerf` = `0.18mm`); assume clean Stage 0 Inkscape input with float-epsilon snapping rather than accommodating arbitrary manual-edit artifacts.

Minor schema/CLI conventions resolved and carried into `/egm-draft`:
* Exact YAML keys (`version: 1`, `deny_unknown_fields`, `"1 kerf"` multiplier semantics, `score_color`) and CLI flags (`--config`, `--output-dir` defaulting to `<input_svg_parent>/generated` and created if needed, `--prefix`, `--overwrite` defaulting to `false`, `--dry-run`, `--no-strict`, stale `-stage4-page*.svg` pruning when `--overwrite` is enabled).
* Stage 4 layer separation (`Cut` `#000000` vs. `PageNumbers` `score_color`, default `#0000ff`) and strict-by-default Stage 1 validation (missing internal tabs fail unless `--no-strict` downgrades them to warnings).
