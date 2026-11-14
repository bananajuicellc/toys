# Goldfish Review Report (Stage 7: Critic Review)

**Target:** `docs/designs/puzzle-gen.md` (`Lines 1-382`)

## Reviewer Status

| Reviewer | Status |
| --- | --- |
| `egm-design` | completed |
| `egm-structural` | completed |
| `egm-style` | completed |

## Prioritized Findings (Blockers -> Major -> Nits)

### 1. Missing Dedicated `Risks & Limits` / `Risks & Mitigations` Section - Severity: Blocker
* **Reviewer Lens:** `egm-design`, `egm-structural`
* **Affected Scope:** Lines `1-382` (entire `FeatureRange`; specifically relative to `## Alternatives Considered` at lines `290-351` and `## Non-Functional Requirements` at lines `354-367`)
* **Discrepancy Description:** `feature-spec-standard.md` requires a dedicated `Risks & Limits` or `Risks & Mitigations` section documenting constraints, failure modes, security/safety implications, scalability limits, and mitigations. While `## Non-Functional Requirements` (lines `354-367`) mentions `--overwrite=false`, `--no-strict`, and pre-write assertions, it does not provide a risk-and-mitigation analysis and omits security bounds (untrusted SVG/YAML parsing, embedded raster memory limits, path/glob safety), geometric failure modes (self-intersecting tabs, neck collapse, `0mm` edge cases), and scalability limits (max grid size, page counts, `i_overlay` integer coordinate bounds at `1000` units/mm, laser CAM compatibility).
* **Rationale & Recommendation:** Add a top-level `## Risks & Mitigations` (or `## Risks & Limits`) section in `FeatureRange` detailing failure modes, security/safety limits, scalability ceilings, and concrete mitigations.

### 2. Double-Shifted `<image>` + `<pattern>` Coordinate Bug and Unhandled `<image>` Transforms - Severity: Blocker
* **Reviewer Lens:** `egm-design`, `egm-style`
* **Affected Scope:** Lines `192`, `242-243`, `253` (`Stage 1: Tab Splicing & 2x2 Mirrored Pattern`), `262` (`Stage 2`), `276` (`Stage 3`)
* **Discrepancy Description:** Line `253` specifies emitting `<image>` inside `<defs>` at transformed `(img_x, img_y)` with size `(W, H)`, AND setting `x="img_x" y="img_y"` on `<pattern id="puzzle-pattern" patternUnits="userSpaceOnUse" ... width="2W" height="2H">`, referenced by four `<use>` nodes (`normal at (0, 0)`, `translate(2W, 0) scale(-1, 1) at (W, 0)`, etc.). In SVG, `<use>` renders `<image>` inside the `<pattern>` coordinate space (which already starts at `(img_x, img_y)`), shifting the normal tile to `(2*img_x, 2*img_y)` and mirroring `[img_x, img_x + W]` into `[W - img_x, 2W - img_x]` whenever `img_x != 0` or `img_y != 0` (such as when `Image` has a layer transform in `no-kerf-manual-attempt` line `85`). Furthermore, `at (W, 0)` vs. `transform="translate(2W, 0) scale(-1, 1)"` is syntactically ambiguous, `image_bounds_mm: Rect` (line `192`) discards rotation/skew/negative-scale `<image>` transforms composed in line `242`, and `preserveAspectRatio` (e.g., `none` in `step0` line `41`) is not preserved.
* **Rationale & Recommendation:** Emit `<image>` inside `<defs>` at `x="0" y="0" width="W" height="H"` preserving `preserveAspectRatio`, apply `x="img_x" y="img_y"` solely on `<pattern>`, separate `<use>` attributes from quadrant bounds prose, reject rotated/skewed/flipped `<image>` transforms during Stage 0 ingest (or preserve residual transforms), and validate `Image` coverage against `Pieces`.

