# Spike: terminal cells from libghostty-vt drawn in GPUI

Date: 2026-11-14. Source: `spikes/terminal/` (throwaway, outside the main workspace).

## Decision: GO libghostty-vt

libghostty-vt built once Zig 0.15.2 was on PATH. It rendered colours, bold, italic, underline,
inverse, 256-colour, truecolour and wide CJK characters correctly. vim ran full screen, a window
resize resized vim, and a long line reflowed when the window shrank. On a 50 MB `cat` it was the
fastest engine and used the least memory: its parser is 4.5 times faster than alacritty_terminal on
plain text, and it adds about 1.4 MB against about 20 MB for alacritty_terminal. The costs are a
Zig toolchain pinned to 0.15.x, a 48 s native build step that clones the Ghostty repository, and a
pre-1.0 API. alacritty_terminal also passed every check and stays the fallback.

## Versions and toolchain

| Item | Version |
| --- | --- |
| `libghostty-vt` / `libghostty-vt-sys` | `=0.2.2`. The build script clones ghostty-org/ghostty at commit `a887df42c56f6de86c0fe6da9c4eeca37931e083` |
| `alacritty_terminal` | `=0.26.0` (uses `vte` 0.15) |
| `portable-pty` | `=0.9.0` |
| `async-channel` | `=2.5.0` (terminal thread to GPUI thread) |
| GPUI | `gpui-pre` / `gpui-pre-platform` `=0.3.12` with `font-kit` and `runtime_shaders`, the same lines as `docs/superpowers/spikes/gpui.md` |
| Zig | **0.15.2** from Homebrew `zig@0.15` (keg-only, `/opt/homebrew/opt/zig@0.15/bin`) |
| Rust | rustc/cargo 1.100.0, Apple M4, macOS 26.5.1, Command Line Tools only |

### Zig install

1. `brew install zig` installed Zig **0.17.1**. The libghostty-vt build failed with it:
   `src/build/zig.zig:13:9: error: Your Zig version v0.17.1 does not meet the required build
   version of v0.15.2`. There were also std API errors (`no field named 'build_root' in struct
   'Build'` and `invalid builtin function: '@cImport'`).
2. `brew install zig@0.15` installed **0.15.2**. `brew uninstall zig` then removed 0.17.1 and its
   now-unused dependencies `llvm@22` and `lld@22` (1.6 GB).
3. Build with `export PATH="/opt/homebrew/opt/zig@0.15/bin:$HOME/.cargo/bin:$PATH"`. This
   worked on the first try. The build script also needs `git`, and network access, to clone Ghostty.

### Build time

| What | Result |
| --- | --- |
| Cold release build of the whole spike (`cargo clean && cargo build --release --timings`, crates already downloaded) | **87 s wall**, 387 s CPU |
| Of which: `libghostty-vt-sys` build script (git clone of Ghostty plus `zig build`, ReleaseFast) | **47.9 s**, the longest single unit (`gpui-pre` is second at 16.2 s) |
| Debug build of `libghostty-vt` alone, with the clone already present (zig `Debug`) | 44 s wall |
| Release binary | 10.8 MB |
| `target/` after the release build | 1.2 GB |

The build script clones into `target/<profile>/build/libghostty-vt-sys-*/out/ghostty-src`, so
debug and release each clone and build once, and every `cargo clean` repeats it.

## What was built

- `portable-pty` opens an 80x24 PTY and runs `$SHELL -l` (`/bin/zsh`), with
  `TERM=xterm-256color` and `COLORTERM=truecolor`.
- A `pty-reader` thread reads 64 KB chunks into a **bounded** `sync_channel(256)`. When the parser
  falls behind, the reader blocks, so a flood cannot grow memory.
- An `emulator` thread creates the engine. libghostty-vt handles are `!Send`, so the engine is
  created on that thread and never leaves it. The thread feeds bytes and handles resizes (it
  resizes both the engine and the PTY). It also answers terminal queries back to the PTY through
  `on_pty_write` (ghostty) or `Event::PtyWrite` (alacritty). At most every 16 ms it builds an
  engine-neutral `Frame`: per row, a `String` plus styled byte ranges. It sends the frame to the GPUI
  thread over `async_channel`.
- On the GPUI thread, a task drains the channel, keeps only the latest frame and calls
  `cx.notify()`. `render` draws one `StyledText` per row, in JetBrains Mono 13 px with a fixed 17 px
  line height, using `HighlightStyle` for fg, bg, bold, italic and underline. The cursor is a block
  that inverts its cell. The cell width comes from `text_system().advance(font, 13px, 'm')`
  (7.8 px). On each render, the view works out cols and rows from the viewport and sends a resize
  when they change.
- The default fg and bg are the design tokens `#ECEAE4` and `#18181A`. The ghostty engine takes
  its palette from libghostty. The alacritty engine uses a hand-written xterm palette with the same
  base 16 colours.
- Scripted modes (`--demo`, `--bench FILE`, `--idle`), a `--headless` mode with no GPUI, a
  `--parse FILE` mode (feeds a file straight into the engine, with no PTY), and `--engine none`
  (the PTY and plumbing with no parser, as a baseline).
