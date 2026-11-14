# Consolidated Goldfish Review Report

**Target Document:** [puzzle-gen.md](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/docs/designs/puzzle-gen.md) (`Lines 1-478`)  
**Semantic Split:** `FeatureRange` (`Lines 1-382`) | `DetailedRange` (`Lines 383-478`)  
**Execution Mode:** Consolidated Full Flow (Stages 5–9)

---

## Executive Summary & Reviewer Status

All 10 reviewer assignments across Stages 5–9 completed without timeouts or failures (`0` `not run`). Across all five stage reports, reviewers reported **70 raw findings** (`9` Blocker, `48` Major, `13` Nit), consolidated below into **32 deduplicated action items** (`7` Blocker, `17` Major, `8` Nit).

| Stage | Report File | Reviewer Lens | Status | Raw Blocker | Raw Major | Raw Nit |
| --- | --- | --- | --- | --- | --- | --- |
| Stage 5: Initial Zoom Out & TRIZ | [zoom_out_report.md](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/zoom_out_report.md) | `egm-zoom-out`, `egm-inventive-catalyst` | completed | 1 | 10 | 3 |
| Stage 6: Comprehension Test | [comprehension_report.md](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/comprehension_report.md) | `egm-comprehension` | completed | 0 | 7 | 2 |
| Stage 7: Critic Review | [critic_report.md](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/critic_report.md) | `egm-design`, `egm-structural`, `egm-style` | completed | 3 | 15 | 5 |
| Stage 8: Intermediate Zoom Out | [intermediate_zoom_out_report.md](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/intermediate_zoom_out_report.md) | `egm-zoom-out` | completed | 0 | 6 | 1 |
| Stage 9: Implementation Readiness | [implementation_readiness_report.md](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/implementation_readiness_report.md) | `egm-implementation-readiness`, `egm-codebase-details`, `egm-style` | completed | 5 | 10 | 2 |
| **Total (Raw / Consolidated)** | [consolidated_review_report.md](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/consolidated_review_report.md) | **10 / 10 completed** | **all completed** | **9 (7 consolidated)** | **48 (17 consolidated)** | **13 (8 consolidated)** |

---

## Stage 6 Comprehension Paraphrase

**What this proposes**
This document proposes building `puzzle-gen` in `src/puzzle-gen`, a local Rust CLI and pure-geometric library that transforms a clean "Stage 0" Inkscape SVG (containing `Image`, `Pieces`, and `Tabs` layers) and an optional YAML configuration file into four sequential SVG outputs through an in-memory millimeter pipeline. Stage 1 splices shared cubic-Bezier tabs into a rectangular grid of `<rect>` pieces, embeds the raster image once inside `<defs>` as a 2x2 mirrored `<pattern>`, and adds debug row/column `GridLabels`; Stage 2 flattens curves into polylines, applies outward `kerf / 2` cut and `print_bleed` offsets using integer-grid polygon buffering, and spaces pieces across a uniform grid while locking each piece's pattern to its original artwork slice. Stage 3 reflows stroke-free `print_bleed` pieces across Inkscape `<inkscape:page>` sheets with stroked corner vector page numbers for PDF printing, while Stage 4 outputs one standalone, transform-free and pattern-free SVG per page with separate `Cut` and `PageNumbers` layers for laser cutting. Before writing to disk, the tool enforces geometric and structural invariants (including missing-tab checks unless `--no-strict` is set, minimum `cols * rows >= 3` page capacity, margin/spacing clearances, and Stage 3/4 parity) and protects existing files unless `--overwrite` is enabled.

**How the existing system works, per this document**
According to The Problem section, preparing a custom Inkscape jigsaw puzzle for laser print-and-cut fabrication is currently a manual, error-prone Inkscape workflow: authors must manually duplicate, reverse, split, and join shared internal tab paths across every grid edge, manually maintain `patternTransform` offsets and mirrored pixel bleed, space pieces to avoid kerf-expanded tab-bulb collisions, and manually align multi-page print sheets with per-page laser files. Within the repository, the Overview describes `puzzle-gen` in present tense as a local Rust CLI utility in `src/puzzle-gen`, and the Detailed Implementation Plan marks `src/puzzle-gen/Cargo.toml` and `src/puzzle-gen/src/main.rs` as existing files to modify alongside example SVGs in `src/puzzle-gen/examples/` and a sibling WASM crate `src/sticker-size`. However, the document never describes what functionality currently exists in `src/puzzle-gen` or `src/sticker-size` prior to these proposed changes.

---

## Consolidated Prioritized Action Items

### 🔴 Blocker

#### `ACT-B1`: Fix Stage 1 Tab-to-Edge Matching, 3-Part Splicing, and `path19` Test Assertion
* **Sources:** `Z5-1`, `C6-5` (part), `CR7-4`, `IR9-1`
* **Affected Scope:** `#### Stage 1: Tab Splicing & 2x2 Mirrored Pattern` (lines `248-252`), `### Additional Resources` (line `372`), `#### 7. src/puzzle-gen/src/stage1.rs` (line `420`), `#### 15. src/puzzle-gen/tests/integration_pipeline.rs` (line `464`), `### Verification Plan` (line `472`)
* **Discrepancy:** Lines `248-249` and `420` require matching `TabCandidate` endpoints against internal edge corner vertices within `0.5 mm`, snapping endpoints onto clustered grid vertices, and replacing the entire straight edge directly with the curve. In `rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg`, all 39 tab `<path>` curves start and end in the **interior** of internal edge segments (5–10 mm from corners; e.g., `path1` spans `(0.80000027, 0.3202571)` to `(0.79999998, 0.4526716)` on `x = 0.8`), so literal vertex matching rejects every tab and breaks line `464`/`472`. Additionally, `step0` omits `path19` from the XML entirely, so strict mode cannot cite `"path19"`; the actual missing edge is `((2, 0), (3, 0))` along `y = 2.4in, x in [0, 0.8in]`.
* **Action:** Specify point-to-segment matching (`perp_dist <= 0.5 mm` and longitudinal projection within `[-0.5 mm, edge_len + 0.5 mm]`, with outer-border and 4-way corner disambiguation), project/snap endpoints onto the clustered edge line, reverse when `proj(end) < proj(start)`, and splice 3 parts (`LineTo(tab_entry)` if non-zero, `CubicSeg` chain, `LineTo(edge_end)` if non-zero). Update line `464` (and `372`) to assert missing internal edge `((2, 0), (3, 0))`.

#### `ACT-B2`: Heal Self-Intersecting Tab-Neck Loops Before Stage 2 Offsetting
* **Sources:** `Z5-1` (neck bowtie), `CR7-3`, `IZ8-5`
* **Affected Scope:** `#### Stage 1` (lines `248-252`), `#### Stage 2` (lines `256-261`), `### Additional Resources` (line `372`)
* **Discrepancy:** Multiple tabs in `step0` (`path1` line `205`, `path2` line `208`, `path10` line `234`) have Inkscape Bezier handles that double back across straight grid lines, creating bowtie self-intersections when spliced into Stage 1 contours and risking multi-island or inverted-loop failures in Stage 2's single-ring check (line `261`).
* **Action:** Keep Stage 1 topological and run an `i_overlay` self-union healing pass (`OverlayRule::Subject` with explicit `FillRule::NonZero`) in Stage 2 after flattening and before measuring un-offset area and running outward buffering, avoiding custom cubic-line root-finding in Stage 1.

