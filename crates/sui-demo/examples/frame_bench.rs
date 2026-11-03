//! Scripted frame-time benchmark over the dev application's pages.
//!
//! Opens every demo from the picker, scrolls and hovers it, switches back to
//! each open tab, and cycles the theme. Every input is timed from dispatch
//! until the frames it caused have been presented, so slow frames show up as
//! the lag a user would feel. Run a release build without VSync:
//!
//! ```sh
//! cargo run -p sinomo-ui-demo --release --example frame_bench
//! cargo run -p sinomo-ui-demo --release --example frame_bench -- --detailed
//! ```
//!
//! `--detailed` turns on per-phase scene statistics (slower) and prints the
//! phase breakdown of the slowest inputs. `--trace` lists every frame inside
//! slow inputs. `--pages a,b` limits the pages by title prefix, `--repeat N`
//! repeats the warm passes. See `docs/frame-benchmark.md`.
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use sui::diagnostics::{
    SceneStatisticsDetailMode, WindowPerformanceSnapshot, set_window_scene_statistics_detail_mode,
};
use sui::{
    Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect, Result,
    ScrollDelta, SemanticsNode, SemanticsRole, Vector,
};
use sui_testing::{TestApp, TestWindow};

const SCROLL_STEPS: usize = 48;
const SCROLL_DELTA: f32 = -64.0;
const HOVER_STEPS: usize = 48;
const THEME_TOGGLES: usize = 3;
/// Inputs arrive at about this interval, so animations started by one input
/// advance before the next, as they would under a person's hand.
const INPUT_INTERVAL: Duration = Duration::from_millis(4);
const TOOLBAR_HEIGHT: f32 = 44.0;

#[derive(Debug, Clone)]
struct Sample {
    scenario: &'static str,
    page: String,
    wall_ms: f64,
    work_ms: f64,
    frames: u64,
    phases: String,
    /// The last frame's phase timings, with `--detailed`.
    phase_ms: Vec<(&'static str, f64)>,
}

struct TracedFrame {
    at: Instant,
    index: u64,
    work_ms: f64,
    /// Phases and hottest widgets of heavy frames, with `--detailed`.
    detail: Option<String>,
}

struct Bench {
    window: TestWindow,
    samples: Vec<Sample>,
    detailed: bool,
    /// Frames seen by the `--trace` observer: when, which, and their work.
    trace: Option<Arc<Mutex<Vec<TracedFrame>>>>,
}

impl Bench {
    fn performance(&self) -> Option<WindowPerformanceSnapshot> {
        sui::diagnostics::window_performance_snapshot(self.window.id())
    }

