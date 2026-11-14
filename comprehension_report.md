**What this proposes**
This document proposes building `puzzle-gen` in `src/puzzle-gen`, a local Rust CLI and pure-geometric library that transforms a clean "Stage 0" Inkscape SVG (containing `Image`, `Pieces`, and `Tabs` layers) and an optional YAML configuration file into four sequential SVG outputs through an in-memory millimeter pipeline. Stage 1 splices shared cubic-Bezier tabs into a rectangular grid of `<rect>` pieces, embeds the raster image once inside `<defs>` as a 2x2 mirrored `<pattern>`, and adds debug row/column `GridLabels`; Stage 2 flattens curves into polylines, applies outward `kerf / 2` cut and `print_bleed` offsets using integer-grid polygon buffering, and spaces pieces across a uniform grid while locking each piece's pattern to its original artwork slice. Stage 3 reflows stroke-free `print_bleed` pieces across Inkscape `<inkscape:page>` sheets with stroked corner vector page numbers for PDF printing, while Stage 4 outputs one standalone, transform-free and pattern-free SVG per page with separate `Cut` and `PageNumbers` layers for laser cutting. Before writing to disk, the tool enforces geometric and structural invariants (including missing-tab checks unless `--no-strict` is set, minimum `cols * rows >= 3` page capacity, margin/spacing clearances, and Stage 3/4 parity) and protects existing files unless `--overwrite` is enabled.

**How the existing system works, per this document**
According to The Problem section, preparing a custom Inkscape jigsaw puzzle for laser print-and-cut fabrication is currently a manual, error-prone Inkscape workflow: authors must manually duplicate, reverse, split, and join shared internal tab paths across every grid edge, manually maintain `patternTransform` offsets and mirrored pixel bleed, space pieces to avoid kerf-expanded tab-bulb collisions, and manually align multi-page print sheets with per-page laser files. Within the repository, the Overview describes `puzzle-gen` in present tense as a local Rust CLI utility in `src/puzzle-gen`, and the Detailed Implementation Plan marks `src/puzzle-gen/Cargo.toml` and `src/puzzle-gen/src/main.rs` as existing files to modify alongside example SVGs in `src/puzzle-gen/examples/` and a sibling WASM crate `src/sticker-size`. However, the document never describes what functionality currently exists in `src/puzzle-gen` or `src/sticker-size` prior to these proposed changes.

---

# Goldfish Review Report (Stage 6: Comprehension Test)

