//! libghostty-vt engine. Every handle here is `!Send`: create and use it on one thread only.

use std::io::Write;

use libghostty_vt::{
    RenderState, Terminal, TerminalOptions,
    render::{CellIterator, Dirty, RowIterator},
    screen::CellWide,
    style::{RgbColor, Underline},
};

use crate::SharedWriter;
use crate::frame::{Attrs, BG, Engine, FG, Frame, LineBuilder};

fn to_rgb(v: u32) -> RgbColor {
    RgbColor {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    }
}

fn from_rgb(c: RgbColor) -> u32 {
    (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
}

pub struct Ghostty {
    term: Terminal<'static, 'static>,
    state: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
    chars: Vec<char>,
    text: String,
}

impl Ghostty {
    pub fn new(cols: u16, rows: u16, writer: SharedWriter) -> Self {
        let mut term = Terminal::new(TerminalOptions {
            cols,
            rows,
            max_scrollback: 10_000,
        })
        .expect("ghostty terminal");
        // Answers to queries (cursor position, device attributes) go back to the shell.
        term.on_pty_write(move |_term, data| {
            let mut w = writer.lock().expect("pty writer");
            let _ = w.write_all(data);
            let _ = w.flush();
        })
        .expect("on_pty_write");
        term.set_default_fg_color(Some(to_rgb(FG)))
            .expect("default fg")
            .set_default_bg_color(Some(to_rgb(BG)))
            .expect("default bg");
        Self {
            term,
            state: RenderState::new().expect("render state"),
            rows: RowIterator::new().expect("row iterator"),
            cells: CellIterator::new().expect("cell iterator"),
            chars: Vec::new(),
            text: String::new(),
        }
    }
}

impl Engine for Ghostty {
    fn feed(&mut self, bytes: &[u8]) {
        self.term.vt_write(bytes);
    }

    fn resize(&mut self, cols: u16, rows: u16, cell_w: u32, cell_h: u32) {
        self.term
            .resize(cols, rows, cell_w, cell_h)
            .expect("ghostty resize");
    }

    fn frame(&mut self) -> Frame {
        let Self {
            term,
            state,
            rows,
            cells,
            chars,
            text,
        } = self;
        let snap = state.update(term).expect("render update");
        let colors = snap.colors().expect("colors");
        let default_fg = from_rgb(colors.foreground);
        let default_bg = from_rgb(colors.background);
        let cursor = if snap.cursor_visible().unwrap_or(false) {
            snap.cursor_viewport().ok().flatten().map(|c| (c.x, c.y))
        } else {
            None
        };
        let mut frame = Frame {
            cols: snap.cols().unwrap_or(0),
            rows: snap.rows().unwrap_or(0),
            lines: Vec::new(),
        };
        let mut row_iter = rows.update(&snap).expect("rows");
        let mut y: u16 = 0;
        while let Some(row) = row_iter.next() {
            let mut lb = LineBuilder::new();
            let mut cell_iter = cells.update(row).expect("cells");
            let mut x: u16 = 0;
            while let Some(cell) = cell_iter.next() {
                let wide = cell.raw_cell().and_then(|c| c.wide());
                if matches!(wide, Ok(CellWide::SpacerTail)) {
                    x += 1;
                    continue;
                }
                let n = cell.graphemes_len().unwrap_or(0);
                text.clear();
                if n == 0 {
                    text.push(' ');
                } else {
                    chars.clear();
                    chars.resize(n, '\0');
                    if cell.graphemes_buf(chars).is_ok() {
                        text.extend(chars.iter());
                    } else {
                        text.push(' ');
                    }
                }
                let mut attrs = Attrs {
                    fg: cell.fg_color().ok().flatten().map_or(default_fg, from_rgb),
                    bg: cell.bg_color().ok().flatten().map_or(default_bg, from_rgb),
                    ..Attrs::DEFAULT
                };
                if cell.has_styling().unwrap_or(false)
                    && let Ok(style) = cell.style()
                {
                    attrs.bold = style.bold;
                    attrs.italic = style.italic;
                    attrs.underline = style.underline != Underline::None;
                    if style.inverse {
                        std::mem::swap(&mut attrs.fg, &mut attrs.bg);
                    }
                }
                lb.push(text, attrs, cursor == Some((x, y)));
                x += 1;
            }
            let _ = row.set_dirty(false);
            frame.lines.push(lb.finish());
            y += 1;
        }
        let _ = snap.set_dirty(Dirty::Clean);
        frame
    }
}