- There is **no keyboard input**. The spike types into the PTY from a script thread. Key encoding
  (libghostty's `key::Encoder`) is left to Plan 6.

## Checks (step 2 of the brief)

The session was driven by `--demo`, which types into the shell. Screenshots were taken with
`screencapture -x -o -l <window id>`; the window id comes from `tools/winid.swift`, compiled with
`swiftc -O tools/winid.swift -o <scratch>/winid`. I read the screenshots afterwards to check them.

```sh
SPIKE_SHOT_DIR=<scratch>/shots-ghostty SPIKE_WINID=<scratch>/winid \
  ./target/release/terminal-spike --engine ghostty --demo
# same with --engine alacritty
```

| Check | libghostty-vt | alacritty_terminal |
| --- | --- | --- |
| `printf` with SGR 31-36, bold, italic (`3m`), underline, inverse, `38;5;208`, `38;2;111;124;242`, `48;5;22` background | correct | correct |
| Wide CJK (`日本`) and box drawing take the right number of cells; no misalignment after them | correct | correct |
| `ls -G /` (BSD `ls` colours; macOS `ls` has no `--color`) | directory colours correct | correct |
| zsh prompt with colours and glyphs | correct | correct |
| `vim -u NONE -N -c 'syntax on' -c 'set number laststatus=2 ruler' src/frame.rs` | full screen 80x24: syntax colours, inverse status line, cursor | same |
| Window resized programmatically to 120x36 while vim runs (`window.resize`) | grid and PTY resized to 120x36; vim redrew at the new size (line 1 no longer wraps) | same |
| 150-character line at 120 cols, then the window shrunk to 80x24 | reflowed from 120+30 to 80+70 characters on the primary screen | same |
| Queries answered back to the shell (`on_pty_write`) | zsh and vim started normally | same |

The two engines produced pixel-identical screenshots: same palette, same layout.

## Performance: `cat` of a 50 MB file (step 3)

Test data, made in the scratch directory (never in the repo):

```sh
base64 -b 79 -i /dev/urandom | head -c 52428800 > plain-50mb.txt     # 50 MiB, 79-char lines
python3 -I spikes/terminal/tools/gen_ansi.py ansi-50mb.txt  # 50 MiB of lines with 8 coloured words
                       # each (SGR 31-36, 1;37, 38;5;208, 38;2;r;g;b, each followed by 0m)
```

Run per engine, file and variant, 3 times (`spikes/terminal/tools/bench.sh`):

```sh
/usr/bin/time -l ./target/release/terminal-spike --engine <none|alacritty|ghostty> \
  --bench <file> [--headless]          # SPIKE_FORCE_DRAW=1 for the forced-draw variant
```

`--bench` waits 3 s for the shell, then types `cat FILE; echo $((40+2))MARK`. It prints the time
from typing that command until the emulator thread has fed the bytes containing `42MARK` to the
engine, so the whole file has been parsed. Each run fed 53,086,081 bytes; the PTY turns every `\n`
into `\r\n`. The memory columns come from `/usr/bin/time -l` and are in MiB.

Medians of 3 runs:

| Variant | Engine | plain: seconds | ansi: seconds | max RSS | peak footprint |
| --- | --- | --- | --- | --- | --- |
| GUI window | none (baseline) | 0.323 | 0.396 | 62.6 | 74 |
| GUI window | alacritty_terminal | 0.421 | 0.476 | 87.1 | 96.5 |
| GUI window | **libghostty-vt** | **0.323** | **0.384** | **68.4** | **77.6** |
| GUI, a draw forced for each frame | none | 0.321 | 0.392 | 62.6 | 80 |
| GUI, a draw forced for each frame | alacritty_terminal | 0.413 | 0.461 | 87.8 | 102 |
| GUI, a draw forced for each frame | **libghostty-vt** | **0.319** | **0.376** | **69.1** | **84** |
| headless (no GPUI) | none | 0.323 | 0.390 | n/a | 2.5 |
| headless (no GPUI) | alacritty_terminal | 0.413 | 0.478 | n/a | 23.1 |
| headless (no GPUI) | **libghostty-vt** | **0.318** | **0.380** | n/a | **3.9** |

Idle GUI, 10 s (`--idle`): footprint 71.8 (none), 72.0 (alacritty), 72.2 (ghostty) MiB.

In headless mode the max RSS (about 30 MiB) is the login zsh child, not the spike, so only the
footprint column is meaningful there.

Raw parser throughput, with no PTY or UI. The file is fed in 64 KB chunks, then one snapshot is
built (`--parse FILE`), 3 runs:

| Engine | plain | ansi | snapshot of 80x24 |
| --- | --- | --- | --- |
| alacritty_terminal | 0.26 s (191 MB/s) | 0.27 s (183 MB/s) | 30-45 µs |
| **libghostty-vt** | **0.06 s (840 MB/s)** | **0.215 s (232 MB/s)** | 66-97 µs |

What the numbers say:

- In the end-to-end runs **the PTY is the bottleneck**, at about 165 MB/s. libghostty-vt matches
  the no-parser baseline, so its cost is hidden. alacritty_terminal adds about 0.1 s (+30 %) to a
  50 MB cat.
- **Memory:** with 10,000 lines of scrollback in both engines, libghostty-vt adds about 1.4 MiB
  over the baseline and alacritty_terminal about 20.5 MiB (headless footprint). In the GUI, max RSS
  over the baseline is +6 MiB against +25 MiB.
- The snapshot copy into the engine-neutral frame takes under 0.1 ms for 80x24, so 60 snapshots
  a second cost under 1 % of a core.
- Frames sent during a flood are capped at about 60 per second (29 to 40 per 0.3 to 0.5 s run).

## What broke or surprised

1. **Zig version pin.** libghostty-vt 0.2.2 needs **exactly Zig 0.15.x**: Ghostty's `build.zig`
   checks the version, and 0.17 also breaks the std API. Homebrew's `zig` is already 0.17, so
   `zig@0.15` (keg-only) is needed, along with a PATH entry. CI and every developer machine need
   the same pin.
2. **The native build fetches from the network.** The build script runs `git clone` of the whole
   Ghostty repository (blob-filtered) into `OUT_DIR`, so an offline build fails. `GHOSTTY_SOURCE_DIR`
   (a local checkout) and `GHOSTTY_ZIG_SYSTEM_DIR` (Zig package cache) exist for offline or packaged
   builds. The default `kitty-graphics` feature is on.
3. **A covered or locked GPUI window does not redraw.** During the run the macOS session was
   locked, and AppKit reported the window as occluded. GPUI then stops its display link
   (`window_did_change_occlusion_state`), so `cx.notify()` does not lead to a draw; only a resize
   draws, synchronously. The first screenshots were blank or stale for this reason. The spike works
   around it by changing the window width by 1 px (`TermView::nudge`) before each screenshot, and on
   every frame with `SPIKE_FORCE_DRAW=1`. This is correct behaviour for a real app: a covered window
   does not need to paint. It also means **the unforced GUI runs measured no GPUI drawing during the
   flood.** The forced-draw runs (one synchronous draw per frame, about 15 to 20 draws a run, close
   to 60 a second) show that drawing did not slow throughput, but a run on an unlocked, visible
   display was not possible here.
4. **The display slept while the session was locked**, and `screencapture -l` then failed with
   "could not create image from window". The spike runs `caffeinate -u -t 2` before each screenshot.
5. A `WindowKind::PopUp` window (tried so it would stay above other windows) could not be captured
   with `screencapture -l`, so the spike uses a normal window.
6. libghostty-vt's default 16-colour palette is the same as the one written by hand for the
   alacritty engine, so the two engines looked identical.

## Not tested

- Interactive typing, IME, mouse, selection, scrollback scrolling, paste: there is no input path
  in the spike.
- A user dragging the window edge. Resizing was done programmatically with `window.resize`, which
  goes through the same `render` path to the grid and then the PTY resize.
- GPUI drawing cost under sustained output on a visible display (see item 3 above).
- Linux and Windows builds, and cross-compiling libghostty-vt.

## What Plan 6 (terminal) must watch for

- **Toolchain:** document and check Zig 0.15.x (`zig version`) in `scripts/check.sh` or in the
  build. Consider `GHOSTTY_SOURCE_DIR` with a vendored or cached Ghostty checkout so builds do not
  clone on every clean and work offline. Expect about 48 s for the native step on a cold build.
- **Threading:** keep the libghostty `Terminal`, `RenderState` and iterators on one dedicated
  thread per terminal. They are `!Send`; build them on that thread. Send owned snapshots out; never
  share the handles.
- **Back-pressure:** use a bounded channel from the PTY reader. Build snapshots at most once per
  frame (16 ms) and keep only the latest on the UI side.
- **Dirty tracking:** libghostty's `RenderState` has global and per-row dirty flags. The spike
  rebuilds all rows; Plan 6 can send only the dirty rows. The caller must reset both levels of
  dirty flags.
- **Wide cells:** skip `CellWide::SpacerTail` cells (ghostty) or `WIDE_CHAR_SPACER` (alacritty).
  Otherwise every row after a wide character shifts.
- **Queries:** set `on_pty_write` and write responses back to the PTY. Without it, programs that
  query the terminal (DA, cursor position) wait or misbehave.
- **Occlusion:** a covered window does not draw. Do not tie any terminal state progress to
  `render` being called; the spike sends resizes from `render`, and Plan 6 should drive them from a
  bounds observer instead.
- **Pre-1.0 API:** the Rust bindings say breaking changes are expected. Keep the libghostty calls
  behind one small module (as `ghostty.rs` here) so an upgrade or a switch to alacritty_terminal
  touches one file. The `Engine` trait in the spike (feed, resize, frame) was enough for both
  libraries.
- **Fonts:** the cell width is measured from JetBrains Mono at runtime (7.8 px at 13 px). Embed the
  font, as `gpui.md` already notes, so the grid does not depend on installed fonts.
