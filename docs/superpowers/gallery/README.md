# Component gallery

One window that shows every ui-kit component in the dark palette (left half) and the light palette (right half).
It exists so a GPUI upgrade can be checked by comparing a screenshot taken before the upgrade with one taken after.

## Run it

```sh
cargo run -p almahrya-ui-kit --example gallery
```

The window is 1440 x 900. Close it or press Cmd-Q to quit. The sample text lives in
`crates/ui-kit/examples/gallery.rs`. The new components (label, status bar, decision card, toggle, chip, list card)
take all their text from the caller, and `crates/ui-kit/tests/gallery.rs` checks that their source holds no
user-facing string. The sidebar is a known exception: it still has its own English strings, and will be converted
when its text moves into the localisation work.

## Take a screenshot

```sh
cargo run -p almahrya-ui-kit --example gallery -- --screenshot docs/superpowers/gallery/baseline/gallery.png
```

The example opens the window, waits for it to settle, renders the window's scene to an image in-process (GPUI's
`Window::render_to_image`), writes the PNG and exits. It prints `wrote <path>` and exits with 0. If the capture
fails, it prints the reason and exits with 1.

- No screen-recording permission is needed and the window may be covered by other windows, because the frame is
  rendered offscreen rather than read from the display.
- The image has the display's pixel density: about 2880 x 1800 on a Retina screen. The native title bar is not part of
  the image.
- The capture needs GPUI's test support. The `screenshot` feature of `almahrya-ui-kit` turns it on, and the crate's
  dev-dependency on itself enables that feature for its examples and tests, so the command above needs no extra
  flags. The app never enables it.

## Baseline

`baseline/gallery.png` is the reference image, taken with the GPUI versions pinned in the root `Cargo.toml`
(`gpui-pre` and `gpui-pre-platform` 0.3.12, `gpui-component` 0.9.1).

## Checking a GPUI upgrade

Follow "Upgrading GPUI" in [ARCHITECTURE.md](../../../ARCHITECTURE.md). For the gallery step:

1. Before bumping the versions, on the upgrade branch, regenerate the baseline so it reflects the current code:
   run the screenshot command above and commit the image if it changed.
2. Bump the versions, fix `ui-kit`, and run `scripts/check.sh`.
3. Take the after image next to the baseline:

   ```sh
   cargo run -p almahrya-ui-kit --example gallery -- --screenshot docs/superpowers/gallery/after/gallery.png
   ```

4. Compare the two images side by side (or diff them with an image tool). Check colours against DESIGN.md, radii,
   the 3 px `waiting` left edge, the 38 x 22 toggle, text wrapping, and that both fonts render (Plus Jakarta Sans
   for interface text, JetBrains Mono for the mono chips and the mono label).
5. In the pull request, show both images. Once it is accepted, replace `baseline/gallery.png` with the after image
   and delete `after/`.

## Adding a component

A new component goes in `crates/ui-kit/src/components/`, one file per component, re-exported from
`components/mod.rs` and `lib.rs`. Add it to the gallery in the same change: a `GalleryItem` variant in
`crates/ui-kit/src/gallery.rs`, sample data in `crates/ui-kit/examples/gallery.rs`, and a check in
`crates/ui-kit/tests/gallery.rs` that its accessible label is not empty. Then regenerate the baseline.

## Known limits

- gpui-component widgets (the sidebar's "New chat session" button) read one app-wide theme. The gallery applies the
  first column's palette to them, so in the light column that button still uses the dark `agent` colour.
- The sidebar hard-codes its English text (see above).
- The toggle's off colour `#3A3A40` is the single value DESIGN.md gives; it is used in both palettes.
