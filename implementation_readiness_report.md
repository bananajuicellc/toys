# Goldfish Review Report (Stage 9: Implementation Readiness Test)

**Target:** `docs/designs/puzzle-gen.md` (`DetailedRange: Lines 383-478`, with context `Lines 1-382`)

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-implementation-readiness` | completed |
| `egm-codebase-details` | completed |
| `egm-style` | completed |

## Workspace Baseline Check (`egm-codebase-details`)

Inspected `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` (`channel = "nightly"`), `src/puzzle-gen/Cargo.toml` (`edition = "2024"`, empty `[dependencies]`), `src/puzzle-gen/src/main.rs` (3-line `"Hello, world!"` stub), `src/sticker-size/Cargo.toml`, `src/sticker-size/src/main.rs`, `src/sticker-size/src/app.rs`, and `src/sticker-size/src/sprite_inspector.rs`. `src/sticker-size` is a browser/WASM Leptos application for PNG/GIF pixel inspection (`ril`, `base64`, `urlencoding`) and contains no reusable SVG, YAML, CLI, unit-conversion, or vector-geometry utilities. Root `Cargo.toml` defines no `[workspace.dependencies]`; `Cargo.lock` includes only `serde` (`1.0.229`) transitively via `sticker-size`.

## Prioritized Findings (Blockers -> Major -> Nits)

### 1. Stage 1 Tab-to-Edge Matching & Splicing Contradicts Reference Stage 0 SVG Geometry; Impossible `path19` Test Assertion - Severity: Blocker
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`, `egm-style`
* **Affected Scope:** Lines `417-420` (`#### 7. src/puzzle-gen/src/stage1.rs`), Lines `461-464` (`#### 15. src/puzzle-gen/tests/integration_pipeline.rs`), Line `472` (`### Verification Plan`), plus context lines `181-187`, `248-252`, `372`
* **Discrepancy Description:**
  * Line `420` instructs `splice_tabs` to `"match TabCandidate endpoints against edge endpoints within 0.5 mm, reverse curve orientation when needed, snap exact endpoints onto clustered grid vertices"`, and line `252` replaces the straight rectangle segment directly with the curve. In `src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` (`4in x 4in`, `0.8in = 20.32mm` cells), **none** of the 39 tab `<path>` elements start or end at grid corners—all terminate in the interior of shared internal edges 5–10 mm from corners (e.g., `path1` spans `(0.80000027, 0.3202571)` to `(0.79999998, 0.4526716)` on edge `x = 0.8, y in [0, 0.8]`, >16x the `0.5 mm` tolerance from corners). `rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg` (`rect1`, `rect1-5`, `rect1-18`) confirms each tabbed edge consists of `[corner -> tab_start straight line] + [tab curve] + [tab_end -> corner straight line]`. Following line `420` literally rejects all 39 tabs and breaks line `464`'s `--no-strict` test and line `472`'s smoke run.
  * Line `464` asserts strict mode will fail `"citing missing internal edge path19's (row, col) coordinates"`. `step0` omits `path19` entirely (`path18` is followed by `path20`), and the actual missing internal edge is `((2, 0), (3, 0))` along `y = 2.4in, x in [0, 0.8in]`. Because `path19` is absent from the SVG XML, `puzzle-gen` cannot identify or cite `"path19"`.
* **Rationale & Recommendation:** Update `stage1.rs` (line `420`) and Stage 1 (lines `248-252`) to specify point-to-segment matching (`perp_dist <= 0.5 mm` and longitudinal projection within `[-0.5 mm, edge_len + 0.5 mm]`, rejecting outer-border and ambiguous/duplicate matches), projecting/snapping endpoints onto the clustered edge line (updating first/last `CubicSeg` endpoints), orienting via longitudinal projection, and emitting 3-part splicing (`LineTo(tab_start)` if non-zero, `CubicSeg` chain, `LineTo(edge_end)` if non-zero). Update line `464` (and `372`) to assert missing internal edge `((2, 0), (3, 0))`.

