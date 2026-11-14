use std::str::FromStr;

use kurbo::{Affine, BezPath, ParamCurveArclen, PathEl, PathSeg, Point, QuadBez, Rect, Shape};
use roxmltree::{Document, Node};
use svgtypes::{SimplePathSegment, SimplifyingPathParser, TransformListParser, TransformListToken};

use crate::model::{
    CoordFrame, GridCell, IngestedPuzzle, LengthUnit, NoticeSeverity, PipelineError,
    PipelineNotice, TabCandidate,
};

const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const MAX_GROUP_DEPTH: usize = 64;
const MAX_COORD_MM: f64 = 10_000.0;

/// Parses and validates a Stage 0 Inkscape SVG into canonical millimeters.
pub fn ingest_svg(
    svg_xml: &str,
    notices: &mut Vec<PipelineNotice>,
) -> Result<IngestedPuzzle, PipelineError> {
    let doc = Document::parse(svg_xml)
        .map_err(|err| PipelineError::new("ingest", format!("invalid SVG XML: {err}")))?;
    let frame = CoordFrame::from_svg_root(&doc)?;

    let user_to_mm_affine = Affine::scale_non_uniform(frame.user_to_mm.0, frame.user_to_mm.1)
        * Affine::translate((-frame.viewbox_min_user.0, -frame.viewbox_min_user.1));

    let image_layer = find_required_layer(&doc, "Image")?;
    let pieces_layer = find_required_layer(&doc, "Pieces")?;
    let tabs_layer = find_required_layer(&doc, "Tabs")?;

    let image_info = extract_image(image_layer, user_to_mm_affine, &frame)?;
    let raw_rects = extract_rects(pieces_layer, user_to_mm_affine, &frame)?;
    let (rows, cols, x_lines_mm, y_lines_mm, cells) = build_rectilinear_grid(raw_rects)?;

    // Check whether the artwork bounds cover the full Pieces grid bounds.
    let grid_bounds = Rect::new(
        *x_lines_mm.first().unwrap(),
        *y_lines_mm.first().unwrap(),
        *x_lines_mm.last().unwrap(),
        *y_lines_mm.last().unwrap(),
    );
    let tol = 1e-3;
    if image_info.bounds_mm.x0 > grid_bounds.x0 + tol
        || image_info.bounds_mm.y0 > grid_bounds.y0 + tol
        || image_info.bounds_mm.x1 < grid_bounds.x1 - tol
        || image_info.bounds_mm.y1 < grid_bounds.y1 - tol
    {
        notices.push(PipelineNotice {
            severity: NoticeSeverity::Warning,
            code: "IMAGE_COVERAGE",
            message: format!(
                "image bounds [{:.2}, {:.2}]..[{:.2}, {:.2}] mm do not fully cover Pieces grid [{:.2}, {:.2}]..[{:.2}, {:.2}] mm",
                image_info.bounds_mm.x0,
                image_info.bounds_mm.y0,
                image_info.bounds_mm.x1,
                image_info.bounds_mm.y1,
                grid_bounds.x0,
                grid_bounds.y0,
                grid_bounds.x1,
                grid_bounds.y1
            ),
        });
    }

    let tabs = extract_tabs(tabs_layer, user_to_mm_affine, notices)?;

    Ok(IngestedPuzzle {
        frame,
        image_href: image_info.href,
        image_bounds_mm: image_info.bounds_mm,
        image_preserve_aspect_ratio: image_info.preserve_aspect_ratio,
        image_rendering: image_info.image_rendering,
        rows,
        cols,
        x_lines_mm,
        y_lines_mm,
        cells,
        tabs,
    })
}