### 3. Unhealed Self-Intersecting Tab-to-Edge Splices in Stage 1/2 - Severity: Blocker
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines `248-252` (`Stage 1: Tab Splicing`), `256-261` (`Stage 2: Outward Polygon Offsetting`), `372` (`Additional Resources`)
* **Discrepancy Description:** Stage 1 splices `Tabs` curves directly into straight grid edges without self-intersection healing, and Stage 2 requires every `i_overlay` output ring to be "a single non-self-intersecting closed polygon with positive area larger than the un-offset piece" (line `261`). Direct inspection of `rainy-day-puzzle-4x4-25piece-step0-piece-shape.svg` reveals multiple Inkscape hand-drawn tabs that pinch or cross their adjacent straight edge: `path1` (line `205`) doubles back onto vertical edge `x = 0.8`, `path2` (line `208`) crosses horizontal edge `y = 0.8` at `t in [0.15, 0.20]`, and `path10` (line `234`) crosses vertical edge `x = 0.8` near `t ≈ 0.14`. Feeding self-intersecting rings directly into polygon offsetting can produce inverted loops or holes, failing valid Inkscape inputs (including the `step0` reference file under `--no-strict`).
* **Rationale & Recommendation:** Add pre-offset topology healing in Stage 2 (or Stage 1): after flattening to integer micrometer coordinates, resolve self-intersections via `i_overlay` (`OverlayRule::Subject` with explicit `FillRule`) before computing un-offset piece area and outward buffering, or trim tiny boundary-crossing loops during splicing.

### 4. Tab Endpoint Snapping Onto "Clustered Grid Vertices" vs. Interior Edge Lines - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-style`
* **Affected Scope:** Lines `248-252` (`Stage 1: Tab-to-Edge Matching & Clockwise Contour Assembly`)
* **Discrepancy Description:** Line `249` states: `"snap exact endpoints onto the clustered grid vertices so adjacent corners meet at identical coordinates"`, contradicting line `252` and actual Stage 0 SVGs (`step0` `path1`, line `205`: `(0.80000027, 0.3202571)` to `~ (0.800, 0.453)`), where tabs attach to the *interior* of shared internal edge segments rather than corner vertices. In addition, line `250` does not specify edge-pair disambiguation when a valid tab endpoint lies within the `0.5 mm` snap tolerance of both an internal edge and an outer puzzle border or 4-way grid intersection.
* **Rationale & Recommendation:** Specify projecting/snapping tab endpoints onto the matched internal edge's **clustered grid line** strictly inside `(edge_start, edge_end)` so adjacent pieces connect corner vertices to identical tab endpoints. Specify pair-matching rules across candidate edges (both endpoints within `0.5 mm` of the same edge, non-zero span along the edge, proximity tie-breaking, and rejecting only when the matched edge is an outer border).

### 5. Undefined `<stroke_w>`, Omitted Stage 2 Styling, and Inconsistent `score_color` Scope - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-style`
* **Affected Scope:** Lines `38`, `150` (`score_color`), `253` (`Stage 1`), `262` (`Stage 2`), `277`, `281-282` (`Stage 3 & Stage 4`)
* **Discrepancy Description:** Stage 3 (`stroke-width: <stroke_w>`, line `277`) and Stage 4 (`stroke-width: <stroke_w>`, lines `281-282`) reference `<stroke_w>`, which is never defined in the YAML schema, `Config`, or pipeline text. Stage 1 pieces and Stage 1/2 `GridLabels` specify `stroke` colors without `stroke-width` (lines `253`, `262`), and Stage 2 pieces omit `fill`, `stroke`, and `stroke-width` completely (line `262`). Furthermore, line `150` restricts `score_color` to Stage 4 `PageNumbers` even though lines `253` and `262` apply `<score_color>` to Stage 1/2 `GridLabels`, while line `38` writes `"a configurable PageNumbers layer/color"` when only `score_color` (not the layer name) is configurable.
* **Rationale & Recommendation:** Define `<stroke_w>` explicitly in canonical `mm` (projected to target document units), specify Stage 2 piece styling and Stage 1/2 stroke widths, specify `GridLabels` gutter/digit sizing rules for multi-digit `>= 10x10` grids, reword line `38` to `"a dedicated PageNumbers layer styled with score_color"`, and update line `150` to include Stage 1/2 `GridLabels`.

