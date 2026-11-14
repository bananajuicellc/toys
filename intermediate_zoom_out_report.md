# Goldfish Review Report (Stage 8: Intermediate Zoom Out Review)

**Target:** `docs/designs/puzzle-gen.md` (`Lines 1-478`)

**Prior Stage Reports Reviewed:**
* `zoom_out_report.md`
* `comprehension_report.md`
* `critic_report.md`

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-zoom-out` | completed |

## Overall Assessment

`docs/designs/puzzle-gen.md` has not been revised since Stage 5, so all prior architectural simplicity issues in `zoom_out_report.md` (Findings #2–#7, #12–#13) remain open. Applying several remediation recommendations from `comprehension_report.md` and `critic_report.md` verbatim would compound over-engineering, deepen altitude drops into code weeds, and introduce YAGNI feature creep. Several Comprehension/Critic findings dissolve completely when replaced by simpler architectural choices.

*(Workspace check via `README.md`, `Cargo.toml`, `src/puzzle-gen/Cargo.toml`, `src/puzzle-gen/src/main.rs`, and `src/sticker-size/*` confirms `src/puzzle-gen` is currently an empty 3-line `Hello, world!` binary stub and `src/sticker-size` is an unrelated Leptos/WASM PNG/GIF sprite inspector with no SVG, YAML, or 2D geometry utilities to reuse.)*

## Prioritized Findings (Blockers -> Major -> Nits)

### 1. Remediation of Type/Diagnostic/Verification Gaps Would Compound Over-Fragmentation and Code Weeds - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `### Architecture Overview` (lines `83-107`), `### Core Domain Data Structures` & pipeline (lines `155-286`), `### Target Files to Create/Modify` (lines `385-464`); `zoom_out_report.md` Findings #2–#5, #12; `comprehension_report.md` Findings #6, #8; `critic_report.md` Findings #6, #13, #16
* **Simplicity Rationale:** Stage 5's over-fragmentation and altitude drops remain unaddressed (12 Rust source files for a single-user local CLI, 80 lines of Rust struct definitions and ~20 function signatures in the feature spec, a `DiagnosticBag`/`Severity`/`StageId` framework, a redundant `verify.rs` pass comparing `PaginatedLayout` against itself, and bespoke `CubicSeg`/`PolygonRing` wrappers around `kurbo`). Adopting `comprehension_report.md` #8 and `critic_report.md` #16 verbatim would worsen this by adding 7 more concrete Rust types (`CubicSeg`, `TabCandidate`, `PolygonRing`, `StageId`, `SplicedPuzzle`, `KerfedGrid`, `PipelineResult`) to `Core Domain Data Structures`, patching `Diagnostic.severity` with `Info` (`critic_report.md` #13), and keeping/tuning `verify.rs` (`critic_report.md` #6/#7, `comprehension_report.md` #6).
* **Recommendation:** Consolidate into 3–4 modules (`config`, `pipeline`/`geometry`, `svg`, `main`), eliminate `DiagnosticBag`/`Severity`/`StageId`, `verify.rs`, Stage 2 fields on `KerfedPiece`, and `CubicSeg`/`PolygonRing` wrappers, and lift `Core Domain Data Structures` to high-level stage inputs/outputs and invariants. This simultaneously dissolves `critic_report.md` #13 (`Diagnostic.severity`), `critic_report.md` #16 (missing `SplicedPuzzle`/`KerfedGrid` definitions), and `comprehension_report.md` #6 / `critic_report.md` #6's `verify.rs` wording conflicts.

### 2. Full-Slot Corner Page Numbers + Proposed Crosshairs / Dynamic Slack Packing Overcomplicate Sheet Layout - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `## Goals` (lines `37-38`), `#### Stage 3 & Stage 4` (lines `265-277`); `critic_report.md` Finding #11; `comprehension_report.md` Finding #8; `zoom_out_report.md` Finding #8
* **Simplicity Rationale:** `critic_report.md` #11 proposes keeping full piece-sized (`slot_w x slot_h`) corner page-number slots (which waste 2 piece slots per page and force `page_cols * page_rows >= 3`), adding explicit multi-digit glyph aspect-ratio/gap/padding specs, adding another `Alternatives Considered` entry, and adding explicit laser-alignment crosshairs/ticks inside corner anchors—a new geometric feature. Meanwhile, `zoom_out_report.md` #8's dynamic residual-slack anchor placement introduces dual-box (`anchor_w x anchor_h` vs. `slot_w x slot_h`) packing and collision checks.
* **Recommendation:** Decouple corner page numbers from the piece grid once, statically: place compact fixed-size vector digits (incorporating a sharp baseline/corner L-tick as part of the static glyph if a registration point is wanted) inside a fixed corner margin or compact top/bottom corner strip outside the piece grid (`page_cols * page_rows >= 1`). That avoids losing piece slots, eliminates `cols * rows >= 3`, avoids dynamic slack tracking, and removes Critic #11's multi-digit slot-scaling weeds and extra crosshair subsystem.

### 3. Speculative Config DSL, Multi-Unit Projection, and Debug `GridLabels` Create Downstream Patching Churn - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `### YAML Configuration Schema & Unit Normalization` (lines `129-153`), `Stage 1` (line `253`), `Stage 2` (lines `256-262`), `config.rs` (line `405`); `zoom_out_report.md` Findings #6–#7; `comprehension_report.md` Findings #1, #7, #8; `critic_report.md` Findings #5, #7, #8
* **Simplicity Rationale:** Several Comprehension and Critic recommendations patch edge cases caused solely by speculative complexity already in `puzzle-gen.md`:
  * **Unit/`CoordFrame` bloat (`comprehension_report.md` #7, `critic_report.md` #8):** Adding `width`/`height` vs. `inkscape:document-units` precedence rules, aspect-ratio tolerances, missing-attribute fallbacks, and a `viewBox` `(min_x, min_y)` origin field on `CoordFrame` adds plumbing largely required to preserve `doc_unit` across Stages 1–3 while outputting Stage 4 in a separate configurable `page_unit`.
  * **Config DSL/knob patches (`comprehension_report.md` #7, `critic_report.md` #7):** Documenting non-`1` `version` rejection, per-knob bounds (`segment_length > 0` to prevent `kerf: 0mm` infinite subdivision), a `print_bleed < kerf/2` warning, dynamic `min(0.005 mm, segment_length_mm * 0.05)` simplification, and `verify.rs` rounding tolerances patches traps created by `version: 1`, `"N kerf"` mini-DSLs, `segment_length`, and `verify.rs`.
  * **Debug `GridLabels` expansion (`comprehension_report.md` #7–#8, `critic_report.md` #5):** Specifying `GridLabels` `anchor_box_mm` dimensions, `>= 10x10` multi-digit sizing, and `score_color` consistency across Stages 1, 2, and 4 deepens a debug-only vector-digit gutter engine (`digits.rs` + negative-`viewBox` expansion).
* **Recommendation:**
  1. Standardize outputs on `mm` (eliminating `page_unit` and `doc_unit` back-projection—or keep Stages 1–3 in input units without `page_unit`), drop `version: 1`, `segment_length` (derive internally from `kerf` with a safe floor), `score_color` (fix `#000000` / `#0000ff`), and `"N kerf"` / `"W x H"` string DSLs.
  2. Drop `GridLabels` vector-digit gutters and negative-`viewBox` expansion in debug-only Stages 1–2 (use piece `id`s or standard `<text>`), dissolving `anchor_box_mm`, multi-digit `GridLabels` sizing, and `score_color` scope mismatches.
  3. Define one fixed stroke width (e.g., `0.1 mm`) for stroked paths rather than adding another config parameter (minimally resolving `comprehension_report.md` #1).

### 4. Scope Creep Risks in Critic Recommendations for `## Risks & Limits` and `## Non-Functional Requirements` - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `## Non-Functional Requirements` (lines `354-367`); `critic_report.md` Findings #1, #22
* **Simplicity Rationale:** Adding a brief `## Risks & Limits` section satisfies `feature-spec-standard.md`, but `critic_report.md` #1 asks for security bounds on *"untrusted SVG/YAML parsing, embedded raster memory limits, path/glob safety"* and scalability ceilings (*"max grid size, page counts"*), while `critic_report.md` #22 suggests *"either add measurable NFR targets or consolidate duplicated functional rules."* For a local single-user CLI in a small-experiments repository (`README.md`), inventing adversarial raster-memory caps, max grid/page ceilings, or synthetic latency/file-size SLAs introduces YAGNI scope creep and gratuitous runtime checks.
* **Recommendation:** Keep `## Risks & Limits` tightly scoped to real geometric/operational risks (`i_overlay` `i32` coordinate bounds at micrometer scale, zero-kerf/collapsed-neck validation, overwrite protection), and prune redundant functional repetition from `## Non-Functional Requirements` instead of adding speculative SLA numbers.

### 5. Avoid Custom Bezier-Intersection Math in Stage 1; Heal Neck Bowties via Single `i_overlay` Pass in Stage 2 - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `#### Stage 1` & `#### Stage 2` (lines `248-261`), `src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` (lines `203-324`); `zoom_out_report.md` Finding #1; `critic_report.md` Finding #3
* **Simplicity Rationale:** Both `zoom_out_report.md` #1 and `critic_report.md` #3 identify that tabs in the reference Stage 0 SVG attach to edge interiors and have Bezier handles (`path1`, `path2`, `path10`) that cross straight edge lines, causing bowtie self-intersections when spliced. Implementing `zoom_out_report.md` #1's *"cubic-line root-finding or planar cleanup in `stage1.rs`"* alongside `i_overlay` offsetting in Stage 2 risks building custom Bezier-intersection math in `stage1.rs`.
* **Recommendation:** Keep Stage 1 purely topological (project tab endpoints onto interior internal-edge segments and splice `[Line, Tab, Line]`) and rely on a single `i_overlay` self-union pass (`OverlayRule::Subject`, already a chosen dependency) at Stage 2 flattening/offsetting time to heal boundary-crossing loops before buffering.

### 6. Drop Speculative `run-metadata` Comment, Non-Axis-Aligned `<image>` Transforms, and Parser Weeds - Severity: Major
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** `### CLI Interface & File Lifecycle` (lines `114-127`), `Stage 0` (lines `241-245`), `svg_emit.rs` (lines `448-449`); `zoom_out_report.md` Finding #13; `comprehension_report.md` Findings #3, #5; `critic_report.md` Findings #2, #8, #9
* **Simplicity Rationale:**
  * `comprehension_report.md` #3 notes that the `run-metadata XML comment` in `svg_emit.rs` (lines `448-449`) is unspecified in the feature spec. Defining a comment format would cement an unmotivated speculative feature (`zoom_out_report.md` #13); deleting that comment from lines `448-449` resolves the finding with zero complexity.
  * `critic_report.md` #2's fallback suggestion (*"or preserve residual transforms"* for rotated/skewed `<image>` elements) would add non-axis-aligned `<pattern>` matrix math; simply rejecting rotated/skewed `<image>` transforms on ingest is cleaner.
  * `comprehension_report.md` #5 and `critic_report.md` #8 risk pulling parser implementation weeds into the feature spec (exhaustive rules for rounded `<rect>`, foreign children, duplicate layers, `no-kerf-manual-attempt` `z`-closed stray paths despite `Non-Goals` line `46`). Keep Stage 0 rules concise in the spec (axis-aligned `<rect>` grid + single `data:` `<image>` + single-subpath open `<path>` tabs after `< 0.05 mm` stray filtering).
  * `critic_report.md` #9's `--prefix` path-separator guard and pruning stale `stage4-page{K}.svg` files (`K > page_count`) *after* writing new files are simple and sound; avoid turning local SVG file output into a complex multi-file transactional rollback scheme.

### 7. Consolidate Redundant WASM Isolation Claims & Document Empty Stub Baseline - Severity: Nit
* **Reviewer Lens:** `egm-zoom-out`
* **Affected Scope:** Lines `49`, `109-111`, `390`, `468`; `Cargo.toml` (lines `1-3`); `comprehension_report.md` Finding #2; `critic_report.md` Finding #16
* **Simplicity Rationale:** Because `Cargo.toml` defines `src/puzzle-gen` and `src/sticker-size` as separate crates with no `[workspace.dependencies]`, `src/puzzle-gen/Cargo.toml` cannot affect `src/sticker-size` by Cargo semantics. Repeating WASM-isolation caveats in four sections is unnecessary clutter.
* **Recommendation:** Replace repeated `src/sticker-size` WASM caveats with one sentence stating `src/puzzle-gen` is currently an empty binary stub (`src/puzzle-gen/src/main.rs`) alongside the sibling `src/sticker-size` WASM utility (resolving `comprehension_report.md` #2), and commit to one YAML crate on line `111` (`critic_report.md` #16).

---

## Safe Prior-Stage Fixes (Minimal / Complexity-Reducing)

The following prior-stage recommendations cleanly fix real defects or simplify the spec without introducing bloat and should be retained during revision:
* **Edge-interior tab matching & `<image>` `(0, 0)` pattern fix:** `critic_report.md` #4, #2 (excluding `"or preserve residual transforms"`), and `comprehension_report.md` #4.
* **Accurate `path19` test expectation & documentation cleanup:** `comprehension_report.md` #5 (`path19` test phrasing) and #9 (`EGM` footer), plus `critic_report.md` #10 (dropping `"Rejected by author:"` and deduplicating `Cons`), #12 (H2 headings), #14–#15, #17–#21, and #23.
* **Minimal Stage 2 `bleed_polygon` handling:** `zoom_out_report.md` #9 (enforce single-ring neck preservation only on `cut_polygon`, allowing `bleed_polygon` to keep its positive-area outer boundary).