fn find_required_layer<'a>(
    doc: &'a Document<'a>,
    label: &str,
) -> Result<Node<'a, 'a>, PipelineError> {
    let mut matches = doc.descendants().filter(|n| {
        n.is_element()
            && n.tag_name().name() == "g"
            && n.attribute((INKSCAPE_NS, "groupmode")) == Some("layer")
            && n.attribute((INKSCAPE_NS, "label")) == Some(label)
    });

    let first = matches.next().ok_or_else(|| {
        PipelineError::new(
            "ingest",
            format!("missing required Inkscape layer with inkscape:label=\"{label}\""),
        )
    })?;
    if matches.next().is_some() {
        return Err(PipelineError::new(
            "ingest",
            format!("duplicate Inkscape layer with inkscape:label=\"{label}\""),
        ));
    }
    Ok(first)
}

fn node_id(node: Node<'_, '_>) -> String {
    node.attribute("id")
        .map(str::to_string)
        .unwrap_or_else(|| "<no-id>".to_string())
}

fn parse_transform_attr(attr: Option<&str>, elem_id: &str) -> Result<Affine, PipelineError> {
    let Some(raw) = attr else {
        return Ok(Affine::IDENTITY);
    };
    let mut acc = Affine::IDENTITY;
    for token in TransformListParser::from(raw) {
        let tok = token.map_err(|err| {
            PipelineError::new(
                "ingest",
                format!("invalid transform attribute on element \"{elem_id}\": {err}"),
            )
        })?;
        let step = match tok {
            TransformListToken::Matrix { a, b, c, d, e, f } => Affine::new([a, b, c, d, e, f]),
            TransformListToken::Translate { tx, ty } => Affine::translate((tx, ty)),
            TransformListToken::Scale { sx, sy } => Affine::scale_non_uniform(sx, sy),
            TransformListToken::Rotate { angle } => Affine::rotate(angle.to_radians()),
            TransformListToken::SkewX { angle } => {
                Affine::new([1.0, 0.0, angle.to_radians().tan(), 1.0, 0.0, 0.0])
            }
            TransformListToken::SkewY { angle } => {
                Affine::new([1.0, angle.to_radians().tan(), 0.0, 1.0, 0.0, 0.0])
            }
        };
        acc *= step;
    }
    Ok(acc)
}

fn ancestor_user_transform(layer: Node<'_, '_>) -> Result<Affine, PipelineError> {
    let mut chain = Vec::new();
    let mut cur = layer.parent();
    while let Some(node) = cur {
        if node.is_element() {
            if node.tag_name().name() == "svg" {
                break;
            }
            chain.push(node);
        }
        cur = node.parent();
    }
    let mut acc = Affine::IDENTITY;
    for node in chain.into_iter().rev() {
        acc *= parse_transform_attr(node.attribute("transform"), &node_id(node))?;
    }
    Ok(acc)
}

fn walk_layer_elements<'a, F>(
    layer: Node<'a, 'a>,
    layer_name: &str,
    allowed_tag: &str,
    user_to_mm_affine: Affine,
    visitor: &mut F,
) -> Result<(), PipelineError>
where
    F: FnMut(Node<'a, 'a>, Affine) -> Result<(), PipelineError>,
{
    let base_affine = user_to_mm_affine
        * ancestor_user_transform(layer)?
        * parse_transform_attr(layer.attribute("transform"), &node_id(layer))?;

    walk_recursive(layer, layer_name, allowed_tag, base_affine, 0, visitor)
}

fn walk_recursive<'a, F>(
    parent: Node<'a, 'a>,
    layer_name: &str,
    allowed_tag: &str,
    parent_affine: Affine,
    depth: usize,
    visitor: &mut F,
) -> Result<(), PipelineError>
where
    F: FnMut(Node<'a, 'a>, Affine) -> Result<(), PipelineError>,
{
    if depth > MAX_GROUP_DEPTH {
        return Err(PipelineError::new(
            "ingest",
            format!(
                "layer \"{layer_name}\" exceeds maximum <g> nesting depth of {MAX_GROUP_DEPTH}"
            ),
        ));
    }

    for child in parent.children() {
        if !child.is_element() {
            continue;
        }
        let tag = child.tag_name().name();
        let id = node_id(child);
        if tag == "title" || tag == "desc" {
            continue;
        }
        if tag == "g" {
            let local_tf = parse_transform_attr(child.attribute("transform"), &id)?;
            walk_recursive(
                child,
                layer_name,
                allowed_tag,
                parent_affine * local_tf,
                depth + 1,
                visitor,
            )?;
            continue;
        }
        if tag == allowed_tag {
            let local_tf = parse_transform_attr(child.attribute("transform"), &id)?;
            visitor(child, parent_affine * local_tf)?;
            continue;
        }
        return Err(PipelineError::new(
            "ingest",
            format!(
                "disallowed element <{tag}> (id=\"{id}\") inside \"{layer_name}\" layer; only <{allowed_tag}>, <g>, <title>, and <desc> are permitted"
            ),
        ));
    }
    Ok(())
}