**Target:** `docs/designs/puzzle-gen.md` (`FeatureRange: Lines 1-382`, `DetailedRange: Lines 383-478`)

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-comprehension` | completed |

## Findings

### 1. Undefined SVG Stroke Width (`<stroke_w>`) and Stage 2 Styling — Severity: Major
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** Stage 3 & Stage 4 SVG styling (`<stroke_w>`), lines 253, 262, 277, 281–282, 446–449
* **Discrepancy Description & Rationale:** Stage 3 corner page numbers (line 277) and Stage 4 `Cut` and `PageNumbers` paths (lines 281–282) all specify `stroke-width: <stroke_w>`, yet `<stroke_w>` is never defined anywhere in the document—neither as a fixed physical constant, nor as a YAML schema key or `Config` field (lines 133–173), nor in `svg_emit.rs` (lines 446–449). In addition, Stage 1 pieces and Stage 1/2 `GridLabels` specify `stroke` colors without a `stroke-width` (lines 253, 262), and Stage 2 pieces omit `fill` and `stroke` styling entirely (lines 262, 447). Because laser CAM importers and print legibility depend on stroke width and whether Stage 2 pieces are stroked, leaving `<stroke_w>` and Stage 2 piece stroke behavior undefined forces an implementer to guess.

### 2. Missing Baseline Context for `src/puzzle-gen` and `src/sticker-size` — Severity: Major
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** Baseline system & sibling crate references (`src/puzzle-gen`, `src/sticker-size`), lines 11, 49, 109, 387, 390, 392–465, 468
* **Discrepancy Description & Rationale:** Line 11 describes `puzzle-gen` in the present tense as "a local Rust CLI utility in `src/puzzle-gen`," and lines 387 and 451 mark `Cargo.toml` and `src/main.rs` as `(Modify)` while marking `src/lib.rs` and all submodules as `(Create)`. A fresh reader cannot tell what `src/puzzle-gen` currently implements before this plan. Similarly, lines 49, 109, 390, and 468 refer to "the sibling `src/sticker-size` WASM crate" without defining what `src/sticker-size` does, linking to it, or explaining why `src/puzzle-gen`'s `Cargo.toml` dependencies could affect a sibling workspace member.

### 3. Unspecified `run-metadata` XML Comment — Severity: Major
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** `src/puzzle-gen/src/svg_emit.rs` (`run-metadata XML comment`), lines 448–449 vs. lines 1–382
* **Discrepancy Description & Rationale:** Lines 448 and 449 require `emit_stage3` and `emit_stage4_page` to embed a matching "run-metadata XML comment tying printed pages to Stage 4 outputs." This requirement is completely absent from the feature specification (lines 1–382), and even lines 448–449 give no specification of the comment's format or contents (e.g., prefix, configuration snapshot, deterministic run hash, or timestamp—the last of which would conflict with deterministic serialization on line 445).

### 4. Ambiguous Stage 1 `2x2` Mirrored Pattern & Stage 0 `Image` Extraction Semantics — Severity: Major
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** Stage 1 `2x2` Mirrored Pattern & Stage 0 `Image` extraction, lines 192, 242–243, 253
* **Discrepancy Description & Rationale:** Line 253 instructs emitting `<image>` inside `<defs>` "at transformed `(img_x, img_y)` with size `(W, H)`" and defining `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse" x="img_x" y="img_y" width="2W" height="2H">` with four `<use>` children: "normal at `(0, 0)`, horizontally mirrored `translate(2W, 0) scale(-1, 1)` at `(W, 0)`," etc. This specification is internally ambiguous in SVG coordinate semantics:
  * If the `<image>` inside `<defs>` carries `x="img_x" y="img_y"`, `<use>` inherits `(img_x, img_y)` inside the pattern tile (which already originates at `(img_x, img_y)`), shifting the normal tile to `(2*img_x, 2*img_y)` and mirroring the flipped tiles around `(0, 0)` to `[W - img_x, 2W - img_x]` unless `<image>` is actually emitted at `(0, 0)` in `<defs>`.
  * Writing `translate(2W, 0) scale(-1, 1) at (W, 0)` leaves unclear whether `(W, 0)` is prose describing the resulting tile location or an `x="W" y="0"` attribute on `<use>` (in SVG, setting `x="W"` *plus* `transform="translate(2W, 0) scale(-1, 1)"` maps `[0, W]` back onto `[0, W]`).
  * Additionally, line 242 composes ancestor transforms into `<image>`, while line 192 stores only an axis-aligned `image_bounds_mm: Rect`; the document never states how rotated/skewed/flipped `<image>` transforms in Stage 0 are handled or whether the `Image` bounds must cover the `Pieces` grid.

### 5. Stage 0–1 Ingestion, Matching Rules, and `path19` Test Expectation Contradictions — Severity: Major
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** Stage 0–1 Ingestion & Matching rules + `path19` test assertion, lines 244–245, 248–250, 372 vs. 415, 420, 464
* **Discrepancy Description & Rationale:**
  * **Detailed-only validation & `Z`/`z` contradiction:** Line 415 introduces `reject rotated <rect> grid cells` and `rejecting closed or disconnected multi-subpath tab elements`, neither of which appears in the Stage 0 feature description (lines 244–245). Moreover, line 245 explicitly lists `Z`/`z` (closepath) among supported `Tabs` commands, leaving it ambiguous whether `Z`/`z` is parsed solely to reject closed paths, whether an open path whose endpoints coincide counts as "closed," whether a stray zero-span click with `Z` is filtered as `Info` or rejected, and how non-`<rect>`/`non-<path>` elements or rounded `<rect>` attributes (`rx`/`ry`) in `Pieces`/`Tabs` are treated.
  * **Edge vs. endpoint matching:** Line 248 says tabs match "against the internal shared edges" within `0.5 mm`, whereas line 420 clarifies this is "against edge endpoints within `0.5 mm`."
  * **Impossible `path19` error-citation phrasing:** Line 372 states the reference SVG has "39 tab `<path>` elements with `path19` omitted," yet line 464 specifies a strict-mode test "asserting failure citing missing internal edge `path19`'s `(row, col)` coordinates." Because `path19` is omitted from the file, `puzzle-gen` cannot identify the missing element as `path19`; this phrasing makes it unclear what the test actually asserts against the error message.