    /// Dispatch `events` and record how long they took to reach the screen.
    fn timed(&mut self, scenario: &'static str, page: &str, events: Vec<Event>) -> Result<()> {
        let before = self.performance().map(|frame| frame.frame_index);
        let started = Instant::now();
        let root = self.window.root();
        for event in events {
            root.dispatch_event_now(event)?;
        }
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        let ended = Instant::now();
        let after = self.performance();
        let frames = match (&after, before) {
            (Some(after), Some(before)) => after.frame_index.saturating_sub(before),
            (Some(after), None) => after.frame_index,
            _ => 0,
        };
        let phase_ms = after
            .as_ref()
            .filter(|_| frames > 0 && self.detailed)
            .map(|frame| {
                frame
                    .phase_timings
                    .iter()
                    .map(|sample| (sample.phase.label(), sample.duration_ms))
                    .collect()
            })
            .unwrap_or_default();
        let (work_ms, phases) = after
            .filter(|_| frames > 0)
            .map(|frame| {
                let phases = if self.detailed {
                    let mut parts = frame
                        .phase_timings
                        .iter()
                        .filter(|sample| sample.duration_ms >= 0.05)
                        .map(|sample| format!("{}={:.2}", sample.phase.label(), sample.duration_ms))
                        .collect::<Vec<_>>();
                    let text = frame.runtime_text_timing;
                    if text.request_count > 0 {
                        parts.push(format!(
                            "| text n={} miss={} total={:.2} miss_layout={:.2} size_only={}/{:.2}",
                            text.request_count,
                            text.cache_miss_count,
                            text.total_time_us as f64 / 1000.0,
                            text.miss_layout_time_us as f64 / 1000.0,
                            text.size_only_request_count,
                            text.size_only_time_us as f64 / 1000.0,
                        ));
                    }
                    // Inclusive, so nested widgets repeat their parents' time.
                    for sample in frame.widget_timings.iter().take(8) {
                        parts.push(format!(
                            "| {}:{}={:.2}x{}",
                            sample.widget_name,
                            sample.phase.label(),
                            sample.duration_ms,
                            sample.calls
                        ));
                    }
                    parts.join(" ")
                } else {
                    String::new()
                };
                (frame.total_time_ms, phases)
            })
            .unwrap_or_default();
        let mut phases = phases;
        if let Some(trace) = &self.trace
            && wall_ms > 12.0
        {
            let frames = trace.lock().expect("trace lock");
            let observed = frames
                .iter()
                .filter(|frame| frame.at >= started && frame.at <= ended)
                .map(|frame| {
                    format!(
                        "#{}@{:.1}:{:.2}{}",
                        frame.index,
                        frame.at.duration_since(started).as_secs_f64() * 1000.0,
                        frame.work_ms,
                        frame
                            .detail
                            .as_ref()
                            .map(|detail| format!(" [{detail}]"))
                            .unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>();
            phases.push_str(&format!(" || frames {}", observed.join(" ")));
        }
        self.samples.push(Sample {
            scenario,
            page: page.to_string(),
            wall_ms,
            work_ms,
            frames,
            phases,
            phase_ms,
        });
        Ok(())
    }

    fn nodes(&self) -> Result<Vec<SemanticsNode>> {
        Ok(self.window.snapshot()?.accessibility.nodes)
    }

    fn find(&self, role: SemanticsRole, name: &str) -> Result<Option<SemanticsNode>> {
        Ok(self
            .nodes()?
            .into_iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name)))
    }

    /// Wait for the input's animations to finish so they do not leak into
    /// the next scenario's samples.
    fn settle(&self) -> Result<()> {
        for _ in 0..4 {
            self.window.pump_frames(8)?;
        }
        self.window.run_until_idle()
    }
}

fn center(bounds: Rect) -> Point {
    Point::new(
        bounds.x() + bounds.width() * 0.5,
        bounds.y() + bounds.height() * 0.5,
    )
}

fn click(point: Point) -> Vec<Event> {
    let mut down = PointerEvent::new(PointerEventKind::Down, point);
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    let mut up = PointerEvent::new(PointerEventKind::Up, point);
    up.button = Some(PointerButton::Primary);
    vec![
        Event::Pointer(PointerEvent::new(PointerEventKind::Move, point)),
        Event::Pointer(down),
        Event::Pointer(up),
    ]
}

fn scroll(point: Point, delta: f32) -> Vec<Event> {
    let mut scroll = PointerEvent::new(PointerEventKind::Scroll, point);
    scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, delta)));
    vec![Event::Pointer(scroll)]
}

fn hover(point: Point) -> Vec<Event> {
    vec![Event::Pointer(PointerEvent::new(
        PointerEventKind::Move,
        point,
    ))]
}

/// The page's largest vertical scroll view below the shell's toolbar.
fn page_scroll_point(nodes: &[SemanticsNode], viewport: Rect) -> Option<Point> {
    nodes
        .iter()
        .filter(|node| {
            node.role == SemanticsRole::ScrollView
                && node.bounds.y() >= TOOLBAR_HEIGHT - 1.0
                && node.bounds.height() > 120.0
        })
        .filter_map(|node| {
            let visible = node.bounds.intersection(viewport)?;
            Some((visible.width() * visible.height(), visible))
        })
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, visible)| {
            Point::new(
                visible.x() + visible.width().min(96.0) * 0.5,
                visible.y() + visible.height() * 0.5,
            )
        })
}

