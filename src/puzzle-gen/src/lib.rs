use std::sync::atomic::AtomicBool;

pub mod config;
pub mod ingest;
pub mod model;
mod nest;
pub mod paginate;
pub mod stage1;
pub mod stage2;
pub mod svg_emit;

use model::{
    Config, NestProgress, PaginateOptions, PipelineArtifacts, PipelineError, PipelineNotice,
};

/// Executes the complete Stage 0–4 geometric pipeline in memory in canonical millimeters.
pub fn run_pipeline(
    svg_xml: &str,
    config: &Config,
    strict: bool,
    notices: &mut Vec<PipelineNotice>,
) -> Result<PipelineArtifacts, PipelineError> {
    run_pipeline_with_options(
        svg_xml,
        config,
        strict,
        notices,
        &PaginateOptions::from_config(config),
        &AtomicBool::new(false),
        |_| {},
    )
}

/// Executes the Stage 0–4 pipeline with configurable nesting options, cancellation, and progress streaming.
pub fn run_pipeline_with_options<F>(
    svg_xml: &str,
    config: &Config,
    strict: bool,
    notices: &mut Vec<PipelineNotice>,
    options: &PaginateOptions,
    cancel: &AtomicBool,
    on_progress: F,
) -> Result<PipelineArtifacts, PipelineError>
where
    F: FnMut(NestProgress),
{
    let ingested = ingest::ingest_svg(svg_xml, notices)?;
    let spliced = stage1::splice_tabs(&ingested, strict, notices)?;
    let kerfed = stage2::heal_and_offset(&spliced, config, notices)?;
    let layout = paginate::paginate_pieces_with_options(&kerfed, config, options, cancel, on_progress)
        .map_err(|err| {
            err.with_debug_spliced(spliced.clone())
                .with_debug_kerfed(kerfed.clone())
        })?;

    Ok(PipelineArtifacts {
        spliced,
        kerfed,
        layout,
    })
}
