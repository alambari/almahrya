//! Throwaway spike: `$SHELL -l` in a PTY -> terminal library on its own thread ->
//! cell snapshots over a channel -> grid drawn in a GPUI window (JetBrains Mono).
//!
//! Usage:
//!   terminal-spike [--engine ghostty|alacritty|none] [--demo | --bench FILE | --idle] [--headless]
//!
//! `--demo` types a scripted session (colours, `ls -G`, vim, window resizes) and takes
//! screenshots when `SPIKE_SHOT_DIR` and `SPIKE_WINID` (a tool printing the window id) are set.
//! `--bench FILE` runs `cat FILE` and prints the time until the engine has parsed it all.

mod alacritty;
mod frame;
mod ghostty;

use std::io::{Read, Write};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use gpui::{
    App, AsyncWindowContext, Bounds, Context, FontStyle, FontWeight, HighlightStyle, IntoElement,
    ParentElement, Pixels, Render, Size, Styled, StyledText, UnderlineStyle, WeakEntity, Window,
    WindowBounds, WindowOptions, div, font, prelude::*, px, rgb, size,
};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::frame::{Attrs, BG, Engine, FG, Frame, NullEngine};

pub type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

const FONT: &str = "JetBrains Mono";
const FONT_SIZE: f32 = 13.0;
const LINE_HEIGHT: f32 = 17.0;
const PAD: f32 = 8.0;
/// Output printed by `echo $((40+2))MARK`; the typed command itself never contains it.
const MARKER: &[u8] = b"42MARK";
/// Number of `TermView::render` calls (printed on quit).
static RENDERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EngineKind {
    Ghostty,
    Alacritty,
    None,
}

#[derive(Clone, Debug)]
enum Mode {
    Interactive,
    Demo,
    Idle,
    Bench(String),
}

enum ToEmu {
    Bytes(Vec<u8>),
    Resize {
        cols: u16,
        rows: u16,
        cell_w: u32,
        cell_h: u32,
    },
    Eof,
    Shutdown,
}

enum ToUi {
    Frame(Arc<Frame>),
    ResizeGrid(u16, u16),
    /// Force one draw (see `TermView::nudge`).
    Redraw,
    Quit,
}

#[derive(Default)]
struct Bench {
    armed_at: Mutex<Option<Instant>>,
    done: AtomicBool,
}