### 2. Broken `2x2` Mirrored `<pattern>` Double-Offset and Missing `preserveAspectRatio` - Severity: Blocker
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`
* **Affected Scope:** Lines `445-448` (`#### 12. src/puzzle-gen/src/svg_emit.rs`), plus context lines `253`, `262`, `276`
* **Discrepancy Description:**
  * `emit_stage1`–`emit_stage3` rely on line `253`, which emits `<image>` inside `<defs>` at transformed `(img_x, img_y)` with size `(W, H)` AND sets `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse" x="img_x" y="img_y" width="2W" height="2H">` with four `<use>` tiles (`normal at (0, 0)`, `translate(2W, 0) scale(-1, 1) at (W, 0)`, etc.). In SVG, `<use>` evaluates `<image>` inside `<pattern>`'s `(img_x, img_y)` origin, double-shifting the normal tile to `(2*img_x, 2*img_y)` and corrupting the mirrored quadrants whenever `(img_x, img_y) != (0, 0)` (note `manual-attempt.svg` line `85` has a non-zero layer translation).
  * Setting `x="W"` alongside `transform="translate(2W, 0) scale(-1, 1)"` on `<use>` maps `[0, W]` back onto `[0, W]`.
  * `step0` line `41` sets `preserveAspectRatio="none"` on `<image>`; omitting it causes SVG's default `xMidYMid meet` to letterbox non-square rasters.
* **Rationale & Recommendation:** Retain `image_preserve_aspect_ratio` on `IngestedPuzzle` (default `"none"` if omitted), emit `<image id="puzzle-image" x="0" y="0" width="W" height="H" preserveAspectRatio="..." xlink:href="..."/>` at `(0, 0)` inside `<defs>`, apply `x="img_x" y="img_y"` solely on `<pattern>`, and emit the four `<use>` nodes without `x`/`y` attributes on the transformed tiles.

### 3. Non-Existent `i_overlay::JoinType::Round` API, Missing Winding/Hole Checks, Zero-Offset Contradiction, and S-Curve Subdivision Flaw - Severity: Blocker
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`
* **Affected Scope:** Line `390` (`Cargo.toml`), Lines `422-425` (`#### 8. src/puzzle-gen/src/stage2.rs`), plus context lines `112`, `256-261`
* **Discrepancy Description:**
  * `i_overlay` has no `JoinType::Round` enum; outline buffering uses `LineJoin::Round(<precision>)` with `OutlineStyle` and `IntOutlineOffset`/`OutlineOffset`, requiring an explicit arc-precision parameter not specified in the plan.
  * Line `420` builds `SplicedPiece.contour` `"clockwise"` in SVG Y-down coordinates (positive signed area = mathematical CCW in `(x, y)`), which `i_overlay` treats as outer contours vs. negative-area holes, yet `stage2.rs` never specifies winding orientation or area-sign normalization prior to offsetting.
  * Line `425` validates `"a single positive-area non-self-intersecting ring larger than the un-offset piece"`, omitting explicit hole rejection (`shapes.len() == 1 && shapes[0].len() == 1`) and failing when `kerf_mm == 0` or `print_bleed_mm == 0`. Note also that curves in `step0` (`path2` line `208`) self-intersect the straight shoulder slightly, requiring explicit pre-offset `i_overlay` self-intersection resolution (`FillRule::NonZero`).
  * Line `112` attributes `"bounded-length flattening"` to `kurbo`, whereas `kurbo::flatten` bounds sagitta error rather than chord length (`<= segment_length_mm`); line `425`'s fallback midpoint chordal deviation check misses S-curves crossing the chord at `t = 0.5` and lacks a recursion depth limit.
* **Rationale & Recommendation:** Pin the exact `i_overlay` version and API (`IntOutlineOffset`, `OutlineStyle`, `LineJoin::Round(<value>)`), require positive-area `(x, y)` winding normalization and pre-offset self-intersection healing, assert `shapes.len() == 1 && shapes[0].len() == 1` (allowing `>=` area at zero offset or forbidding `0`), specify max control-point-to-chord flatness + chord length `<= segment_length_mm` with a recursion cap, and define collinear simplification details.

