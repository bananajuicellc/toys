# Goldfish Annotated Review Report

**Status:** resolved
**Target:** `docs/designs/puzzle-gen.md` (`Lines 1-478`)

## Prioritized Findings & AI Suggestions

### [Issue 1] `ACT-B1`: Fix Stage 0/1 Bounded Early Snapping, Interior-Edge Tab Matching, 3-Part Splicing, and `path19` Test Assertion [`egm-inventive-catalyst, egm-comprehension, egm-design, egm-style, egm-implementation-readiness, egm-codebase-details`] - Severity: Blocker
*   **Affected Scope:** [Lines 244-252](docs/designs/puzzle-gen.md#L244-L252) (`Stage 0: Input SVG Ingestion`, `Stage 1: Tab Splicing & 2x2 Mirrored Pattern`); also [Line 372](docs/designs/puzzle-gen.md#L372), [Line 415](docs/designs/puzzle-gen.md#L415), [Line 420](docs/designs/puzzle-gen.md#L420), [Line 464](docs/designs/puzzle-gen.md#L464), [Line 472](docs/designs/puzzle-gen.md#L472)
*   **Discrepancy Description:** Stage 0 clusters `<rect>` edges within `0.01 mm` (float-noise scale only), and Stage 1 (lines `248-252`, `420`) matches `TabCandidate` endpoints against internal edge *corner vertices* within `0.5 mm`, snapping them onto grid corners and replacing the whole straight edge with the curve. In `src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg`, all 39 `<path>` tabs begin and end in the interior of internal edge segments (5–10 mm from corners; for example, `path1` spans `(0.80000027, 0.3202571)` to `(0.79999998, 0.4526716)` on `x = 0.8in`). Additionally, manual Inkscape sketches cannot maintain exact node snapping on piece `<rect>` boundaries or tab endpoints, and `path19` is absent from `step0` XML—the missing internal edge in `step0` is `((2, 0), (3, 0))` along `y = 2.4in, x in [0, 0.8in]`.
*   **Rationale:** Matching only corner vertices rejects every real tab in `step0`, breaks lines `464`/`472`, and tight `0.01 mm` / `0.5 mm` tolerances fail on hand-placed Inkscape pieces and tabs unless early bounded snapping aligns rectangles and interior tab endpoints onto canonical grid edges.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid blocker confirmed against `rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg`. Applying bounded early snapping across Stage 0 (`1.0 mm` `<rect>` edge clustering) and Stage 1 (`2.0 mm` joint endpoint-pair segment matching, 4-way intersection ambiguity rejection, orthogonal projection onto the edge line, and 3-part `[LineTo, Tab, LineTo]` splicing) handles imprecise Inkscape snapping while rejecting ambiguous corner placements.
*   ***Proposed Edit:***
    *   Location: Lines `244`, `248-252`, `372`, `415`, `420`, `464`, `472`
    *   Change:
        1. In Stage 0 (line `244`) and `ingest.rs` (line `415`), widen `<rect>` edge coordinate clustering tolerance from `0.01 mm` to `1.0 mm` so slightly misaligned adjacent piece boundaries snap early onto shared canonical `x`/`y` grid lines (and reject `<rect>` elements whose clustered bounds collapse or overlap).
        2. In Stage 1 (lines `248-250`) and `stage1.rs` (line `420`), match each `TabCandidate` jointly as an `(start, end)` endpoint pair against internal edge segments: require both endpoints to have perpendicular distance `<= 2.0 mm` to the same internal edge line, longitudinal projections `t_start, t_end` in `[-2.0 mm, edge_len + 2.0 mm]`, and nonzero span `|t_end - t_start| >= 1.0 mm`; reject any tab whose endpoint falls within `2.0 mm` of two perpendicular edges at a 4-way corner intersection.
        3. Project both matched tab endpoints orthogonally onto the clustered internal edge line (`t = clamp(t, 0.0, edge_len)`), update the first and last segment knots to `snapped_start` and `snapped_end`, reverse the curve when `t_end < t_start`, and in contour assembly (line `252`) splice three parts: `LineTo(snapped_start)` when `t_start > 1e-4 mm`, the tab Bezier chain, and `LineTo(edge_end)` when `edge_len - t_end > 1e-4 mm`.
        4. Update lines `372` and `464` to reference missing internal edge `((2, 0), (3, 0))` along `y = 2.4in, x in [0, 0.8in]` (noting `path19` is absent from `step0`) rather than asserting `"path19"` in diagnostics.

### [Issue 2] `ACT-B2`: Heal Self-Intersecting Tab-Neck Loops Before Stage 2 Offsetting [`egm-inventive-catalyst, egm-design, egm-zoom-out`] - Severity: Blocker
*   **Affected Scope:** [Lines 248-261](docs/designs/puzzle-gen.md#L248-L261) (`Stage 1` & `Stage 2`); also [Line 372](docs/designs/puzzle-gen.md#L372), [Line 425](docs/designs/puzzle-gen.md#L425)
*   **Discrepancy Description:** Multiple tabs in `step0` (`path1` line `205`, `path2` line `208`, `path10` line `234`) have cubic Bezier handles that double back across the straight edge line, and orthogonal early endpoint projection onto an edge line can also cross the adjacent straight segment. When spliced into Stage 1 contours, these overshoots produce small bowtie self-intersections at tab necks that break Stage 2's strict un-healed single-ring assumption (line `261`).
*   **Rationale:** Left unhealed, self-intersecting loops cause inverted rings or multi-island rejections during `i_overlay` buffering even on valid artwork sketches.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid blocker verified on `step0` paths. Healing self-intersections with `i_overlay` after flattening keeps Stage 1 topological and avoids brittle custom cubic-to-line root finding.
*   ***Proposed Edit:***
    *   Location: Stage 2 (lines `256-261`) and `stage2.rs` (line `425`)
    *   Change: Specify an `i_overlay` self-union pass (`OverlayRule::Subject`, `FillRule::NonZero`) immediately after Stage 2 curve flattening and before measuring the un-offset piece area and running `+kerf / 2` and `+print_bleed` outward buffering. Verify the healed un-offset contour produces a single positive-area ring with zero holes (`shapes.len() == 1 && shapes[0].len() == 1`), failing with `(row, col)` and `id` if it splits into disconnected pieces.

### [Issue 3] `ACT-B3`: Fix `2x2` Mirrored `<pattern>` Double-Offset, `<use>` Transform Syntax, and `preserveAspectRatio` [`egm-inventive-catalyst, egm-comprehension, egm-design, egm-style, egm-implementation-readiness, egm-codebase-details`] - Severity: Blocker
*   **Affected Scope:** [Lines 192, 242-243, 253, 262, 276, 415, 446-448](docs/designs/puzzle-gen.md#L253) (`IngestedPuzzle`, `Stage 0`, `Stage 1`, `Stage 2`, `Stage 3`, `ingest.rs`, `svg_emit.rs`)
*   **Discrepancy Description:** Putting `(img_x, img_y)` on `<image>` inside `<defs>` AND `x="img_x" y="img_y"` on `<pattern patternUnits="userSpaceOnUse">` shifts `<use>` tiles twice whenever `img_x != 0 || img_y != 0`. Setting `x="W"` alongside `transform="translate(2W, 0) scale(-1, 1)"` on `<use>` mirrors `[W, 2W]` back onto `[0, W]`. Omitting `preserveAspectRatio` defaults to `xMidYMid meet` instead of source `"none"` (`step0`), letterboxing non-square rasters. Finally, `image_bounds_mm: Rect` drops rotated or skewed `<image>` transforms.
*   **Rationale:** Double offsets and incorrect `<use>` coordinates misalign artwork against cut paths on Stages 1–3, and `preserveAspectRatio` omission distorts fitted images.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid SVG geometry defect verified against SVG 1.1/2 `<pattern>` and `<use>` semantics and `step0`.
*   ***Proposed Edit:***
    *   Location: Lines `192`, `242-243`, `253`, `415`, `446-448`
    *   Change:
        1. Store `image_preserve_aspect_ratio: String` (defaulting to `"none"` when omitted or preserved from source `<image>`) on `IngestedPuzzle`.
        2. On Stage 0 ingest, allow only translation and positive axis scaling transforms on `<image>`, folding them into `image_bounds_mm: Rect`; reject rotated, skewed, or negatively scaled `<image>` transforms with an error citing element `id`. Emit a `Warning` if `image_bounds_mm` does not cover the full `Pieces` grid bounds.
        3. In `svg_emit.rs` (`emit_stage1`, `emit_stage2`, `emit_stage3`), emit `<image id="puzzle-image" x="0" y="0" width="W" height="H" preserveAspectRatio="..." xlink:href="..." />` inside `<defs>`, set `x="img_x" y="img_y" width="2W" height="2H" patternUnits="userSpaceOnUse"` on `<pattern>`, and emit four `<use href="#puzzle-image">` nodes with NO `x`/`y` attributes on transformed quadrants (`<use href="#puzzle-image" />`, `transform="translate(2W, 0) scale(-1, 1)"`, `transform="translate(0, 2H) scale(1, -1)"`, `transform="translate(2W, 2H) scale(-1, -1)"`).

### [Issue 4] `ACT-B4`: Replace Non-Existent `i_overlay::JoinType::Round` API, Specify Winding/Hole Checks, and Fix S-Curve Subdivision & Zero-Offset Validation [`egm-implementation-readiness, egm-codebase-details, egm-inventive-catalyst`] - Severity: Blocker
*   **Affected Scope:** [Lines 112, 209, 256-261, 390, 422-425](docs/designs/puzzle-gen.md#L256-L261) (`Architecture Overview`, `Core Domain Data Structures`, `Stage 2`, `Cargo.toml`, `stage2.rs`)
*   **Discrepancy Description:**
    1. `i_overlay` uses `IntOutlineOffset` / `OutlineOffset`, `OutlineStyle`, and `LineJoin::Round(<precision>)`, not `JoinType::Round`, and omits the round-join precision value.
    2. Pre-offset winding normalization (positive signed area in `(x, y)` coordinates) and explicit hole checks (`shapes.len() == 1 && shapes[0].len() == 1`) are missing.
    3. Enforcing the strict single-ring check on `bleed_polygon` (`+print_bleed`) rejects valid runs when larger print bleed bridges across narrow empty socket cavities (`Z5-9`). Requiring offset area strictly `> unoffset_area` fails when `kerf == 0` or `print_bleed == 0`.
    4. Midpoint chordal deviation misses symmetric S-curves crossing the baseline at `t = 0.5` and has no recursion depth cap.
*   **Rationale:** Code generated against nonexistent `i_overlay` symbols fails to compile, S-curves under-tessellate, and zero-offset or large-bleed configurations fail unnecessarily.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid compilation and geometry defects identified in `IR9-3` and `Z5-9`.
*   ***Proposed Edit:***
    *   Location: Lines `112`, `209`, `256-261`, `422-425`
    *   Change:
        1. Specify `i_overlay` `OutlineOffset` / `IntOutlineOffset` with `OutlineStyle` and `LineJoin::Round(0.02)` (in `mm` before integer scaling, or unit-scaled equivalent).
        2. Normalize every closed flattened contour to positive signed `(x, y)` area before `i_overlay` operations.
        3. On `cut_polygon`, require `shapes.len() == 1 && shapes[0].len() == 1` (single outer ring, zero holes) and `area > unoffset_area` when `kerf > 0` (`area >= unoffset_area - 1e-3 && area > 0.0` when `kerf == 0`). On `bleed_polygon`, keep the largest positive-area outer boundary ring containing `cut_polygon` (allowing merged inner socket holes from bleed bridge-over to drop without failing) and require `area >= unoffset_area - 1e-3 && area > 0.0` when `print_bleed == 0`.
        4. Replace midpoint chordal deviation in curve flattening with max control-point-to-chord perpendicular flatness (`max(dist(c1, chord), dist(c2, chord)) <= segment_length_mm * 0.1`, where `chord = (p0, p3)`) and chord length `<= segment_length_mm`, capped at recursion depth `16`.

### [Issue 5] `ACT-B5`: Fix `CoordFrame::from_svg_root` (`<sodipodi:namedview>` Units, `viewBox` Origin, `ingest_svg` Signature Conflict) [`egm-codebase-details, egm-comprehension, egm-design`] - Severity: Blocker
*   **Affected Scope:** [Lines 151-163, 242, 253, 262, 275, 279, 402-405, 412-415, 442-449](docs/designs/puzzle-gen.md#L151-L163) (`YAML Configuration Schema & Unit Normalization`, `CoordFrame`, `Stage 0`, `Stages 1-4`, `config.rs`, `ingest.rs`, `svg_emit.rs`)
*   **Discrepancy Description:** Both reference Inkscape SVGs place `inkscape:document-units="in"` on `<sodipodi:namedview>`, not `<svg>`, so `CoordFrame::from_svg_root` fails on actual Inkscape inputs. Stages 1, 2, and 4 also omit `<sodipodi:namedview>` despite Stage 4 requiring `inkscape:document-units`. Additionally, `CoordFrame` declares `page_unit` (which belongs only in `Config`, not SVG ingest), omits `viewbox_min_user: (f64, f64)` needed to compose `viewBox` offset in Stage 0, leaves unitless `width`/`height` (`96 DPI px`) precedence unclear, omits aspect-ratio tolerance, and omits `1 user unit = 1 doc_unit` scaling in Stages 1–3.
*   **Rationale:** Without reading `<sodipodi:namedview>` and carrying `viewbox_min_user`, parsing fails on `rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` and emitted Stage 1–4 SVGs lack matching document units.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid blocker verified directly in `step0` and `manual-attempt` SVG headers.
*   ***Proposed Edit:***
    *   Location: Lines `151-163`, `242`, `253`, `262`, `275`, `279`, `405`, `415`, `446-449`
    *   Change:
        1. Remove `page_unit` from `CoordFrame` (keeping `page_unit` solely on `Config`), and add `viewbox_min_user: (f64, f64)` (defaulting to `(0.0, 0.0)` when `viewBox` is absent).
        2. Specify `CoordFrame::from_svg_root` precedence: read `inkscape:document-units` from `<sodipodi:namedview>` first, falling back to `<svg>` attribute and then `width`/`height` length unit (`mm`, `cm`, `in`, `px`); treat unitless `width`/`height` as CSS `px` (`25.4 / 96.0 mm`); require explicit `width`/`height` or fail loudly; derive `user_to_mm = (phys_w_mm / vb_w, phys_h_mm / vb_h)` (or `(1 px in mm, 1 px in mm)` when `viewBox` is omitted) and reject non-uniform scaling exceeding `1e-4` relative aspect difference (`|user_to_mm.0 - user_to_mm.1| / max > 1e-4`).
        3. Normalize Stage 1, Stage 2, and Stage 3 SVGs so `1 user unit = 1 doc_unit` (`viewBox` size in `doc_unit` matches physical `width`/`height` in `doc_unit`) and emit `<sodipodi:namedview inkscape:document-units="...">` across Stages 1, 2, 3, and 4.

### [Issue 6] `ACT-B6`: Add Dedicated `## Risks & Limits` Section [`egm-design, egm-structural, egm-zoom-out`] - Severity: Blocker
*   **Affected Scope:** [Lines 354-382](docs/designs/puzzle-gen.md#L354-L382) (`FeatureRange`, after `## Non-Functional Requirements`)
*   **Discrepancy Description:** `feature-spec-standard.md` requires a dedicated `Risks & Limits` or `Risks & Mitigations` section documenting failure modes, system limits, and mitigations, which is absent from `FeatureRange`.
*   **Rationale:** Missing operational limits and failure modes leaves key mathematical bounds and safety limits undocumented.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid structural blocker under `feature-spec-standard.md`, bounded per `IZ8-4` so only real geometric and CLI operational limits are added.
*   ***Proposed Edit:***
    *   Location: After `## Non-Functional Requirements` (around line `368`, before `## Additional Resources`)
    *   Change: Add `## Risks & Limits` covering:
        1. Integer coordinate range in `i_overlay` at `1000` units/mm (`1 um`): safe up to ~`2,000,000 mm` before `i32` overflow, rejecting inputs beyond safe bounds.
        2. Narrow tab necks or large `kerf` collapsing pieces into disconnected islands (mitigated by Stage 2 healed-contour and single-ring validation).
        3. Non-exact Inkscape node snapping or Bezier handle overshoots across grid edges (mitigated by `1.0 mm` Stage 0 `<rect>` clustering, `2.0 mm` Stage 1 joint segment snapping with 4-way corner rejection, and Stage 2 `i_overlay` self-union).
        4. Large `print_bleed` bridging socket cavities or zero `kerf` / zero `print_bleed` runs (mitigated by keeping outer positive-area boundary on `bleed_polygon` and `>= 0` offset area checks).
        5. Existing file collisions or leftover Stage 4 cut sheets on `--overwrite` (mitigated by collision abort, strict filename regex, and write-before-prune lifecycle) and laser CAM compatibility (`fill: none`, zero `<defs>`/`<image>`/`<pattern>`/`transform` attributes in Stage 4).

### [Issue 7] `ACT-B7`: Select `saphyr` for YAML Parsing, Pin Crate Versions, Add `Cargo.lock`, and Fix Smoke-Run Output Directory [`egm-implementation-readiness, egm-codebase-details, egm-style, egm-zoom-out`] - Severity: Blocker
*   **Affected Scope:** [Lines 111, 385-391, 464, 472](docs/designs/puzzle-gen.md#L385-L391) (`Architecture Overview`, `Target Files to Create/Modify`, `Verification Plan`)
*   **Discrepancy Description:** Lines `111` and `390` leave YAML crate choice undecided (`serde_norway (or serde_yaml_ng)`), `Cargo.toml` omits pinned versions/features for every dependency, the root `Cargo.lock (Modify)` is missing from Target Files, and line `472`'s smoke command omits `--output-dir`, writing unignored SVGs into `src/puzzle-gen/examples/generated/`.
*   **Rationale:** Implementers cannot lock `Cargo.toml` or run verification without polluting the repository without pinned dependencies and `--output-dir`. Furthermore, the author selected `saphyr` directly for YAML parsing.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid blocker resolved directly with `saphyr` (`0.0.6`), removing `serde` completely from `src/puzzle-gen`.
*   ***Proposed Edit:***
    *   Location: Lines `111`, `385-391`, `472`
    *   Change:
        1. Replace `serde` + `serde_norway (or serde_yaml_ng)` on lines `111` and `390` with `saphyr = "0.0.6"`.
        2. Pin exact dependency versions and features in `src/puzzle-gen/Cargo.toml`: `clap = { version = "4.5", features = ["derive"] }`, `roxmltree = "0.20"`, `svgtypes = "0.15"`, `xmlwriter = "0.1"`, `saphyr = "0.0.6"`, `kurbo = "0.11"`, `i_overlay = "1.10"`, and `[dev-dependencies] tempfile = "3.10"`.
        3. Add `Cargo.lock (Modify)` right after `src/puzzle-gen/Cargo.toml (Modify)` in `Target Files to Create/Modify`.
        4. Change line `472`'s smoke command to pass `--output-dir target/smoke-out` so test outputs remain inside `target/` (gitignored).

### [Issue 8] `ACT-M1`: Lift `FeatureRange` Altitude, Consolidate Modules, Use `kurbo` Directly, and Define Exact Cross-Module Signatures [`egm-zoom-out, egm-comprehension, egm-style, egm-implementation-readiness, egm-codebase-details`] - Severity: Major
*   **Affected Scope:** [Lines 83-112, 155-286, 385-464](docs/designs/puzzle-gen.md#L155-L236) (`Architecture Overview`, `Core Domain Data Structures`, `Stage-by-Stage Geometric Pipeline`, `Target Files to Create/Modify`)
*   **Discrepancy Description:** `FeatureRange` drops into 80 lines of concrete Rust struct fields, magic tolerances, and raw XML snippets, while `DetailedRange` fragments one local CLI into 12 Rust modules with redundant wrappers (`CubicSeg`, `PolygonRing` with ambiguous closure, `DiagnosticBag`/`StageId`). Conversely, `PipelineResult`, `SplicedPuzzle`, `KerfedGrid`, `TabCandidate`, `StageOutputs`, and exact `svg_emit` signatures are missing or conflict across modules (`Vec<Vec<Point>>` vs `Polyline`).
*   **Rationale:** Keeping high-level data contracts and invariants in `FeatureRange` while putting cohesive types and exact function signatures in `DetailedRange` avoids both feature-spec bloat and compiler mismatches during implementation.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Aligns `FeatureRange` with feature-spec altitude and gives implementers complete, unambiguous module boundaries and Rust signatures in `DetailedRange`.
*   ***Proposed Edit:***
    *   Location: Lines `83-107`, `155-286`, `385-464`
    *   Change:
        1. In `FeatureRange` (`Core Domain Data Structures`), replace Rust struct field dumps with high-level stage inputs/outputs, coordinate-unit contracts (internal canonical `mm`), and key invariants.
        2. Consolidate the Rust crate layout in `DetailedRange` into cohesive modules: `config.rs`, `model.rs`, `ingest.rs`, `stage1.rs`, `stage2.rs`, `paginate.rs` (including built-in vector digits), `svg_emit.rs`, `lib.rs`, and `main.rs` — dropping `DiagnosticBag`/`StageId` (returning typed warnings `Vec<Warning>` alongside `Result<PipelineResult, PipelineError>`), using `kurbo::{Point, Vec2, Rect, CubicBez, Line, PathEl, BezPath}` directly instead of custom `CubicSeg`/`PolygonRing` wrappers (with closed `Vec<kurbo::Point>` non-duplicated end rings for offset polygons).
        3. Document exact field lists and function signatures in `DetailedRange` for `PipelineResult`, `StageOutputs`, `IngestedPuzzle`, `SplicedPuzzle`, `KerfedGrid`, `PaginatedLayout`, and every public module entry point.

### [Issue 9] `ACT-M2`: Replace Redundant `verify.rs` Runtime Module With Construction Invariants & Output Tests [`egm-zoom-out, egm-comprehension, egm-design, egm-style, egm-implementation-readiness, egm-codebase-details`] - Severity: Major
*   **Affected Scope:** [Lines 283-286, 366, 437-440](docs/designs/puzzle-gen.md#L437-L440) (`Stage 3 & Stage 4`, `Non-Functional Requirements`, `verify.rs`)
*   **Discrepancy Description:** `verify_invariants` compares `PaginatedLayout` against itself before SVG serialization (`in canonical mm before unit projection`), mislabels Stage 3 as emitting `cut_polygon` instead of `bleed_polygon`, and uses zero-epsilon `[margin_mm, page_size_mm - margin_mm]` checks that fail on `1e-15` IEEE-754 noise or `0.005 mm` simplification shift.
*   **Rationale:** Re-checking in-memory arithmetic against itself catches serialization bugs poorly and introduces false failures near float tolerances.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Enforcing piece conservation, slot bounds, and symmetric cut/bleed clearance directly inside pagination plus verifying actual serialized SVG coordinates in unit/integration tests eliminates a tautological runtime pass.
*   ***Proposed Edit:***
    *   Location: Lines `283-286`, `366`, `395`, `437-440`
    *   Change: Remove `src/puzzle-gen/src/verify.rs` and the runtime `verify` module pass; enforce piece-count conservation, symmetric cut-centered `slot_w x slot_h` sizing (`>= piece_spacing_mm`), and `[margin_mm - 1e-4, page_w/h_mm - margin_mm + 1e-4]` margin checks directly inside `paginate.rs`; verify serialized Stage 3 vs. Stage 4 `<path>` parity (`<= 1e-4 mm` mapped back to `mm`) in `integration_pipeline.rs`.

### [Issue 10] `ACT-M3`: Specify `saphyr` Mapping Validation Schema, Missing Bounds, and Strict Config Error Terminology [`egm-zoom-out, egm-comprehension, egm-design, egm-style, egm-implementation-readiness, egm-codebase-details`] - Severity: Major
*   **Affected Scope:** [Lines 34, 39, 129-153, 256-260, 402-405, 456-459, 464](docs/designs/puzzle-gen.md#L129-L153) (`Goals`, `YAML Configuration Schema`, `Stage 2`, `config.rs`, `default-config.yaml`, `integration_pipeline.rs`)
*   **Discrepancy Description:** Lines `131`, `405`, and `464` rely on `#[serde(deny_unknown_fields)]` (incompatible with `saphyr`'s untyped DOM parser), omit behavior for empty YAML vs. multi-document YAML (`---`), leave `version != 1` and `#RRGGBB` case sensitivity/unquoted-hex comment stripping unspecified, omit `>= 0` vs `> 0` bounds, allow `segment_length: "0mm"` or `kerf: "0mm" + segment_length: "1 kerf"` division/infinite-subdivision errors, and break when `kerf < 0.05 mm` under `0.005 mm` simplification. Furthermore, calling schema rejection `"strict YAML"` conflicts with `--no-strict`.
*   **Rationale:** Unspecified YAML DOM checks and unguarded zero/small bounds produce crashes or infinite loops on valid user configs.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Adapts config schema validation cleanly to `saphyr = "0.0.6"` while closing every bounds/edge-case hole.
*   ***Proposed Edit:***
    *   Location: Lines `129-153`, `256-260`, `402-405`, `464`
    *   Change:
        1. Replace `#[serde(deny_unknown_fields)]` (`RawYamlConfig`) with `saphyr::Yaml::load_from_str` validation in `config.rs`: treat empty/comment-only YAML (`docs.is_empty()`) as default `Config`; reject multi-document YAML (`docs.len() > 1`) or non-mapping roots; iterate mapping entries, rejecting any unknown key or duplicate key; require integer `1` for `version` when present; require string scalars for length/color fields (adding a hint to quote `"#RRGGBB"` when `score_color` parses as null because unquoted `#` opens a YAML comment).
        2. Specify bounds: `kerf_mm >= 0.0`, `margin_mm >= 0.0`, `piece_spacing_mm >= 0.0`, `print_bleed_mm >= 0.0`, `segment_length_mm > 0.0` (if `kerf_mm == 0.0` causes `segment_length_mm == 0.0` via `"N kerf"`, reject `segment_length` or fall back to an explicit validation error naming both fields), and normalize `#rrggbb` case-insensitively.
        3. Scale Stage 2 collinear simplification tolerance to `min(0.005, segment_length_mm * 0.05)` so small `kerf` / `segment_length` values stay accurate.
        4. Rename `"strict YAML"` to `"schema-validated YAML (rejecting unknown keys)"` to reserve `"strict"` for `--no-strict` tab validation.

### [Issue 11] `ACT-M4`: Decouple Corner Page Numbers from Full Piece-Sized Slots [`egm-inventive-catalyst, egm-design, egm-style, egm-zoom-out, egm-implementation-readiness, egm-codebase-details`] - Severity: Major
*   **Affected Scope:** [Lines 37-38, 74, 78, 265-273, 307, 435](docs/designs/puzzle-gen.md#L265-L273) (`Goals`, `CUJ 3`, `Stage 3 & Stage 4`, `Alternative C`, `paginate.rs`)
*   **Discrepancy Description:** Reserving two full `slot_w x slot_h` puzzle piece cells (`(0, 0)` and `(page_rows - 1, page_cols - 1)`) solely for tiny corner page numbers wastes two piece positions per sheet, rejects sheets with `page_cols * page_rows < 3` (even when 2 pieces fit), leaves `"minimum page_size needed for 3 slots"` ambiguous across 1D/2D grids, and omits multi-digit spacing and an exact laser alignment mark.
*   **Rationale:** Placing compact corner numbers inside reserved top-left / bottom-right corner bands (or small fixed corner margins outside the piece grid) avoids wasting two full pieces per sheet and removes the artificial 3-slot threshold.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** `IZ8-2` / `Z5-8` / `CR7-11` show that fixed corner strips with a sharp baseline/L-tick corner anchor simplify pagination (`page_cols * page_rows >= 1`), eliminate slot waste, and give laser operators an unambiguous alignment point.
*   ***Proposed Edit:***
    *   Location: Lines `37-38`, `74`, `78`, `265-273`, `277`, `282`, `307`, `430`, `435`, and `## Alternatives Considered`
    *   Change: Place compact fixed-height vector page numbers (e.g., `4.0 mm` digit height with fixed horizontal inter-digit gap and a sharp L-shaped corner/baseline registration tick at known margin offsets) in the top-left and bottom-right corners outside the piece grid (reserving a fixed corner strip height inside `usable_h`, or diagonally opposite corner zones clear of pieces), simplify capacity checks to `page_cols >= 1 && page_rows >= 1` (`1` or more pieces per page, with a clear minimum `page_size` error stating `max_piece_slot + 2*margin + strip`), and add an entry in `## Alternatives Considered` explaining corner band placement vs. sacrificing full piece slots.

### [Issue 12] `ACT-M5`: Simplify Screen-Only Debug `GridLabels` Gutters and Negative-`viewBox` Logic [`egm-zoom-out, egm-comprehension, egm-design, egm-style, egm-implementation-readiness, egm-codebase-details`] - Severity: Major
*   **Affected Scope:** [Lines 15-16, 34, 38, 150, 253, 262, 430, 446-447](docs/designs/puzzle-gen.md#L253) (`Overview`, `Goals`, `YAML Configuration Schema`, `Stage 1`, `Stage 2`, `digits.rs`, `svg_emit.rs`)
*   **Discrepancy Description:** Stages 1 and 2 build a second vector-digit layout engine (`layout_gutter_index`), expand SVG `viewBox` into negative top/left coordinates, and reference `<score_color>` without defining gutter dimensions, `anchor_box_mm`, `>= 10x10` multi-digit sizing, proportional `width`/`height` expansion, or updating line `150` (which restricts `score_color` to Stage 4).
*   **Rationale:** Stages 1 and 2 are opened in Inkscape or browsers for debugging, never sent to a font-less laser cutter; standard SVG `<text>` elements or `r{row}_c{col}` IDs / `<title>` tooltips achieve identical inspection without gutter geometry or `score_color` coupling.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Aligns with `Z5-7` and `IZ8-3`, cutting redundant `layout_gutter_index` code while resolving all missing `anchor_box_mm` / gutter-width / `score_color` contradictions.
*   ***Proposed Edit:***
    *   Location: Lines `15-16`, `34`, `253`, `262`, `430`, `446-447`, `464`
    *   Change: In debug Stages 1 and 2, render row (`0..rows-1`) and column (`0..cols-1`) indices inside `GridLabels` using centered SVG `<text>` labels near the top/left edges (plus `id="piece-r{row}-c{col}"` on every piece `<path>`/`<g>`), removing `layout_gutter_index`, `anchor_box_mm`, and negative-`viewBox` gutter expansion (while expanding Stage 1/2 root `width`/`height`/`viewBox` by a fixed small label band above/left if needed, or keeping `<text>` inside a deterministic margin). Keep `score_color` scoped solely to Stage 4 `PageNumbers`.

### [Issue 13] `ACT-M6`: Define `<stroke_w>`, Stage 2 Styling, Digit Padding/Precision, Stage 3 Layers, `--dry-run`, and Drop `run-metadata` Comment [`egm-zoom-out, egm-comprehension, egm-design, egm-style, egm-implementation-readiness, egm-codebase-details`] - Severity: Major
*   **Affected Scope:** [Lines 122, 253, 262, 277, 281-282, 430, 442-449, 454](docs/designs/puzzle-gen.md#L442-L449) (`CLI Interface`, `Stages 1-4`, `digits.rs`, `svg_emit.rs`, `main.rs`)
*   **Discrepancy Description:**
    1. `<stroke_w>` (lines `277`, `281`, `282`) is undefined, Stage 1 omits `stroke-width`, and Stage 2 omits piece `fill`, `stroke`, and `stroke-width`.
    2. `digits.rs` leaves digit aspect ratio, inter-digit spacing, baseline gap, and `<path>` grouping unspecified; Stage 3 leaves piece layer wrapping vs. page groups unspecified; float formatting precision is omitted (4 decimal inches breaks `<= 1e-4 mm` parity); `--dry-run` stdout format is undefined; line `448` misplaces `"sized to page 1"` on `<inkscape:page>`.
    3. Lines `448-449` introduce an unspecified `"run-metadata XML comment"` absent from `FeatureRange`.
*   **Rationale:** Missing stroke widths, Stage 2 styles, digit geometry, and float precision leave serializers underspecified and break numeric parity tests.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Fixes every unspecified SVG rendering constant and removes the unrequested XML comment (`Z5-13`, `IZ8-6`).
*   ***Proposed Edit:***
    *   Location: Lines `122`, `253`, `262`, `277`, `281-282`, `430`, `446-449`, `454`
    *   Change:
        1. Pin physical hairline stroke width to `0.1 mm` (projected into `doc_unit` on Stages 1–3 and `page_unit` on Stage 4) for Stage 1–2 piece outlines, Stage 3–4 page numbers, and Stage 4 `Cut` paths; specify Stage 2 piece styling as `fill: url(#puzzle-pattern); stroke: #000000; stroke-width: <stroke_w>`.
        2. Specify vector digit aspect ratio (`0.6` width-to-height), inter-digit spacing (`0.2 * digit_w`), `10%` padding on each side (`80%` drawable box) or exact corner band bounds, baseline bar offset (`0.15 * digit_h` below digits), and single-`<path>` or grouped `<g>` emission.
        3. Wrap Stage 3 artwork in `<g inkscape:groupmode="layer" inkscape:label="PrintPieces">` and page numbers in `<g inkscape:groupmode="layer" inkscape:label="PageNumbers">`; move `"sized to page 1"` in line `448` explicitly onto the root `<svg>` element; serialize SVG floating-point coordinates with at least `6` decimal places (`{:.6}`) when projecting to inches/centimeters (`>= 4` in `mm`/`px`); specify `--dry-run` stdout summary fields; and delete the `"run-metadata XML comment"` from lines `448-449`.

### [Issue 14] `ACT-M7`: Define Stage 2 & Stage 3/4 Slot Anchoring Formulas, Fix `slot_w` Shadowing, and Enforce Symmetric Bleed Bounds [`egm-inventive-catalyst, egm-comprehension, egm-design, egm-style, egm-codebase-details, egm-implementation-readiness`] - Severity: Major
*   **Affected Scope:** [Lines 210, 215-216, 262, 266-277, 425, 435](docs/designs/puzzle-gen.md#L266-L277) (`KerfedPiece`, `PageSlot`, `Stage 2`, `Stage 3 & 4`, `stage2.rs`, `paginate.rs`)
*   **Discrepancy Description:** Stage 2 (`stage2_offset_mm`) and `FeatureRange` Stage 3/4 omit the exact slot translation formula within `[slot_w x slot_h]` (cell-pitch shifts alone cause neighbor cut overlaps when tabs extend asymmetrically). Page grid origin (`(margin_mm, margin_mm)` vs centered in `usable_w x usable_h`) is unstated. Line `262` reuses `slot_w`/`slot_h` with `cut_aabb` before lines `266-267` redefine them with `max(cut_aabb, bleed_aabb)`. Finally, centering by `cut_aabb.center()` while sizing `slot_w`/`slot_h` by `bleed_aabb.width` allows `bleed_polygon` to extend up to `0.005 mm` past margins when simplification shifts `bleed_aabb.center()`.
*   **Rationale:** Ambiguous slot anchoring and non-symmetric AABB bounds cause off-center cuts, bleed/margin violations, or Stage 3 vs. Stage 4 drift.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Defining one symmetric cut-centered slot size and explicit `(margin_mm, margin_mm)` grid origin keeps Stage 3 `bleed_polygon` and Stage 4 `cut_polygon` aligned within margins without floating-point surprise.
*   ***Proposed Edit:***
    *   Location: Lines `262`, `266-277`, `425`, `435`
    *   Change:
        1. Rename Stage 2 pitch variables to `stage2_slot_w` / `stage2_slot_h`, placed at `slot_min = (col * (stage2_slot_w + piece_spacing_mm), row * (stage2_slot_h + piece_spacing_mm))`, with piece translation `stage2_offset_mm = slot_center_mm - cut_aabb.center()` (so `target_pos - source_pos` maps `cut_aabb` squarely inside the Stage 2 cell).
        2. In Stage 3/4 pagination, anchor the top-left of the slot grid at `(margin_mm, margin_mm)` (below any reserved corner number strip if decoupled), compute `piece_offset_mm = slot_center_mm - cut_aabb.center()` cleanly, and size `slot_w = max_pieces(2.0 * max(|min_x - cut_cx|, |max_x - cut_cx|))` and `slot_h = max_pieces(2.0 * max(|min_y - cut_cy|, |max_y - cut_cy|))` across both `cut_polygon` and `bleed_polygon` vertices relative to `cut_aabb.center()` so `bleed_polygon` never exceeds `[slot_cx - slot_w/2, slot_cx + slot_w/2]`.

### [Issue 15] `ACT-M8`: Harden Stage 4 Filename Matching, Validate `--prefix`, Fix Deletion Ordering, and Support Debug Outputs on Errors [`egm-inventive-catalyst, egm-design, egm-style, egm-zoom-out`] - Severity: Major
*   **Affected Scope:** [Lines 40, 114-127, 365-366, 395, 425, 435, 454](docs/designs/puzzle-gen.md#L114-L127) (`Goals`, `CLI Interface & File Lifecycle`, `Non-Functional Requirements`, `lib.rs`, `stage2.rs`, `paginate.rs`, `main.rs`)
*   **Discrepancy Description:** Globbing `<prefix>-stage4-page*.svg` can match unrelated user files (e.g., `foo-stage4-page-notes.svg`); `--prefix` accepts slashes or `..` traversal; deleting existing Stage 4 pages *before* writing replacement files risks data loss on write failure (overclaimed as multi-file `"atomically"` on lines `40`, `366`, `454`); and downstream Stage 2/3 errors prevent Stage 1/2 debug SVGs from being written, hiding visual clues needed to fix tabs or pagination.
*   **Rationale:** Unsafe deletion ordering can destroy working outputs on disk errors, unvalidated prefixes risk unintended path writes, and emitting valid early debug stages helps authors diagnose broken SVGs.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Fixes filename matching, path validation, crash-safe write-then-prune ordering, accurate single-file replacement wording, and debug-on-error ergonomics (`Z5-10`, `CR7-9`).
*   ***Proposed Edit:***
    *   Location: Lines `40`, `118-127`, `365-366`, `454`
    *   Change:
        1. Match Stage 4 filenames strictly via exact `<prefix>-stage4-page<K>.svg` where `<K>` matches `[1-9][0-9]*` (non-empty positive base-10 digits, no leading zero).
        2. Validate `--prefix` on startup to be non-empty and contain no path separators (`/`, `\`) or `.` / `..` segments.
        3. Write and rename new stage files via per-file temporary files *before* pruning leftover `<prefix>-stage4-page<K>.svg` files where `K > new_page_count`, and phrase lines `40`/`366`/`454` accurately as per-file temp-file replacement followed by stale-page pruning rather than multi-file atomic transactions.
        4. Add `--emit-debug-on-error` (or write valid completed Stage 1 / Stage 2 debug SVGs when `--overwrite` and debug paths are safe and Stage 2/3 fails, if requested) or note clearly how Stage 1/2 partial artifacts behave on Stage 2/3 failure.

### [Issue 16] `ACT-M9`: Clarify Stage 0 Layer/Path Descriptions, Quadratic Elevation, and Stray-Click / `Z` Precedence [`egm-comprehension, egm-design, egm-style, egm-zoom-out, egm-implementation-readiness, egm-codebase-details`] - Severity: Major
*   **Affected Scope:** [Lines 13, 33, 59, 94, 241-245, 412-415](docs/designs/puzzle-gen.md#L241-L245) (`Overview`, `Goals`, `CUJ 1`, `Architecture Overview`, `Stage 0`, `ingest.rs`)
*   **Discrepancy Description:** Early sections say `"three layers"` and `"cubic-Bezier <path>"`, while Stage 0 ignores helper guide layers and parses `M/L/H/V/C/S/Q/A/Z`. In `ingest.rs`, `svgtypes::SimplifyingPathParser` yields `SimplePathSegment::Quadratic` and `ClosePath` without specifying `kurbo::QuadBez::raise()` elevation to cubic segments, running `< 0.05 mm` stray-click filtering *before* rejecting `ClosePath` (broken on closed stray clicks like `path4` in `manual-attempt.svg`), rotated `<rect>` epsilon, duplicate layer labels or unexpected child element errors, or complete `R x C` grid verification (`count == R * C`, no overlaps/gaps).
*   **Rationale:** Running `ClosePath` rejection before stray-click filtering aborts on accidental 0-size closed clicks in real Inkscape files, and omitting `Quadratic` elevation creates an unhandled match arm in Rust.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Resolves all `ingest.rs` edge cases and aligns Feature prose with Stage 0 rules (`IZ8-6`).
*   ***Proposed Edit:***
    *   Location: Lines `13`, `33`, `59`, `94`, `241-245`, `412-415`
    *   Change:
        1. Update lines `13`, `33`, `59`, `94` to `"required Image, Pieces, and Tabs layers"` and `"open <path> curves"`.
        2. Rename `"zero-span stray clicks"` to `"sub-0.05 mm stray paths"`.
        3. In `ingest.rs` (lines `241-245`, `415`): reject duplicate required `inkscape:label` layers or non-`<rect>`/`non-<image>`/`non-<path>` shapes (ignoring nested `<g>` transform groups or `<title>`/`<desc>` metadata); enforce rotated/skewed `<rect>` shear/rotation epsilon `<= 1e-4`; elevate `Quadratic` segments to cubic via `kurbo::QuadBez::raise()` (and convert `Arc` via `kurbo::Arc::to_cubic_beziers()`); filter stray paths (`aabb_diag < 0.05 mm && arc_len < 0.05 mm`) *before* rejecting non-trivial closed (`Z`) or multi-subpath (`> 1` `M`) tab elements; and verify clustered `<rect>` grid completeness (`cols >= 1 && rows >= 1`, count `== rows * cols`, contiguous non-overlapping cell coordinates).

### [Issue 17] `ACT-M10`: Partition `Detailed Implementation Plan` Into Incremental Buildable/Testable Phases [`egm-implementation-readiness, egm-codebase-details`] - Severity: Major
*   **Affected Scope:** [Lines 383-478](docs/designs/puzzle-gen.md#L383-L478) (`Detailed Implementation Plan`)
*   **Discrepancy Description:** Lists all 15 files linearly with `lib.rs` (item `2`) declaring submodules (`stage1`..`verify`) that are only created in items `3-12`, preventing `cargo check` or unit testing until nearly every file exists.
*   **Rationale:** Phasing implementation into incremental compile-and-test milestones aligns the spec with the target repository structure and makes verification incremental.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid implementation readiness gap in `IR9-6`.
*   ***Proposed Edit:***
    *   Location: `## Detailed Implementation Plan` (lines `383-478`)
    *   Change: Group the target file edits into three ordered buildable phases:
        *   **Phase 1 — Manifest, Config & Domain Model:** `src/puzzle-gen/Cargo.toml`, root `Cargo.lock`, `config.rs`, `model.rs`, plus partial `lib.rs` exports and Phase 1 unit tests (`saphyr` parsing, unit normalization, `CoordFrame`). Verify `cargo test -p puzzle-gen --lib`.
        *   **Phase 2 — Stage 0–2 Ingestion, Early Snapping & Buffering:** `ingest.rs`, `stage1.rs`, `stage2.rs`, plus `lib.rs` wiring and Phase 2 unit tests (`<rect>` clustering, `2.0 mm` endpoint-pair matching, 3-part splicing, `i_overlay` self-union + offsetting). Verify `cargo test -p puzzle-gen --lib`.
        *   **Phase 3 — Pagination, SVG Emission, CLI & Integration Suite:** `paginate.rs` (with built-in digits), `svg_emit.rs`, `lib.rs` `run_pipeline`, `main.rs`, `default-config.yaml`, `integration_pipeline.rs`. Verify full `cargo check --workspace`, `cargo clippy`, `cargo fmt --check`, and `cargo test`.

### [Issue 18] `ACT-M11`: Remove Unlinked Author Attribution in `## Alternatives Considered` [`egm-structural, egm-style`] - Severity: Major
*   **Affected Scope:** [Lines 290-351](docs/designs/puzzle-gen.md#L290-L351) (`Alternatives Considered`)
*   **Discrepancy Description:** Every `Reason for Rejection` bullet begins with `"Rejected by author:"` without an accompanying issue/PR/discussion link, violating `feature-spec-standard.md`, and most repeat the preceding `Cons` bullet.
*   **Rationale:** Unlinked author attribution adds clutter and obscures the technical trade-off that drove the rejection.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid structural and style defect (`CR7-10`).
*   ***Proposed Edit:***
    *   Location: Lines `296..350` (`**Reason for Rejection**` lines across Alternatives A–J)
    *   Change: Remove the `"Rejected by author:"` prefix on all alternatives and rewrite each `Reason for Rejection` to state the concrete scope/engineering trade-off without duplicating `Cons`.

### [Issue 19] `ACT-M12`: Fix Broken Heading Hierarchy After `## Non-Functional Requirements` [`egm-structural, egm-style`] - Severity: Major
*   **Affected Scope:** [Lines 354-381](docs/designs/puzzle-gen.md#L354-L381) (`Non-Functional Requirements`, `Additional Resources`, `Future Improvements`)
*   **Discrepancy Description:** `### Additional Resources` (line `370`) and `### Future Improvements` (line `375`) use H3 after a horizontal divider (`368`) without an H2 parent, nesting them accidentally under `## Non-Functional Requirements`.
*   **Rationale:** Broken heading nesting degrades document navigation and structure checks.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid structural defect (`CR7-12`).
*   ***Proposed Edit:***
    *   Location: Lines `370` and `375`
    *   Change: Promote `### Additional Resources` to `## Additional Resources` and `### Future Improvements` to `## Future Improvements`.

### [Issue 20] `ACT-M13`: Resolve `Diagnostic.severity` `Info` Schema Contradiction [`egm-style`] - Severity: Major
*   **Affected Scope:** [Lines 231, 245, 400](docs/designs/puzzle-gen.md#L230-L236) (`Diagnostic`, `Stage 0`, `diagnostics.rs`)
*   **Discrepancy Description:** `Diagnostic.severity` lists only `Warning | Error` (line `231`), while Stage 0 (line `245`) emits an `"informational diagnostic"` and `diagnostics.rs` (line `400`) defines `Info`, `Warning`, `Error`.
*   **Rationale:** Schema enum mismatch between FeatureRange and DetailedRange.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Eliminating `DiagnosticBag`/`StageId` per `ACT-M1` (`IZ8-1`) removes the redundant enum completely, leaving `Vec<Warning>` (or `Info`/`Warning` notes printed to `stderr`).
*   ***Proposed Edit:***
    *   Location: Lines `230-236`, `245`, `400`
    *   Change: Replace `DiagnosticBag`/`Diagnostic` with a minimal warning/notice type (`Severity::Info | Severity::Warning` printed to `stderr`, or a `PipelineNotice` struct on `PipelineResult`) so Stage 0 filtered stray-path counts and `--no-strict` missing-tab notices share one consistent schema across `FeatureRange` and `DetailedRange`.

### [Issue 21] `ACT-M14`: Calibrate White-Edge Bleed Guarantees and Curve Deviation / Vertex Claims [`egm-design, egm-style`] - Severity: Major
*   **Affected Scope:** [Lines 25, 63, 256, 331-332, 359](docs/designs/puzzle-gen.md#L256) (`The Problem`, `CUJ 1`, `Stage 2`, `Alternative G`, `Non-Functional Requirements`)
*   **Discrepancy Description:** Lines `25` and `63` state white stock `"never shows"`, which holds only up to `print_bleed - kerf / 2` registration offset (`0.09 mm` default). Lines `332` and `359` claim `"near 0.002mm"` sagitta error whereas line `256` specifies `segment_length_mm * 0.1` (`0.018 mm` at default `0.18 mm`). Line `331` makes an unverified `"nearly twice as many path vertices"` claim for `0.1mm`.
*   **Rationale:** Unqualified physical guarantees and mismatched tolerance math mislead readers and test authors.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid quantitative accuracy defect (`CR7-7`, `CR7-14`).
*   ***Proposed Edit:***
    *   Location: Lines `25`, `63`, `256`, `331-332`, `359`
    *   Change: Qualify lines `25` and `63` with `up to print_bleed - kerf / 2 manual alignment tolerance (0.09mm default)`; align lines `332` and `359` with the `<= segment_length * 0.1` (`0.018 mm` at default `0.18 mm`) flatness bound; and replace the unverified `"nearly twice as many"` claim in line `331` with factual proportionality to `1 / segment_length`.

### [Issue 22] `ACT-M15`: Fix CUJ 2 Arithmetic/Terminology, Pagination Notation Collisions, and Parameter/Flag Naming [`egm-style`] - Severity: Major
*   **Affected Scope:** [Lines 37, 67, 70, 78, 272-273, 285, 308, 420, 464, 469](docs/designs/puzzle-gen.md#L67-L70) (`Goals`, `CUJ 2`, `CUJ 3`, `Stage 3 & 4`, `Alternative C`, `stage1.rs`, `integration_pipeline.rs`, `Verification Plan`)
*   **Discrepancy Description:** CUJ 2 assumes `"print_bleed (0.24mm)"` from `1 kerf` without mentioning it in the override list, conflates `8.5in x 11in` sheet dimensions with printable area (`7.5in x 10in` inside `0.5in` margins), and uses unanchored `"thicker wood"`. Lines `37`, `78`, `464`, `469` write `cols * rows` instead of `page_cols * page_rows` (colliding with input puzzle `R x C`). Lines `37`, `285`, `308` write 1D `[margin, page_size - margin]` for 2D bounds. Line `464` writes `--overwrite=false/true` on a boolean CLI flag. Line `420` writes `(!no_strict)` for `strict: bool`.
*   **Rationale:** Reused variable names (`cols * rows`) and boolean flag syntax errors confuse implementers and CLI test cases.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid clarity and correctness cleanup (`CR7-17`, `CR7-18`, `IR9-13`).
*   ***Proposed Edit:***
    *   Location: Lines `37`, `67`, `70`, `78`, `272-273`, `285`, `308`, `420`, `464`, `469`
    *   Change: Specify exact CUJ 2 material thickness/config values and distinguish `8.5in x 11in` sheet size from `7.5in x 10in` usable area; use `page_cols * page_rows` consistently (per `ACT-M4`, `>= 1` once corner numbers use fixed corner strips); replace 1D `[margin, page_size - margin]` with explicit 2D margin rectangle bounds (`[margin, page_w - margin] x [margin, page_h - margin]`); write `without --overwrite` / `with --overwrite`; and fix line `420` to `strict == true` / `strict == false`.

### [Issue 23] `ACT-M16`: Enumerate CLI & Edge-Case Integration Tests, Separate Manual Inkscape Checks, and Calibrate Workspace Check Claims [`egm-codebase-details, egm-style`] - Severity: Major
*   **Affected Scope:** [Lines 461-472](docs/designs/puzzle-gen.md#L461-L472) (`integration_pipeline.rs`, `Verification Plan`)
*   **Discrepancy Description:** `integration_pipeline.rs` omits CLI binary invocation tests (`--dry-run`, `--output-dir`, `--prefix`, `--overwrite` error codes) and edge cases (`<sodipodi:namedview>` units, interior tab matching + `1.0 mm` / `2.0 mm` bounded early snapping, 4-way corner intersection rejection, closed stray clicks, rotated `<rect>`, rotated `<image>`, Stage 3 vs. Stage 4 `<path>` parity, `Quadratic`/`Arc` elevation, large `print_bleed`). Line `468` overclaims `cargo check --workspace` proves `src/sticker-size` `"remains unaffected"`; line `471` claims covering `"every"` branch and misphrases `"--overwrite vs. stale-page pruning"`; line `472` mixes manual Inkscape inspection into automated E2E without pass criteria.
*   **Rationale:** Incomplete integration coverage lets real CLI/SVG failures slip through, and uncalibrated verification claims violate technical rigor.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid test coverage and verification calibration gap (`IR9-12`, `IR9-14`).
*   ***Proposed Edit:***
    *   Location: Lines `464` and `468-472`
    *   Change: Add explicit CLI binary tests (using `tempfile::tempdir()`), Stage 0/1/2 edge cases (early snapping, 4-way corner rejection, self-union healing, `<sodipodi:namedview>` units, `saphyr` unknown-key/duplicate-key/unquoted-color rejection), and serialized Stage 3 vs. Stage 4 SVG coordinate parity checks (`<= 1e-4 mm`); calibrate line `468` to `"compiles cleanly"`; revise line `471` to `"--overwrite disabled vs. enabled (including stale-page pruning)"` listing required branches; and separate manual Inkscape PDF inspection into its own verification bullet with concrete pass criteria (page count, physical dimensions, unclipped bleed artwork, separated `Cut` and `PageNumbers` layers).

### [Issue 24] `ACT-M17`: Split Overlong Run-On Paragraphs in `Target Files to Create/Modify` [`egm-style`] - Severity: Major
*   **Affected Scope:** [Lines 385-464](docs/designs/puzzle-gen.md#L385-L464) (`Target Files to Create/Modify`)
*   **Discrepancy Description:** Eight `Proposed Changes` bullets (lines `390`, `405`, `415`, `420`, `425`, `435`, `454`, `464`) are 78–131 word semicolon-chained sentences obscuring precedence and error-handling flow.
*   **Rationale:** Dense single-paragraph module specs hide control-flow order and make requirements easy to miss during implementation.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid readability and clarity issue (`IR9-15`).
*   ***Proposed Edit:***
    *   Location: Lines `390`, `405`, `415`, `420`, `425`, `435`, `454`, `464`
    *   Change: Reformat each multi-responsibility `Proposed Changes` description into ordered sub-bullets matching `svg_emit.rs` (lines `446-449`).

### [Issue 25] `ACT-N1`: Document Empty Stub Baseline of `src/puzzle-gen` and Consolidate `src/sticker-size` WASM Caveats [`egm-comprehension, egm-zoom-out`] - Severity: Nit
*   **Affected Scope:** [Lines 11, 49, 109-111, 387, 390, 451, 468](docs/designs/puzzle-gen.md#L11) (`Overview`, `Non-Goals`, `Architecture Overview`, `Target Files`)
*   **Discrepancy Description:** Uses present tense for `puzzle-gen` and repeats `src/sticker-size` WASM caveats in four places without stating that `src/puzzle-gen` currently holds only a 3-line `"Hello, world!"` stub (`src/puzzle-gen/src/main.rs`) in a multi-crate workspace with no shared `[workspace.dependencies]`.
*   **Rationale:** Fresh readers lack baseline repository state and see duplicated caveats.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Adding one baseline sentence in `Overview` / `Architecture Overview` removes four scattered caveats cleanly.
*   ***Proposed Edit:***
    *   Location: Lines `11`, `109`, `390`, `468`
    *   Change: State once in `## Overview` and `### Architecture Overview` that `src/puzzle-gen` is currently an empty 3-line `"Hello, world!"` binary stub in the repository workspace alongside sibling crate `src/sticker-size`, and remove repeated WASM isolation notes on lines `49`, `390`, and `468`.

### [Issue 26] `ACT-N2`: Remove Stage 2 Slot Fields from `KerfedPiece` [`egm-zoom-out`] - Severity: Nit
*   **Affected Scope:** [Lines 204-212, 262, 266-267](docs/designs/puzzle-gen.md#L204-L212) (`KerfedPiece`, `Stage 2`, `Stage 3 & 4`)
*   **Discrepancy Description:** `KerfedPiece` stores Stage 2 debug grid fields (`stage2_offset_mm`, `stage2_aabb_mm`) computed from `cut_aabb` alongside cut/bleed polygons needed downstream.
*   **Rationale:** Computing Stage 2 debug grid offsets locally during Stage 2 emission (or reusing unified slot pitch) keeps `KerfedPiece` minimal.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Valid domain model simplification (`Z5-12`).
*   ***Proposed Edit:***
    *   Location: Lines `204-212`, `262`, `425`, `447`
    *   Change: Keep `KerfedPiece` focused on `(row, col, id, cut_polygon, bleed_polygon)` and compute the Stage 2 debug spacing offsets inside `emit_stage2` (or store a dedicated `stage2_layout` pitch on `KerfedGrid` if needed).

### [Issue 27] `ACT-N3`: Standardize Dividers, Blank Lines, List Formatting, and Stage 1/2 Sub-Bullets [`egm-structural, egm-style`] - Severity: Nit
*   **Affected Scope:** [Lines 3-288, 294-381](docs/designs/puzzle-gen.md#L253) (`Document Formatting`, `Stage 1`, `Stage 2`)
*   **Discrepancy Description:** Inconsistent horizontal divider usage and a double blank line (`79-80`); colons inside bold on lines `3-286` (`**Author:**`) vs. outside on lines `294-379` (`**Pros**:`); `1. ` vs. `1.  ` spacing; and dense 148–195 word single paragraphs in Stage 1 item `3` (`line 253`) and Stage 2 item `3` (`line 262`).
*   **Rationale:** Consistent Markdown formatting and broken-out Stage 1/2 sub-bullets improve scanability.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Low-cost formatting cleanup (`CR7-19`, `CR7-20`).
*   ***Proposed Edit:***
    *   Location: Across `docs/designs/puzzle-gen.md` (notably lines `7`, `79-80`, `253`, `262`, `294-379`)
    *   Change: Unify bold-colon style (`**Label:**`), single blank lines between sections, consistent ordered-list spacing, and split Stage 1 item `3` (`253`) and Stage 2 item `3` (`262`) into sub-bullets separating pattern emission from debug label emission.

### [Issue 28] `ACT-N4`: Link Example SVG in `## Non-Goals` and Correct `step0` `path19` Missing-Edge Note [`egm-structural, egm-design, egm-style`] - Severity: Nit
*   **Affected Scope:** [Lines 46, 372-373](docs/designs/puzzle-gen.md#L372-L373) (`Non-Goals`, `Additional Resources`)
*   **Discrepancy Description:** Line `46` mentions `rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg` without a clickable link and frames it negatively vs. line `373`; line `372` implies `path19` is the omitted edge id without noting the non-monotonic Inkscape numbering (`path1`–`path40` minus `path19`) and that the missing edge is actually `(2, 0)-(3, 0)` at `y = 2.4in, x in [0, 0.8in]`.
*   **Rationale:** Broken or inconsistent example cross-references confuse contributors inspecting reference files.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Aligns with verified XML inspection of `step0` and `CR7-21`.
*   ***Proposed Edit:***
    *   Location: Lines `46` and `372`
    *   Change: Link `[rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg](../../src/puzzle-gen/examples/rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg)` on line `46` consistently with line `373` (noting partial/duplicate manual joins as an incomplete layout PoC), and clarify on line `372` that `step0` contains 39 of 40 internal edges (`path1`–`path40` with id `path19` absent, leaving internal edge `(2, 0)-(3, 0)` along `y = 2.4in, x in [0, 0.8in]` un-tabbed).

### [Issue 29] `ACT-N5`: Copy-Edit Hygiene Across `FeatureRange` (`"clean Stage 0"`, Subjectivity, Process Chatter, Redundant NFRs) [`egm-style, egm-zoom-out`] - Severity: Nit
*   **Affected Scope:** [Lines 13, 22-29, 33-85, 109, 121, 125, 131, 149, 152, 243, 260, 338, 354-366](docs/designs/puzzle-gen.md#L22-L29) (`FeatureRange`)
*   **Discrepancy Description:** `"clean Stage 0"` conflicts with float-noise and stray-path cleanup; `## The Problem` mixes opinion and solutions; `## Open Questions` includes drafting-process chatter; promotional/filler modifiers appear (`"without Bloat"`, `"high-resolution"`, `"actionable error"`, `"failing loudly"`, `"completely"`, `"well below"`); `--overwrite` flag summary omits its behavior; and `## Non-Functional Requirements` repeats functional pipeline steps.
*   **Rationale:** Factual, non-promotional technical writing avoids ambiguity and repetition.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Enforces concise plain-English doc standards across `FeatureRange` (`CR7-22`, `IZ8-4`).
*   ***Proposed Edit:***
    *   Location: Lines `13`, `22-29`, `33-85`, `121`, `125`, `131`, `152`, `243`, `260`, `338`, `354-366`
    *   Change: Replace `"clean Stage 0"` with `"Stage 0"` / `"unjoined Stage 0"`; trim `## Open Questions` to `"None."`; explain `--overwrite` behavior on line `121`; remove filler/intensifier phrasing; and deduplicate `## Non-Functional Requirements` so it keeps only cross-cutting precision/safety/performance constraints not already repeated in the pipeline section.

### [Issue 30] `ACT-N6`: Standardize Minor Notation Across `FeatureRange` [`egm-style`] - Severity: Nit
*   **Affected Scope:** [Lines 17, 35-36, 153, 159, 244, 253, 275, 320](docs/designs/puzzle-gen.md#L153) (`FeatureRange`)
*   **Discrepancy Description:** Mixes `"full kerf"` vs `"1 kerf"`, `"opposite corners"` vs `"top-left and bottom-right"`, `"at most once"` vs `"exactly once"`, `"all print stages"` vs `"Stages 1-3"`, open-ended `e.g.`/`etc.` on the closed 4-value `LengthUnit` enum, unitless `1e-8` shifts, and `pageP` vs `pageN`.
*   **Rationale:** Uniform terminology prevents minor misreadings across sections.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Straightforward terminology alignment (`CR7-23`).
*   ***Proposed Edit:***
    *   Location: Lines `17`, `35-36`, `153`, `159`, `244`, `253`, `275`, `320`
    *   Change: Standardize on `"1 kerf"`, `"top-left and bottom-right corners"`, `"exactly once"`, `"Stages 1-3"`, explicit `(mm, cm, in, px)` enumeration, `"1e-8 in shifts"`, and consistent `pageN` / `N` placeholder notation.

### [Issue 31] `ACT-N7`: Clarify Minor CLI Filesystem Guards in `main.rs` [`egm-implementation-readiness, egm-codebase-details`] - Severity: Nit
*   **Affected Scope:** [Lines 420, 454](docs/designs/puzzle-gen.md#L454) (`stage1.rs`, `main.rs`)
*   **Discrepancy Description:** Line `454` checks pre-existing `<prefix>-stage*` collisions inside `--output-dir` before `create_dir_all` without mentioning `ErrorKind::NotFound` handling when `--output-dir` does not yet exist, temp-file naming rules avoiding collisions with `<prefix>-stage4-page*` checks, and reading `--config` YAML inside `main.rs` before `Config::from_yaml`.
*   **Rationale:** Clarifying those three filesystem guards avoids panics on first run and self-colliding temp files.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Worth resolving directly alongside `main.rs` edits (`IR9-16`).
*   ***Proposed Edit:***
    *   Location: Line `454` (`main.rs`)
    *   Change: Specify reading `--config` (if passed) before `Config::from_yaml`; treating `ErrorKind::NotFound` on missing `--output-dir` as zero existing collisions prior to `create_dir_all`; and naming temp files `.<prefix>.tmp-<pid>-...` (not matching `<prefix>-stage4-page[1-9][0-9]*.svg`) before renaming into place.

### [Issue 32] `ACT-N8`: Copy-Edit Hygiene Across `DetailedRange` and Define `EGM` Footer Acronym [`egm-comprehension, egm-style`] - Severity: Nit
*   **Affected Scope:** [Lines 390-478](docs/designs/puzzle-gen.md#L476-L478) (`DetailedRange`, footer)
*   **Discrepancy Description:** `DetailedRange` contains subjective/jargon phrasing (`"top-level orchestration API"`, `"without touching the filesystem"`, `"actionable"`, `"clears margins cleanly"`, `"for CUJ 2 discovery"`), empty modifiers (`"strictly"`, `"exact"`, `"completely"`, `"visible stroked"`), shorthand redundancies (`"bare unitless"`, `"single 2-point lines"`), and undefined `"EGM"` acronym on lines `476` and `478`.
*   **Rationale:** Cleaning prose and spelling out `"Elephant-Goldfish Model (EGM)"` (or dropping the internal provenance comment) improves clarity for readers unfamiliar with the review tool.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Simple editorial pass (`C6-9`, `IR9-17`).
*   ***Proposed Edit:***
    *   Location: Lines `390-478`
    *   Change: Replace promotional/redundant phrasing with concise factual descriptions and either spell out `Elephant-Goldfish Model (EGM)` on lines `476`/`478` or remove the HTML provenance comment.
