use std::fmt;

/// Physical or CSS length unit supported by puzzle-gen configuration and SVG roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    Mm,
    Cm,
    In,
    Px,
}

impl LengthUnit {
    /// Converts a scalar value in `self` units to canonical millimeters.
    pub fn to_mm(self, val: f64) -> f64 {
        match self {
            LengthUnit::Mm => val,
            LengthUnit::Cm => val * 10.0,
            LengthUnit::In => val * 25.4,
            LengthUnit::Px => val * (25.4 / 96.0),
        }
    }

    /// Converts a scalar value in canonical millimeters to `self` units.
    pub fn from_mm(self, mm: f64) -> f64 {
        match self {
            LengthUnit::Mm => mm,
            LengthUnit::Cm => mm / 10.0,
            LengthUnit::In => mm / 25.4,
            LengthUnit::Px => mm * (96.0 / 25.4),
        }
    }

    /// Returns the SVG/Inkscape unit string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            LengthUnit::Mm => "mm",
            LengthUnit::Cm => "cm",
            LengthUnit::In => "in",
            LengthUnit::Px => "px",
        }
    }

    /// Returns the fixed decimal precision used during SVG serialization.
    pub fn decimal_places(self) -> usize {
        match self {
            LengthUnit::In | LengthUnit::Cm => 6,
            LengthUnit::Mm | LengthUnit::Px => 4,
        }
    }
}

/// Coordinate mapping between input SVG user units, original `doc_unit`, and canonical millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoordFrame {
    pub doc_unit: LengthUnit,
    pub viewbox_min_user: (f64, f64),
    pub user_to_mm: (f64, f64),
    pub doc_to_mm: f64,
}

/// Validated pipeline configuration parameters in canonical millimeters.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub page_unit: LengthUnit,
    pub margin_mm: f64,
    pub kerf_mm: f64,
    pub segment_length_mm: f64,
    pub piece_spacing_mm: f64,
    pub print_bleed_mm: f64,
    pub score_color: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            page_width_mm: LengthUnit::In.to_mm(6.0),
            page_height_mm: LengthUnit::In.to_mm(6.0),
            page_unit: LengthUnit::Mm,
            margin_mm: LengthUnit::In.to_mm(0.25),
            kerf_mm: 0.18,
            segment_length_mm: 0.18,
            piece_spacing_mm: 0.18,
            print_bleed_mm: 0.18,
            score_color: "#0000ff".to_string(),
        }
    }
}

/// A single rectilinear grid cell parsed from the Stage 0 `Pieces` layer.
#[derive(Debug, Clone, PartialEq)]
pub struct GridCell {
    pub row: usize,
    pub col: usize,
    pub id: String,
    pub bounds_mm: kurbo::Rect,
}

/// Axis orientation of a rectilinear grid edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Canonical internal grid edge shared between two adjacent puzzle cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InternalEdge {
    pub orientation: Orientation,
    pub coord_mm: f64,
    pub start_mm: f64,
    pub end_mm: f64,
    pub cell_a: (usize, usize),
    pub cell_b: (usize, usize),
}

/// Open curve candidate extracted from the Stage 0 `Tabs` layer in canonical `mm`.
#[derive(Debug, Clone, PartialEq)]
pub struct TabCandidate {
    pub id: String,
    pub path_mm: kurbo::BezPath,
}

/// Stage 0 ingestion output in canonical `mm`.
#[derive(Debug, Clone, PartialEq)]
pub struct IngestedPuzzle {
    pub frame: CoordFrame,
    pub image_href: String,
    pub image_bounds_mm: kurbo::Rect,
    pub image_preserve_aspect_ratio: String,
    pub image_rendering: Option<String>,
    pub rows: usize,
    pub cols: usize,
    pub x_lines_mm: Vec<f64>,
    pub y_lines_mm: Vec<f64>,
    pub cells: Vec<GridCell>,
    pub tabs: Vec<TabCandidate>,
}

/// Closed clockwise contour for one puzzle piece after Stage 1 tab splicing.
#[derive(Debug, Clone, PartialEq)]
pub struct SplicedPiece {
    pub row: usize,
    pub col: usize,
    pub id: String,
    pub path_mm: kurbo::BezPath,
}

