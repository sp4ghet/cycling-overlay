use activity::{load_fit, load_gpx, metric_present_on_activity, Activity, Metric};
use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use layout::{Layout, MetricCatalog};
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::cell::RefCell;
use std::fs;
use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::Duration;
use tiny_skia::{Color, Pixmap};

use crate::args::RenderArgs;
use crate::ffmpeg::{EncodeOpts, FfmpegWriter};
use crate::pipeline::{FrameScheduler, ReorderBuffer};

/// Progress sink. `Bar` draws an indicatif bar; `Json` emits one JSON
/// line per event to stderr (consumed by the GUI's export pipeline).
enum Progress {
    Bar(ProgressBar),
    Json { total: u64 },
}

impl Progress {
    fn inc(&self, frame: u64) {
        match self {
            Progress::Bar(pb) => pb.inc(1),
            Progress::Json { total } => {
                eprintln!(
                    r#"{{"type":"progress","frame":{},"total":{}}}"#,
                    frame, total
                );
            }
        }
    }
    fn finish(&self) {
        match self {
            Progress::Bar(pb) => pb.finish_and_clear(),
            Progress::Json { .. } => eprintln!(r#"{{"type":"done"}}"#),
        }
    }
}

/// Parse a hex color `#rrggbb` into an opaque `tiny_skia::Color`. Used for
/// chromakey fills. Alpha shorthand (`#rrggbbaa`) is not accepted here — the
/// chromakey must be opaque.
fn parse_chromakey(hex: &str) -> Result<Color> {
    let s = hex
        .strip_prefix('#')
        .ok_or_else(|| anyhow!("chromakey must start with '#': got '{}'", hex))?;
    if s.len() != 6 {
        return Err(anyhow!(
            "chromakey must be #rrggbb (6 hex digits): got '{}'",
            hex
        ));
    }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| anyhow!("bad hex in chromakey"))?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| anyhow!("bad hex in chromakey"))?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| anyhow!("bad hex in chromakey"))?;
    Ok(Color::from_rgba8(r, g, b, 255))
}

/// Everything a render path needs once parsing + validation have succeeded.
pub struct Loaded {
    pub activity: Activity,
    pub layout: Layout,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub fps: u32,
    pub from: Duration,
    pub to: Duration,
    pub warnings: Vec<layout::Warning>,
}

/// Load the activity + layout, apply CLI overrides, validate the layout, and
/// compute the time range. Shared between `--dry-run` and the real render path.
pub fn load_and_validate(args: &RenderArgs) -> Result<Loaded> {
    // 1. Load activity by extension.
    let input_ext = args
        .input
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| anyhow!("input file has no extension: {:?}", args.input))?
        .to_ascii_lowercase();
    let mut activity = match input_ext.as_str() {
        "gpx" => load_gpx(&args.input).with_context(|| format!("loading GPX {:?}", args.input))?,
        "fit" => load_fit(&args.input).with_context(|| format!("loading FIT {:?}", args.input))?,
        other => {
            return Err(anyhow!(
                "unsupported input extension '{}'; use .gpx or .fit",
                other
            ))
        }
    };

    // 2. Prepare derived metrics (distance, speed, gradient, elev-gain, etc.).
    activity.prepare();

    // 3. Load and parse layout.
    let layout_text = fs::read_to_string(&args.layout)
        .with_context(|| format!("reading layout {:?}", args.layout))?;
    let mut layout: Layout = serde_json::from_str(&layout_text)
        .with_context(|| format!("parsing layout {:?}", args.layout))?;

    // 4. Apply CLI overrides (size + fps) before validation so that rect
    //    bounds checks use the final canvas dimensions.
    if let Some((w, h)) = args.size {
        layout.canvas.width = w;
        layout.canvas.height = h;
    }
    if let Some(fps) = args.fps {
        layout.canvas.fps = fps;
    }

    // 5. Build metric catalog.
    let known_owned: Vec<&'static str> = Metric::ALL.iter().map(|m| m.as_str()).collect();
    let present_owned: Vec<&'static str> = Metric::ALL
        .iter()
        .filter(|m| metric_present_on_activity(**m, &activity.samples))
        .map(|m| m.as_str())
        .collect();
    let catalog = MetricCatalog {
        known: &known_owned,
        present: &present_owned,
    };

    // 6. Validate.
    let warnings = layout
        .validate(&catalog)
        .with_context(|| "validating layout")?;

    // 7. Compute from/to with defaults and sanity-check.
    let activity_duration = activity.duration();
    let from = args.from.unwrap_or(Duration::ZERO);
    let to = args.to.unwrap_or(activity_duration);
    if from > to {
        return Err(anyhow!("--from ({:?}) is after --to ({:?})", from, to));
    }
    if from > activity_duration {
        return Err(anyhow!(
            "--from ({:?}) exceeds activity duration ({:?})",
            from,
            activity_duration
        ));
    }

    Ok(Loaded {
        canvas_width: layout.canvas.width,
        canvas_height: layout.canvas.height,
        fps: layout.canvas.fps,
        activity,
        layout,
        from,
        to,
        warnings,
    })
}

