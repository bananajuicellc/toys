use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use puzzle_gen::model::{
    Config, NestProgress, PaginateOptions, PipelineArtifacts, PipelineError, PipelineNotice,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, Gauge, GraphType, Paragraph};
use ratatui::{Frame, Terminal};

struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(err) = execute!(io::stderr(), EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(err);
        }
        Ok(Self { active: true })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
            let _ = execute!(io::stderr(), LeaveAlternateScreen);
            self.active = false;
        }
    }
}

pub(crate) fn run_interactive_pipeline(
    svg_xml: &str,
    config: &Config,
    strict: bool,
    notices: &mut Vec<PipelineNotice>,
) -> Result<PipelineArtifacts, PipelineError> {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = Arc::clone(&cancel);
    let (tx, rx) = mpsc::channel::<NestProgress>();

    std::thread::scope(|scope| {
        let handle = scope.spawn(move || {
            let mut worker_notices = Vec::new();
            let options = PaginateOptions {
                seed: config.seed,
                max_iters: None,
            };
            let result = puzzle_gen::run_pipeline_with_options(
                svg_xml,
                config,
                strict,
                &mut worker_notices,
                &options,
                &cancel_worker,
                move |progress| {
                    let _ = tx.send(progress);
                },
            );
            (result, worker_notices)
        });

        let ui_result: io::Result<()> = (|| {
            let _guard = TerminalGuard::enter()?;
            let backend = CrosstermBackend::new(io::stderr());
            let mut terminal = Terminal::new(backend)?;
            terminal.clear()?;

            let start_time = Instant::now();
            let mut latest: Option<NestProgress> = None;
            let mut history: Vec<(f64, f64)> = Vec::new();
            let mut disconnected = false;

            loop {
                loop {
                    match rx.try_recv() {
                        Ok(progress) => {
                            history.push((progress.iter as f64, progress.bbox_area_pct));
                            latest = Some(progress);
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }

                let stopping = cancel.load(Ordering::Relaxed);
                let elapsed = start_time.elapsed();
                terminal.draw(|frame| {
                    draw_ui(frame, latest.as_ref(), &history, elapsed, stopping);
                })?;

                if disconnected {
                    break;
                }

                if event::poll(Duration::from_millis(50))?
                    && let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press
                    && is_stop_key(key.code, key.modifiers)
                {
                    cancel.store(true, Ordering::Relaxed);
                }
            }

            Ok(())
        })();

        if let Err(err) = ui_result {
            cancel.store(true, Ordering::Relaxed);
            let (pipe_res, worker_notices) = handle
                .join()
                .map_err(|_| PipelineError::new("tui", "nesting worker thread panicked"))?;
            notices.extend(worker_notices);
            return pipe_res.and(Err(PipelineError::new(
                "tui",
                format!("terminal UI error: {err}"),
            )));
        }

        let (result, worker_notices) = handle
            .join()
            .map_err(|_| PipelineError::new("tui", "nesting worker thread panicked"))?;
        notices.extend(worker_notices);
        result
    })
}

fn is_stop_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(
        code,
        KeyCode::Char('q' | 'Q') | KeyCode::Esc | KeyCode::Enter
    ) || (matches!(code, KeyCode::Char('c' | 'C')) && modifiers.contains(KeyModifiers::CONTROL))
}

fn draw_ui(
    frame: &mut Frame<'_>,
    latest: Option<&NestProgress>,
    history: &[(f64, f64)],
    elapsed: Duration,
    stopping: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(11),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let stop_suffix = if stopping { " | stopping..." } else { "" };
    let summary_text = if let Some(p) = latest {
        let iter_cap = p
            .total_iters
            .map(|t| format!("/{t}"))
            .unwrap_or_default();
        format!(
            "grid={}x{} | pages={} | bbox_area_pct={:.1}% | iter={}{} | elapsed={:.1}s{}",
            p.rows,
            p.cols,
            p.pages,
            p.bbox_area_pct,
            p.iter,
            iter_cap,
            elapsed.as_secs_f64(),
            stop_suffix
        )
    } else {
        format!(
            "Computing Stage 0–2 geometry... | elapsed={:.1}s{}",
            elapsed.as_secs_f64(),
            stop_suffix
        )
    };
    let summary = Paragraph::new(summary_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("puzzle-gen nesting"),
    );
    frame.render_widget(summary, chunks[0]);

    render_chart(frame, chunks[1], history);
    render_page_gauges(frame, chunks[2], latest);

    let footer = Paragraph::new("q / Enter / Esc / Ctrl+C: stop early and emit best layout")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[3]);
}

fn render_chart(frame: &mut Frame<'_>, area: ratatui::layout::Rect, history: &[(f64, f64)]) {
    let (x_min, x_max, y_min, y_max) = if history.is_empty() {
        (0.0, 1.0, 0.0, 100.0)
    } else {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for &(x, y) in history {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        (min_x, max_x.max(min_x + 1.0), min_y, max_y.max(min_y + 1.0))
    };

    let dataset = Dataset::default()
        .name("bbox_area_pct")
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Cyan))
        .data(history);

    let x_labels = [format!("{x_min:.0}"), format!("{x_max:.0}")];
    let y_labels = [format!("{y_min:.1}%"), format!("{y_max:.1}%")];

    let chart = Chart::new(vec![dataset])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Optimization Progress (bbox_area_pct vs iter)"),
        )
        .x_axis(
            Axis::default()
                .title("iter")
                .style(Style::default().fg(Color::Gray))
                .bounds([x_min, x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title("area %")
                .style(Style::default().fg(Color::Gray))
                .bounds([y_min, y_max])
                .labels(y_labels),
        );
    frame.render_widget(chart, area);
}

fn render_page_gauges(
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    latest: Option<&NestProgress>,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Per-Page Fill");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(p) = latest else {
        if inner.height > 0 {
            frame.render_widget(Paragraph::new("Waiting for first layout..."), inner);
        }
        return;
    };

    if p.page_fill_ratios.is_empty() || inner.height == 0 {
        return;
    }

    let ratios = &p.page_fill_ratios;
    let has_overflow = ratios.len() > 8 && inner.height > 1;
    let max_bars = if has_overflow {
        8.min(inner.height.saturating_sub(1) as usize)
    } else {
        8.min(inner.height as usize)
    };

    let mut constraints = vec![Constraint::Length(1); max_bars];
    if has_overflow {
        constraints.push(Constraint::Length(1));
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (idx, &ratio) in ratios.iter().take(max_bars).enumerate() {
        let clamped = ratio.clamp(0.0, 1.0);
        let gauge = Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(Color::Green)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .ratio(clamped)
            .label(format!("Page {:>2}: {:>5.1}%", idx + 1, ratio * 100.0));
        frame.render_widget(gauge, rows[idx]);
    }

    if has_overflow {
        let rem = &ratios[max_bars..];
        let avg_pct = (rem.iter().sum::<f64>() / rem.len() as f64) * 100.0;
        let more_line = Paragraph::new(format!(
            "... +{} more pages (avg fill {:.1}%)",
            rem.len(),
            avg_pct
        ))
        .style(Style::default().fg(Color::Yellow));
        frame.render_widget(more_line, rows[max_bars]);
    }
}