### 4. `CoordFrame::from_svg_root` Reads Wrong Element for `inkscape:document-units`, Omits `viewBox` Origin, and Conflicts With `ingest_svg` Signature - Severity: Blocker
* **Reviewer Lens:** `egm-codebase-details`
* **Affected Scope:** Lines `402-405` (`#### 4. config.rs`), `412-415` (`#### 6. ingest.rs`), `442-449` (`#### 12. svg_emit.rs`), plus context lines `152`, `158-163`, `242`, `279`
* **Discrepancy Description:**
  * Lines `405` and `152` read `inkscape:document-units` from the root `<svg>` element. In both reference Inkscape SVGs (`step0` line `25`, `manual-attempt` line `58`), `inkscape:document-units="in"` is an attribute of **`<sodipodi:namedview>`**, not `<svg>`. Reading only `<svg>` misses `inkscape:document-units` on Inkscape inputs. Conversely, line `279` requires `inkscape:document-units` in Stage 4, while lines `446-447` and `449` omit `<sodipodi:namedview>` from Stages 1, 2, and 4.
  * `CoordFrame` (lines `158-163`) contains `page_unit: LengthUnit` (which comes from `Config`), yet `ingest_svg(svg_xml: &str, bag: &mut DiagnosticBag)` (line `415`) accepts neither `Config` nor `page_unit`.
  * Line `242` requires applying root `viewBox` offset/scale via `CoordFrame`, yet `CoordFrame` has no `viewbox_min_user: (f64, f64)` field and line `405` omits `viewBox` origin, aspect-ratio tolerance, omitted-`viewBox`, and unitless `width`/`height` rules.
* **Rationale & Recommendation:** Parse `inkscape:document-units` from `<sodipodi:namedview>` (falling back to `<svg>`), emit `<sodipodi:namedview inkscape:document-units="...">` in Stages 1–4, remove `page_unit` from `CoordFrame` (already on `Config`) or pass it to `ingest_svg`, add `viewbox_min_user: (f64, f64)` to `CoordFrame`, and specify exact unit/aspect-ratio resolution rules.

### 5. Unresolved Dependency Choice, Unpinned Crate Versions, Omitted `Cargo.lock`, and Repo-Polluting Smoke Path - Severity: Blocker
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`, `egm-style`
* **Affected Scope:** Lines `385-391` (`Target Files`, `1. src/puzzle-gen/Cargo.toml`), Line `472` (`Verification Plan — E2E/Integration`), plus context line `111`
* **Discrepancy Description:**
  * Line `390` leaves the YAML crate unresolved (`serde_norway (or serde_yaml_ng)`) and omits version numbers and required features for all 8 dependencies (`clap`, `roxmltree`, `svgtypes`, `xmlwriter`, `serde`, YAML crate, `kurbo`, `i_overlay`).
  * Root `Cargo.lock` (tracked at workspace root for `puzzle-gen`) is omitted from `Target Files to Create/Modify`.
  * Line `472`'s CLI smoke command omits `--output-dir`, causing `puzzle-gen` to write outputs into tracked `src/puzzle-gen/examples/generated/` (unignored by `.gitignore`).
* **Rationale & Recommendation:** Violates the Zero-Decision Rule and Target Scope & File Boundary. Select one YAML crate explicitly, pin dependency versions/features (plus dev-dependencies or `std::env::temp_dir()` for tests), add `Cargo.lock (Modify)` to Target Files, and pass `--output-dir target/smoke-out` in line `472`.

### 6. Missing Phasing & Buildable/Testable Checkpoints - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`
* **Affected Scope:** Lines `383-478` (`## Detailed Implementation Plan`)
* **Discrepancy Description:** Items 1–15 are listed as a flat 15-file manifest where `lib.rs` (Item 2) declares all 10 submodules and `run_pipeline` before Items 3–12 exist, preventing incremental `cargo check`/`cargo test` checkpoints until nearly every module is created.
* **Rationale & Recommendation:** Partition the plan into incremental phases (e.g., Phase 1: `Cargo.toml`/`Cargo.lock` + `diagnostics`/`config`/`model` + unit tests; Phase 2: `ingest`/`stage1`/`stage2`/`digits` + unit tests; Phase 3: `stage3_4`/`verify`/`svg_emit`/`lib`/`main`/`default-config.yaml`/`integration_pipeline.rs` + full checks), declaring only implemented submodules per phase in `lib.rs`.