fn check_point_bounds(pt: Point, elem_id: &str) -> Result<(), PipelineError> {
    if !pt.x.is_finite() || !pt.y.is_finite() {
        return Err(PipelineError::new(
            "ingest",
            format!("non-finite coordinate on element \"{elem_id}\""),
        ));
    }
    if pt.x.abs() > MAX_COORD_MM || pt.y.abs() > MAX_COORD_MM {
        return Err(PipelineError::new(
            "ingest",
            format!(
                "coordinate ({:.3}, {:.3}) mm on element \"{elem_id}\" exceeds safe physical bound (+/-{MAX_COORD_MM} mm)",
                pt.x, pt.y
            ),
        ));
    }
    Ok(())
}

fn parse_attr_len_in_user_units(
    node: Node<'_, '_>,
    attr_name: &str,
    default_if_missing: Option<f64>,
    axis_user_to_mm: f64,
    elem_id: &str,
) -> Result<f64, PipelineError> {
    let Some(raw) = node.attribute(attr_name) else {
        return default_if_missing.ok_or_else(|| {
            PipelineError::new(
                "ingest",
                format!("element \"{elem_id}\" is missing required attribute \"{attr_name}\""),
            )
        });
    };
    let len = svgtypes::Length::from_str(raw.trim()).map_err(|err| {
        PipelineError::new(
            "ingest",
            format!("invalid {attr_name}=\"{raw}\" on element \"{elem_id}\": {err}"),
        )
    })?;
    if !len.number.is_finite() {
        return Err(PipelineError::new(
            "ingest",
            format!("non-finite {attr_name}=\"{raw}\" on element \"{elem_id}\""),
        ));
    }
    let val_user = match len.unit {
        svgtypes::LengthUnit::None => len.number,
        svgtypes::LengthUnit::Mm => LengthUnit::Mm.to_mm(len.number) / axis_user_to_mm,
        svgtypes::LengthUnit::Cm => LengthUnit::Cm.to_mm(len.number) / axis_user_to_mm,
        svgtypes::LengthUnit::In => LengthUnit::In.to_mm(len.number) / axis_user_to_mm,
        svgtypes::LengthUnit::Px => LengthUnit::Px.to_mm(len.number) / axis_user_to_mm,
        other => {
            return Err(PipelineError::new(
                "ingest",
                format!(
                    "unsupported unit {other:?} in {attr_name}=\"{raw}\" on element \"{elem_id}\""
                ),
            ));
        }
    };
    Ok(val_user)
}

struct ImageInfo {
    href: String,
    bounds_mm: Rect,
    preserve_aspect_ratio: String,
    image_rendering: Option<String>,
}

