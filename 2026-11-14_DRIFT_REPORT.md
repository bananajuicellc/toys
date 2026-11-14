# Drift Report: 2026-11-14

## docs/designs/puzzle-gen.md

* **Superceded Pagination Architecture:** The document specifies Stage 3 and Stage 4 layout using uniform rectangular grid pagination (`compute_slot_bounds` worst-case bounding-box grid) reserving two full-width 6.0 mm horizontal corner bands (`paginate_pieces` reserving `usable_h = page_height_mm - 2.0 * margin_mm - 12.0 mm`, with `page_cols * page_rows >= 1` slot layout). In code, this uniform pagination has been superseded by irregular 2D polygon bin-packing across right-angle rotations (`0°`, `90°`, `180°`, `270°`) and localized corner registration keep-out boxes implemented in `src/puzzle-gen/src/nest.rs` and `paginate_pieces_with_options` in `src/puzzle-gen/src/paginate.rs` (as specified in `docs/designs/puzzle-gen-nesting.md`).
* **Default Page Size Discrepancy:** The design document states default `page_size: 6in x 6in` in CUJ 1 and technical plan (e.g. line 62, 498). In code (`Config::default()` in `src/puzzle-gen/src/model.rs` and `src/puzzle-gen/examples/default-config.yaml`), the default page size is `8.5in x 11in`.
* **CLI Output Metrics:** The design document specifies CLI summary metrics printing `slot_mm=`, `pages=`, and `page_grid=CxR`. In code (`src/puzzle-gen/src/main.rs`), `page_grid=` has been replaced by `puzzle_area_mm2=`, `total_bbox_mm2=`, and `bbox_area_pct=`.
* **Registration Mark Positioning:** The design document specifies fixed bottom-right registration mark placement at `(page_width_mm - margin_mm, page_height_mm - margin_mm)` on every sheet. In code, the bottom-right mark is dynamically compacted inward to the tightest collision-free anchor enclosing placed pieces on each page (`PagePlan.bottom_right_anchor_mm`).
* **Missing CLI & Model Parameters:** The design doc does not specify `--seed <U64>` or `--interactive` flags, nor `PaginateOptions`, `NestProgress`, or `Rotation90` types present in the codebase.

## docs/designs/puzzle-gen-nesting.md

No drift detected.

## README.md

* **Missing Project Documentation:** The root `README.md` lists `puzzle-gen` with a reference only to `docs/designs/puzzle-gen.md`. It does not mention or link to the subsequent nesting design document `docs/designs/puzzle-gen-nesting.md` ([`docs/designs/puzzle-gen-nesting.md`](file:///c:/Users/Tim/src/github.com/bananajuicellc/toys/docs/designs/puzzle-gen-nesting.md)).