/// Stage 1 output containing closed piecewise-cubic piece contours in canonical `mm`.
#[derive(Debug, Clone, PartialEq)]
pub struct SplicedPuzzle {
    pub frame: CoordFrame,
    pub image_href: String,
    pub image_bounds_mm: kurbo::Rect,
    pub image_preserve_aspect_ratio: String,
    pub image_rendering: Option<String>,
    pub rows: usize,
    pub cols: usize,
    pub matched_tabs: usize,
    pub missing_tabs: usize,
    pub pieces: Vec<SplicedPiece>,
}

/// Flattened, healed, and offset cut/bleed contours for one puzzle piece in canonical `mm`.
#[derive(Debug, Clone, PartialEq)]
pub struct KerfedPiece {
    pub row: usize,
    pub col: usize,
    pub id: String,
    pub cut_aabb_mm: kurbo::Rect,
    pub cut_polygon: Vec<kurbo::Point>,
    pub bleed_polygon: Vec<kurbo::Point>,
}

/// Stage 2 output containing healed and offset piece polygons in canonical `mm`.
#[derive(Debug, Clone, PartialEq)]
pub struct KerfedGrid {
    pub frame: CoordFrame,
    pub image_href: String,
    pub image_bounds_mm: kurbo::Rect,
    pub image_preserve_aspect_ratio: String,
    pub image_rendering: Option<String>,
    pub rows: usize,
    pub cols: usize,
    pub matched_tabs: usize,
    pub missing_tabs: usize,
    pub pieces: Vec<KerfedPiece>,
}

/// Page corner band anchor for registration ticks and vector page numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerAnchor {
    TopLeft,
    BottomRight,
}

/// Placement assignment for a single piece on a paginated sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSlot {
    pub piece_index: usize,
    pub row: usize,
    pub col: usize,
    pub offset_mm: kurbo::Vec2,
}

/// Planned print/cut sheet with piece translations and corner registration strokes.
#[derive(Debug, Clone, PartialEq)]
pub struct PagePlan {
    pub page_number: usize,
    pub slots: Vec<PageSlot>,
    pub corner_strokes_mm: Vec<Vec<kurbo::Point>>,
}

/// Stage 3 and Stage 4 shared pagination layout in canonical `mm`.
#[derive(Debug, Clone, PartialEq)]
pub struct PaginatedLayout {
    pub slot_w_mm: f64,
    pub slot_h_mm: f64,
    pub page_cols: usize,
    pub page_rows: usize,
    pub pages: Vec<PagePlan>,
}

/// Complete in-memory outputs across Stages 1–4.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineArtifacts {
    pub spliced: SplicedPuzzle,
    pub kerfed: KerfedGrid,
    pub layout: PaginatedLayout,
}

/// Severity level for non-fatal diagnostic notices emitted during pipeline execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeSeverity {
    Info,
    Warning,
}

/// Non-fatal diagnostic notice emitted during SVG ingestion, splicing, or offsetting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineNotice {
    pub severity: NoticeSeverity,
    pub code: &'static str,
    pub message: String,
}

/// Pipeline error with stage attribution and optional completed debug artifacts.
#[derive(Debug, Clone)]
pub struct PipelineError {
    pub stage: &'static str,
    pub message: String,
    pub debug_spliced: Option<Box<SplicedPuzzle>>,
    pub debug_kerfed: Option<Box<KerfedGrid>>,
}

impl PipelineError {
    /// Creates a new `PipelineError` without debug artifacts.
    pub fn new(stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
            debug_spliced: None,
            debug_kerfed: None,
        }
    }

    /// Attaches completed Stage 1 debug state if available.
    pub fn with_debug_spliced(mut self, spliced: SplicedPuzzle) -> Self {
        self.debug_spliced = Some(Box::new(spliced));
        self
    }

    /// Attaches completed Stage 2 debug state if available.
    pub fn with_debug_kerfed(mut self, kerfed: KerfedGrid) -> Self {
        self.debug_kerfed = Some(Box::new(kerfed));
        self
    }
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.stage, self.message)
    }
}

impl std::error::Error for PipelineError {}