/// Per-thread scratch buffers reused across frames.
struct Scratch {
    pixmap: Pixmap,
    text: render::TextCtx,
    width: u32,
    height: u32,
}

thread_local! {
    static SCRATCH: RefCell<Option<Scratch>> = const { RefCell::new(None) };
}

/// Run the parallel render pipeline to produce an overlay video.
///
/// The pipeline has three stages:
///   1. A `FrameScheduler` emits `(idx, t)` pairs.
///   2. Rayon workers render each frame into a thread-local `Pixmap` and send
///      `(idx, bytes)` through a bounded mpsc channel.
///   3. A dedicated flusher thread pushes incoming frames into a `ReorderBuffer`,
///      drains contiguous runs, and writes them to ffmpeg in order.
///
/// The bounded channel provides back-pressure: when the flusher is slower than
/// rendering, workers block on `send`, preventing unbounded memory growth.
pub fn render(args: &RenderArgs) -> Result<()> {
    // Hint rayon's thread count if user passed --threads.
    // Best-effort: silently ignore if a pool is already built.
    if let Some(n) = args.threads {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global();
    }

    let loaded = load_and_validate(args)?;

    // Print warnings up front (they shouldn't block rendering).
    for w in &loaded.warnings {
        match w {
            layout::Warning::MetricAbsent { widget_id, metric } => {
                eprintln!(
                    "warning: widget '{}' references metric '{}' absent from activity",
                    widget_id, metric
                );
            }
        }
    }

    let total = {
        let sch = FrameScheduler::new(loaded.from, loaded.to, loaded.fps);
        sch.total_frames()
    };
    if total == 0 {
        return Err(anyhow!("nothing to render: time range is empty"));
    }

    // Progress sink: indicatif bar by default, or JSON lines for the GUI.
    let progress = if args.progress_json {
        Progress::Json { total }
    } else {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} frames | ETA {eta}")
                .unwrap(),
        );
        Progress::Bar(pb)
    };

    // Bounded channel; capacity equals reorder-buffer cap so back-pressure
    // aligns with the buffer. 128 × 8.3 MB ≈ 1 GB peak at 1080p RGBA.
    const BUFFER_CAP: usize = 128;
    let (tx, rx) = sync_channel::<(u64, Vec<u8>)>(BUFFER_CAP);

    // Derive the background color for render_frame. ProRes keeps alpha;
    // non-ProRes codecs drop alpha and expect a solid chromakey fill.
    let background = if args.codec.needs_chromakey() {
        parse_chromakey(&args.chromakey)?
    } else {
        Color::TRANSPARENT
    };

    // Spawn flusher thread. It owns the FfmpegWriter and ReorderBuffer.
    let out_path = args.output.clone();
    let (w, h, fps) = (loaded.canvas_width, loaded.canvas_height, loaded.fps);
    let encode_opts = EncodeOpts {
        codec: args.codec,
        qscale: args.qscale,
        crf: args.crf,
    };
    // Progress is moved into the flusher closure; nothing else reads it.
    // A local counter tracks the absolute frame number so the JSON sink can
    // report `frame=N` instead of a "+1" increment.
    let flusher = thread::spawn(move || -> anyhow::Result<()> {
        let mut writer = FfmpegWriter::new(w, h, fps, encode_opts, &out_path)?;
        let mut buf = ReorderBuffer::new(BUFFER_CAP);
        let mut done: u64 = 0;
        while let Ok((idx, bytes)) = rx.recv() {
            buf.push(idx, bytes);
            for ready in buf.drain_ready() {
                writer
                    .write_frame(&ready)
                    .map_err(|e| anyhow!("ffmpeg write_frame failed: {}", e))?;
                done += 1;
                progress.inc(done);
            }
        }
        // Channel closed — drain anything left (shouldn't have gaps if
        // every scheduled frame was sent).
        for ready in buf.drain_ready() {
            writer
                .write_frame(&ready)
                .map_err(|e| anyhow!("ffmpeg write_frame failed: {}", e))?;
            done += 1;
            progress.inc(done);
        }
        progress.finish();
        writer.finish()
    });

    // Render in parallel. We borrow `layout` + `activity` for the duration
    // of the parallel section; `par_bridge().try_for_each_init` is a blocking
    // call, so these references remain valid.
    let layout = &loaded.layout;
    let activity = &loaded.activity;
    let sch = FrameScheduler::new(loaded.from, loaded.to, loaded.fps);

    let render_result: Result<()> = sch.par_bridge().try_for_each_init(
        || tx.clone(),
        |sender, (idx, t)| -> Result<()> {
            SCRATCH.with(|cell| -> Result<()> {
                let mut borrow = cell.borrow_mut();
                // Lazily build the per-thread scratch or rebuild if the
                // canvas size changed (should not happen within one run).
                let need_new = match borrow.as_ref() {
                    None => true,
                    Some(s) => s.width != w || s.height != h,
                };
                if need_new {
                    *borrow = Some(Scratch {
                        pixmap: Pixmap::new(w, h)
                            .ok_or_else(|| anyhow!("failed to allocate {}x{} pixmap", w, h))?,
                        text: render::TextCtx::new(),
                        width: w,
                        height: h,
                    });
                }
                let scratch = borrow.as_mut().unwrap();
                render::render_frame(
                    layout,
                    activity,
                    t,
                    &mut scratch.text,
                    &mut scratch.pixmap,
                    background,
                )?;
                let bytes = scratch.pixmap.data().to_vec();
                sender
                    .send((idx, bytes))
                    .map_err(|e| anyhow!("flusher channel closed unexpectedly: {}", e))?;
                Ok(())
            })
        },
    );

    // Drop the original sender so the flusher observes channel close once all
    // worker-cloned senders are dropped.
    drop(tx);

    // Join the flusher regardless of render_result so we don't leak the thread.
    // The flusher is responsible for calling `progress.finish()` internally,
    // since it owns the `Progress` sink.
    let flush_result = flusher
        .join()
        .map_err(|_| anyhow!("flusher thread panicked"))?;

    // Propagate errors. Prefer the render error since it's the upstream cause.
    render_result?;
    flush_result?;
    Ok(())
}

