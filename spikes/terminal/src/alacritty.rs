//! alacritty_terminal engine (the fallback candidate).

use std::io::Write;

use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::Dimensions,
    term::{Config, cell::Flags},
    vte::ansi::{Color, CursorShape, NamedColor, Processor, StdSyncHandler},
};

use crate::SharedWriter;
use crate::frame::{Attrs, BG, Engine, FG, Frame, LineBuilder, xterm_palette};

pub struct Listener(SharedWriter);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            let mut w = self.0.lock().expect("pty writer");
            let _ = w.write_all(text.as_bytes());
            let _ = w.flush();
        }
    }
}

struct Size {
    cols: usize,
    rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct Alacritty {
    term: Term<Listener>,
    parser: Processor<StdSyncHandler>,
    palette: [u32; 256],
    buf: String,
}

impl Alacritty {
    pub fn new(cols: u16, rows: u16, writer: SharedWriter) -> Self {
        let size = Size {
            cols: cols.into(),
            rows: rows.into(),
        };
        let config = Config {
            scrolling_history: 10_000,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &size, Listener(writer)),
            parser: Processor::new(),
            palette: xterm_palette(),
            buf: String::new(),
        }
    }

    fn resolve(&self, color: Color, overrides: &alacritty_terminal::term::color::Colors) -> u32 {
        let rgb = |c: alacritty_terminal::vte::ansi::Rgb| {
            (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
        };
        match color {
            Color::Spec(c) => rgb(c),
            Color::Indexed(i) => overrides[i as usize].map_or(self.palette[i as usize], rgb),
            Color::Named(n) => {
                let i = n as usize;
                if let Some(c) = overrides[i] {
                    rgb(c)
                } else if i < 16 {
                    self.palette[i]
                } else if n == NamedColor::Background {
                    BG
                } else {
                    FG
                }
            }
        }
    }
}

impl Engine for Alacritty {
    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn resize(&mut self, cols: u16, rows: u16, _cell_w: u32, _cell_h: u32) {
        self.term.resize(Size {
            cols: cols.into(),
            rows: rows.into(),
        });
    }

    fn frame(&mut self) -> Frame {
        let content = self.term.renderable_content();
        let offset = content.display_offset as i32;
        let cursor = (content.cursor.shape != CursorShape::Hidden).then(|| {
            (
                content.cursor.point.column.0,
                (content.cursor.point.line.0 + offset) as usize,
            )
        });
        let colors = content.colors;
        let rows = self.term.screen_lines();
        let cols = self.term.columns();
        let mut frame = Frame {
            cols: cols as u16,
            rows: rows as u16,
            lines: Vec::with_capacity(rows),
        };
        let mut lb = LineBuilder::new();
        let mut current = 0usize;
        for indexed in self.term.renderable_content().display_iter {
            let y = (indexed.point.line.0 + offset) as usize;
            while y > current {
                frame
                    .lines
                    .push(std::mem::replace(&mut lb, LineBuilder::new()).finish());
                current += 1;
            }
            let cell = indexed.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let mut attrs = Attrs {
                fg: self.resolve(cell.fg, colors),
                bg: self.resolve(cell.bg, colors),
                bold: cell.flags.contains(Flags::BOLD),
                italic: cell.flags.contains(Flags::ITALIC),
                underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
            };
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut attrs.fg, &mut attrs.bg);
            }
            self.buf.clear();
            self.buf.push(cell.c);
            if let Some(zw) = cell.zerowidth() {
                self.buf.extend(zw.iter());
            }
            lb.push(
                &self.buf,
                attrs,
                cursor == Some((indexed.point.column.0, y)),
            );
        }
        frame.lines.push(lb.finish());
        frame
    }
}