#### `ACT-B3`: Fix `2x2` Mirrored `<pattern>` Double-Offset, `<use>` Transform Syntax, and `preserveAspectRatio`
* **Sources:** `Z5-11`, `C6-4`, `CR7-2`, `IR9-2`
* **Affected Scope:** `IngestedPuzzle` (line `192`), `Stage 0` (lines `242-243`), `Stage 1` (line `253`), `Stage 2` (line `262`), `Stage 3` (line `276`), `svg_emit.rs` (lines `446-448`)
* **Discrepancy:** Specifying `<image>` at `(img_x, img_y)` inside `<defs>` AND `<pattern patternUnits="userSpaceOnUse" x="img_x" y="img_y" ...>` double-shifts `<use>` tiles whenever `img_x != 0 || img_y != 0`; setting `x="W"` alongside `transform="translate(2W, 0) scale(-1, 1)"` on `<use>` maps `[0, W]` back onto `[0, W]`; omitting `preserveAspectRatio="none"` causes default `xMidYMid meet` letterboxing on non-square rasters; and `image_bounds_mm: Rect` discards non-axis-aligned `<image>` transforms.
* **Action:** Retain `image_preserve_aspect_ratio` on `IngestedPuzzle` (default `"none"`), emit `<image>` inside `<defs>` at `x="0" y="0" width="W" height="H"`, apply `x="img_x" y="img_y"` solely on `<pattern>`, omit `x`/`y` attributes on transformed `<use>` elements, reject rotated/skewed/flipped `<image>` transforms on Stage 0 ingest, and warn if `image_bounds_mm` does not cover `Pieces`.

#### `ACT-B4`: Replace Non-Existent `i_overlay::JoinType::Round` API, Specify Winding/Hole Checks, and Fix S-Curve Subdivision & Zero-Offset Validation
* **Sources:** `IR9-3`, `Z5-9`
* **Affected Scope:** `Cargo.toml` (line `390`), `### Architecture Overview` (line `112`), `### Core Domain Data Structures` (`KerfedPiece.bleed_polygon`, line `209`), `#### Stage 2` (lines `256-261`), `stage2.rs` (lines `422-425`)
* **Discrepancy:**
  1. `i_overlay` uses `IntOutlineOffset` / `OutlineOffset`, `OutlineStyle`, and `LineJoin::Round(<precision>)`—not `JoinType::Round`—and leaves the arc-precision parameter unspecified.
  2. SVG Y-down clockwise (`+area` / math CCW) winding normalization and explicit no-hole rejection (`shapes.len() == 1 && shapes[0].len() == 1`) are omitted.
  3. Applying the strict single-ring neck check to `bleed_polygon` (`+print_bleed`) causes unnecessary pipeline failures when large bleed values bridge across empty socket cavities (`Z5-9`), and requiring `"larger than the un-offset piece"` breaks when `kerf == 0` or `print_bleed == 0`.
  4. Midpoint chordal deviation misses S-curves crossing the chord at `t = 0.5` and lacks a recursion depth cap.
* **Action:** Pin exact `i_overlay` APIs and `LineJoin::Round` precision; normalize contours to positive signed area before offsetting; require `shapes.len() == 1 && shapes[0].len() == 1` on `cut_polygon` while letting `bleed_polygon` retain its outer positive-area ring containing `cut_polygon`; allow `area >= unoffset_area` when offset is `0` (or require `> 0.0`); and use control-point-to-chord flatness + chord length `<= segment_length_mm` with a recursion depth cap.

#### `ACT-B5`: Fix `CoordFrame::from_svg_root` (`<sodipodi:namedview>` Units, `viewBox` Origin, `ingest_svg` Signature Conflict)
* **Sources:** `IR9-4`, `C6-7` (part), `CR7-8` (part)
* **Affected Scope:** `### YAML Configuration Schema & Unit Normalization` (lines `151-153`), `CoordFrame` (lines `158-163`), `Stage 0` (line `242`), `Stage 1–4` (lines `253`, `262`, `275`, `279`), `config.rs` (lines `402-405`), `ingest.rs` (lines `412-415`), `svg_emit.rs` (lines `442-449`)
* **Discrepancy:** Both Inkscape reference SVGs place `inkscape:document-units="in"` on `<sodipodi:namedview>`, not `<svg>`, so `CoordFrame::from_svg_root` fails on real Inkscape inputs, and Stages 1, 2, and 4 omit `<sodipodi:namedview>` despite line `279` requiring `inkscape:document-units`. Additionally, `CoordFrame` declares `page_unit` (not passed into `ingest_svg`), omits `viewbox_min_user: (f64, f64)` needed by line `242`, misstates SVG unitless `width`/`height` (`96 DPI px`) precedence, and omits aspect-ratio tolerance and Stage 1–3 `1 user unit = 1 doc_unit` normalization.
* **Action:** Read `inkscape:document-units` from `<sodipodi:namedview>` (falling back to `<svg>`), emit `<sodipodi:namedview inkscape:document-units="...">` in Stages 1–4 (or standardize outputs on `mm` per `ACT-M3`), move `page_unit` off `CoordFrame` (or pass it to `ingest_svg`), add `viewbox_min_user`, and specify unitless `width`/`height`, aspect-ratio tolerance, and Stage 1–3 unit-scale normalization rules.

#### `ACT-B6`: Add Dedicated `## Risks & Limits` / `## Risks & Mitigations` Section
* **Sources:** `CR7-1`, `IZ8-4` (scope guard)
* **Affected Scope:** `FeatureRange` (`lines 1-382`, after `## Non-Functional Requirements`)
* **Discrepancy:** `feature-spec-standard.md` requires a dedicated `Risks & Limits` or `Risks & Mitigations` section documenting failure modes, limits, and mitigations, which is absent from `FeatureRange`.
* **Action:** Add a concise `## Risks & Limits` section inside `FeatureRange` covering real geometric and operational risks (`i_overlay` `i32` coordinate bounds at micrometer scale, self-intersecting/collapsed-neck inputs, zero-kerf edge cases, overwrite/glob safety, and laser CAM stroke/layer compatibility) without adding speculative enterprise SLAs or adversarial memory caps (`IZ8-4`).

#### `ACT-B7`: Resolve Dependency Choice, Pin Crate Versions, Add `Cargo.lock`, and Fix Repo-Polluting Smoke Path
* **Sources:** `IR9-5`, `CR7-16` (YAML part), `IZ8-7` (YAML part)
* **Affected Scope:** `### Architecture Overview` (line `111`), `Target Files to Create/Modify` (lines `385-391`), `### Verification Plan` (line `472`)
* **Discrepancy:** Leaves `serde_norway (or serde_yaml_ng)` undecided, omits version/feature constraints for all 8 crates, omits tracked root `Cargo.lock` from Target Files, and omits `--output-dir` in line `472`'s smoke command (writing unignored outputs into `src/puzzle-gen/examples/generated/`).
* **Action:** Commit to one YAML crate, pin major/minor versions and features in `Cargo.toml` item 1 (plus test temp-dir strategy), add `Cargo.lock (Modify)` to Target Files, and pass `--output-dir target/smoke-out` on line `472`.

---

### 🟡 Major

