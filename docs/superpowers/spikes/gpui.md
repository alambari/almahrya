# Spike: GPUI window with design tokens

Date: 2026-11-14. Source: `spikes/gpui-window/` (throwaway, outside the main workspace).

## Verdict: GO

GPUI and gpui-component built on the first attempt, the window opened at the requested size, both
fonts rendered from the user font folder with no fallback, and a gpui-component button took the
design tokens once they were mapped onto its own theme slots. Build time, binary size and idle memory
are all fine for a desktop app.

## Pinned versions (copy into the root `Cargo.toml` in Task 9)

```toml
gpui = { package = "gpui-pre", version = "=0.3.12" }
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.12", features = ["font-kit", "runtime_shaders"] }
gpui-component = "=0.9.1"
```

- **gpui-component `=0.9.1`** (crates.io, published from longbridge/gpui-kit
  commit `39ab849a93b5f15507b969164d49891db8d5b470`).
- **GPUI: crates.io `gpui-pre` `=0.3.12`**, a published snapshot of zed-industries/zed at
  **`d2a4fb6ded7c470be25d1541281da3882495b65b`** (recorded in its manifest as `zed-rev`).
- **Why there is no git `rev`:** gpui-component 0.9.1 no longer takes GPUI from git. It depends on
  `gpui = { package = "gpui-pre", version = "=0.3.12" }` from crates.io. A git dependency on the
  zed repository would be a separate crate, so its types would not match gpui-component's. The pin
  that agrees with gpui-component is therefore the exact crates.io version above. The zed commit is
  recorded here so the source can be traced. Update all three lines together; gpui-component always
  states the `gpui-pre` version it needs.
- The application entry point (`application()`) lives in `gpui-pre-platform`, so that crate is a
  direct dependency, pinned to the same version.
- `gpui-kit = "=0.9.1"` is an alternative single dependency that re-exports the same three crates.
  The spike uses the separate crates so each version is visible.

## Environment

Apple M4, macOS 26.5.1, rustc/cargo 1.100.0, Command Line Tools only (no full Xcode).

## Measurements

| What | Result |
| --- | --- |
| Cold debug build (`cargo build`, empty `target/`, crates already downloaded) | 57 s wall, 216 s CPU |
| Cold release build (`cargo build --release`, after the debug build) | 97 s wall, 629 s CPU |
| Incremental rebuild after editing `main.rs` | about 3 s |
| Debug binary | 105 MB |
| Release binary | 19.5 MB (14.9 MB after `strip`) |
| Idle memory, debug, RSS 10 s after launch | 95 MB (86 MB at 30 s) |
| Idle memory, release, RSS at 10 s and 30 s | 82 MB |
| Idle memory, release, `top` MEM (the Activity Monitor figure) | 65 MB |
| Idle CPU, release (`top`) | 0.0 % |
| `target/` size after the debug and release builds | 4.6 GB |

Download time is not included: `cargo fetch` ran first.

## What was built

A 1440x900 window (the native title bar adds 33 pt, so the window frame is 1440x933), a 244 px
sidebar `#1F1F22`, a main area `#18181A`, text in Plus Jakarta Sans 13 px `#ECEAE4`, a line in
JetBrains Mono 13 px, and one gpui-component `Button` (primary) in the `agent` colour `#6F7CF2`.
A window screenshot confirmed the layout. In the raw framebuffer the pixels are exactly `#1F1F22`
and `#18181A`.

## Font loading

- `font_family("Plus Jakarta Sans")` and `font_family("JetBrains Mono")` resolve the fonts installed
  in `~/Library/Fonts` through `font-kit`. No loading code was needed. The screenshot shows real
  Jakarta glyphs and real JetBrains Mono glyphs, with no fallback font.
- The app will ship on machines without these fonts, so it should embed the `.ttf` files and
  register them with `cx.text_system().add_fonts(...)` at startup. The spike does not test this; it
  belongs in the real UI crate.
- gpui-component has its own `theme.font_family` / `theme.font_size`. The spike sets them to
  Plus Jakarta Sans and 13 px, and the button label used Jakarta as a result.

## What broke or surprised

1. **Metal shaders need full Xcode unless `runtime_shaders` is on.** Without the feature,
   `gpui-pre-apple` fails at build time with `xcrun: error: unable to find utility "metal"`.
   With `runtime_shaders`, shaders compile at runtime and the build works on Command Line Tools.
   Keep the feature, or require full Xcode on build machines.
2. **The default component theme is light.** At first the primary button was near-black on the dark
   background and almost invisible. Fix: call `Theme::change(ThemeMode::Dark, None, cx)`, then call
   `Theme::update` a second time to apply the tokens. The second call is needed because a mode
   change reloads the colours and would overwrite edits made in the same closure.
3. **Buttons read their own slots.** Setting `theme.colors.primary` did not change the button.
   `Button` paints from `button_primary`, `button_primary_hover`, `button_primary_active` and
   `button_primary_foreground`. Mapping the design tokens means mapping every component slot, not
   only the shadcn-style base colours. Plan a single `tokens -> Theme` function in the UI crate.
4. **The native title bar follows the system appearance.** It was light on this machine, above a dark
   app. The real app should use a transparent or custom title bar (gpui-component has a `TitleBar`),
   or follow the system appearance.
5. **Colour space.** The framebuffer is tagged Display P3, and GPUI writes the sRGB hex values into
   it unconverted. Grey tokens look right, but a colour-managed reading of the screenshot reports
   them a little lighter (for example `#29292D` for `#1F1F22`). This is unlikely to matter for the
   neutral palette. Check saturated tokens (`agent` and the status colours) against the design file
   once the real UI exists.
6. `screencapture` of the whole display failed ("could not create image from display"). Capturing
   one window with `screencapture -l <window id>` worked.
7. During one run the click handler printed `clicked` twice with no deliberate input. This was
   probably a stray click on the window. It was not investigated further.

## Not covered

Embedded fonts, text input, lists, scrolling performance, IME, accessibility, and dark/light
switching at runtime. These belong in the UI tasks, not this spike.
