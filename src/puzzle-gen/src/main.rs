mod tui;

use std::fs;
use std::io::{ErrorKind, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use puzzle_gen::model::{Config, NoticeSeverity, PipelineNotice};
use puzzle_gen::{run_pipeline, svg_emit};

#[derive(Debug, Parser)]
#[command(
    name = "puzzle-gen",
    about = "Four-stage laser jigsaw puzzle generator from Stage 0 Inkscape SVGs"
)]
struct Cli {
    /// Path to Stage 0 Inkscape SVG containing Image, Pieces, and Tabs layers.
    input_svg: PathBuf,

    /// Optional YAML configuration file path.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Output directory for generated SVGs (defaults to <input_svg_parent>/generated).
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Output filename prefix (defaults to <INPUT_SVG> file stem).
    #[arg(long)]
    prefix: Option<String>,

    /// Allow replacing existing outputs and pruning stale Stage 4 page files.
    #[arg(long)]
    overwrite: bool,

    /// Run all geometry and pagination checks without creating directories or writing SVG files.
    #[arg(long)]
    dry_run: bool,

    /// Downgrade missing internal tabs from errors to warnings and keep those edges straight.
    #[arg(long)]
    no_strict: bool,

    /// Write completed Stage 1 (and Stage 2 if available) debug SVGs when a downstream stage fails.
    #[arg(long)]
    emit_debug_on_error: bool,

    /// Deterministic PRNG seed overriding `seed` from configuration YAML.
    #[arg(long)]
    seed: Option<u64>,

    /// Launch live terminal UI on stderr showing nesting optimization progress.
    #[arg(long)]
    interactive: bool,
}

fn main() -> ExitCode {
    match run_cli() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_cli() -> Result<(), String> {
    let cli = Cli::parse();

    let prefix = match cli.prefix {
        Some(p) => p,
        None => cli
            .input_svg
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .ok_or_else(|| {
                format!(
                    "cannot derive output prefix from input path \"{}\"; specify --prefix",
                    cli.input_svg.display()
                )
            })?,
    };
    validate_prefix(&prefix)?;

    let output_dir = match cli.output_dir {
        Some(dir) => dir,
        None => match cli.input_svg.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join("generated"),
            _ => PathBuf::from("generated"),
        },
    };

    let svg_xml = fs::read_to_string(&cli.input_svg).map_err(|e| {
        format!(
            "failed to read input SVG \"{}\": {e}",
            cli.input_svg.display()
        )
    })?;

    let yaml_text = match &cli.config {
        Some(cfg_path) => Some(
            fs::read_to_string(cfg_path)
                .map_err(|e| format!("failed to read config \"{}\": {e}", cfg_path.display()))?,
        ),
        None => None,
    };
    let mut config = Config::from_yaml(yaml_text.as_deref()).map_err(|e| e.to_string())?;
    if let Some(seed) = cli.seed {
        config.seed = seed;
    }

    let mut notices = Vec::new();
    let result = if cli.interactive {
        if std::io::stderr().is_terminal() && std::io::stdin().is_terminal() {
            tui::run_interactive_pipeline(&svg_xml, &config, !cli.no_strict, &mut notices)
        } else {
            notices.push(PipelineNotice {
                severity: NoticeSeverity::Warning,
                code: "INTERACTIVE_NON_TTY",
                message:
                    "--interactive requested on a non-TTY stream; falling back to headless optimization"
                        .to_string(),
            });
            run_pipeline(&svg_xml, &config, !cli.no_strict, &mut notices)
        }
    } else {
        run_pipeline(&svg_xml, &config, !cli.no_strict, &mut notices)
    };
    emit_notices(&notices);

    let artifacts = match result {
        Ok(a) => a,
        Err(err) => {
            if cli.emit_debug_on_error && !cli.dry_run {
                emit_debug_outputs(&output_dir, &prefix, cli.overwrite, &config, &err)?;
            }
            return Err(err.to_string());
        }
    };

    let (slot_w_mm, slot_h_mm) = puzzle_gen::paginate::compute_slot_bounds(&artifacts.kerfed);
    println!("grid={}x{}", artifacts.kerfed.rows, artifacts.kerfed.cols);
    println!("matched_tabs={}", artifacts.kerfed.matched_tabs);
    println!("missing_tabs={}", artifacts.kerfed.missing_tabs);
    println!("slot_mm={slot_w_mm:.4}x{slot_h_mm:.4}");
    println!("pages={}", artifacts.layout.pages.len());
    println!(
        "puzzle_area_mm2={:.4}",
        artifacts.layout.puzzle_area_mm2
    );
    println!(
        "total_bbox_mm2={:.4}",
        artifacts.layout.total_bbox_area_mm2
    );
    println!("bbox_area_pct={:.2}", artifacts.layout.bbox_area_pct);

    if cli.dry_run {
        return Ok(());
    }

    let stage1_path = output_dir.join(format!("{prefix}-stage1.svg"));
    let stage2_path = output_dir.join(format!("{prefix}-stage2.svg"));
    let stage3_path = output_dir.join(format!("{prefix}-stage3.svg"));
    let mut target_paths = vec![
        stage1_path.clone(),
        stage2_path.clone(),
        stage3_path.clone(),
    ];
    for page in &artifacts.layout.pages {
        target_paths.push(output_dir.join(format!("{prefix}-stage4-page{}.svg", page.page_number)));
    }

    let existing_stage4 = scan_output_dir(&output_dir, &prefix)?;

    if !cli.overwrite {
        let collisions: Vec<String> = target_paths
            .iter()
            .filter(|p| p.exists())
            .map(|p| p.display().to_string())
            .collect();
        if !collisions.is_empty() {
            return Err(format!(
                "refusing to overwrite existing output file(s) without --overwrite: {}",
                collisions.join(", ")
            ));
        }
    }

    fs::create_dir_all(&output_dir).map_err(|e| {
        format!(
            "failed to create output directory \"{}\": {e}",
            output_dir.display()
        )
    })?;

    // Serialize and write lazily one stage at a time.
    {
        let s1 = svg_emit::emit_stage1(&artifacts.spliced);
        write_atomic(&output_dir, &stage1_path, &s1)?;
        println!("wrote={}", stage1_path.display());
    }
    {
        let s2 = svg_emit::emit_stage2(&artifacts.kerfed, &config);
        write_atomic(&output_dir, &stage2_path, &s2)?;
        println!("wrote={}", stage2_path.display());
    }
    {
        let s3 = svg_emit::emit_stage3(&artifacts.kerfed, &artifacts.layout, &config);
        write_atomic(&output_dir, &stage3_path, &s3)?;
        println!("wrote={}", stage3_path.display());
    }
    let new_page_count = artifacts.layout.pages.len();
    for page in &artifacts.layout.pages {
        let p_path = output_dir.join(format!("{prefix}-stage4-page{}.svg", page.page_number));
        let s4 = svg_emit::emit_stage4_page(&artifacts.kerfed, page, &config);
        write_atomic(&output_dir, &p_path, &s4)?;
        println!("wrote={}", p_path.display());
    }

    if cli.overwrite {
        for (page_num, stale_path) in existing_stage4 {
            if page_num > new_page_count {
                fs::remove_file(&stale_path).map_err(|e| {
                    format!(
                        "failed to remove stale Stage 4 file \"{}\": {e}",
                        stale_path.display()
                    )
                })?;
            }
        }
    }

    Ok(())
}