#### `ACT-M1`: Resolve Altitude Drops vs. Module Fragmentation & Missing Cross-Module Type Definitions
* **Sources:** `Z5-2`, `Z5-3`, `Z5-5`, `C6-8` (types part), `CR7-16` (types part), `IZ8-1`, `IR9-7`
* **Affected Scope:** `### Architecture Overview` (lines `83-107`, `112`), `### Core Domain Data Structures` (lines `155-236`), `### Stage-by-Stage Geometric Pipeline` (lines `238-286`), `### Target Files to Create/Modify` (lines `385-464`)
* **Discrepancy:** `FeatureRange` drops into 80 lines of concrete Rust structs, ~10 hardcoded numeric tolerances, and SVG transform syntax while splitting a single-user local CLI into 12 Rust files with `DiagnosticBag`/`StageId` ceremony and custom `CubicSeg`/`PolygonRing` wrappers around `kurbo`. Conversely, at the implementation-plan level, `PipelineResult`, `SplicedPuzzle`, `KerfedGrid`, `TabCandidate`, `CubicSeg`, `StageOutputs`, and `svg_emit` signatures are missing or mismatched (`PolygonRing` closure, `Polyline` vs. `Vec<Vec<Point>>`).
* **Action:** Follow Stage 8 (`IZ8-1`): lift `Core Domain Data Structures` in `FeatureRange` to high-level stage inputs/outputs and invariants, consolidate the crate into 3–4 cohesive modules (`config`, `geometry`/`pipeline`, `svg`, `main`) using `kurbo` primitives directly and standard `Result` + `stderr` warnings instead of `DiagnosticBag` / `CubicSeg` / `PolygonRing`, and define exact cross-module function signatures and types in `DetailedRange`.

#### `ACT-M2`: Eliminate or Fix Redundant `verify.rs` Runtime Pass
* **Sources:** `Z5-4`, `C6-6` (parity part), `CR7-6` (parity part), `IZ8-1` (verify part), `IR9-10` (`verify.rs` part), `IR9-14` (`verify.rs` part)
* **Affected Scope:** `Stage 3 & Stage 4` item 5 (lines `283-286`), `## Non-Functional Requirements` (line `366`), `#### 11. src/puzzle-gen/src/verify.rs` (lines `437-440`)
* **Discrepancy:** `verify_invariants` re-checks `PaginatedLayout` against itself (`in canonical mm before unit projection`), mislabels Stage 3 as emitting `cut_polygon` instead of `bleed_polygon`, and enforces zero-epsilon `[margin_mm, page_size_mm - margin_mm]` bounds that fail under `1e-15` IEEE-754 error or `0.005 mm` simplification center drift.
* **Action:** Remove `verify.rs` and enforce layout invariants by construction plus unit/integration tests on serialized Stage 3/4 SVGs (`Z5-4`, `IZ8-1`); or, if kept, verify projected Stage 3/4 SVG output coordinates with a `1e-4 mm` margin epsilon and `page_width_mm` / `page_height_mm` bounds.

#### `ACT-M3`: Simplify Speculative YAML Config DSL & Knobs (or Pin Missing Validation Rules)
* **Sources:** `Z5-6`, `C6-7` (YAML part), `CR7-7`, `IZ8-3`, `IR9-10` (`config.rs` part), `IR9-13` (strict YAML part)
* **Affected Scope:** `## Goals` (lines `34`, `39`), `### YAML Configuration Schema` (lines `129-153`), `#### Stage 2` (lines `256-260`), `config.rs` (lines `402-405`), `default-config.yaml` (lines `456-459`)
* **Discrepancy:** `version: 1`, `"6in x 6in"` / `"1 kerf"` string mini-DSLs, `segment_length`, `page_unit`, and `score_color` add speculative parsing and cross-field failure modes (`kerf: 0mm` causes infinite subdivision when `segment_length = 1 kerf`; `0.005 mm` simplification fails `kerf < 0.05 mm`; `version != 1`, empty YAML, `>= 0` vs. `> 0` bounds, and `#RRGGBB` case sensitivity are unspecified; `"strict YAML"` overloads `--no-strict`).
* **Action:** Simplify by dropping `version: 1`, string DSLs (`"N kerf"`, `"W x H"`), `segment_length` (derive safely from `kerf`), `page_unit` (standardize on `mm`), and `score_color` (`Z5-6`, `IZ8-3`); or explicitly document all missing YAML/bounds validation rules (`segment_length > 0`, `min(0.005 mm, segment_length_mm * 0.05)` simplification, `kerf/margin/spacing/bleed >= 0`, `version == 1`, empty YAML defaults) and rename `"strict YAML"` to `"schema-validated YAML (deny_unknown_fields)"`.

#### `ACT-M4`: Decouple Corner Page Numbers from Full Piece-Sized Slots (`cols * rows >= 3`)
* **Sources:** `Z5-8`, `CR7-11`, `CR7-18` (part), `IZ8-2`, `IR9-10` (min-size part)
* **Affected Scope:** `## Goals` (lines `37-38`), `### CUJ 3` (lines `74`, `78`), `#### Stage 3 & Stage 4` (lines `265-273`), `### Alternative C` (line `307`), `stage3_4.rs` (line `435`)
* **Discrepancy:** Coupling tiny corner page numbers to full `slot_w x slot_h` grid slots sacrifices two piece slots per sheet, rejects sheets where `page_cols * page_rows < 3`, leaves `"minimum page_size needed for 3 slots"` ambiguous (`1x3`, `3x1`, `2x2`), omits multi-digit spacing and registration point targets, and has no comparison in `Alternatives Considered`.
* **Action:** Per Stage 8 (`IZ8-2`), statically place compact fixed-size vector digits (with a sharp baseline/corner L-tick as a laser alignment target) in a corner margin or compact strip outside the piece grid (`page_cols * page_rows >= 1`), and add the margin-vs-slot trade-off to `## Alternatives Considered` (or fully specify multi-digit slot scaling, padding, and 2D minimum `page_size`).

#### `ACT-M5`: Simplify or Specify Debug `GridLabels` Vector-Digit Gutters and Negative `viewBox`
* **Sources:** `Z5-7`, `C6-8` (gutters part), `CR7-5` (`GridLabels`/score_color part), `IZ8-3` (`GridLabels` part), `IR9-9` (`GridLabels` part)
* **Affected Scope:** `## Overview` (lines `15-16`), `## Goals` (line `34`), `Stage 1` (line `253`), `Stage 2` (line `262`), `digits.rs` (line `430`), `svg_emit.rs` (lines `446-447`)
* **Discrepancy:** Screen-only debug Stages 1 and 2 add a second vector-digit layout engine (`layout_gutter_index`), gutters, and negative-`viewBox` expansion while leaving gutter dimensions, `anchor_box_mm`, `>= 10x10` multi-digit sizing, proportional root `width`/`height` expansion, and `score_color`'s use in Stages 1–2 (`line 150` vs. `253/262`) unspecified.
* **Action:** Drop vector-digit gutters and negative `viewBox` expansion in debug-only Stages 1–2 (rely on `r{row}_c{col}` IDs, `<title>`, or standard `<text>`), dissolving `anchor_box_mm` and `score_color` drift (`Z5-7`, `IZ8-3`); or define exact gutter geometry and update lines `38` and `150` to clarify `score_color` covers Stages 1–2 and Stage 4 (`PageNumbers` layer name is fixed).

#### `ACT-M6`: Define `<stroke_w>`, Stage 2 Styling, Digit Padding, Float Precision, Stage 3 Layers, `--dry-run`, and Drop `run-metadata` Comment
* **Sources:** `Z5-13`, `C6-1`, `C6-3`, `CR7-5` (`<stroke_w>`/Stage 2 part), `IZ8-3` (`stroke_w` part), `IZ8-6` (comment part), `IR9-9`
* **Affected Scope:** `CLI Interface` (line `122`), `Stage 1–4` (lines `253`, `262`, `277`, `281-282`), `digits.rs` (line `430`), `svg_emit.rs` (lines `442-449`), `main.rs` (line `454`)
* **Discrepancy:**
  1. `<stroke_w>` (lines `277`, `281`, `282`) is never defined, Stage 1 lacks `stroke-width`, and Stage 2 pieces omit `fill`/`stroke`/`stroke-width`.
  2. `digits.rs` leaves `10%` padding (per-side vs. total), inter-digit spacing, baseline gap, and `<path>` grouping open; Stage 3 leaves layer wrapping and digit transforms open; serialization precision is unstated (4-decimal inches breaks `<= 1e-4 mm` parity); `--dry-run` stdout metrics are undefined; line `448` misplaces `"sized to page 1"` on `<inkscape:page>`.
  3. Lines `448-449` introduce an unspecified `"run-metadata XML comment"` absent from `FeatureRange`.