fn hover_path(viewport: Rect, steps: usize) -> Vec<Point> {
    let content = Rect::new(
        viewport.x() + 16.0,
        viewport.y() + TOOLBAR_HEIGHT + 16.0,
        (viewport.width() - 32.0).max(1.0),
        (viewport.height() - TOOLBAR_HEIGHT - 32.0).max(1.0),
    );
    let rows = 6;
    let columns = steps.div_ceil(rows);
    (0..steps)
        .map(|index| {
            let row = index / columns;
            let column = if row % 2 == 0 {
                index % columns
            } else {
                columns - 1 - index % columns
            };
            Point::new(
                content.x() + content.width() * (column as f32 + 0.5) / columns as f32,
                content.y() + content.height() * (row as f32 + 0.5) / rows as f32,
            )
        })
        .collect()
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn report(samples: &[Sample], detailed: bool) {
    let mut groups: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
    for sample in samples {
        groups.entry(sample.scenario).or_default().push(sample);
    }
    println!();
    println!(
        "{:<14} {:>5} {:>8} {:>8} {:>8} {:>8} {:>8} {:>6} {:>6} {:>9} {:>7}",
        "scenario", "n", "mean", "p50", "p95", "p99", "max", ">8ms", ">16ms", "work_p95", "frames"
    );
    for (scenario, group) in &groups {
        let mut wall: Vec<f64> = group.iter().map(|sample| sample.wall_ms).collect();
        wall.sort_by(f64::total_cmp);
        let mut work: Vec<f64> = group.iter().map(|sample| sample.work_ms).collect();
        work.sort_by(f64::total_cmp);
        let mean = wall.iter().sum::<f64>() / wall.len().max(1) as f64;
        println!(
            "{:<14} {:>5} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>6} {:>6} {:>9.2} {:>7.2}",
            scenario,
            wall.len(),
            mean,
            percentile(&wall, 0.5),
            percentile(&wall, 0.95),
            percentile(&wall, 0.99),
            wall.last().copied().unwrap_or_default(),
            wall.iter().filter(|value| **value > 8.0).count(),
            wall.iter().filter(|value| **value > 16.7).count(),
            percentile(&work, 0.95),
            group.iter().map(|sample| sample.frames).sum::<u64>() as f64
                / group.len().max(1) as f64,
        );
    }

    if detailed {
        println!();
        println!("mean last-frame phase ms by scenario:");
        for (scenario, group) in &groups {
            let mut totals: BTreeMap<&str, f64> = BTreeMap::new();
            for sample in group {
                for (phase, ms) in &sample.phase_ms {
                    *totals.entry(phase).or_default() += ms;
                }
            }
            let mut totals: Vec<_> = totals.into_iter().collect();
            totals.sort_by(|left, right| right.1.total_cmp(&left.1));
            println!(
                "  {:<10} {}",
                scenario,
                totals
                    .iter()
                    .map(|(phase, total)| format!("{phase}={:.3}", total / group.len() as f64))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    }

    let mut pages: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for sample in samples
        .iter()
        .filter(|sample| matches!(sample.scenario, "scroll" | "hover"))
    {
        pages.entry(&sample.page).or_default().push(sample.wall_ms);
    }
    println!();
    println!(
        "{:<24} {:>8} {:>8} {:>8}  (scroll+hover)",
        "page", "p50", "p95", "max"
    );
    for (page, mut wall) in pages {
        wall.sort_by(f64::total_cmp);
        println!(
            "{:<24} {:>8.2} {:>8.2} {:>8.2}",
            page,
            percentile(&wall, 0.5),
            percentile(&wall, 0.95),
            wall.last().copied().unwrap_or_default()
        );
    }

    let mut slowest: Vec<&Sample> = samples.iter().collect();
    slowest.sort_by(|left, right| right.wall_ms.total_cmp(&left.wall_ms));
    println!();
    println!("slowest inputs:");
    for sample in slowest.iter().take(if detailed { 20 } else { 12 }) {
        println!(
            "  {:>8.2}ms work={:>7.2}ms frames={:<2} {:<12} {:<24} {}",
            sample.wall_ms,
            sample.work_ms,
            sample.frames,
            sample.scenario,
            sample.page,
            sample.phases
        );
    }
    let total: f64 = samples.iter().map(|sample| sample.wall_ms).sum();
    println!();
    println!(
        "total input time {total:.1}ms over {} inputs",
        samples.len()
    );
}

fn main() -> Result<()> {
    let mut detailed = false;
    let mut trace = false;
    let mut page_filter: Option<Vec<String>> = None;
    let mut repeat = 1usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--detailed" => detailed = true,
            "--trace" => trace = true,
            "--pages" => {
                page_filter = args.next().map(|value| {
                    value
                        .split(',')
                        .map(|page| page.trim().to_ascii_lowercase())
                        .collect()
                });
            }
            "--repeat" => {
                repeat = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(1)
                    .max(1);
            }
            other => {
                return Err(sui::Error::new(format!(
                    "unsupported argument `{other}`; supported: --detailed, --trace, --pages a,b, --repeat N"
                )));
            }
        }
    }

    let launched = Instant::now();
    let app = TestApp::builder(sui_demo_app::build_dev_application)
        .vsync(false)
        .visible(true)
        .launch()?;
    app.set_default_timeout(30.0)?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    println!(
        "startup to first settled frame: {:.1}ms",
        launched.elapsed().as_secs_f64() * 1000.0
    );
    if detailed {
        set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);
    }

    let stop = Arc::new(AtomicBool::new(false));
    let trace = trace.then(|| {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let (observed, stop, window_id) = (Arc::clone(&frames), Arc::clone(&stop), window.id());
        thread::spawn(move || {
            let mut last = None;
            while !stop.load(Ordering::Relaxed) {
                if let Some(frame) = sui::diagnostics::window_performance_summary(window_id)
                    && last != Some(frame.frame_index)
                {
                    let at = Instant::now();
                    last = Some(frame.frame_index);
                    let detail = (detailed && frame.total_time_ms > 3.0)
                        .then(|| sui::diagnostics::window_performance_snapshot(window_id))
                        .flatten()
                        .filter(|snapshot| snapshot.frame_index == frame.frame_index)
                        .map(|snapshot| {
                            let mut parts = snapshot
                                .phase_timings
                                .iter()
                                .filter(|sample| sample.duration_ms >= 0.1)
                                .map(|sample| {
                                    format!("{}={:.2}", sample.phase.label(), sample.duration_ms)
                                })
                                .collect::<Vec<_>>();
                            parts.extend(snapshot.widget_timings.iter().take(4).map(|sample| {
                                format!(
                                    "{}:{}={:.2}",
                                    sample.widget_name.rsplit("::").next().unwrap_or_default(),
                                    sample.phase.label(),
                                    sample.duration_ms
                                )
                            }));
                            parts.join(" ")
                        });
                    observed.lock().expect("trace lock").push(TracedFrame {
                        at,
                        index: frame.frame_index,
                        work_ms: frame.total_time_ms,
                        detail,
                    });
                }
                thread::sleep(Duration::from_micros(100));
            }
        });
        frames
    });
    let mut bench = Bench {
        window,
        samples: Vec::new(),
        detailed,
        trace,
    };
    let nodes = bench.nodes()?;
    let viewport = nodes
        .iter()
        .map(|node| node.bounds)
        .max_by(|left, right| {
            (left.width() * left.height()).total_cmp(&(right.width() * right.height()))
        })
        .unwrap_or(Rect::new(0.0, 0.0, 1280.0, 720.0));
    let picker = nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView && node.name.as_deref() == Some("Demo picker")
        })
        .map(|node| node.bounds)
        .unwrap_or(viewport);
    let mut pages: Vec<String> = nodes
        .iter()
        .filter(|node| {
            node.role == SemanticsRole::Button
                && node.bounds.y() >= picker.y()
                && node.bounds.height() > 80.0
        })
        .filter_map(|node| node.name.clone())
        .collect();
    pages.dedup();
    if let Some(filter) = &page_filter {
        pages.retain(|page| {
            let page = page.to_ascii_lowercase();
            filter.iter().any(|prefix| page.starts_with(prefix))
        });
    }
    println!("pages: {}", pages.join(", "));

    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            let open = bench
                .find(SemanticsRole::Button, "Open demo")?
                .ok_or_else(|| sui::Error::new("missing Open demo button"))?;
            bench.timed("picker", page, click(center(open.bounds)))?;
            bench.settle()?;
        }
        // Cards scroll inside the picker; bring this one into view first.
        let mut card = None;
        for _ in 0..40 {
            card = bench
                .find(SemanticsRole::Button, page)?
                .filter(|node| node.bounds.height() > 80.0);
            if let Some(found) = &card
                && found.bounds.y() >= picker.y()
                && found.bounds.y() + found.bounds.height() <= picker.y() + picker.height()
            {
                break;
            }
            let root = bench.window.root();
            for event in scroll(center(picker), -120.0) {
                root.dispatch_event(event)?;
            }
        }
        let card = card.ok_or_else(|| sui::Error::new(format!("missing picker card {page}")))?;
        bench.timed("open", page, click(center(card.bounds)))?;
        bench.settle()?;
        run_page(&mut bench, page, viewport)?;
    }

    for _ in 0..repeat {
        for page in &pages {
            // Browser tabs are buttons named for their page, in the toolbar.
            let tab = bench.nodes()?.into_iter().find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(page.as_str())
                    && node.bounds.y() < TOOLBAR_HEIGHT
            });
            let Some(tab) = tab else {
                println!("skipping switch to {page}: no tab");
                continue;
            };
            bench.timed("switch", page, click(center(tab.bounds)))?;
            bench.settle()?;
            run_page(&mut bench, page, viewport)?;
        }
    }

    if let Some(toggle) = bench.find(SemanticsRole::Switch, "Theme mode")? {
        for _ in 0..THEME_TOGGLES {
            bench.timed("theme", "shell", click(center(toggle.bounds)))?;
            bench.settle()?;
        }
    }

    stop.store(true, Ordering::Relaxed);
    report(&bench.samples, detailed);
    Ok(())
}

fn run_page(bench: &mut Bench, page: &str, viewport: Rect) -> Result<()> {
    let nodes = bench.nodes()?;
    if let Some(point) = page_scroll_point(&nodes, viewport) {
        for step in 0..SCROLL_STEPS {
            let delta = if step < SCROLL_STEPS / 2 {
                SCROLL_DELTA
            } else {
                -SCROLL_DELTA
            };
            bench.timed("scroll", page, scroll(point, delta))?;
            std::thread::sleep(INPUT_INTERVAL);
        }
        bench.settle()?;
    }
    for point in hover_path(viewport, HOVER_STEPS) {
        bench.timed("hover", page, hover(point))?;
        std::thread::sleep(INPUT_INTERVAL);
    }
    // Leave the pointer off the content so hover styles relax.
    bench.timed(
        "hover",
        page,
        hover(Point::new(viewport.x() + 2.0, viewport.y() + 2.0)),
    )?;
    bench.settle()
}