fn extract_image(
    layer: Node<'_, '_>,
    user_to_mm_affine: Affine,
    frame: &CoordFrame,
) -> Result<ImageInfo, PipelineError> {
    let mut images = Vec::new();
    walk_layer_elements(
        layer,
        "Image",
        "image",
        user_to_mm_affine,
        &mut |node, aff| {
            images.push((node, aff));
            Ok(())
        },
    )?;

    if images.is_empty() {
        return Err(PipelineError::new(
            "ingest",
            "Image layer must contain exactly one <image> element (found 0)",
        ));
    }
    if images.len() > 1 {
        return Err(PipelineError::new(
            "ingest",
            format!(
                "Image layer must contain exactly one <image> element (found {})",
                images.len()
            ),
        ));
    }

    let (node, aff) = images.pop().unwrap();
    let id = node_id(node);

    let href = node
        .attribute("href")
        .or_else(|| node.attribute((XLINK_NS, "href")))
        .ok_or_else(|| {
            PipelineError::new(
                "ingest",
                format!("<image id=\"{id}\"> is missing href / xlink:href"),
            )
        })?
        .trim()
        .to_string();

    if !is_allowed_data_image_uri(&href) {
        return Err(PipelineError::new(
            "ingest",
            format!(
                "<image id=\"{id}\"> must use an embedded data:image/(png|jpeg|webp|gif);base64, URI"
            ),
        ));
    }

    let x = parse_attr_len_in_user_units(node, "x", Some(0.0), frame.user_to_mm.0, &id)?;
    let y = parse_attr_len_in_user_units(node, "y", Some(0.0), frame.user_to_mm.1, &id)?;
    let w = parse_attr_len_in_user_units(node, "width", None, frame.user_to_mm.0, &id)?;
    let h = parse_attr_len_in_user_units(node, "height", None, frame.user_to_mm.1, &id)?;
    if w <= 0.0 || h <= 0.0 {
        return Err(PipelineError::new(
            "ingest",
            format!("<image id=\"{id}\"> must have positive width and height"),
        ));
    }

    let p00 = aff * Point::new(x, y);
    let p10 = aff * Point::new(x + w, y);
    let p01 = aff * Point::new(x, y + h);
    let p11 = aff * Point::new(x + w, y + h);
    for p in [p00, p10, p01, p11] {
        check_point_bounds(p, &id)?;
    }

    let vx = p10 - p00;
    let vy = p01 - p00;
    if vx.y.abs() > 1e-6 || vy.x.abs() > 1e-6 || vx.x <= 0.0 || vy.y <= 0.0 {
        return Err(PipelineError::new(
            "ingest",
            format!(
                "<image id=\"{id}\"> has a rotated, skewed, or negatively scaled transform; only translation and positive axis scaling are allowed"
            ),
        ));
    }

    let preserve_aspect_ratio = node
        .attribute("preserveAspectRatio")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("none")
        .to_string();

    let image_rendering = extract_image_rendering(node);

    let bounds_mm = Rect::new(p00.x, p00.y, p11.x, p11.y);

    Ok(ImageInfo {
        href,
        bounds_mm,
        preserve_aspect_ratio,
        image_rendering,
    })
}