### 6. Missing Stage 2 & Stage 3/4 Slot Placement Rules, `slot_w` Shadowing, and Stage 3 `cut_polygon` Misreference - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-style`
* **Affected Scope:** Lines `210-216` (`KerfedPiece` / `PageSlot`), `262` (`Stage 2`), `266-277`, `286` (`Stage 3 & Stage 4`)
* **Discrepancy Description:** Lines `210`, `215-216`, `262`, and `276` omit how each piece is placed inside its Stage 2 and Stage 3/4 `slot_w x slot_h` cell (e.g., centering `cut_aabb.center()`, aligning `min_x/min_y`, and whether the page slot grid is anchored at `(margin_mm, margin_mm)` or centered in `usable_w x usable_h`). Naive placement without aligning `cut_aabb` clips asymmetric protruding tabs or breaks Stage 3/4 slot parity. In addition, line `262` defines `slot_w = max(cut_aabb.width)` and `slot_h = max(cut_aabb.height)`, which is shadowed on lines `266-267` by `max(cut_aabb, bleed_aabb)`, and line `286` verifies `"Stage 3 piece cut_polygon slot centers"` even though Stage 3 emits `bleed_polygon` (line `276`), not `cut_polygon`.
* **Rationale & Recommendation:** Specify exact translation formulas (`stage2_offset_mm`, `page_local_center_mm`, and `piece_offset_mm = slot_center_mm - cut_aabb.center()`), clarify slot-grid origin at `(margin_mm, margin_mm)`, rename Stage 2's variables (`stage2_slot_w`, `stage2_slot_h`), and update line `286` to state `"Stage 3 piece placement offsets (from cut_polygon centers in PaginatedLayout)"`.

### 7. Missing Parameter Bounds, Zero-Offset Failures, Flawed Simplification Claim, and Verification Tolerances - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-style`
* **Affected Scope:** Lines `145-150` (`Dimension Grammar`), `256-261` (`Stage 2`), `285` (`Pre-Write Verification`), `331-332` (`Alternative G`), `359` (`NFRs`)
* **Discrepancy Description:**
  * Configured dimensions lack lower/upper bounds: `kerf: "0mm"` with default `segment_length: "1 kerf"` yields `segment_length_mm == 0.0`, causing infinite subdivision (line `256`).
  * Line `261` requires post-offset area to exceed un-offset area, which fails when `kerf == 0` or `print_bleed == 0`.
  * Line `260` claims `0.005 mm` collinear simplification stays `"well below kerf / 10"` (false when `kerf < 0.05 mm` or `segment_length < 0.05 mm`).
  * Lines `332` and `359` claim tight-curve sagitta deviation stays `"near 0.002mm"`, contradicting line `256`'s `segment_length_mm * 0.1` (`0.018mm` at `0.18mm` default); line `331` also asserts `"nearly twice as many path vertices"` at `0.1mm` without empirical evidence.
  * Line `285` specifies exact margin and `>= piece_spacing_mm` assertions without accounting for `0.001 mm` micrometer integer rounding (line `257`), and `1000` units/mm lacks an input coordinate ceiling against integer overflow.
* **Rationale & Recommendation:** Define explicit bounds (`page_size > 0`, `segment_length > 0`, `kerf/margin/piece_spacing/print_bleed >= 0`, max coordinate ceiling), allow `area >= unoffset_area` at zero offset (warning if `print_bleed < kerf / 2`), bind simplification error to `min(0.005 mm, segment_length_mm * 0.05)` when positive, calibrate sagitta/vertex claims to the specified `segment_length_mm * 0.1` bound, and include explicit micrometer tolerances in line `285`.