* **Action:** Pin a concrete physical stroke width (e.g., `0.1 mm` projected per stage) and Stage 2 piece styling; specify digit padding/grouping, Stage 3 layer layout, float precision (`>= 6` decimal inches / `>= 4` decimal `mm`), and `--dry-run` stdout; fix line `448` modifier placement; and delete the speculative `"run-metadata XML comment"` (`Z5-13`, `IZ8-6`).

#### `ACT-M7`: Define Stage 2 & Stage 3/4 Slot Anchoring Formulas, Fix `slot_w` Shadowing, and Symmetric Bleed Bounds
* **Sources:** `Z5-14`, `C6-6` (slot part), `CR7-6` (slot part), `IR9-8`, `IR9-10` (grid-origin/centering part)
* **Affected Scope:** `KerfedPiece` (line `210`), `PageSlot` (lines `215-216`), `Stage 2` (line `262`), `Stage 3 & 4` (lines `266-277`), `stage2.rs` (line `425`), `stage3_4.rs` (line `435`)
* **Discrepancy:** Stage 2 (`stage2_offset_mm`) and `FeatureRange` Stage 3/4 omit piece-anchoring formulas inside `[slot_w x slot_h]` cells (naive cell-pitch shifts cause neighbor cut collisions) and leave page slot-grid origin (`(margin_mm, margin_mm)` vs. centered in `usable_w x usable_h`) unspecified. Line `262` defines `slot_w`/`slot_h` using `cut_aabb`, which is shadowed on lines `266-267` by `max(cut_aabb, bleed_aabb)`. Additionally, centering by `cut_aabb.center()` while sizing slots by `bleed_aabb` risks pushing `bleed_polygon` up to `0.005 mm` outside margins when simplification shifts `bleed_aabb.center()`.
* **Action:** Specify slot-centered (or min-corner) placement for Stage 2 (`stage2_slot_w`, `stage2_slot_h`), specify `(margin_mm, margin_mm)` slot-grid origin and `piece_offset_mm = slot_center_mm - cut_aabb.center()` in `FeatureRange`, and size `slot_w`/`slot_h` symmetrically around `cut_aabb.center()` (`2 * max(|min - cut_center|, |max - cut_center|)` across both polygons).

#### `ACT-M8`: Harden Stage 4 Filename Matching, Validate `--prefix`, Fix Deletion Ordering, and Support Debug Outputs on Downstream Errors
* **Sources:** `Z5-10`, `CR7-9`, `IZ8-6` (CLI part), `IR9-14` (`main.rs` atomicity part)
* **Affected Scope:** `## Goals` (line `40`), `### CLI Interface & File Lifecycle` (lines `114-127`), `## Non-Functional Requirements` (lines `365-366`), `lib.rs` (line `395`), `stage2.rs` (line `425`), `stage3_4.rs` (line `435`), `main.rs` (line `454`)
* **Discrepancy:** `<prefix>-stage4-page*.svg` globbing risks false collision blocks or deleting unrelated files; `--prefix` allows `/` or `..` path traversal; deleting old Stage 4 pages *before* writing new files leaves a torn directory on write failure (overclaimed as multi-file `"atomically"` on lines `40`, `366`, `454`); and downstream Stage 2/3 errors suppress Stage 1/2 debug SVGs and stop on the first defective piece.
* **Action:** Match Stage 4 files strictly via `^<prefix>-stage4-page[1-9][0-9]*\.svg$`; validate `--prefix` as a single path-free filename stem; write/rename new stage files *before* pruning old `stage4-page{K}.svg` (`K > new_page_count`); describe per-file temp-file replacement accurately; and consider retaining valid Stage 1/2 debug outputs when Stage 2/3 fails (`Z5-10`, e.g., `--emit-debug-on-error`).

#### `ACT-M9`: Clarify Stage 0 Layer/Path Descriptions, Quadratic Elevation, and Stray-Click / `Z` Precedence
* **Sources:** `C6-5` (Stage 0 part), `CR7-8` (Stage 0 part), `CR7-15`, `IZ8-6` (Stage 0 scope part), `IR9-11`
* **Affected Scope:** `## Overview` (line `13`), `## Goals` (line `33`), `### CUJ 1` (line `59`), `### Architecture Overview` (line `94`), `#### Stage 0` (lines `241-245`), `ingest.rs` (lines `412-415`)
* **Discrepancy:** Prose says `"three layers"` and `"cubic-Bezier <path>"` while Stage 0 ignores extra helper layers and parses general path commands (`M/L/H/V/C/S/Q/A/Z`). `ingest.rs` omits `SimplePathSegment::Quadratic` elevation (`kurbo::QuadBez::raise()`), precedence between `< 0.05 mm` stray-path filtering and `ClosePath` rejection (broken by closed stray clicks like `path4` in `manual-attempt.svg`), misleading `"zero-span stray clicks"` naming, rotated-`<rect>` epsilon, duplicate-layer/foreign-child handling, and `R x C` grid verification rules.
* **Action:** Update prose to `"required Image, Pieces, and Tabs layers"` and `"open <path> curves"`; keep Stage 0 feature rules concise (`IZ8-6`); and in `ingest.rs`, document `Quadratic` elevation, running sub-`0.05 mm` stray-path filtering before `ClosePath`/multi-subpath checks, rotated-`<rect>` tolerance, duplicate-layer/child errors, and exact `R x C` clustering checks.

#### `ACT-M10`: Partition `Detailed Implementation Plan` Into Incremental Buildable/Testable Phases
* **Sources:** `IR9-6`
* **Affected Scope:** `## Detailed Implementation Plan` (lines `383-478`)
* **Discrepancy:** Lists 15 files linearly with `lib.rs` (item 2) declaring submodules created in items 3–12, preventing incremental builds or unit tests until nearly all files exist.
* **Action:** Group file changes into ordered phases (e.g., Phase 1: `Cargo.toml`/`Cargo.lock` + config/model + unit tests; Phase 2: Stage 0–2 geometry + unit tests; Phase 3: Pagination + SVG emit + CLI + integration tests), verifying `cargo check` and `cargo test` after each phase.

#### `ACT-M11`: Remove Unlinked Author Attribution in `## Alternatives Considered`
* **Sources:** `CR7-10`
* **Affected Scope:** `## Alternatives Considered` (lines `290-351`, lines `296..350`)
* **Discrepancy:** All 10 `Reason for Rejection` bullets start with `"Rejected by author:"` without linking an issue, discussion, or PR, violating `feature-spec-standard.md` (`## The Alternatives Rule`), and most repeat the preceding `Cons` bullet.
* **Action:** Remove `"Rejected by author:"` (or add issue/PR links), lead with the concrete technical/scope trade-off, and eliminate duplication of `Cons`.

#### `ACT-M12`: Fix Broken Heading Hierarchy After `## Non-Functional Requirements`
* **Sources:** `CR7-12`
* **Affected Scope:** Lines `354-381` (`## Non-Functional Requirements`, line `368`, `### Additional Resources`, `### Future Improvements`)
* **Discrepancy:** `### Additional Resources` (line `370`) and `### Future Improvements` (line `375`) use H3 below a top-level divider (line `368`) without an H2 parent, nesting them under `## Non-Functional Requirements`.
* **Action:** Promote both headings to `##` (`## Additional Resources`, `## Future Improvements`).

