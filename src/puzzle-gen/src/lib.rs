pub mod config;
pub mod ingest;
pub mod model;
pub mod paginate;
pub mod stage1;
pub mod stage2;
pub mod svg_emit;

use model::{Config, PipelineArtifacts, PipelineError, PipelineNotice};

/// Executes the complete Stage 0–4 geometric pipeline in memory in canonical millimeters.
pub fn run_pipeline(
    svg_xml: &str,
    config: &Config,
    strict: bool,
    notices: &mut Vec<PipelineNotice>,
) -> Result<PipelineArtifacts, PipelineError> {
    let ingested = ingest::ingest_svg(svg_xml, notices)?;
    let spliced = stage1::splice_tabs(&ingested, strict, notices)?;
    let kerfed = stage2::heal_and_offset(&spliced, config, notices)?;
    let layout = paginate::paginate_pieces(&kerfed, config).map_err(|err| {
        err.with_debug_spliced(spliced.clone())
            .with_debug_kerfed(kerfed.clone())
    })?;

    Ok(PipelineArtifacts {
        spliced,
        kerfed,
        layout,
    })
}
