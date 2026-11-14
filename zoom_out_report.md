# Goldfish Review Report (Stage 5: Initial Zoom Out & TRIZ Review)

**Target:** `docs/designs/puzzle-gen.md` (`Lines 1-478`)

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-zoom-out` | completed |
| `egm-inventive-catalyst` | completed |

## Context Sufficiency & Baseline Check

* **Context Sufficiency (`zoom-out-standard.md` §0):** Passes. The Overview, Problem, Goals, and CUJs establish the objective and motivation. Codebase inspection (`Cargo.toml`, `Cargo.lock`, `README.md`, `src/puzzle-gen/*`, `src/sticker-size/*`) confirms no existing SVG parsing/writing, YAML configuration, or 2D geometry utilities exist in the workspace for `puzzle-gen` to reuse.

## Prioritized Findings (Blockers -> Major -> Nits)

### 1. Partial-Edge Tab Attachment & Neck Handle Bowtie Self-Intersection (TRIZ) - Severity: Blocker
* **Reviewer Lens:** `egm-inventive-catalyst`
* **Affected Scope:** `#### Stage 1: Tab Splicing & 2x2 Mirrored Pattern` (lines `248-252`), `### Additional Resources` (line `372`), `#### 7. src/puzzle-gen/src/stage1.rs (Create)` (line `420`), `#### 15. src/puzzle-gen/tests/integration_pipeline.rs (Create)` (line `464`)
* **Discrepancy Description (Contradiction Analysis):**
  * **False Compromise (Authoring Ergonomics vs. Contour Topology):** Stage 0 authoring uses a `<rect>` grid overlaid with short open `<path>` tab loops, yet Stage 1 specifies an algorithm written for full-edge divider curves: lines `248-249` and `420` match `TabCandidate` endpoints against internal **edge endpoints (grid corners)** within `0.5 mm`, snap endpoints onto **clustered grid vertices**, and splice the curve *"directly in place of the straight rectangle segment"* (line `252`).
  * **Empirical Breakage on Reference Input:** In `src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` (lines `203-324`), all 39 tabs (`path1`–`path40`) start and end strictly in the *interior* of their `0.8 in` (`20.32 mm`) edges—typically `5–10 mm` from grid corners (e.g., `path1` on `x = 0.8` spans `y = 0.3203..0.4527`). Matching endpoints to edge vertices within `0.5 mm` fails on every tab in the reference file.
  * **Neck Handle Bowtie Contradiction:** Smooth Bezier handles at tab junctions overshoot backward across the grid line (`path2` line `208` crosses back over `y = 0.8` before `0.29408`; `path1` line `205` loops back onto `x = 0.8`). Naive `[straight_prefix, tab, straight_suffix]` splicing produces bowtie self-intersections in Stage 1 and risks multi-island failures in Stage 2's single-ring validation (line `261`).
* **Rationale & Recommendation (Inventive Resolution — Segmentation + Preliminary Action):**
  1. Segment internal edges dynamically around the tab interval: project both `TabCandidate` endpoints perpendicularly onto internal edge line segments `[start_mm, end_mm]` (`perp_dist <= 0.5 mm`, `0 <= t_0 < t_1 <= 1`, breaking corner-proximity ties via interior curve-point distance) and emit `[Line(corner_a -> p_0), TabCurve(p_0 -> p_1), Line(p_1 -> corner_b)]`.
  2. Apply Preliminary Action in Stage 1 to trim cubic-line neck crossings or resolve local bowtie overlaps (`FillRule::NonZero`) before emitting Stage 1 contours.
  * *Secondary Liabilities:* Requires 4-way corner tie-breaking logic and cubic-line root-finding or planar cleanup in `stage1.rs`.

### 2. Severe Altitude Drops Into Implementation Weeds - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `### Core Domain Data Structures` (lines `155-236`), `### Stage-by-Stage Geometric Pipeline` (lines `238-286`), `### Target Files to Create/Modify` (lines `385-464`)
* **Discrepancy Description:** Violates Zoom-Out Standard §3 (Anti-Weeds: Premature Detail). Rather than staying at architectural boundaries, component responsibilities, and data flows, the document prescribes exact Rust struct definitions and concrete field types (`CoordFrame`, `Config`, `GridCell`, `InternalEdge`, `IngestedPuzzle`, `SplicedPiece`, `KerfedPiece`, `PageSlot`, `PagePlan`, `PaginatedLayout`, `Diagnostic`), ~10 hardcoded algorithmic tolerances and constants (`0.01 mm`, `0.05 mm`, `0.5 mm`, `0.001 mm`, `1000` units/mm, `0.005 mm`, `0.1` midpoint deviation ratio, `10%` padding, `1e-4 mm`), exact edge traversal order (`top` L→R, `right` T→B, `bottom` R→L, `left` B→T), exact SVG transform formulas (`translate(2W, 0) scale(-1, 1)`), and ~20 internal function names across 15 file entries.
* **Rationale & Recommendation:** Elevate the Feature Specification to architectural interfaces, invariants, and data flows; move concrete struct fields, internal function names, and minor numeric tolerances into the implementation plan or code.

### 3. Over-Engineered Module Fragmentation and Library/Diagnostic Ceremony - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `### Architecture Overview` (lines `85-107`), `### Target Files to Create/Modify` items 2–13 (lines `392-454`)
* **Discrepancy Description:** Violates Zoom-Out Standard §2 (Simplicity & Extensibility: Minimalist Data Flow, YAGNI) and §1 (Project Alignment). In a repository of small experiments (`README.md`: "small interactive experiments"; sibling `src/sticker-size` has 3 `.rs` files) and with WASM/UI integration explicitly excluded (`## Non-Goals`, line `49`), splitting a single-user local CLI into **12 Rust source files** (`lib.rs`, `diagnostics.rs`, `config.rs`, `model.rs`, `ingest.rs`, `stage1.rs`, `stage2.rs`, `digits.rs`, `stage3_4.rs`, `verify.rs`, `svg_emit.rs`, `main.rs`) is excessive fragmentation. Adding a dedicated `DiagnosticBag`/`Severity`/`StageId` framework and a `lib.rs` boundary returning `PipelineResult` with all four intermediate typed domain models (which `main.rs` does not need to emit SVGs or print a summary) introduces unnecessary boilerplate over a compact binary crate.
* **Rationale & Recommendation:** Consolidate into 3–4 cohesive modules (e.g., `config`, `geometry`/`pipeline`, `svg`, `main`) using standard `Result` errors and `stderr` warnings for `--no-strict`.

### 4. Redundant Runtime Verification Module (`verify.rs`) Checking a Single Source of Truth Against Itself - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `#### Stage 3 & Stage 4: Full-AABB Pagination, Vector Digits & Layer Separation` item 5 (lines `283-286`), `## Non-Functional Requirements` (line `366`), `#### 11. src/puzzle-gen/src/verify.rs (Create)` (lines `437-440`)
* **Discrepancy Description:** Violates Zoom-Out Standard §2 (Minimalist Data Flow, YAGNI). Introducing a dedicated `verify_invariants` production stage to re-verify piece counts, AABB clearances, and Stage 3 vs. Stage 4 coordinate parity (`<= 1e-4 mm`) before disk writes is redundant ceremony:
  * Stages 1–4 are constructed 1:1 in memory from `IngestedPuzzle.cells`, and `PaginatedLayout` (lines `213-228`, `435`) already stores the single shared `PageSlot` centers and corner digit polylines used by both Stage 3 and Stage 4.
  * Checking parity "in canonical `mm` (before unit projection)" (lines `286`, `440`) compares `PaginatedLayout` against itself (and Stage 3 does not even emit `cut_polygon`, line `276`), so it cannot catch actual SVG serialization or unit-projection bugs in `svg_emit.rs`.
* **Rationale & Recommendation:** Enforce margin/spacing math by construction in pagination and validate SVG outputs in unit/integration tests, eliminating `verify.rs` and the redundant runtime pass.

### 5. Reinventing `kurbo` Primitives and Hand-Rolling Bezier Subdivision - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `### Architecture Overview` (line `112`), `#### Stage 0` item 5 (line `245`), `#### Stage 2` items 1–2 (lines `256-260`), `model.rs` (line `410`), `ingest.rs` (line `415`), `stage2.rs` (line `425`)
* **Discrepancy Description:** Violates Zoom-Out Standard §4 (Leverage Existing: Platform Standards) and §2 (Simplicity). Line `112` adds `kurbo` for "(exact cubic/line segments, reversal, bounded-length flattening)", yet `model.rs` (lines `187`, `410`) defines bespoke `CubicSeg` and `PolygonRing` (`Vec<(f64, f64)>`) wrappers, `ingest.rs` (line `415`) converts `svgtypes::SimplifyingPathParser` into `CubicSeg` chains, and `stage2.rs` (line `425`) specifies custom recursive Bezier subdivision and arc-length calculation (line `245`).
* **Rationale & Recommendation:** Use `kurbo`'s native segment, path, point, flattening, and arc-length primitives directly instead of building redundant wrappers and custom curve subdivision.

### 6. Speculative YAML Configuration DSL and Unnecessary Knobs - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `## Goals` (lines `34`, `39`), `### YAML Configuration Schema & Unit Normalization (CoordFrame)` (lines `129-153`), `#### Stage 2` item 1 (line `256`), `config.rs` (lines `402-405`), `default-config.yaml` (lines `456-459`)
* **Discrepancy Description:** Violates Zoom-Out Standard §2 (YAGNI). Multiple elements of the config design serve speculative future requirements rather than the core workflow:
  * `version: 1` (lines `131`, `134`, `405`): Purely speculative schema versioning with no v2 format or migration behavior.
  * Bespoke string grammar parsers (`parse_page_size` for `"6in x 6in"`, `parse_kerf_relative_length` for `"1 kerf"` / `"1.5 kerf"`, lines `147`, `149`, `405`): Adds a mini-DSL parser and cross-field resolution solely to avoid explicit fields or lengths in an optional YAML file.
  * Speculative user-facing knobs: `segment_length` (lines `139`, `256`) exposes an internal numerical flattening tolerance that could be derived from `kerf` or fixed safely; `page_unit` (lines `34`, `148`, `153`) adds multi-unit output projection specifically for Stage 4 even though `mm` is the default laser CAM standard; and `score_color` (lines `39`, `142`, `150`) adds hex validation for one layer while `#000000` for `Cut` is hardcoded.
* **Rationale & Recommendation:** Drop `version: 1`, replace string mini-DSLs with direct physical fields or minimal units, derive flattening tolerance automatically from `kerf`, fix Stage 4 to `mm`, and use fixed layer colors unless a concrete CAM requirement demands configurability.

### 7. Over-Engineered `GridLabels` Vector-Digit Layer and Negative-`viewBox` Gutter Expansion in Debug Stages 1 & 2 - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `## Overview` items 1–2 (lines `15-16`), `## Goals` (line `34`), `#### Stage 1` item 3 (line `253`), `#### Stage 2` item 3 (line `262`), `digits.rs` (line `430`), `svg_emit.rs` (lines `446-447`)
* **Discrepancy Description:** Violates Zoom-Out Standard §2 (YAGNI, Minimalist Data Flow). Font-free single-stroke vector digits are justified for Stage 3/4 corner page numbers because laser CAM software needs font-independent paths. However, Stage 1 and Stage 2 are strictly screen-only debug inspection SVGs (lines `29`, `314`), never printed or laser-cut. Adding a second digit layout engine (`layout_gutter_index`), computing outer top/left gutters, and expanding Stage 1 and Stage 2 root `width`, `height`, and `viewBox` into negative `min-x`/`min-y` coordinates while keeping `userSpaceOnUse` `<pattern>` coordinates anchored introduces non-trivial coordinate-framing complexity for debug convenience.
* **Rationale & Recommendation:** Preserve piece `id` attributes (e.g., `r{row}_c{col}` or `<title>` metadata, or standard `<text>` if visual labels are wanted in a viewer) instead of building a vector-gutter layout engine and negative-`viewBox` mutations for Stages 1 and 2.

### 8. Coupling Tiny Corner Page Numbers to Full Piece-Sized Slots (`cols * rows >= 3`) (TRIZ) - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst`
* **Affected Scope:** `## Goals` (lines `37-38`), `### CUJ 3` (lines `74`, `78`), `#### Stage 3 & Stage 4` (lines `265-273`), `### Alternative C` (line `307`), `stage3_4.rs` (line `435`)
* **Discrepancy Description (Contradiction Analysis):** To keep slot pitch uniform (line `307`), the design couples small 1–2 digit alignment labels to full piece-sized `slot_w x slot_h` grid cells—reserving slot `(0, 0)` and slot `(page_rows - 1, page_cols - 1)` on every page. Improving grid-layout uniformity degrades sheet capacity and supported piece dimensions: every sheet loses two full piece slots (50% on `2x2`), and any puzzle whose piece slot fits `1x2`, `2x1`, or `1x1` hard-fails (`cols * rows < 3`, line `272`) even when margin/slack space exists for compact registration numbers.
* **Rationale & Recommendation (Inventive Resolution — Extraction + Dynamics):** Extract corner page-number anchors out of the `slot_w x slot_h` piece grid into compact fixed-physical-dimension anchor boxes (`anchor_w x anchor_h`, e.g., `8 mm x 5 mm`) inside page margins; place them dynamically in residual horizontal/vertical grid slack when available (zero piece loss), or reserve a compact corner band (`cols * rows >= 1` minimum).
  * *Secondary Liabilities:* `stage3_4.rs` must track and check clearance across two box sizes, and tightly fitted sheets without slack could still lose an outer band unless corner-only exclusion is used.

### 9. Coupling Cut-Contour Single-Ring Neck Checks to `bleed_polygon` (TRIZ) - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst`
* **Affected Scope:** `### Core Domain Data Structures` (`KerfedPiece.bleed_polygon`, line `209`), `#### Stage 2` (lines `257-261`), `stage2.rs` (line `425`)
* **Discrepancy Description (Contradiction Analysis):** Stage 2 enforces the identical single-ring topological invariant on both `cut_polygon` (`+kerf / 2`) and `bleed_polygon` (`+print_bleed`) (lines `209`, `261`). Increasing `print_bleed` narrows concave socket mouths by `2 * print_bleed`; whenever `print_bleed >= min_socket_neck / 2`, opposing lips bridge across the empty waste cavity and emit an outer ring plus an interior pocket hole in `i_overlay`. While neck collapse on `cut_polygon` ruins physical piece cutting, bridging a discarded socket waste cavity on `bleed_polygon` (`fill: url(...); stroke: none`) is harmless—yet the shared check aborts the pipeline.
* **Rationale & Recommendation (Inventive Resolution — Segmentation):** Enforce the strict single-ring, neck-preservation invariant solely on `cut_polygon`; allow `bleed_polygon` to be a multi-ring `MultiPolygon` (or keep its outer positive-area boundary when inner pockets pinch shut) verified to contain `cut_polygon` and stay within the Stage 3 slot envelope.
  * *Secondary Liabilities:* Retaining inner `holes` requires compound `<path>` serialization with explicit `fill-rule="evenodd"`; dropping pinched inner holes prints ink across small discarded waste pockets.

### 10. Output Atomicity vs. Visual Debuggability Suppresses Stage 1/2 Inspection SVGs on Errors (TRIZ) - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst`
* **Affected Scope:** `### CLI Interface & File Lifecycle` (lines `125-127`), `### Alternative D` (lines `310-314`), `lib.rs` (line `395`), `stage2.rs` (line `425`), `stage3_4.rs` (line `435`), `main.rs` (line `454`)
* **Discrepancy Description (Contradiction Analysis):** Alternative D (lines `310-314`) retains Stage 1 and Stage 2 SVGs with `(row, col)` `GridLabels` to debug geometry defects visually. However, `run_pipeline` (line `395`) and the disk pre-gate (line `125`) withhold all SVG files when Stage 2 or Stage 3 fails, and `stage2.rs`/`stage3_4.rs` take no `&mut DiagnosticBag` and fail fast on the first defective piece. Protecting disk state from broken Stage 3/4 fabrication files removes Stage 1/2 visual debugging artifacts precisely when defects occur.
* **Rationale & Recommendation (Inventive Resolution — Extraction + Blessing in Disguise):** Accumulate non-cascading errors across pieces via `DiagnosticBag` (or aggregate `Result` errors) and allow retaining valid Stage 1/2 outputs on downstream errors (e.g., via `--emit-debug-on-error` honoring `--overwrite=false`) while blocking unverified Stage 3/4 writes. *(Note tension with Finding 3 above: keep diagnostic accumulation minimal rather than building heavy `DiagnosticBag` infrastructure if consolidated error reporting suffices.)*

### 11. Tight Coupling of `<image>` Asset Definition `(img_x, img_y)` to `<pattern>` World Origin (TRIZ) - Severity: Major
* **Reviewer Lens:** `egm-inventive-catalyst`
* **Affected Scope:** `#### Stage 0` (lines `242-243`), `#### Stage 1` (line `253`), `ingest.rs` (line `415`), `svg_emit.rs` (line `446`)
* **Discrepancy Description (Contradiction Analysis):** Stage 1 emits `<image>` inside `<defs>` at world `(img_x, img_y)` while setting `<pattern x="img_x" y="img_y" ...>` with four `<use>` tiles (`(0, 0)`, `translate(2W, 0) scale(-1, 1)`, etc.). Because `<use>` inside `<pattern>` evaluates `<image>` relative to the `<pattern>` origin `(img_x, img_y)`, non-zero `img_x`/`img_y` double-shifts the normal tile to `(2*img_x, 2*img_y)` and corrupts mirror coordinates. Furthermore, Stage 0 composes arbitrary ancestor transforms onto `<image>` (line `242`) without rejecting rotated/skewed `<image>` transforms or checking grid coverage.
* **Rationale & Recommendation (Inventive Resolution — Extraction):** Emit `<image>` inside `<defs>` at pattern-local `x="0" y="0" width="W" height="H"`, apply `(img_x, img_y)` solely on `<pattern>`, reject rotated/skewed `<image>` transforms during Stage 0 ingest, and warn if `image_bounds_mm` does not cover the `Pieces` grid.
  * *Secondary Liabilities:* Authors with rotated `<image>` layers must flatten/bake image rotation in Inkscape before Stage 0.

### 12. Separate Stage 2 Grid Slot Model on `KerfedPiece` - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `### Core Domain Data Structures` (`KerfedPiece`, lines `204-212`), `#### Stage 2` item 3 (line `262`), `#### Stage 3 & Stage 4` item 1 (lines `266-267`)
* **Discrepancy Description:** `KerfedPiece` stores `stage2_offset_mm` and `stage2_aabb_mm` derived strictly from `cut_aabb` (line `262`), separate from the unified `max(cut_aabb, bleed_aabb)` slot size used in Stages 3–4 (lines `266-267`).
* **Rationale & Recommendation:** Compute Stage 2's debug spacing locally during Stage 2 emission or reuse the unified slot pitch so `KerfedPiece` only holds actual geometry (`cut_polygon`, `bleed_polygon`).

### 13. Unexplained "Run-Metadata XML Comment" in `svg_emit.rs` - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `#### 12. src/puzzle-gen/src/svg_emit.rs (Create)` (lines `448-449`)
* **Discrepancy Description:** Lines `448-449` require embedding a "run-metadata XML comment tying printed pages to Stage 4 outputs" in Stages 3 and 4, which is neither specified nor motivated in Overview, Goals, CUJs, or the Stage 3/4 pipeline sections.
* **Rationale & Recommendation:** Omit speculative XML metadata comments not tied to any user journey or requirement.

### 14. Numerical Tolerance Coupling Between `cut_aabb.center()` Slot Alignment and `bleed_aabb` Bounds (TRIZ) - Severity: Nit
* **Reviewer Lens:** `egm-inventive-catalyst`
* **Affected Scope:** `#### Stage 3 & Stage 4` (lines `266-267`, `285`), `stage3_4.rs` (line `435`), `verify.rs` (line `440`)
* **Discrepancy Description (Contradiction Analysis):** Line `435` centers pieces by `cut_aabb.center()`, while `slot_w` / `slot_h` use `max(cut_aabb, bleed_aabb)` (lines `266-267`). Independent round-join offsetting and `0.005 mm` collinear simplification (line `260`) can shift `bleed_aabb.center()` by up to `~0.006 mm` relative to `cut_aabb.center()`, pushing boundary `bleed_polygon` edges outside strict `[margin_mm, page_size_mm - margin_mm]` bounds (line `285`) beyond `1e-4 mm` (line `440`).
* **Rationale & Recommendation (Inventive Resolution — Beforehand Cushioning):** Size `slot_w` and `slot_h` symmetrically around `cut_aabb.center()` using `2 * max(|min - cut_center|, |max - cut_center|)` across both `cut_polygon` and `bleed_polygon`, and apply a consistent `0.01 mm` margin/spacing tolerance.