#### `ACT-M13`: Resolve `Diagnostic.severity` `Info` Schema Contradiction
* **Sources:** `CR7-13`
* **Affected Scope:** `Diagnostic` (line `231`), `Stage 0` (line `245`)
* **Discrepancy:** `Diagnostic.severity` lists only `Warning | Error`, while line `245` emits an `"informational diagnostic"` for filtered stray clicks.
* **Action:** Add `Info` to `Diagnostic.severity` (or eliminate `DiagnosticBag` per `ACT-M1`) or change stray-click reporting to a `Warning` / stdout note.

#### `ACT-M14`: Calibrate White-Edge Bleed Guarantees and Curve Deviation / Vertex Claims
* **Sources:** `CR7-7` (sagitta/vertex part), `CR7-14`
* **Affected Scope:** `## The Problem` (line `25`), `### CUJ 1` (line `63`), `#### Stage 2` (line `256`), `### Alternative G` (lines `331-332`), `## Non-Functional Requirements` (line `359`)
* **Discrepancy:** Lines `25` and `63` unconditionally promise zero white edge slivers (valid only within `print_bleed - kerf / 2`); lines `332` and `359` claim `"near 0.002mm"` sagitta error while line `256` specifies `segment_length_mm * 0.1` (`0.018mm` at default `0.18mm`); line `331` makes an unverified `"nearly twice as many path vertices"` claim for `0.1mm`.
* **Action:** Qualify lines `25` and `63` with the `print_bleed - kerf / 2` alignment tolerance, align lines `332` and `359` with line `256`'s `<= segment_length * 0.1` bound, and remove the unverified `2x` vertex-count claim in Alternative G.

#### `ACT-M15`: Fix CUJ 2 Arithmetic/Terminology, Pagination Notation Collisions, and Parameter/Flag Naming
* **Sources:** `CR7-17`, `CR7-18`, `IR9-13`
* **Affected Scope:** `## Goals` (line `37`), `### CUJ 2` (lines `67`, `70`), `### CUJ 3` (line `78`), `#### Stage 3 & 4` (lines `272-273`, `285`), `Alternative C` (line `308`), `stage1.rs` (line `420`), `integration_pipeline.rs` (line `464`), `Verification Plan` (line `469`)
* **Discrepancy:** CUJ 2 implies `print_bleed: "1 kerf"` without listing the override value, conflates `8.5in x 11in` page size with printable area (`7.5in x 10in`), and uses unanchored `"thicker wood"`; lines `37`, `78`, `464`, `469` write `cols * rows` instead of `page_cols * page_rows`; lines `37`, `285`, `308` use 1D `[margin, page_size - margin]` for 2D bounds; line `464` writes `--overwrite=false/true` for a boolean CLI flag; line `420` writes `(!no_strict)` for `strict: bool`.
* **Action:** State explicit CUJ 2 config overrides and distinguish page size from printable area; use `page_cols * page_rows`, 2D margin rectangles, `without --overwrite` / `with --overwrite`, and `(strict == true)` / `(when strict is false)`.

#### `ACT-M16`: Enumerate CLI & Edge-Case Integration Tests, Separate Manual Inkscape Checks, and Calibrate Workspace Check Claims
* **Sources:** `IR9-12`, `IR9-14` (`Verification Plan` part)
* **Affected Scope:** `#### 15. integration_pipeline.rs` (line `464`), `### Verification Plan` (lines `466-472`)
* **Discrepancy:** `integration_pipeline.rs` omits CLI binary tests (`--dry-run`, `--output-dir`, `--prefix`, non-zero exit codes) and edge-case tests (`<sodipodi:namedview>` units, interior tab splicing, closed stray clicks, rotated `<rect>`, duplicate/border tabs, Stage 3/4 parity, `Quadratic`/`Arc`); line `468` overclaims `cargo check --workspace` as verifying `src/sticker-size` `"remains unaffected"`; line `471` claims covering `"every"` branch and misphrases `"--overwrite vs. stale-page pruning"`; line `472` embeds manual Inkscape PDF inspection inside automated tests without pass criteria.
* **Action:** Enumerate all required CLI/edge-case tests in item `15`, calibrate line `468` (`"compiles cleanly"`) and line `471` (`"--overwrite disabled vs. enabled (including stale-page pruning)"`, enumerating error paths), and split manual Inkscape verification into a separate bullet with concrete pass criteria (page count, dimensions, unclipped artwork, layer separation).

#### `ACT-M17`: Split Overlong Run-On Paragraphs in `Target Files to Create/Modify`
* **Sources:** `IR9-15`
* **Affected Scope:** `### Target Files to Create/Modify` (lines `390`, `405`, `415`, `420`, `425`, `435`, `454`, `464`)
* **Discrepancy:** Eight `Proposed Changes` descriptions are 78–131 word semicolon-chained run-on sentences that obscure control flow, precedence, and modifier attachment.
* **Action:** Break each multi-responsibility entry into concise sub-bullets matching `svg_emit.rs` (lines `446-449`).

---

### 🟢 Nit

#### `ACT-N1`: Document Empty Stub Baseline of `src/puzzle-gen` and Consolidate `src/sticker-size` WASM Caveats
* **Sources:** `C6-2`, `IZ8-7`
* **Affected Scope:** `## Overview` (line `11`), `## Non-Goals` (line `49`), `### Architecture Overview` (lines `109-111`), `Target Files` (lines `387`, `390`, `451`, `468`)
* **Discrepancy:** Describes `puzzle-gen` in present tense and references `src/sticker-size` WASM isolation in four places without explaining that `src/puzzle-gen` is currently an empty 3-line stub (`src/puzzle-gen/src/main.rs`) and `src/sticker-size` is a separate workspace member (`Cargo.toml` has no `[workspace.dependencies]`).
* **Action:** State once in `Overview` / `Architecture Overview` that `src/puzzle-gen` is currently a 3-line `"Hello, world!"` stub in a multi-crate workspace alongside the sibling `src/sticker-size` WASM sprite inspector, and drop repeated WASM-isolation caveats.

#### `ACT-N2`: Remove Stage 2 Slot Fields from `KerfedPiece`
* **Sources:** `Z5-12`
* **Affected Scope:** `### Core Domain Data Structures` (`KerfedPiece`, lines `204-212`), `Stage 2` (line `262`), `Stage 3 & 4` (lines `266-267`)
* **Discrepancy:** `KerfedPiece` stores `stage2_offset_mm` and `stage2_aabb_mm` computed from `cut_aabb` separately from the unified `max(cut_aabb, bleed_aabb)` slot size.
* **Action:** Compute Stage 2's debug grid translation locally during Stage 2 emission or reuse the unified slot pitch so `KerfedPiece` holds only `cut_polygon` and `bleed_polygon`.

#### `ACT-N3`: Standardize Dividers, Blank Lines, List Formatting, and Stage 1/2 Sub-Bullets
* **Sources:** `CR7-19`, `CR7-20`
* **Affected Scope:** Lines `3-286` vs. `294-379`, lines `7`, `15-18` vs. `241-286`, lines `19-81`, `253`, `262`, `288`, `352`, `368`, `381`
* **Discrepancy:** Asymmetric H2 horizontal rules and double blank line (`79-80`); colons placed inside bold in lines `3-286` (`**Author:**`) vs. outside in lines `294-379` (`**Pros**:`); `1. ` vs. `1.  ` ordered-list spacing; and 148–195 word single paragraphs in Stage 1 item `3` (line `253`) and Stage 2 item `3` (line `262`).
* **Action:** Standardize horizontal rules and spacing, unify bold-colon and ordered-list formatting, and split Stage 1 item `3` and Stage 2 item `3` into structured sub-bullets.

