# PicTrim

PicTrim is a portable desktop tool for high-throughput batch image resizing.

The app uses Tauri v2 for the GUI and Rust + libvips for image processing.

## Features

- Pick input and output folders from a simple GUI.
- Resize images by longest side without enlarging small images.
- Choose JPG, PNG, WebP, or keep original format.
- Tune quality and worker concurrency.
- Skip existing outputs by default.
- Optionally copy non-image files.
- Preserve relative directory structure.
- Stream progress and failure lists back to the UI.

## Defaults

- Longest side: `2000`
- Quality: `85`
- Concurrency: `20`
- Output format: `JPG`
- Skip existing files: enabled
- Copy non-image files: disabled

## Development

```bash
npm install
npm run dev
```

`npm run dev` starts the desktop development mode and opens the PicTrim GUI.

For a production frontend build and Rust checks:

```bash
npm run build
cd src-tauri
cargo check
```

Running or linking the Rust binary requires libvips to be installed on the build machine. On macOS, for example:

```bash
brew install vips
```

## Portable Build Notes

Build on the target platform for the intended release artifact.

1. Install Rust stable, Node.js, and the Tauri prerequisites for the target platform.
2. Install or stage a libvips distribution for the target platform.
3. Ensure libvips runtime libraries are discoverable at runtime. For portable builds, place the required libvips dynamic libraries next to the app binary or configure the platform's library search path.
4. Run:

```bash
npm install
npm run tauri:build
```

The Tauri config currently uses the `app` bundle target so the release can be distributed as a portable folder. If installers are needed later, add platform-specific bundle targets in `src-tauri/tauri.conf.json`.

## Verification

Validated in this workspace:

- `npm run build`
- `cargo check`

`cargo test` reached the linker and failed on this macOS machine because system libvips is not installed (`ld: library 'vips' not found`). Install libvips locally or run tests on a build machine with libvips available.
