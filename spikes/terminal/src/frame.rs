//! Engine-neutral cell snapshot sent from the emulator thread to the GPUI thread.

use std::ops::Range;

/// Default background (design token `#18181A`).
pub const BG: u32 = 0x18181A;
/// Default foreground (design token `#ECEAE4`).
pub const FG: u32 = 0xECEAE4;
/// Cursor block colour.
pub const CURSOR: u32 = 0xECEAE4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Attrs {
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Attrs {
    pub const DEFAULT: Attrs = Attrs {
        fg: FG,
        bg: BG,
        bold: false,
        italic: false,
        underline: false,
    };
}

/// One screen row: its text plus styled byte ranges (default-styled ranges are omitted).
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub text: String,
    pub runs: Vec<(Range<usize>, Attrs)>,
}

#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub cols: u16,
    pub rows: u16,
    pub lines: Vec<Line>,
}

/// Builds a [`Line`] cell by cell, merging neighbouring cells with equal attributes.
pub struct LineBuilder {
    line: Line,
    run_start: usize,
    run_attrs: Attrs,
}

impl LineBuilder {
    pub fn new() -> Self {
        Self {
            line: Line::default(),
            run_start: 0,
            run_attrs: Attrs::DEFAULT,
        }
    }

    /// Append one cell. `cursor` draws it as a block cursor.
    pub fn push(&mut self, text: &str, mut attrs: Attrs, cursor: bool) {
        if cursor {
            attrs.bg = CURSOR;
            attrs.fg = BG;
        }
        if attrs != self.run_attrs {
            self.flush();
            self.run_attrs = attrs;
        }
        self.line.text.push_str(text);
    }

    fn flush(&mut self) {
        let end = self.line.text.len();
        if end > self.run_start && self.run_attrs != Attrs::DEFAULT {
            self.line.runs.push((self.run_start..end, self.run_attrs));
        }
        self.run_start = end;
    }

    pub fn finish(mut self) -> Line {
        self.flush();
        self.line
    }
}

/// Standard xterm 256-colour palette (used by the alacritty engine; ghostty has its own).
pub fn xterm_palette() -> [u32; 256] {
    const BASE: [u32; 16] = [
        0x1D1F21, 0xCC6666, 0xB5BD68, 0xF0C674, 0x81A2BE, 0xB294BB, 0x8ABEB7, 0xC5C8C6, 0x666666,
        0xD54E53, 0xB9CA4A, 0xE7C547, 0x7AA6DA, 0xC397D8, 0x70C0B1, 0xEAEAEA,
    ];
    let mut p = [0u32; 256];
    p[..16].copy_from_slice(&BASE);
    let level = |v: u32| if v == 0 { 0 } else { 55 + v * 40 };
    for i in 0..216u32 {
        let (r, g, b) = (level(i / 36), level((i / 6) % 6), level(i % 6));
        p[16 + i as usize] = (r << 16) | (g << 8) | b;
    }
    for i in 0..24u32 {
        let v = 8 + i * 10;
        p[232 + i as usize] = (v << 16) | (v << 8) | v;
    }
    p
}

/// The pieces every terminal library must provide for the spike.
pub trait Engine {
    fn feed(&mut self, bytes: &[u8]);
    fn resize(&mut self, cols: u16, rows: u16, cell_w: u32, cell_h: u32);
    fn frame(&mut self) -> Frame;
}

/// Baseline engine: discards bytes. Measures the PTY and plumbing without a parser.
pub struct NullEngine;

impl Engine for NullEngine {
    fn feed(&mut self, _bytes: &[u8]) {}
    fn resize(&mut self, _cols: u16, _rows: u16, _cell_w: u32, _cell_h: u32) {}
    fn frame(&mut self) -> Frame {
        Frame::default()
    }
}