#### `ACT-N4`: Link Example SVG in `## Non-Goals` and Correct `step0` `path19` Missing-Edge Note
* **Sources:** `CR7-21`
* **Affected Scope:** `## Non-Goals` (line `46`), `### Additional Resources` (lines `372-373`)
* **Discrepancy:** Line `46` cites `rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg` unlinked and frames it strictly as `"broken manual path edits"` vs. line `373`'s positive framing; line `372` implies `path19` corresponds directly to the omitted edge without noting non-monotonic ID numbering around `y = 1.6..2.4` and that the missing edge in `step0` is `(2, 0)-(3, 0)` at `y = 2.4in, x in [0, 0.8in]`.
* **Action:** Link the file on line `46` consistently with line `373` (noting both layout PoC and incomplete/duplicate paths) and clarify the exact `(2, 0)-(3, 0)` missing edge on line `372`.

#### `ACT-N5`: Copy-Edit Hygiene Across `FeatureRange` (`"clean Stage 0"`, Subjectivity, Process Chatter, Redundant NFRs)
* **Sources:** `CR7-22`, `IZ8-4` (NFR pruning part)
* **Affected Scope:** Lines `13`, `22-29`, `33-85`, `109`, `121`, `125`, `131`, `149`, `152`, `243`, `260`, `338`, `354-366`
* **Discrepancy:** `"clean Stage 0"` contradicts float-noise/stray-click cleanup; `## The Problem` mixes subjective opinions and solution steps; `## Open Questions` includes drafting-process chatter; filler/buzzwords appear (`"without Bloat"`, `"high-resolution"`, `"actionable error"`, `"Before touching disk"`, `"failing loudly"`, `"well below kerf / 10"`, `"completely"`); `--overwrite` flag summary (line `121`) omits its purpose; and `## Non-Functional Requirements` repeats functional pipeline behavior already specified earlier.
* **Action:** Use dry, factual phrasing; shorten `## Open Questions` to `"None."`; document `--overwrite`'s purpose in the flag list; and prune redundant functional repetition from `## Non-Functional Requirements`.

#### `ACT-N6`: Standardize Minor Notation Across `FeatureRange`
* **Sources:** `CR7-23`
* **Affected Scope:** Lines `17`, `35-36`, `153`, `159`, `244`, `253`, `275`, `320`
* **Discrepancy:** Mixes `"full kerf"` vs. `"1 kerf"`, `"opposite corners"` vs. `"top-left and bottom-right"`, `"at most once"` vs. `"exactly once"`, `"all print stages"` for Stages 1–3, open-ended `e.g.`/`etc.` for the closed 4-value `LengthUnit` enum, unitless `1e-8` shifts, and `pageP` vs. `pageN`.
* **Action:** Standardize on `"1 kerf"`, `"top-left and bottom-right corners"`, `"exactly once"`, `"Stages 1-3"`, the explicit 4-unit list, `1e-8 in` shifts, and consistent page placeholder notation.

#### `ACT-N7`: Clarify Minor CLI Filesystem Guards in `main.rs`
* **Sources:** `IR9-16`
* **Affected Scope:** `#### 7. stage1.rs` (line `420`), `#### 13. main.rs` (line `454`)
* **Discrepancy:** Line `454` checks pre-existing Stage 4 collisions before `create_dir_all` without stating `ErrorKind::NotFound` handling on a missing `--output-dir`, temp-file naming rules avoiding `<prefix>-stage4-page*` collisions, and reading `--config` YAML before `Config::from_yaml`.
* **Action:** Document `NotFound` handling, non-colliding temp-file names, and reading `--config` in `main.rs`.

#### `ACT-N8`: Copy-Edit Hygiene Across `DetailedRange` and Define `EGM` Footer Acronym
* **Sources:** `C6-9`, `IR9-17`
* **Affected Scope:** Lines `390`, `395`, `400`, `405`, `410`, `415`, `425`, `430`, `435`, `447-449`, `454`, `459`, `464`, `469`, `472`, `476`, `478`
* **Discrepancy:** Subjective/jargon phrasing (`"top-level orchestration API"`, `"without touching the filesystem"`, `"structured"`, `"actionable"`, `"clears margins cleanly"`, `"concise"`, `"for CUJ 2 discovery"`), empty intensifiers/misleading modifiers (`"strictly"`, `"exact"`, `"reuse"`, `"visible stroked"`, `"completely"`, `"bake ... into flat"`), synonym/shorthand redundancy (`"bare unitless"`, `"only operates"`, `"within 0.01 mm epsilon"`, `"single 2-point lines"`, `"flag/serif"`, `"E2E/Integration ... end-to-end"`), and undefined `EGM` acronym on lines `476` and `478`.
* **Action:** Replace with factual, concise wording and spell out or link `EGM` (Elephant-Goldfish Model) or remove the provenance note.

---

## Traceability Matrix