fn validate_prefix(prefix: &str) -> Result<(), String> {
    if prefix.is_empty() || prefix == "." || prefix == ".." || prefix.contains(['/', '\\']) {
        return Err(format!(
            "invalid --prefix \"{prefix}\": must be a non-empty filename prefix without path separators, '.', or '..'"
        ));
    }
    Ok(())
}

fn emit_notices(notices: &[PipelineNotice]) {
    for n in notices {
        let level = match n.severity {
            NoticeSeverity::Info => "info",
            NoticeSeverity::Warning => "warning",
        };
        eprintln!("[{level}] {}: {}", n.code, n.message);
    }
}

fn emit_debug_outputs(
    output_dir: &Path,
    prefix: &str,
    overwrite: bool,
    config: &Config,
    err: &puzzle_gen::model::PipelineError,
) -> Result<(), String> {
    let mut debug_targets = Vec::new();
    let s1_path = output_dir.join(format!("{prefix}-stage1.svg"));
    let s2_path = output_dir.join(format!("{prefix}-stage2.svg"));

    if err.debug_spliced.is_some() {
        debug_targets.push(s1_path.clone());
    }
    if err.debug_kerfed.is_some() {
        debug_targets.push(s2_path.clone());
    }
    if debug_targets.is_empty() {
        return Ok(());
    }

    if !overwrite {
        let collisions: Vec<String> = debug_targets
            .iter()
            .filter(|p| p.exists())
            .map(|p| p.display().to_string())
            .collect();
        if !collisions.is_empty() {
            return Err(format!(
                "{err}; also refused to overwrite existing debug file(s) without --overwrite: {}",
                collisions.join(", ")
            ));
        }
    }

    fs::create_dir_all(output_dir).map_err(|e| {
        format!(
            "{err}; failed to create output directory \"{}\": {e}",
            output_dir.display()
        )
    })?;

    if let Some(spliced) = &err.debug_spliced {
        let s1 = svg_emit::emit_stage1(spliced);
        write_atomic(output_dir, &s1_path, &s1)?;
        eprintln!("wrote debug Stage 1: {}", s1_path.display());
    }
    if let Some(kerfed) = &err.debug_kerfed {
        let s2 = svg_emit::emit_stage2(kerfed, config);
        write_atomic(output_dir, &s2_path, &s2)?;
        eprintln!("wrote debug Stage 2: {}", s2_path.display());
    }

    Ok(())
}

fn scan_output_dir(output_dir: &Path, prefix: &str) -> Result<Vec<(usize, PathBuf)>, String> {
    let entries = match fs::read_dir(output_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(format!(
                "failed to scan output directory \"{}\": {e}",
                output_dir.display()
            ));
        }
    };

    let mut stage4_files = Vec::new();
    for entry_res in entries {
        let entry = entry_res
            .map_err(|e| format!("failed to read entry in \"{}\": {e}", output_dir.display()))?;
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if let Some(page_num) = parse_stage4_page_number(name_str, prefix) {
            stage4_files.push((page_num, entry.path()));
        }
    }
    Ok(stage4_files)
}

fn parse_stage4_page_number(file_name: &str, prefix: &str) -> Option<usize> {
    let expected_prefix = format!("{prefix}-stage4-page");
    let rest = file_name.strip_prefix(&expected_prefix)?;
    let digits = rest.strip_suffix(".svg")?;
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let page_num: usize = digits.parse().ok()?;
    (page_num >= 1).then_some(page_num)
}

fn write_atomic(dir: &Path, target: &Path, contents: &str) -> Result<(), String> {
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .map_err(|e| format!("failed to create temp file in \"{}\": {e}", dir.display()))?;
    tmp.write_all(contents.as_bytes()).map_err(|e| {
        format!(
            "failed to write temp file for \"{}\": {e}",
            target.display()
        )
    })?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| format!("failed to sync temp file for \"{}\": {e}", target.display()))?;
    tmp.persist(target)
        .map_err(|e| format!("failed to persist \"{}\": {}", target.display(), e.error))?;
    Ok(())
}