fn extract_image_rendering(node: Node<'_, '_>) -> Option<String> {
    let mut cur = Some(node);
    while let Some(el) = cur {
        if !el.is_element() {
            break;
        }
        if let Some(style) = el.attribute("style") {
            for decl in style.split(';').rev() {
                if let Some((key, val)) = decl.split_once(':')
                    && key.trim().eq_ignore_ascii_case("image-rendering")
                {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        if let Some(attr) = el.attribute("image-rendering") {
            let trimmed = attr.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        cur = el.parent();
    }
    None
}

fn is_allowed_data_image_uri(uri: &str) -> bool {
    let prefixes = [
        "data:image/png;base64,",
        "data:image/jpeg;base64,",
        "data:image/webp;base64,",
        "data:image/gif;base64,",
    ];
    for p in prefixes {
        if let Some(rest) = uri.get(..p.len())
            && rest.eq_ignore_ascii_case(p)
        {
            return !uri[p.len()..].trim().is_empty();
        }
    }
    false
}

struct RawPieceRect {
    id: String,
    bounds_mm: Rect,
}

fn extract_rects(
    layer: Node<'_, '_>,
    user_to_mm_affine: Affine,
    frame: &CoordFrame,
) -> Result<Vec<RawPieceRect>, PipelineError> {
    let mut rects = Vec::new();
    walk_layer_elements(
        layer,
        "Pieces",
        "rect",
        user_to_mm_affine,
        &mut |node, aff| {
            let id = node_id(node);
            let x = parse_attr_len_in_user_units(node, "x", Some(0.0), frame.user_to_mm.0, &id)?;
            let y = parse_attr_len_in_user_units(node, "y", Some(0.0), frame.user_to_mm.1, &id)?;
            let w = parse_attr_len_in_user_units(node, "width", None, frame.user_to_mm.0, &id)?;
            let h = parse_attr_len_in_user_units(node, "height", None, frame.user_to_mm.1, &id)?;
            if w <= 0.0 || h <= 0.0 {
                return Err(PipelineError::new(
                    "ingest",
                    format!("<rect id=\"{id}\"> must have positive width and height"),
                ));
            }

            let rx = parse_attr_len_in_user_units(node, "rx", Some(0.0), frame.user_to_mm.0, &id)?;
            let ry = parse_attr_len_in_user_units(node, "ry", Some(0.0), frame.user_to_mm.1, &id)?;

            let p00 = aff * Point::new(x, y);
            let p10 = aff * Point::new(x + w, y);
            let p01 = aff * Point::new(x, y + h);
            let p11 = aff * Point::new(x + w, y + h);
            for p in [p00, p10, p01, p11] {
                check_point_bounds(p, &id)?;
            }

            let vx = p10 - p00;
            let vy = p01 - p00;
            let len_x = vx.hypot();
            let len_y = vy.hypot();
            if len_x <= 1e-6 || len_y <= 1e-6 {
                return Err(PipelineError::new(
                    "ingest",
                    format!("<rect id=\"{id}\"> collapsed to near-zero physical size"),
                ));
            }

            let rx_mm = rx.abs() * (len_x / w);
            let ry_mm = ry.abs() * (len_y / h);
            if rx_mm > 1e-4 || ry_mm > 1e-4 {
                return Err(PipelineError::new(
                    "ingest",
                    format!(
                        "<rect id=\"{id}\"> has rounded corners (rx={rx_mm:.4} mm, ry={ry_mm:.4} mm), which are not supported"
                    ),
                ));
            }

            if vx.y.abs() / len_x > 1e-4 || vy.x.abs() / len_y > 1e-4 {
                return Err(PipelineError::new(
                    "ingest",
                    format!(
                        "<rect id=\"{id}\"> is rotated or skewed; Pieces must be axis-aligned rectangles"
                    ),
                ));
            }

            let min_x = p00.x.min(p10.x).min(p01.x).min(p11.x);
            let max_x = p00.x.max(p10.x).max(p01.x).max(p11.x);
            let min_y = p00.y.min(p10.y).min(p01.y).min(p11.y);
            let max_y = p00.y.max(p10.y).max(p01.y).max(p11.y);

            rects.push(RawPieceRect {
                id,
                bounds_mm: Rect::new(min_x, min_y, max_x, max_y),
            });
            Ok(())
        },
    )?;

    if rects.is_empty() {
        return Err(PipelineError::new(
            "ingest",
            "Pieces layer must contain at least one <rect> element",
        ));
    }

    Ok(rects)
}

fn cluster_coords(mut coords: Vec<f64>) -> Vec<f64> {
    coords.sort_by(f64::total_cmp);
    coords.dedup();
    let mut lines = Vec::new();
    let mut cluster_start = coords[0];
    let mut sum = coords[0];
    let mut count = 1usize;

    for &val in &coords[1..] {
        if val - cluster_start <= 1.0 {
            sum += val;
            count += 1;
        } else {
            lines.push(sum / count as f64);
            cluster_start = val;
            sum = val;
            count = 1;
        }
    }
    lines.push(sum / count as f64);
    lines
}

fn nearest_line_index(lines: &[f64], val: f64) -> usize {
    let mut best_idx = 0usize;
    let mut best_dist = f64::INFINITY;
    for (idx, &line) in lines.iter().enumerate() {
        let d = (line - val).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = idx;
        }
    }
    best_idx
}

#[allow(clippy::type_complexity)]
fn build_rectilinear_grid(
    rects: Vec<RawPieceRect>,
) -> Result<(usize, usize, Vec<f64>, Vec<f64>, Vec<GridCell>), PipelineError> {
    let mut raw_xs = Vec::with_capacity(rects.len() * 2);
    let mut raw_ys = Vec::with_capacity(rects.len() * 2);
    for r in &rects {
        raw_xs.push(r.bounds_mm.x0);
        raw_xs.push(r.bounds_mm.x1);
        raw_ys.push(r.bounds_mm.y0);
        raw_ys.push(r.bounds_mm.y1);
    }

    let x_lines_mm = cluster_coords(raw_xs);
    let y_lines_mm = cluster_coords(raw_ys);

    if x_lines_mm.len() < 2 || y_lines_mm.len() < 2 {
        return Err(PipelineError::new(
            "ingest",
            "Pieces rectangles failed to form a non-degenerate grid (collapsed coordinates)",
        ));
    }

    let cols = x_lines_mm.len() - 1;
    let rows = y_lines_mm.len() - 1;
    let mut occupancy: Vec<Option<GridCell>> = vec![None; rows * cols];

    for r in rects {
        let c0 = nearest_line_index(&x_lines_mm, r.bounds_mm.x0);
        let c1 = nearest_line_index(&x_lines_mm, r.bounds_mm.x1);
        let r0 = nearest_line_index(&y_lines_mm, r.bounds_mm.y0);
        let r1 = nearest_line_index(&y_lines_mm, r.bounds_mm.y1);

        if c1 != c0 + 1 || r1 != r0 + 1 {
            return Err(PipelineError::new(
                "ingest",
                format!(
                    "<rect id=\"{}\"> does not span a single contiguous grid cell (x lines {c0}..{c1}, y lines {r0}..{r1})",
                    r.id
                ),
            ));
        }

        let slot_idx = r0 * cols + c0;
        if let Some(existing) = &occupancy[slot_idx] {
            return Err(PipelineError::new(
                "ingest",
                format!(
                    "overlapping <rect> elements \"{}\" and \"{}\" at grid cell ({r0}, {c0})",
                    existing.id, r.id
                ),
            ));
        }

        occupancy[slot_idx] = Some(GridCell {
            row: r0,
            col: c0,
            id: r.id,
            bounds_mm: Rect::new(
                x_lines_mm[c0],
                y_lines_mm[r0],
                x_lines_mm[c1],
                y_lines_mm[r1],
            ),
        });
    }

    let mut cells = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let Some(cell) = occupancy[r * cols + c].take() else {
                return Err(PipelineError::new(
                    "ingest",
                    format!(
                        "Pieces grid is not a contiguous {rows}x{cols} rectangle: missing cell at ({r}, {c})"
                    ),
                ));
            };
            cells.push(cell);
        }
    }

    Ok((rows, cols, x_lines_mm, y_lines_mm, cells))
}

fn extract_tabs(
    layer: Node<'_, '_>,
    user_to_mm_affine: Affine,
    notices: &mut Vec<PipelineNotice>,
) -> Result<Vec<TabCandidate>, PipelineError> {
    let mut tabs = Vec::new();
    walk_layer_elements(
        layer,
        "Tabs",
        "path",
        user_to_mm_affine,
        &mut |node, aff| {
            let id = node_id(node);
            let d = node.attribute("d").ok_or_else(|| {
                PipelineError::new(
                    "ingest",
                    format!("<path id=\"{id}\"> is missing attribute d"),
                )
            })?;

            let mut path_mm = BezPath::new();
            let mut measure_path = BezPath::new();
            let mut current_mm = Point::ZERO;
            let mut move_count = 0usize;
            let mut seg_count = 0usize;
            let mut has_close = false;

            for seg_res in SimplifyingPathParser::from(d) {
                let seg = seg_res.map_err(|err| {
                    PipelineError::new(
                        "ingest",
                        format!("invalid path data on <path id=\"{id}\">: {err}"),
                    )
                })?;
                match seg {
                    SimplePathSegment::MoveTo { x, y } => {
                        let pt = aff * Point::new(x, y);
                        check_point_bounds(pt, &id)?;
                        path_mm.move_to(pt);
                        measure_path.move_to(pt);
                        current_mm = pt;
                        move_count += 1;
                    }
                    SimplePathSegment::LineTo { x, y } => {
                        let pt = aff * Point::new(x, y);
                        check_point_bounds(pt, &id)?;
                        path_mm.line_to(pt);
                        measure_path.line_to(pt);
                        current_mm = pt;
                        seg_count += 1;
                    }
                    SimplePathSegment::Quadratic { x1, y1, x, y } => {
                        let c_mm = aff * Point::new(x1, y1);
                        let end_mm = aff * Point::new(x, y);
                        check_point_bounds(c_mm, &id)?;
                        check_point_bounds(end_mm, &id)?;
                        let cubic = QuadBez::new(current_mm, c_mm, end_mm).raise();
                        path_mm.curve_to(cubic.p1, cubic.p2, cubic.p3);
                        measure_path.curve_to(cubic.p1, cubic.p2, cubic.p3);
                        current_mm = end_mm;
                        seg_count += 1;
                    }
                    SimplePathSegment::CurveTo {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => {
                        let c1_mm = aff * Point::new(x1, y1);
                        let c2_mm = aff * Point::new(x2, y2);
                        let end_mm = aff * Point::new(x, y);
                        check_point_bounds(c1_mm, &id)?;
                        check_point_bounds(c2_mm, &id)?;
                        check_point_bounds(end_mm, &id)?;
                        path_mm.curve_to(c1_mm, c2_mm, end_mm);
                        measure_path.curve_to(c1_mm, c2_mm, end_mm);
                        current_mm = end_mm;
                        seg_count += 1;
                    }
                    SimplePathSegment::ClosePath => {
                        has_close = true;
                        measure_path.close_path();
                    }
                }
            }

            let bbox = measure_path.bounding_box();
            let aabb_diag = (bbox.width().powi(2) + bbox.height().powi(2)).sqrt();
            let arc_len: f64 = measure_path
                .segments()
                .map(|s: PathSeg| s.arclen(1e-3))
                .sum();

            // Filter tiny stray paths before enforcing open single-subpath rules.
            if aabb_diag < 0.05 && arc_len < 0.05 {
                notices.push(PipelineNotice {
                severity: NoticeSeverity::Info,
                code: "STRAY_PATH_FILTERED",
                message: format!(
                    "filtered stray <path id=\"{id}\"> (aabb_diag={aabb_diag:.4} mm, arc_len={arc_len:.4} mm)"
                ),
            });
                return Ok(());
            }

            if has_close {
                return Err(PipelineError::new(
                    "ingest",
                    format!(
                        "<path id=\"{id}\"> in Tabs must be an open curve (found Z/z closepath)"
                    ),
                ));
            }
            if move_count != 1 {
                return Err(PipelineError::new(
                    "ingest",
                    format!(
                        "<path id=\"{id}\"> in Tabs must have exactly 1 subpath (found {move_count} MoveTo commands)"
                    ),
                ));
            }
            if seg_count == 0 {
                return Err(PipelineError::new(
                    "ingest",
                    format!("<path id=\"{id}\"> in Tabs has no drawing segments"),
                ));
            }

            // Verify normalized path has only MoveTo/LineTo/CurveTo elements.
            debug_assert!(path_mm.elements().iter().all(|el| matches!(
                el,
                PathEl::MoveTo(_) | PathEl::LineTo(_) | PathEl::CurveTo(_, _, _)
            )));

            tabs.push(TabCandidate { id, path_mm });
            Ok(())
        },
    )?;

    Ok(tabs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

    fn wrap_svg(extra_defs: &str, body: &str) -> String {
        format!(
            r#"<svg width="40mm" height="40mm" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:xlink="http://www.w3.org/1999/xlink">
  <sodipodi:namedview inkscape:document-units="mm"/>
  {extra_defs}
  {body}
</svg>"#
        )
    }

    #[test]
    fn clusters_perturbed_rects_within_1mm_and_rejects_overlap() {
        // 2x2 grid with <= 0.4 mm perturbations on internal lines and negative scale on one rect.
        let svg = wrap_svg(
            "",
            &format!(
                r#"
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r00" x="0.0" y="0.0" width="20.1" height="19.9"/>
    <rect id="r01" x="-40.0" y="0.0" width="20.3" height="20.1" transform="scale(-1, 1)"/>
    <rect id="r10" x="0.1" y="20.2" width="19.7" height="19.8"/>
    <rect id="r11" x="19.9" y="19.8" width="20.1" height="20.2"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs"/>
"#
            ),
        );
        let mut notices = Vec::new();
        let ingested = ingest_svg(&svg, &mut notices).unwrap();
        assert_eq!(ingested.rows, 2);
        assert_eq!(ingested.cols, 2);
        assert_eq!(ingested.x_lines_mm.len(), 3);
        assert_eq!(ingested.y_lines_mm.len(), 3);
        assert_eq!(ingested.image_preserve_aspect_ratio, "none");
        assert_eq!(ingested.cells[1].id, "r01");

        // Overlapping rect fails.
        let overlap_svg = wrap_svg(
            "",
            &format!(
                r#"
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r1" x="0" y="0" width="20" height="20"/>
    <rect id="r2" x="0.1" y="0" width="20" height="20"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs"/>
"#
            ),
        );
        assert!(ingest_svg(&overlap_svg, &mut notices).is_err());
    }

    #[test]
    fn rejects_rotated_rect_and_rotated_image_and_unexpected_elements() {
        let rot_rect = wrap_svg(
            "",
            &format!(
                r#"
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="rot" x="0" y="0" width="20" height="20" transform="rotate(90 10 10)"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs"/>
"#
            ),
        );
        let mut notices = Vec::new();
        assert!(ingest_svg(&rot_rect, &mut notices).is_err());

        let rot_img = wrap_svg(
            "",
            &format!(
                r#"
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" transform="rotate(5)" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r00" x="0" y="0" width="20" height="20"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs"/>
"#
            ),
        );
        assert!(ingest_svg(&rot_img, &mut notices).is_err());

        let bad_tag = wrap_svg(
            "",
            &format!(
                r#"
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r00" x="0" y="0" width="20" height="20"/>
    <circle id="c1" cx="10" cy="10" r="2"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs"/>
"#
            ),
        );
        let err = ingest_svg(&bad_tag, &mut notices).unwrap_err();
        assert!(err.to_string().contains("c1"));
    }

    #[test]
    fn converts_quadratic_and_arc_and_filters_stray_paths() {
        let svg = wrap_svg(
            "",
            &format!(
                r#"
  <g inkscape:groupmode="layer" inkscape:label="Image">
    <image id="img1" x="0" y="0" width="40" height="40" xlink:href="{PNG_1X1}"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Pieces">
    <rect id="r00" x="0" y="0" width="20" height="40"/>
    <rect id="r01" x="20" y="0" width="20" height="40"/>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Tabs">
    <path id="stray1" d="M 5 5 L 5.01 5.01 Z"/>
    <path id="tab_quad" d="M 20 10 Q 25 20 20 30"/>
    <path id="tab_arc" d="M 20 12 A 4 4 0 0 1 20 28"/>
  </g>
"#
            ),
        );
        let mut notices = Vec::new();
        let ingested = ingest_svg(&svg, &mut notices).unwrap();
        assert_eq!(ingested.tabs.len(), 2);
        assert!(
            notices
                .iter()
                .any(|n| n.code == "STRAY_PATH_FILTERED" && n.message.contains("stray1"))
        );
        for tab in &ingested.tabs {
            assert!(
                tab.path_mm
                    .elements()
                    .iter()
                    .any(|el| matches!(el, PathEl::CurveTo(_, _, _)))
            );
        }
    }
}