| Stage | Raw Finding ID / Title | Reviewer Lens | Raw Severity | Affected Scope | Consolidated Action Item(s) |
| --- | --- | --- | --- | --- | --- |
| Stage 5 | `Z5-1`: Partial-Edge Tab Attachment & Neck Handle Bowtie Self-Intersection | `egm-inventive-catalyst` | Blocker | Lines `248-252`, `372`, `420`, `464` | `ACT-B1`, `ACT-B2` |
| Stage 5 | `Z5-2`: Severe Altitude Drops Into Implementation Weeds | `egm-zoom-out` | Major | Lines `155-236`, `238-286`, `385-464` | `ACT-M1` |
| Stage 5 | `Z5-3`: Over-Engineered Module Fragmentation and Library/Diagnostic Ceremony | `egm-zoom-out` | Major | Lines `85-107`, `392-454` | `ACT-M1` |
| Stage 5 | `Z5-4`: Redundant Runtime Verification Module (`verify.rs`) | `egm-zoom-out` | Major | Lines `283-286`, `366`, `437-440` | `ACT-M2` |
| Stage 5 | `Z5-5`: Reinventing `kurbo` Primitives and Hand-Rolling Bezier Subdivision | `egm-zoom-out` | Major | Lines `112`, `245`, `256-260`, `410`, `415`, `425` | `ACT-M1` |
| Stage 5 | `Z5-6`: Speculative YAML Configuration DSL and Unnecessary Knobs | `egm-zoom-out` | Major | Lines `34`, `39`, `129-153`, `256`, `402-405`, `456-459` | `ACT-M3` |
| Stage 5 | `Z5-7`: Over-Engineered `GridLabels` Vector-Digit Layer and Negative-`viewBox` Gutter | `egm-zoom-out` | Major | Lines `15-16`, `34`, `253`, `262`, `430`, `446-447` | `ACT-M5` |
| Stage 5 | `Z5-8`: Coupling Tiny Corner Page Numbers to Full Piece-Sized Slots | `egm-inventive-catalyst` | Major | Lines `37-38`, `74`, `78`, `265-273`, `307`, `435` | `ACT-M4` |
| Stage 5 | `Z5-9`: Coupling Cut-Contour Single-Ring Neck Checks to `bleed_polygon` | `egm-inventive-catalyst` | Major | Lines `209`, `257-261`, `425` | `ACT-B4` |
| Stage 5 | `Z5-10`: Output Atomicity vs. Visual Debuggability Suppresses Stage 1/2 Outputs | `egm-inventive-catalyst` | Major | Lines `125-127`, `310-314`, `395`, `425`, `435`, `454` | `ACT-M8` |
| Stage 5 | `Z5-11`: Tight Coupling of `<image>` `(img_x, img_y)` to `<pattern>` World Origin | `egm-inventive-catalyst` | Major | Lines `242-243`, `253`, `415`, `446` | `ACT-B3` |
| Stage 5 | `Z5-12`: Separate Stage 2 Grid Slot Model on `KerfedPiece` | `egm-zoom-out` | Nit | Lines `204-212`, `262`, `266-267` | `ACT-N2` |
| Stage 5 | `Z5-13`: Unexplained "Run-Metadata XML Comment" in `svg_emit.rs` | `egm-zoom-out` | Nit | Lines `448-449` | `ACT-M6` |
| Stage 5 | `Z5-14`: Numerical Tolerance Coupling Between `cut_aabb.center()` and `bleed_aabb` | `egm-inventive-catalyst` | Nit | Lines `266-267`, `285`, `435`, `440` | `ACT-M7` |
| Stage 6 | `C6-1`: Undefined SVG Stroke Width (`<stroke_w>`) and Stage 2 Styling | `egm-comprehension` | Major | Lines `253`, `262`, `277`, `281-282`, `446-449` | `ACT-M6` |
| Stage 6 | `C6-2`: Missing Baseline Context for `src/puzzle-gen` and `src/sticker-size` | `egm-comprehension` | Major | Lines `11`, `49`, `109`, `387`, `390`, `392-465`, `468` | `ACT-N1` |
| Stage 6 | `C6-3`: Unspecified `run-metadata` XML Comment | `egm-comprehension` | Major | Lines `448-449` vs. `1-382` | `ACT-M6` |
| Stage 6 | `C6-4`: Ambiguous Stage 1 `2x2` Mirrored Pattern & Stage 0 `Image` Extraction | `egm-comprehension` | Major | Lines `192`, `242-243`, `253` | `ACT-B3` |
| Stage 6 | `C6-5`: Stage 0–1 Ingestion, Matching Rules, and `path19` Test Contradictions | `egm-comprehension` | Major | Lines `244-245`, `248-250`, `372` vs. `415`, `420`, `464` | `ACT-B1`, `ACT-M9` |
| Stage 6 | `C6-6`: Stage 2 & Stage 3/4 Slot Placement and Verification Wording Gaps | `egm-comprehension` | Major | Lines `210`, `215-216`, `262`, `265-277`, `286`, `425`, `435`, `440` | `ACT-M7`, `ACT-M2` |
| Stage 6 | `C6-7`: `CoordFrame` Normalization & YAML Config Schema Ambiguities | `egm-comprehension` | Major | Lines `67`, `131-153`, `158-162`, `242`, `405` | `ACT-B5`, `ACT-M3` |
| Stage 6 | `C6-8`: Domain Model Completeness, `GridLabels` Gutter Sizing, `--dry-run` | `egm-comprehension` | Nit | Lines `97`, `101`, `122`, `157-236`, `253`, `262`, `276`, `395`, `410`, `430`, `454` | `ACT-M1`, `ACT-M5`, `ACT-M6` |
| Stage 6 | `C6-9`: Undefined `EGM` Acronym in Provenance Footer | `egm-comprehension` | Nit | Lines `476`, `478` | `ACT-N8` |
| Stage 7 | `CR7-1`: Missing Dedicated `Risks & Limits` / `Risks & Mitigations` Section | `egm-design`, `egm-structural` | Blocker | Lines `1-382` | `ACT-B6` |
| Stage 7 | `CR7-2`: Double-Shifted `<image>` + `<pattern>` Bug and Unhandled `<image>` Transforms | `egm-design`, `egm-style` | Blocker | Lines `192`, `242-243`, `253`, `262`, `276` | `ACT-B3` |
| Stage 7 | `CR7-3`: Unhealed Self-Intersecting Tab-to-Edge Splices in Stage 1/2 | `egm-design` | Blocker | Lines `248-252`, `256-261`, `372` | `ACT-B2` |
| Stage 7 | `CR7-4`: Tab Endpoint Snapping Onto "Clustered Grid Vertices" vs. Interior Edge Lines | `egm-design`, `egm-style` | Major | Lines `248-252` | `ACT-B1` |
| Stage 7 | `CR7-5`: Undefined `<stroke_w>`, Omitted Stage 2 Styling, `score_color` Scope | `egm-design`, `egm-style` | Major | Lines `38`, `150`, `253`, `262`, `277`, `281-282` | `ACT-M5`, `ACT-M6` |
| Stage 7 | `CR7-6`: Missing Slot Placement Rules, `slot_w` Shadowing, Stage 3 Misreference | `egm-design`, `egm-style` | Major | Lines `210-216`, `262`, `266-277`, `286` | `ACT-M7`, `ACT-M2` |
| Stage 7 | `CR7-7`: Parameter Bounds, Zero-Offset Failures, Simplification & Tolerances | `egm-design`, `egm-style` | Major | Lines `145-150`, `256-261`, `285`, `331-332`, `359` | `ACT-M3`, `ACT-M14` |
| Stage 7 | `CR7-8`: `CoordFrame` Unit/Aspect Semantics and Missing Stage 0 Validation Rules | `egm-design` | Major | Lines `151-153`, `241-245`, `253`, `262`, `275` | `ACT-B5`, `ACT-M9` |
| Stage 7 | `CR7-9`: Unsafe Stage 4 Globbing, Unvalidated `--prefix`, Non-Atomic Deletion | `egm-design`, `egm-style` | Major | Lines `40`, `114-127`, `365-366` | `ACT-M8` |
| Stage 7 | `CR7-10`: Attribution Without Links in `## Alternatives Considered` | `egm-structural`, `egm-style` | Major | Lines `290-351` | `ACT-M11` |
| Stage 7 | `CR7-11`: Full-Slot Corner Page Numbers Lack Alternatives, Multi-Digit & Fiducials | `egm-design`, `egm-style` | Major | Lines `27`, `38`, `63`, `272-273`, `277`, `282`, `290-351` | `ACT-M4` |
| Stage 7 | `CR7-12`: Heading Hierarchy Violation Under `## Non-Functional Requirements` | `egm-structural`, `egm-style` | Major | Lines `354-381` | `ACT-M12` |
| Stage 7 | `CR7-13`: `Diagnostic.severity` Schema Contradiction | `egm-style` | Major | Lines `231`, `245` | `ACT-M13` |
| Stage 7 | `CR7-14`: Uncalibrated White-Edge Bleed Guarantee | `egm-style` | Major | Lines `25`, `63` | `ACT-M14` |
| Stage 7 | `CR7-15`: Input Layer Count and Tab Path Type Contradictions | `egm-style` | Major | Lines `13`, `33`, `59`, `94`, `241`, `245` | `ACT-M9` |
| Stage 7 | `CR7-16`: Missing Architecture Types & Undecided YAML Crate | `egm-style` | Major | Lines `53`, `97`, `101`, `111`, `157-236` | `ACT-M1`, `ACT-B7` |
| Stage 7 | `CR7-17`: CUJ 2 Arithmetic/Terminology Mismatches | `egm-style` | Major | Lines `67`, `70` | `ACT-M15` |
| Stage 7 | `CR7-18`: Pagination Notation Collisions and Ambiguous Error/Padding Terms | `egm-style` | Major | Lines `16`, `37`, `78`, `272-273`, `285`, `308` | `ACT-M4`, `ACT-M15` |
| Stage 7 | `CR7-19`: Asymmetric Dividers and Whitespace | `egm-structural` | Nit | Lines `7`, `19-81`, `288`, `352`, `368`, `381` | `ACT-N3` |
| Stage 7 | `CR7-20`: Markdown List Formatting and Dense Stage 1/2 Paragraphs | `egm-structural`, `egm-style` | Nit | Lines `3-286` vs. `294-379`, `15-18` vs. `241-286`, `253`, `262` | `ACT-N3` |
| Stage 7 | `CR7-21`: Unlinked Example Reference and Inaccurate `path19` Description | `egm-structural`, `egm-design`, `egm-style` | Nit | Lines `46`, `372-373` | `ACT-N4` |
| Stage 7 | `CR7-22`: Style Hygiene: Filler, Subjective Framing, Chatter, Redundant NFRs | `egm-style` | Nit | Lines `13`, `22-29`, `33-85`, `109`, `121`, `125`, `131`, `149`, `152`, `243`, `260`, `338`, `354-366` | `ACT-N5` |
| Stage 7 | `CR7-23`: Minor Notation and Unit-List Inconsistencies | `egm-style` | Nit | Lines `17`, `35-36`, `153`, `159`, `244`, `253`, `275`, `320` | `ACT-N6` |
| Stage 8 | `IZ8-1`: Remediation of Type/Diagnostic/Verification Gaps Compounds Fragmentation | `egm-zoom-out` | Major | Lines `83-107`, `155-286`, `385-464` | `ACT-M1`, `ACT-M2` |
| Stage 8 | `IZ8-2`: Corner Page-Number Slot Coupling vs. Static Corner Anchor Simplicity | `egm-zoom-out` | Major | Lines `37-38`, `265-277` | `ACT-M4` |
| Stage 8 | `IZ8-3`: Complexity Cascade from Config DSL, Multi-Unit Projection, Debug `GridLabels` | `egm-zoom-out` | Major | Lines `129-153`, `253`, `256-262`, `405` | `ACT-M3`, `ACT-M5`, `ACT-M6` |
| Stage 8 | `IZ8-4`: Scope Creep Risks in `## Risks & Limits` and `## Non-Functional Requirements` | `egm-zoom-out` | Major | Lines `354-367` | `ACT-B6`, `ACT-N5` |
| Stage 8 | `IZ8-5`: Avoid Custom Bezier-Intersection Math; Heal Neck Bowties via `i_overlay` | `egm-zoom-out` | Major | Lines `248-261` | `ACT-B2` |
| Stage 8 | `IZ8-6`: Drop `run-metadata` Comment, Non-Axis `<image>` Transforms, Parser Weeds | `egm-zoom-out` | Major | Lines `114-127`, `241-245`, `448-449` | `ACT-M6`, `ACT-M8`, `ACT-M9` |
| Stage 8 | `IZ8-7`: Consolidate WASM Isolation Claims & Document Empty Stub Baseline | `egm-zoom-out` | Nit | Lines `49`, `109-111`, `390`, `468` | `ACT-N1`, `ACT-B7` |
| Stage 9 | `IR9-1`: Stage 1 Tab-to-Edge Matching Contradicts Reference SVGs; `path19` Assertion | `egm-implementation-readiness`, `egm-codebase-details`, `egm-style` | Blocker | Lines `417-420`, `461-464`, `472` | `ACT-B1` |
| Stage 9 | `IR9-2`: Broken `2x2` Mirrored `<pattern>` Double-Offset and Missing `preserveAspectRatio` | `egm-implementation-readiness`, `egm-codebase-details` | Blocker | Lines `445-448` | `ACT-B3` |
| Stage 9 | `IR9-3`: Invalid `i_overlay::JoinType::Round` API, Winding/Hole Checks, Zero-Offset, S-Curve | `egm-implementation-readiness`, `egm-codebase-details` | Blocker | Lines `390`, `422-425` | `ACT-B4` |
| Stage 9 | `IR9-4`: `CoordFrame` Reads `<svg>` Instead of `<sodipodi:namedview>`, Signature Conflict | `egm-codebase-details` | Blocker | Lines `402-405`, `412-415`, `442-449` | `ACT-B5` |
| Stage 9 | `IR9-5`: Unresolved Dependency Choice, Unpinned Versions, Omitted `Cargo.lock`, Smoke Path | `egm-implementation-readiness`, `egm-codebase-details`, `egm-style` | Blocker | Lines `385-391`, `472` | `ACT-B7` |
| Stage 9 | `IR9-6`: Missing Phasing & Buildable/Testable Checkpoints | `egm-implementation-readiness`, `egm-codebase-details` | Major | Lines `383-478` | `ACT-M10` |
| Stage 9 | `IR9-7`: Missing Cross-Module Type Definitions and `svg_emit` Signatures | `egm-implementation-readiness`, `egm-codebase-details` | Major | Lines `392-395`, `407-410`, `420`, `425`, `430`, `442-449`, `454` | `ACT-M1` |
| Stage 9 | `IR9-8`: Unspecified Stage 2 `stage2_offset_mm` Anchoring and Cell-Pitch Collision Hazard | `egm-codebase-details`, `egm-implementation-readiness` | Major | Lines `422-425` | `ACT-M7` |
| Stage 9 | `IR9-9`: Unspecified SVG Styling, Float Precision, Metadata Comment, `--dry-run` Output | `egm-implementation-readiness`, `egm-codebase-details`, `egm-style` | Major | Lines `427-430`, `442-449`, `451-454` | `ACT-M5`, `ACT-M6` |
| Stage 9 | `IR9-10`: Underspecified `config.rs`, `stage3_4.rs`, `verify.rs` Bounds & Tautology | `egm-implementation-readiness`, `egm-codebase-details`, `egm-style` | Major | Lines `402-405`, `432-435`, `437-440` | `ACT-M2`, `ACT-M3`, `ACT-M4`, `ACT-M7` |
| Stage 9 | `IR9-11`: Underspecified Stage 0 `ingest.rs` Elevation, Stray-Click Precedence & Transforms | `egm-implementation-readiness`, `egm-codebase-details`, `egm-style` | Major | Lines `412-415` | `ACT-M9` |
| Stage 9 | `IR9-12`: Missing CLI & Edge-Case Integration Tests and Conflated Manual Inkscape Check | `egm-codebase-details`, `egm-style` | Major | Lines `461-464`, `466-472` | `ACT-M16` |
| Stage 9 | `IR9-13`: Terminology Collisions and Inconsistent Parameter/Flag Naming | `egm-style` | Major | Lines `390`, `420`, `464`, `469` | `ACT-M3`, `ACT-M15` |
| Stage 9 | `IR9-14`: Uncalibrated Safety/Verification Claims (`verify.rs`, `main.rs`, Verification Plan) | `egm-style` | Major | Lines `440`, `454`, `468`, `471` | `ACT-M2`, `ACT-M8`, `ACT-M16` |
| Stage 9 | `IR9-15`: Overlong Run-On Paragraphs in `Target Files to Create/Modify` | `egm-style` | Major | Lines `390`, `405`, `415`, `420`, `425`, `435`, `454`, `464` | `ACT-M17` |
| Stage 9 | `IR9-16`: Minor CLI Filesystem Guards and Parameter Consistency | `egm-implementation-readiness`, `egm-codebase-details` | Nit | Lines `420`, `454` | `ACT-N7` |
| Stage 9 | `IR9-17`: Technical Copy-Editing Hygiene (Buzzwords, Intensifiers, Redundancies) | `egm-style` | Nit | Lines `390-472` | `ACT-N8` |