### 8. `CoordFrame` Unit/Aspect Semantics and Missing Stage 0 Validation Rules - Severity: Major
* **Reviewer Lens:** `egm-design`
* **Affected Scope:** Lines `151-153` (`CoordFrame Unit Resolution`), `241-245` (`Stage 0: Input SVG Ingestion`), `253`, `262`, `275` (`Stages 1-3` projection)
* **Discrepancy Description:**
  * Line `152` implies `inkscape:document-units` acts as a unit hint when `width`/`height` lack physical units, conflicting with SVG semantics where unitless `width`/`height` are `96 DPI px` while `inkscape:document-units` is a UI ruler preference. It also omits `viewBox` vs. `width`/`height` aspect-ratio tolerance, missing-attribute handling, `viewBox` `(min_x, min_y)` origin storage, and explicit normalization of Stages 1–3 to `1 user unit = 1 doc_unit` (required for `<inkscape:page width="page_w">` on line `275`).
  * Lines `241-245` do not state Stage 0 behavior for duplicate `inkscape:label` layers (present in `no-kerf-manual-attempt` lines `243` and `374`), rotated/skewed `<rect>` elements, unexpected non-`<rect>`/`non-<path>` child elements, or closed (`Z`) / multi-subpath non-stray `<path>` elements (and whether stray filtering precedes `Z` rejection, given `path4` in `no-kerf-manual-attempt` line `255` ends with `z`).
* **Rationale & Recommendation:** Specify exact `CoordFrame` precedence (unitless `width`/`height` = `px`; `doc_unit` governs output projection; Stages 1–3 normalize `1 user unit = 1 doc_unit`), define aspect-ratio tolerance and `viewBox` origin handling, and document duplicate-layer, rotated-`<rect>`, foreign-child, and `Z`/multi-subpath validation rules and ordering.