fn main() {
    let mut engine = EngineKind::Ghostty;
    let mut mode = Mode::Interactive;
    let mut headless = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--engine" => {
                engine = match args.next().as_deref() {
                    Some("ghostty") => EngineKind::Ghostty,
                    Some("alacritty") => EngineKind::Alacritty,
                    Some("none") => EngineKind::None,
                    other => panic!("unknown engine {other:?}"),
                }
            }
            "--demo" => mode = Mode::Demo,
            "--idle" => mode = Mode::Idle,
            "--bench" => mode = Mode::Bench(args.next().expect("--bench FILE")),
            "--headless" => headless = true,
            "--parse" => {
                parse_only(engine, &args.next().expect("--parse FILE"));
                return;
            }
            other => panic!("unknown argument {other}"),
        }
    }
    eprintln!("engine={engine:?} mode={mode:?} headless={headless}");

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut cmd = CommandBuilder::new(&shell);
    cmd.arg("-l");
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.cwd(std::env::current_dir().expect("cwd"));
    let child = pair.slave.spawn_command(cmd).expect("spawn shell");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("reader");
    let writer: SharedWriter = Arc::new(Mutex::new(pair.master.take_writer().expect("writer")));

    // Bounded: a slow parser applies back-pressure to the PTY like a real terminal.
    let (emu_tx, emu_rx) = mpsc::sync_channel::<ToEmu>(256);
    {
        let tx = emu_tx.clone();
        thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || {
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => {
                            let _ = tx.send(ToEmu::Eof);
                            break;
                        }
                        Ok(n) => {
                            if tx.send(ToEmu::Bytes(buf[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .expect("reader thread");
    }

    let (ui_tx, ui_rx) = async_channel::unbounded::<ToUi>();
    let bench = Arc::new(Bench::default());
    let emu = {
        let writer = writer.clone();
        let bench = bench.clone();
        let ui_tx = (!headless).then(|| ui_tx.clone());
        let master = pair.master;
        thread::Builder::new()
            .name("emulator".into())
            .spawn(move || emulator(engine, master, child, writer, emu_rx, ui_tx, bench))
            .expect("emulator thread")
    };

    let script = {
        let writer = writer.clone();
        let bench = bench.clone();
        let ui_tx = ui_tx.clone();
        let emu_tx = emu_tx.clone();
        move || run_script(mode, writer, bench, ui_tx, emu_tx, headless)
    };

    if headless {
        script();
        let _ = emu.join();
        return;
    }
    thread::Builder::new()
        .name("script".into())
        .spawn(script)
        .expect("script thread");
    run_ui(emu_tx, ui_rx);
}

/// Feeds a file straight into the engine (no PTY, no UI): raw parser throughput.
/// `--engine` must come before `--parse`.
fn parse_only(kind: EngineKind, file: &str) {
    let data = std::fs::read(file).expect("read file");
    let sink: SharedWriter = Arc::new(Mutex::new(Box::new(std::io::sink())));
    let mut engine: Box<dyn Engine> = match kind {
        EngineKind::Ghostty => Box::new(ghostty::Ghostty::new(80, 24, sink)),
        EngineKind::Alacritty => Box::new(alacritty::Alacritty::new(80, 24, sink)),
        EngineKind::None => Box::new(NullEngine),
    };
    let start = Instant::now();
    for chunk in data.chunks(64 * 1024) {
        engine.feed(chunk);
    }
    let parsed = start.elapsed().as_secs_f64();
    let t = Instant::now();
    let frame = engine.frame();
    let snap_us = t.elapsed().as_micros();
    println!(
        "PARSE engine={kind:?} bytes={} seconds={parsed:.3} mb_per_s={:.0} snapshot_us={snap_us} rows={}",
        data.len(),
        data.len() as f64 / 1048576.0 / parsed,
        frame.lines.len()
    );
}

/// Owns the terminal library (its handles are `!Send`, so it is created here).
fn emulator(
    kind: EngineKind,
    master: Box<dyn MasterPty + Send>,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: SharedWriter,
    rx: mpsc::Receiver<ToEmu>,
    ui: Option<async_channel::Sender<ToUi>>,
    bench: Arc<Bench>,
) {
    let mut engine: Box<dyn Engine> = match kind {
        EngineKind::Ghostty => Box::new(ghostty::Ghostty::new(80, 24, writer)),
        EngineKind::Alacritty => Box::new(alacritty::Alacritty::new(80, 24, writer)),
        EngineKind::None => Box::new(NullEngine),
    };
    let frame_interval = Duration::from_millis(16);
    let mut last_frame = Instant::now() - frame_interval;
    let mut dirty = true;
    let mut tail: Vec<u8> = Vec::new();
    let mut frames: u64 = 0;
    let mut bench_bytes: u64 = 0;
    loop {
        let msg = if dirty {
            match rx.recv_timeout(Duration::from_millis(4)) {
                Ok(m) => Some(m),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => break,
            }
        };
        match msg {
            Some(ToEmu::Bytes(bytes)) => {
                engine.feed(&bytes);
                dirty = true;
                if bench.armed_at.lock().expect("bench").is_some() {
                    bench_bytes += bytes.len() as u64;
                }
                if bench.armed_at.lock().expect("bench").is_some()
                    && !bench.done.load(Ordering::SeqCst)
                    && contains_marker(&tail, &bytes)
                {
                    let started = bench.armed_at.lock().expect("bench").expect("armed");
                    let secs = started.elapsed().as_secs_f64();
                    println!(
                        "BENCH engine={kind:?} seconds={secs:.3} bytes={bench_bytes} frames_sent={frames}"
                    );
                    bench.done.store(true, Ordering::SeqCst);
                }
                let keep = bytes.len().min(MARKER.len());
                tail.clear();
                tail.extend_from_slice(&bytes[bytes.len() - keep..]);
            }
            Some(ToEmu::Resize {
                cols,
                rows,
                cell_w,
                cell_h,
            }) => {
                engine.resize(cols, rows, cell_w, cell_h);
                let _ = master.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: cols * cell_w as u16,
                    pixel_height: rows * cell_h as u16,
                });
                eprintln!("resized grid to {cols}x{rows}");
                dirty = true;
            }
            Some(ToEmu::Eof) => eprintln!("pty closed"),
            Some(ToEmu::Shutdown) => break,
            None => {}
        }
        if dirty && last_frame.elapsed() >= frame_interval {
            let f = engine.frame();
            frames += 1;
            if let Some(ui) = &ui {
                let _ = ui.try_send(ToUi::Frame(Arc::new(f)));
            }
            last_frame = Instant::now();
            dirty = false;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    eprintln!("emulator stopped, shell killed");
}

fn contains_marker(tail: &[u8], bytes: &[u8]) -> bool {
    let find = |hay: &[u8]| hay.windows(MARKER.len()).any(|w| w == MARKER);
    if find(bytes) {
        return true;
    }
    let mut edge = tail.to_vec();
    edge.extend_from_slice(&bytes[..bytes.len().min(MARKER.len())]);
    find(&edge)
}

fn run_script(
    mode: Mode,
    writer: SharedWriter,
    bench: Arc<Bench>,
    ui: async_channel::Sender<ToUi>,
    emu: mpsc::SyncSender<ToEmu>,
    headless: bool,
) {
    let typ = |s: &str| {
        let mut w = writer.lock().expect("writer");
        w.write_all(s.as_bytes()).expect("type");
        w.flush().expect("flush");
    };
    let wait = |ms: u64| thread::sleep(Duration::from_millis(ms));
    let shot = |name: &str| {
        let _ = ui.send_blocking(ToUi::Redraw);
        wait(300);
        take_screenshot(name);
    };
    let quit = || {
        let _ = emu.send(ToEmu::Shutdown);
        wait(200);
        if headless {
            return;
        }
        let _ = ui.send_blocking(ToUi::Quit);
    };
    match mode {
        Mode::Interactive => {}
        Mode::Idle => {
            wait(10_000);
            quit();
        }
        Mode::Demo => {
            wait(3000);
            typ("clear\r");
            wait(500);
            typ(concat!(
                "printf '\\033[31mred \\033[32mgreen \\033[33myellow \\033[34mblue ",
                "\\033[35mmagenta \\033[36mcyan \\033[1mbold\\033[0m \\033[3mitalic\\033[0m ",
                "\\033[4munder\\033[0m \\033[7minverse\\033[0m\\n",
                "\\033[38;5;208m256:208 \\033[38;2;111;124;242mtrue:6F7CF2 ",
                "\\033[48;5;22m bg22 \\033[0m wide:\u{65e5}\u{672c} box:\u{2500}\u{2502}\u{250c}\\n'; ",
                "ls -G /\r"
            ));
            wait(1500);
            shot("1-colors-ls");
            typ("vim -u NONE -N -c 'syntax on' -c 'set number laststatus=2 ruler' src/frame.rs\r");
            wait(2000);
            shot("2-vim-80x24");
            let _ = ui.send_blocking(ToUi::ResizeGrid(120, 36));
            wait(2000);
            shot("3-vim-120x36");
            typ("\x1b:q!\r");
            wait(800);
            typ("clear; printf 'L%.0s' {1..150}; printf '|end\\n'\r");
            wait(1200);
            shot("4-long-line-120");
            let _ = ui.send_blocking(ToUi::ResizeGrid(80, 24));
            wait(2000);
            shot("5-reflow-80");
            quit();
        }
        Mode::Bench(file) => {
            wait(3000);
            typ("clear\r");
            wait(1000);
            *bench.armed_at.lock().expect("bench") = Some(Instant::now());
            typ(&format!("cat {file}; echo $((40+2))MARK\r"));
            while !bench.done.load(Ordering::SeqCst) {
                wait(20);
            }
            wait(500);
            if !headless {
                shot("bench-end");
            }
            quit();
        }
    }
}

fn take_screenshot(name: &str) {
    let (Ok(dir), Ok(winid)) = (
        std::env::var("SPIKE_SHOT_DIR"),
        std::env::var("SPIKE_WINID"),
    ) else {
        return;
    };
    let id = Command::new(winid)
        .arg("terminal-spike")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if id.is_empty() {
        eprintln!("screenshot {name}: window id not found");
        return;
    }
    // The test machine's display sleeps while the session is locked; window capture then
    // fails ("could not create image from window"). Declare user activity first.
    let mut awake = Command::new("caffeinate")
        .args(["-u", "-t", "2"])
        .spawn()
        .ok();
    thread::sleep(Duration::from_millis(800));
    let path = format!("{dir}/{name}.png");
    let ok = Command::new("screencapture")
        .args(["-x", "-o", "-l", &id, &path])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if let Some(c) = awake.as_mut() {
        let _ = c.wait();
    }
    eprintln!("screenshot {name}: {path} ok={ok}");
}

// ---------------------------------------------------------------- GPUI side

struct TermView {
    frame: Option<Arc<Frame>>,
    cell: Size<Pixels>,
    grid: (u16, u16),
    emu: mpsc::SyncSender<ToEmu>,
    nudged: bool,
}

impl TermView {
    /// When the macOS session is locked (or the window is covered) AppKit marks the window
    /// occluded, GPUI stops its display link and `cx.notify()` no longer leads to a draw.
    /// A window resize still draws synchronously, so toggle the width by one pixel (the
    /// window has one pixel of slack, so the grid size does not change).
    fn nudge(&mut self, window: &mut Window) {
        let mut s = window.viewport_size();
        s.width += if self.nudged { px(-1.0) } else { px(1.0) };
        self.nudged = !self.nudged;
        window.resize(s);
    }

    fn window_size_for(&self, cols: u16, rows: u16) -> Size<Pixels> {
        size(
            px(f32::from(cols) * self.cell.width.as_f32() + 2.0 * PAD + 1.0),
            px(f32::from(rows) * LINE_HEIGHT + 2.0 * PAD),
        )
    }
}

fn highlight(a: &Attrs) -> HighlightStyle {
    HighlightStyle {
        color: Some(rgb(a.fg).into()),
        background_color: (a.bg != BG).then(|| rgb(a.bg).into()),
        font_weight: a.bold.then_some(FontWeight::BOLD),
        font_style: a.italic.then_some(FontStyle::Italic),
        underline: a.underline.then(|| UnderlineStyle {
            thickness: px(1.0),
            color: None,
            wavy: false,
        }),
        ..Default::default()
    }
}

impl Render for TermView {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        RENDERS.fetch_add(1, Ordering::Relaxed);
        let vp = window.viewport_size();
        let cols = ((vp.width.as_f32() - 2.0 * PAD) / self.cell.width.as_f32())
            .floor()
            .max(2.0) as u16;
        let rows = ((vp.height.as_f32() - 2.0 * PAD) / LINE_HEIGHT)
            .floor()
            .max(2.0) as u16;
        if (cols, rows) != self.grid {
            self.grid = (cols, rows);
            let _ = self.emu.try_send(ToEmu::Resize {
                cols,
                rows,
                cell_w: self.cell.width.as_f32().round() as u32,
                cell_h: LINE_HEIGHT as u32,
            });
        }
        if std::env::var_os("SPIKE_DEBUG").is_some() {
            eprintln!(
                "render {:?} grid {:?}",
                self.frame.as_ref().map(|f| (f.cols, f.rows)),
                self.grid
            );
        }
        let lines = self
            .frame
            .as_ref()
            .map(|f| f.lines.clone())
            .unwrap_or_default();
        div()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(FG))
            .p(px(PAD))
            .font_family(FONT)
            .text_size(px(FONT_SIZE))
            .line_height(px(LINE_HEIGHT))
            .flex()
            .flex_col()
            .children(lines.into_iter().map(|line| {
                let runs: Vec<_> = line
                    .runs
                    .iter()
                    .map(|(r, a)| (r.clone(), highlight(a)))
                    .collect();
                div()
                    .h(px(LINE_HEIGHT))
                    .whitespace_nowrap()
                    .child(StyledText::new(line.text).with_highlights(runs))
            }))
    }
}

fn run_ui(emu: mpsc::SyncSender<ToEmu>, ui_rx: async_channel::Receiver<ToUi>) {
    gpui_platform::application().run(move |cx: &mut App| {
        let ts = cx.text_system();
        let font_id = ts.resolve_font(&font(FONT));
        let advance = ts.advance(font_id, px(FONT_SIZE), 'm').expect("advance");
        let cell = size(advance.width, px(LINE_HEIGHT));
        let probe = TermView {
            frame: None,
            cell,
            grid: (80, 24),
            emu: emu.clone(),
            nudged: false,
        };
        let force_draw = std::env::var_os("SPIKE_FORCE_DRAW").is_some();
        let bounds = Bounds::centered(None, probe.window_size_for(80, 24), cx);
        eprintln!("cell {:?}, window {:?}", cell, bounds.size);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx: &mut Context<TermView>| {
                    cx.spawn_in(
                        window,
                        async move |this: WeakEntity<TermView>, cx: &mut AsyncWindowContext| {
                            while let Ok(first) = ui_rx.recv().await {
                                let mut batch = vec![first];
                                while let Ok(m) = ui_rx.try_recv() {
                                    batch.push(m);
                                }
                                let mut latest = None;
                                for msg in batch {
                                    match msg {
                                        ToUi::Frame(f) => latest = Some(f),
                                        ToUi::ResizeGrid(c, r) => {
                                            let _ = this.update_in(cx, |v, window, _| {
                                                window.resize(v.window_size_for(c, r));
                                            });
                                        }
                                        ToUi::Redraw => {
                                            let _ =
                                                this.update_in(cx, |v, window, _| v.nudge(window));
                                        }
                                        ToUi::Quit => {
                                            eprintln!(
                                                "renders={}",
                                                RENDERS.load(Ordering::Relaxed)
                                            );
                                            let _ = cx.update(|_, app| app.quit());
                                            return;
                                        }
                                    }
                                }
                                if let Some(f) = latest {
                                    if std::env::var_os("SPIKE_DEBUG").is_some() {
                                        eprintln!("ui frame {}x{}", f.cols, f.rows);
                                    }
                                    let _ = this.update_in(cx, |v, window, cx| {
                                        v.frame = Some(f);
                                        cx.notify();
                                        if force_draw {
                                            v.nudge(window);
                                        }
                                    });
                                }
                            }
                        },
                    )
                    .detach();
                    probe
                })
            },
        )
        .expect("open window");
        cx.activate(true);
    });
}