### 6. Stage 2 & Stage 3/4 Slot Placement and Verification Wording Gaps — Severity: Major
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** Stage 2 & Stage 3/4 slot placement and verification wording, lines 210, 215–216, 262, 265–277, 286, 425, 435, 440
* **Discrepancy Description & Rationale:**
  * **Missing Stage 2 slot placement rule:** Line 262 defines the Stage 2 slot size (`max(cut_aabb.width)` by `max(cut_aabb.height)` separated by `piece_spacing_mm`), and line 210 declares `stage2_offset_mm`, but neither line 262 nor line 425 explains how each piece is anchored within its Stage 2 slot (e.g., centered via `cut_aabb.center()`, top-left aligned, or relative to its Stage 0 position).
  * **Stage 3/4 slot centering & page grid origin:** FeatureRange (lines 265–277) never specifies how pieces are positioned inside a Stage 3/4 `(slot_w, slot_h)` box (`piece_offset_mm = slot_center_mm - cut_aabb.center()` appears only in DetailedRange at line 435), and neither section states whether the `page_cols x page_rows` slot grid is top-left aligned at `(margin_mm, margin_mm)` or centered inside the `usable_w x usable_h` area when remainder space exists.
  * **Contradictory "Stage 3 piece `cut_polygon`" phrasing:** Lines 286 and 440 verify that "Stage 3 piece `cut_polygon` slot centers ... match Stage 4" coordinates within `1e-4 mm`. However, Stage 3 emits `bleed_polygon` (line 276), not `cut_polygon`, making it confusing what Stage 3 geometry is being compared (presumably the `cut_polygon` center after applying Stage 3's `piece_offset_mm` from `PaginatedLayout`).

### 7. `CoordFrame` Normalization & YAML Config Schema Ambiguities — Severity: Major
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** `CoordFrame` normalization & YAML config schema (`version`, optionality, bounds, `score_color`), lines 67, 131–153, 158–162, 242, 405
* **Discrepancy Description & Rationale:**
  * **`CoordFrame` ambiguity & missing `viewBox` origin:** Line 152 says `CoordFrame` parses `width`, `height`, `viewBox`, and `inkscape:document-units`, failing if `width`/`height` are incompatible with `viewBox` aspect ratio or lack physical units "when no unit hint is present." It does not state: (a) precedence between `width`/`height` units and `inkscape:document-units` when setting `doc_unit`, (b) the aspect-ratio compatibility tolerance, (c) behavior when `viewBox` or `width`/`height` is absent, or (d) where the root `viewBox` `(min_x, min_y)` offset referenced on line 242 ("plus any root `viewBox` offset/scale via `CoordFrame`") lives, since `CoordFrame` (lines 158–162) has no origin offset field.
  * **YAML field optionality & bounds:** Line 131 highlights "an optional `version: 1` header" without stating that all other keys are also optional and fall back to defaults (as assumed in CUJ 2, line 67) or how non-`1` `version` values are handled. Similarly, line 405 adds "positive/finite bounds validation" (unmentioned in FeatureRange) without clarifying whether `0mm` is valid for `margin`, `piece_spacing`, or `print_bleed` vs. strictly `> 0`.
  * **`score_color` scope mismatch:** Line 150 defines `score_color` solely as applied to Stage 4's `PageNumbers` layer, omitting that Stages 1 and 2 also use `<score_color>` to stroke `GridLabels` (lines 253, 262, 446).

### 8. Domain Model Completeness, `GridLabels` Gutter Sizing, Translation Naming, and `--dry-run` Output — Severity: Nit
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** Domain model completeness, Stage 1/2 `GridLabels` gutter sizing, Stage 3 translation naming, and `--dry-run` details, lines 97, 101, 122, 157–236, 253, 262, 276, 395, 410, 430, 454
* **Discrepancy Description & Rationale:**
  * The Core Domain Data Structures block (lines 157–236) omits `CubicSeg`, `TabCandidate`, `PolygonRing`, `StageId`, `SplicedPuzzle`, `KerfedGrid`, and `PipelineResult` (referenced in lines 97, 101, 187, 196, 208–209, 232, 395, 410) and does not show how `frame`, `image_href`, `image_bounds_mm`, `rows`, and `cols` pass to downstream stage emitters.
  * Lines 253, 262, and 430 do not specify the gutter width/height or padding used for `GridLabels` (`anchor_box_mm`), unlike Stage 3/4 corner digits (which specify `slot_w x slot_h` with `10%` padding).
  * Line 276 writes `slot_dx, slot_dy` without linking that notation to `PageSlot.piece_offset_mm` (line 216).
  * Line 122 states `--dry-run` prints "layout metrics" without defining them or specifying whether `--dry-run` checks `--overwrite` collisions or skips in-memory SVG emission.

### 9. Undefined `EGM` Acronym in Provenance Footer — Severity: Nit
* **Reviewer Lens:** `egm-comprehension`
* **Affected Scope:** Provenance footer (`EGM`), lines 476, 478
* **Discrepancy Description & Rationale:** Lines 476 and 478 use the acronym `EGM` without defining or linking it anywhere in the document. While this does not impair technical comprehension of `puzzle-gen`, a fresh reader will not know what `EGM` stands for.