### 9. Unsafe Stage 4 Globbing, Unvalidated `--prefix`, and Non-Atomic Batch Deletion - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-style`
* **Affected Scope:** Lines `40` (`Goals`), `114-127` (`CLI Interface & File Lifecycle`), `365-366` (`Non-Functional Requirements`)
* **Discrepancy Description:**
  * Matching `<prefix>-stage4-page*.svg` (lines `40`, `126`, `127`) matches arbitrary filenames such as `<prefix>-stage4-page-notes.svg` or `<prefix>-stage4-page1-backup.svg`, causing false collision blocks or silent data loss under `--overwrite`.
  * `--prefix` is not validated against path separators (`/`, `\`, `..`), allowing writes/deletions outside `--output-dir`.
  * Line `127` deletes stale Stage 4 files *before* writing new Stage 1–4 files, while lines `40` and `366` overclaim this as atomic pruning / atomic multi-file writes. An I/O error mid-write leaves the output directory torn.
* **Rationale & Recommendation:** Restrict matching strictly to `^<prefix>-stage4-page[1-9][0-9]*\.svg$`, validate `--prefix` as a single path-free filename segment, write/rename all new files before pruning stale page numbers `> new_page_count`, and narrow lines `40` and `366` to describe per-file temp-file replacement accurately.

### 10. Attribution Without Links in `## Alternatives Considered` - Severity: Major
* **Reviewer Lens:** `egm-structural`, `egm-style`
* **Affected Scope:** Lines `290-351` (`## Alternatives Considered`, specifically lines `296`, `302`, `308`, `314`, `320`, `326`, `332`, `338`, `344`, `350`)
* **Discrepancy Description:** All 10 `Reason for Rejection` bullets open with `"Rejected by author:"` without linking an issue, discussion, or PR, violating `feature-spec-standard.md` (`## The Alternatives Rule`, lines `19-21`). Most also duplicate the preceding `Cons` bullet.
* **Rationale & Recommendation:** Remove `"Rejected by author:"` (or link the decision record/issue/PR) and lead directly with the concise technical or scope trade-off without duplicating `Cons`.

### 11. Full-Slot Corner Page Numbers Lack Alternative Evaluation, Multi-Digit Layout, and Registration Fiducials - Severity: Major
* **Reviewer Lens:** `egm-design`, `egm-style`
* **Affected Scope:** Lines `27`, `38`, `63`, `272-273`, `277`, `282`, `290-351` (`Alternatives Considered`)
* **Discrepancy Description:** Reserving two full piece-sized `slot_w x slot_h` grid cells per page for vector digits is neither compared against compact margin fiducials/labels in `## Alternatives Considered` nor specified for multi-digit page numbers (`>= 10`: glyph aspect ratio, inter-digit gap, per-side vs. total `10%` padding). Additionally, curved single-stroke digits (`0`, `3`, `6`, `8`, `9`) provide no sharp point target for manual laser crosshair alignment, and reserving two full slots blocks sheets that fit 2 pieces (`page_cols * page_rows < 3`).
* **Rationale & Recommendation:** Add an alternative comparing full-slot page numbers vs. compact margin fiducials, specify multi-digit glyph proportions and padding semantics in line `273`, and add explicit corner crosshairs/alignment ticks inside corner anchors if used for laser registration.

### 12. Heading Hierarchy Violation Under `## Non-Functional Requirements` - Severity: Major
* **Reviewer Lens:** `egm-structural`, `egm-style`
* **Affected Scope:** Lines `354-381` (`## Non-Functional Requirements`, line `368`, lines `370-373` `### Additional Resources`, lines `375-379` `### Future Improvements`)
* **Discrepancy Description:** Line `368` places a horizontal rule after `## Non-Functional Requirements`, followed by H3 headings `### Additional Resources` (line `370`) and `### Future Improvements` (line `375`) with no intervening H2 heading, nesting reference SVGs and future v2 features under `## Non-Functional Requirements`.
* **Rationale & Recommendation:** Promote `Additional Resources` and `Future Improvements` to H2 (`## Additional Resources`, `## Future Improvements`) or insert an explicit H2 parent section after line `368`.

### 13. `Diagnostic.severity` Schema Contradiction - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `231` (`Diagnostic`) and `245` (`Stage 0: Input SVG Ingestion`)
* **Discrepancy Description:** `Diagnostic.severity` (line `231`) allows only `Warning | Error`, whereas line `245` requires `"emitting an informational diagnostic with element id"` when filtering stray clicks.
* **Rationale & Recommendation:** Add `Info` to `Diagnostic.severity` on line `231` or change line `245` to emit `Warning` (or a non-diagnostic log line).

### 14. Uncalibrated White-Edge Bleed Guarantee - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `25` (`## The Problem`) and `63` (`### CUJ 1`)
* **Discrepancy Description:** Lines `25` (`"so white backing paper never shows"`) and `63` (`"without white edge slivers"`) make unconditional guarantees even though default `print_bleed` (`0.18mm`) minus `kerf / 2` (`0.09mm`) covers only `0.09mm` of lateral print/cut misalignment.
* **Rationale & Recommendation:** Calibrate both statements to state the verifiable tolerance (`"within the print_bleed - kerf / 2 alignment margin"`).

### 15. Input Layer Count and Tab Path Type Contradictions - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `13` (`## Overview`), `33` (`## Goals`), `59` (`### CUJ 1`), `94` (`### Architecture Overview`), `241`, `245` (`Stage 0`)
* **Discrepancy Description:** Lines `13`, `33`, `59`, and `94` describe Stage 0 SVGs as `"containing three layers"` and `Tabs` solely as `"cubic-Bezier <path>"` elements, whereas line `241` permits and ignores extra helper/empty layers and line `245` parses general SVG path commands (`M/L/H/V/C/S/Q/A/Z`).
* **Rationale & Recommendation:** Update lines `13`, `33`, `59`, and `94` to state `"containing required Image, Pieces, and Tabs layers"` and `"open <path> curves"`.

### 16. Missing Architecture Types in Core Domain Data Structures & Undecided YAML Crate - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `53` (`## Open Questions`), `97`, `101`, `111` (`### Architecture Overview`), `157-236` (`### Core Domain Data Structures`)
* **Discrepancy Description:** The architecture flow references `SplicedPuzzle` (line `97`) and `KerfedGrid` (line `101`), neither of which is defined in `Core Domain Data Structures` (lines `157-236`). Line `111` also leaves the YAML crate unresolved (`"serde_norway (or serde_yaml_ng)"`) despite line `53` claiming all design questions are resolved.
* **Rationale & Recommendation:** Define `SplicedPuzzle` and `KerfedGrid` in lines `198-211` (or reference `Vec<SplicedPiece>` / `Vec<KerfedPiece>` in the diagram) and commit to one YAML crate on line `111`.

### 17. CUJ 2 Arithmetic/Terminology Mismatches - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `67`, `70` (`### CUJ 2`)
* **Discrepancy Description:** Line `67` lists `print_bleed` among overridden config fields without specifying a value, yet line `70` assumes `print_bleed` resolves to `0.24mm` (`1 kerf`). Line `70` also refers to `"the 8.5in x 11in printable area"`, conflicting with `usable_w = page_width_mm - 2 * margin_mm` (`7.5in x 10in` after `0.5in` margins), and line `67` uses the unanchored comparative `"thicker wood"`.
* **Rationale & Recommendation:** State explicit `config.yaml` values (`print_bleed: "1 kerf"`), distinguish `"8.5in x 11in page size (7.5in x 10in printable area)"`, and replace `"thicker wood"` with `"plywood"` or `"wood stock"`.

### 18. Pagination Notation Collisions and Ambiguous Error/Padding Terms - Severity: Major
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `16`, `37`, `78`, `272-273`, `285`, `308`
* **Discrepancy Description:** Lines `37` and `78` write `cols * rows` (the puzzle grid dimensions) instead of `page_cols * page_rows`; line `37` implies per-piece AABB packing rather than uniform `max` slot dimensions (lines `266-267`); lines `37`, `285`, and `308` use 1D `[margin, page_size - margin]` for 2D bounds; line `272` asks for a non-unique 2D `"minimum page_size needed to fit 3 slots"`; and line `273` leaves `"10% inner padding"` ambiguous between per-side and total padding.
* **Rationale & Recommendation:** Use `page_cols * page_rows`, clarify uniform `max` slot sizing, use 2D bounds `[margin_mm, page_width_mm - margin_mm] x [margin_mm, page_height_mm - margin_mm]`, define how minimum `page_size` is computed, and state exact per-side padding percentage.

### 19. Asymmetric Dividers and Whitespace - Severity: Nit
* **Reviewer Lens:** `egm-structural`
* **Affected Scope:** Lines `7`, `19-81`, `288`, `352`, `368`, `381`
* **Discrepancy Description:** Horizontal rules appear inconsistently across H2 sections (present at lines `7`, `288`, `352`, `368`, `381`; absent before lines `20`, `31`, `42`, `51`, `55`, `81`), and lines `79-80` have two consecutive blank lines before `## Technical Plan (Architecture)`.
* **Rationale & Recommendation:** Standardize horizontal rule usage across H2 boundaries and collapse the extra blank line at lines `79-80`.

### 20. Markdown List Formatting and Dense Stage 1/2 Paragraphs - Severity: Nit
* **Reviewer Lens:** `egm-structural`, `egm-style`
* **Affected Scope:** Lines `3-286` vs. `294-379`, lines `15-18` vs. `241-286`, lines `253`, `262`
* **Discrepancy Description:** Bold-label colons sit inside bold formatting in lines `3-286` (`**Author:**`) and outside bold formatting in lines `294-379` (`**Pros**:`); ordered lists use one space (`1. `) in Overview vs. two spaces (`1.  `) in the pipeline section; and Stage 1 item `3.` (line `253`, 195 words) and Stage 2 item `3.` (line `262`, 148 words) are dense single paragraphs unlike Stage 3/4's structured sub-bullets.
* **Rationale & Recommendation:** Unify colon placement and ordered-list spacing, and split Stage 1 item `3.` and Stage 2 item `3.` into structured sub-bullets (`Pattern Definition`, `Piece Rendering`, `GridLabels Gutter & ViewBox`).

### 21. Unlinked Example Reference and Inaccurate `path19` Description - Severity: Nit
* **Reviewer Lens:** `egm-structural`, `egm-design`, `egm-style`
* **Affected Scope:** Lines `46` (`## Non-Goals`) and `372-373` (`### Additional Resources`)
* **Discrepancy Description:** Line `46` names `rainy-day-puzzle-4x4-25piece-no-kerf-manual-attempt-at-joining-paths.svg` as bare inline code without linking it, and frames it purely as `"broken manual path edits"` vs. line `373`'s positive framing. Additionally, line `372` describes `step0` as having `"39 tab <path> elements with path19 omitted"`, obscuring that ID numbering around `y = 1.6..2.4` is non-monotonic and the missing internal edge is actually horizontal edge `(2, 0)-(3, 0)` along `y = 2.4in, x in [0, 0.8in]`.
* **Rationale & Recommendation:** Link the file on line `46` consistently with line `373` (noting both its multi-page layout PoC and incomplete/duplicate paths), and clarify on line `372` that `path19` is skipped in ID numbering while the missing internal edge is `(2, 0)-(3, 0)` at `y = 2.4in, x in [0, 0.8in]`.

### 22. Style Hygiene: Filler, Subjective Framing, Process Chatter, and Redundant NFRs - Severity: Nit
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `13`, `22-29`, `33-85`, `109`, `121`, `125`, `131`, `149`, `152`, `243`, `260`, `338`, `354-366`
* **Discrepancy Description:**
  * Repeated `"clean Stage 0"` contradicts Stage 0's handling of float noise and stray clicks (lines `13`, `29`, `33`, `59`).
  * `## The Problem` includes subjective claims (`"tedious"`, `"is fast in Inkscape"`) and mixes solution steps into problem item 4 (lines `22-27`).
  * `## Open Questions` includes drafting-process chatter (line `53`).
  * Filler/buzzwords/colloquialisms appear throughout (`"without Bloat"`, `"high-resolution"`, `"concise"`, `"actionable error"`, `"strict split"`, `"thin CLI/filesystem wrapper"`, `"focused"`, `"Before touching disk"`, `"validates strictly"`, `"resolved dynamically"`, `"failing loudly"`, `"well below kerf / 10"`, `"completely"`).
  * `## Non-Functional Requirements` (lines `354-366`) repeats functional pipeline mechanics without measurable latency, memory, or file-size targets; `--overwrite` flag summary (line `121`) omits its purpose; `src/sticker-size` (lines `49`, `109`) lacks context for fresh readers.
* **Rationale & Recommendation:** Replace subjective/colloquial phrasing with objective specifications, shorten `## Open Questions` to `"None."`, document `--overwrite`'s purpose in the flag list, briefly identify `src/sticker-size`, and either add measurable NFR targets or consolidate duplicated functional rules.

### 23. Minor Notation and Unit-List Inconsistencies - Severity: Nit
* **Reviewer Lens:** `egm-style`
* **Affected Scope:** Lines `17`, `35-36`, `153`, `159`, `244`, `253`, `275`, `320`
* **Discrepancy Description:** Uses `"full kerf"` alongside `"1 kerf"` (lines `17`, `36`, `320`), `"opposite corners"` alongside `"top-left and bottom-right"` (lines `17`, `18`), `"at most once"` vs. `"exactly once"` for `<image>` (lines `35`, `253`), `"all print stages"` for Stages 1–3 (line `153`), open-ended `etc.` / `e.g.` for the closed 4-value `LengthUnit` enum (lines `153`, `159`), unitless `1e-8` shifts (line `244`), and `pageP` vs. `pageN` (lines `275`, `18`).
* **Rationale & Recommendation:** Standardize on `"1 kerf"`, `"top-left and bottom-right corners"`, `"exactly once"`, `"Stages 1-3"`, the explicit 4-unit list, `1e-8 in` shifts, and consistent page placeholder notation.