/// Load + validate, then print a summary to stdout. Never writes any output file.
pub fn dry_run(args: &RenderArgs) -> Result<()> {
    let l = load_and_validate(args)?;
    let range = l.to - l.from;
    let frame_count = (range.as_secs_f64() * l.fps as f64).round() as u64;

    println!(
        "Activity: {}s, {} samples",
        l.activity.duration().as_secs(),
        l.activity.samples.len()
    );
    let available: Vec<&str> = Metric::ALL
        .iter()
        .filter(|m| metric_present_on_activity(**m, &l.activity.samples))
        .map(|m| m.as_str())
        .collect();
    println!("Available metrics: {}", available.join(", "));
    println!("Layout: {} widgets", l.layout.widgets.len());
    println!(
        "Time range: {:?} -> {:?} ({}s)",
        l.from,
        l.to,
        range.as_secs()
    );
    println!(
        "Output: {:?}, {}x{} @ {} fps ({} frames)",
        args.output, l.canvas_width, l.canvas_height, l.fps, frame_count
    );
    if !l.warnings.is_empty() {
        println!("Warnings:");
        for w in &l.warnings {
            match w {
                layout::Warning::MetricAbsent { widget_id, metric } => {
                    println!(
                        "  widget '{}' refs metric '{}' absent in activity",
                        widget_id, metric
                    );
                }
            }
        }
    }
    Ok(())
}