### 7. Missing Cross-Module Type Definitions and `svg_emit` Signatures - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`
* **Affected Scope:** Lines `392-395` (`lib.rs`), `407-410` (`model.rs`), `420` (`stage1.rs`), `425` (`stage2.rs`), `430` (`digits.rs`), `442-449` (`svg_emit.rs`), `454` (`main.rs`), context lines `157-236`
* **Discrepancy Description:** `PipelineResult`, `SplicedPuzzle` (needed for `main.rs`'s `matched edge count`), `KerfedGrid`, `TabCandidate`, `CubicSeg`, and `StageOutputs` have no field/variant definitions; `PolygonRing` leaves open whether the closing point is repeated; `PagePlan` references `Polyline` while `digits.rs` returns `Vec<Vec<Point>>`; and `svg_emit.rs` omits function signatures for `emit_stage1..4`.
* **Rationale & Recommendation:** Define exact struct fields, enum variants, `PolygonRing` closure convention, unified polyline types, and `svg_emit.rs` function signatures in `model.rs`/`lib.rs`/`svg_emit.rs`.

### 8. Unspecified Stage 2 `stage2_offset_mm` Anchoring and Cell-Pitch Collision Hazard - Severity: Major
* **Reviewer Lens:** `egm-codebase-details`, `egm-implementation-readiness`
* **Affected Scope:** Lines `422-425` (`#### 8. stage2.rs`), plus context lines `210`, `262`
* **Discrepancy Description:** Line `425` specifies `"compute uniform Stage 2 grid offsets (max(cut_aabb) separated by config.piece_spacing_mm)"` without specifying the formula for `KerfedPiece.stage2_offset_mm` (center vs. min-corner alignment inside `[slot_w x slot_h]`). Naive cell-pitch shifts cause protruding tabs to collide with neighbors whenever tab overhang exceeds `piece_spacing_mm`.
* **Rationale & Recommendation:** Specify slot-aligned placement explicitly (e.g., `slot_center_x = c * (slot_w + spacing) + slot_w / 2.0`, `stage2_offset_mm.x = slot_center_x - cut_aabb.center().x`, or min-corner alignment inside `[slot_w x slot_h]`).

### 9. Unspecified Stage 1–4 SVG Geometry/Styling, Float Precision, Metadata Comment, and `--dry-run` Output - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`, `egm-style`
* **Affected Scope:** Lines `427-430` (`9. digits.rs`), `442-449` (`12. svg_emit.rs`), `451-454` (`13. main.rs`), plus context lines `122`, `253`, `262`, `273`, `277`, `281-282`
* **Discrepancy Description:**
  * `<stroke_w>` (lines `277`, `281`, `282`) is never defined in `mm` or scaled per stage, and Stage 1/2 `GridLabels` and Stage 2 pieces omit `stroke-width` / `fill` / `stroke` values.
  * `GridLabels` gutter dimensions, `anchor_box_mm`, offset from piece AABBs, and proportional root `width`/`height` expansion are omitted (lines `430`, `446-447`).
  * `digits.rs` omits per-side vs. total `10%` padding semantics, multi-digit spacing, baseline bar gap/width, and single-`<path>` vs. multi-`<path>` emission.
  * SVG numeric formatting precision is unspecified (e.g., 4-decimal inches `0.0001 in = 0.00254 mm` breaks `<= 1e-4 mm` parity).
  * Stage 3 omits whether `<g inkscape:groupmode="layer">` groups or baked vs. `<g transform>` corner-digit coordinates are emitted.
  * `"run-metadata XML comment"` (lines `448-449`) has no format/content spec (timestamps would break deterministic emission) and is absent from `FeatureRange`.
  * Line `448` misplaces the `"sized to page 1"` modifier onto `<inkscape:page>` instead of root `<svg>`.
  * `--dry-run` stdout metrics/behavior (line `454` vs. `122`) are unspecified.
* **Rationale & Recommendation:** Pin concrete values/formulas for `stroke_w`, `GridLabels` layout, `digits.rs` spacing, `>= 6` decimal inch / `>= 4` decimal `mm` serialization precision, Stage 3 layer structure, deterministic XML comment schema (or drop it), fixed modifier wording in line `448`, and `--dry-run` output behavior.

### 10. Underspecified `config.rs`, `stage3_4.rs`, and `verify.rs` Validation, Padding, and Tautology - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`, `egm-style`
* **Affected Scope:** Lines `402-405` (`4. config.rs`), `432-435` (`10. stage3_4.rs`), `437-440` (`11. verify.rs`)
* **Discrepancy Description:**
  * `config.rs`: Omits handling of `version != 1`, empty/comment-only YAML files, strict `> 0.0` vs. `>= 0.0` bounds (`margin`, `piece_spacing`, `print_bleed`, `kerf`, `2 * margin < page_size`), case-sensitivity of units/`#RRGGBB` vs. `#RGB`, and rejecting `"N kerf"` on `kerf` itself.
  * `stage3_4.rs`: Omits whether the `page_cols x page_rows` grid starts at `(margin_mm, margin_mm)` or centers in `usable_w x usable_h`, and leaves `"minimum page_size needed for 3 slots"` ambiguous (`1x3`, `3x1`, `2x2`). Also, `slot_center_mm - cut_aabb.center()` can push `bleed_polygon` up to `0.005 mm` beyond `[margin_mm, page_size_mm - margin_mm]` when simplification shifts `bleed_aabb.center()`.
  * `verify.rs`: `verify_invariants` compares `PaginatedLayout` against itself rather than checking serialized/projected Stage 3 and Stage 4 coordinates, uses `"strictly inside [margin_mm, page_size_mm - margin_mm]"` with zero epsilon (unlike `- 1e-4` on spacing) and scalar `page_size_mm`, and mislabels Stage 3's output as `cut_polygon` instead of `bleed_polygon`.
* **Rationale & Recommendation:** Pin exact `config.rs` bounds/syntax rules, define grid anchoring and symmetric half-extent slot sizing around `cut_aabb.center()`, define minimum `page_size` calculation, pass/verify projected coordinates with `1e-4 mm` margin epsilon in `verify.rs`, and fix `page_width_mm`/`page_height_mm` and Stage 3 wording.

### 11. Underspecified Stage 0 `ingest.rs` Path Elevation, Stray-Click Precedence, and Transform/Grid Rules - Severity: Major
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`, `egm-style`
* **Affected Scope:** Lines `412-415` (`6. src/puzzle-gen/src/ingest.rs`), context lines `192`, `241-245`
* **Discrepancy Description:**
  * `svgtypes::SimplifyingPathParser` yields `SimplePathSegment::Quadratic` for `Q`/`q`/`T`/`t`, requiring degree elevation (`kurbo::QuadBez::raise()`) to `CubicSeg`; omitted in line `415`.
  * `manual-attempt.svg` line `255` has a closed stray click `path4` (`m ... c ... z`). Line `415` leaves ambiguous whether `"sub-0.05 mm stray paths"` (misleadingly called `"zero-span stray clicks"`) are filtered before or after `ClosePath` rejection, whether nearly-closed open paths or `M`-only paths count as closed/empty, how rotated/skewed/flipped `<image>` transforms are handled, how duplicate required layers or non-`<rect>`/`<path>`/`<image>` children are handled, and what epsilon/rules govern rotated `<rect>` checks and `R x C` grid verification.
* **Rationale & Recommendation:** Specify `Quadratic` degree elevation, run sub-`0.05 mm` stray-path filtering before closed/multi-subpath checks, rename `"zero-span stray clicks"`, and document all `<image>`, `<rect>`, layer, child-element, and grid-validation rules explicitly.

### 12. Missing CLI & Edge-Case Integration Test Specs and Conflated Manual Inkscape Verification - Severity: Major
* **Reviewer Lens:** `egm-codebase-details`, `egm-style`
* **Affected Scope:** Lines `461-464` (`15. integration_pipeline.rs`), `466-472` (`Verification Plan`)
* **Discrepancy Description:**
  * Line `464` omits CLI binary invocation tests (`--dry-run`, `--output-dir`, `--prefix`, non-zero exit codes) and key edge-case tests (`<sodipodi:namedview>` unit resolution, interior-edge tab splicing, closed stray click `m...c...z`, rotated `<rect>`, duplicate/outer-border tab rejection, Stage 3/4 parity, `Quadratic`/`Arc` parsing) required by line `471`'s coverage claim.
  * Line `471` makes an unbounded universal coverage claim (`"every stage transition ... and verification error path"`), and line `472` mixes automated `cargo test` with manual Inkscape GUI inspection without pass criteria.
* **Rationale & Recommendation:** Enumerate the CLI and edge-case tests in `integration_pipeline.rs`, narrow line `471` to the enumerated paths, and separate manual Inkscape PDF inspection into its own bullet with explicit pass criteria (page count, physical dimensions, unclipped artwork, layer separation).

### 13. Terminology Collisions and Inconsistent Parameter/Flag Naming - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `390`, `420`, `464`, `469`
* **Discrepancy Description:**
  * Lines `464` and `469` write `cols * rows < 3` / `cols * rows >= 3` (colliding with puzzle grid `IngestedPuzzle.rows`/`cols`) instead of `page_cols * page_rows`.
  * Line `464` writes `--overwrite=false` / `--overwrite=true` whereas line `454` defines `--overwrite` as a boolean flag defaulting to `false`.
  * Line `420` writes `strict mode (!no_strict)` though `splice_tabs` takes `strict: bool`.
  * Line `390` writes `"strict YAML config deserialization"`, overloading `"strict"` (missing-tab mode).
* **Rationale & Recommendation:** Standardize on `page_cols * page_rows`, `without --overwrite` / `with --overwrite`, `(strict == true)` / `(when strict is false)`, and `"schema-validated YAML config deserialization (deny_unknown_fields)"`.

### 14. Uncalibrated Safety/Verification Claims (`verify.rs`, `main.rs`, `Verification Plan`) - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `440`, `454`, `468`, `471`
* **Discrepancy Description:**
  * Line `440` misplaces `"before any disk writes"` into `verify_invariants` (an orchestration concern of `main.rs`), uses informal `"baked"`, and writes `"strictly inside [margin_mm, page_size_mm - margin_mm]"`.
  * Line `454` claims `"write outputs atomically via same-directory temporary files + renames"`, overstating per-file temp-file replacement as cross-file atomicity across all outputs and stale deletions.
  * Line `468` claims `cargo check --workspace` verifies `src/sticker-size` `"remains unaffected"`, overstating a compilation check.
  * Line `471` writes `"--overwrite vs. stale-page pruning path"`, implying they are opposing alternatives rather than pruning occurring when `--overwrite` is enabled.
* **Rationale & Recommendation:** Relocate `"before any disk writes"` to `main.rs`, calibrate line `454` to `"write each output file atomically via a same-directory temporary file and rename"`, calibrate line `468` to `"verify src/sticker-size compiles cleanly"`, and rephrase line `471` to `"--overwrite disabled vs. enabled (including stale-page pruning)"`.

### 15. Overlong Run-On Paragraphs in `Target Files to Create/Modify` - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `390`, `405`, `415`, `420`, `425`, `435`, `454`, `464`
* **Discrepancy Description:** Eight `Proposed Changes` entries pack multiple responsibilities into 78–131 word semicolon-chained run-on sentences, obscuring precedence, control flow, and modifier attachment.
* **Rationale & Recommendation:** Split each multi-responsibility `Proposed Changes` entry into concise sub-bullets matching the bulleted structure used in `svg_emit.rs` (lines `446-449`).

### 16. Minor CLI Filesystem Guards and Parameter Consistency - Severity: Nit
* **Reviewer Lens:** `egm-implementation-readiness`, `egm-codebase-details`
* **Affected Scope:** Lines `420` (`7. stage1.rs`), `454` (`13. main.rs`)
* **Discrepancy Description:** Line `454` should note treating `ErrorKind::NotFound` on non-existent `--output-dir` as zero collisions before `create_dir_all`, restricting Stage 4 matching strictly to `<prefix>-stage4-page<digits>.svg` (preventing unrelated deletions and temp-file collisions), and reading `--config` YAML in `main.rs` before `Config::from_yaml`.
* **Rationale & Recommendation:** Clarify these filesystem edge cases in item `13` (`main.rs`).

### 17. Technical Copy-Editing Hygiene (Buzzwords, Intensifiers, Redundancies) - Severity: Nit
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `390`, `395`, `400`, `405`, `410`, `415`, `425`, `430`, `435`, `447-449`, `454`, `459`, `464`, `469`, `472`
* **Discrepancy Description:**
  * Subjective/jargon phrasing: `"top-level orchestration API"`, `"without touching the filesystem"` (line `395`), `"structured PipelineError formatting actionable CLI messages"` (line `400`), `"clears margins cleanly"` (line `435`), `"concise stdout summary"` (line `454`), `"for CUJ 2 discovery"` (line `459`).
  * Empty intensifiers / misleading modifiers: `"scoped strictly"`, `"exact"` (`f64` factors / slot centers), `"reuse"` (across files), `"visible stroked"`, `"completely"`, `"bake ... into flat"`.
  * Synonym/shorthand redundancy: `"bare unitless"`, `"unit conversion projection helpers"`, `"only operates"`, `"within 0.01 mm epsilon"`, `"single 2-point lines"`, `"flag/serif"`, `"Stage 1/2"` / `"Stage 3/4"`, `"E2E/Integration ... end-to-end"`.
* **Rationale & Recommendation:** Tighten to dry, direct, unambiguous engineering prose throughout `DetailedRange`.
