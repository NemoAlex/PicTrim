<p>
  <img src="docs/readme/assets/pictrim-logo.png" alt="PicTrim - Ultra-fast image processing tool" width="360">
</p>

[English](README.md) | [中文](docs/readme/README.zh-CN.md) | [日本語](docs/readme/README.ja.md) | [한국어](docs/readme/README.ko.md) | [Français](docs/readme/README.fr.md) | [Deutsch](docs/readme/README.de.md)

PicTrim is a free and open-source, high-performance batch image processor for resizing, compressing, rotating, and converting images. In addition to regular images, it can slim down PDF files, with especially noticeable results for PDFs that contain many images.

## Screenshot

![PicTrim screenshot](docs/readme/assets/pictrim-screenshot-en.png)

## Features

- Streaming processing makes it easy to handle tens of millions of files.
- Batch resize by longest side, bounding box, fixed crop, width, or height.
- Export as JPG, PNG, or WebP, or keep the original format.
- Configure common options such as quality, rotation, concurrency, upscaling, and whether to skip existing files.
- Process PDF files by either preserving the original PDF structure and compressing its images, or exporting the embedded images in an image format.

## PDF processing

- **Input**: Add PDF files directly, or add folders that contain PDF files. PicTrim processes the images embedded in PDF page content.
- **Keep original**: The output remains a PDF. Only its images are replaced; all other content remains unchanged.
- **Export as JPG / PNG / WebP**: Embedded PDF images are exported as individual files under paths such as `<PDF name>/page-0001-image-0001.jpg`.

## Performance

- The core uses Rust and libvips for high throughput and low memory usage.
- A streaming task queue plus multithreaded processing makes it easy to handle tens of millions of images.
- Existing output files can be skipped, which is useful for repeated or incremental runs.

## Usage

1. Add image files, PDF files, or folders.
2. Choose an output folder.
3. Set the size, format, quality, and batch options.
4. Click "Start".

## Platform Support

- macOS
- Windows 10 / Windows 11 64-bit

Current release builds target macOS and Windows x64. Linux packages are not provided yet.

## Development

```bash
npm install
npm run dev
```

Check the frontend build:

```bash
npm run build
```

Check the Rust side:

```bash
cd src-tauri
cargo check
```

Building or linking the Rust binary requires libvips to be installed locally. QPDF 12.3.2, zlib 1.3.1, and IJG libjpeg 9f are pinned and built statically; QPDF, CMake, and libclang do not need to be installed.

macOS:

```bash
brew install vips
```

On Windows, use the official prebuilt libvips package and set `VIPS_DIR` to the extracted folder:

```powershell
$env:VIPS_DIR = "C:\path\to\vips-dev-8.x"
$env:Path = "$env:VIPS_DIR\bin;$env:Path"
```

You can also refer to the [official libvips installation guide](https://www.libvips.org/install.html).

## Build

```bash
npm install
npm run tauri:build
```

After the build finishes, `release/PicTrim/` contains the local portable build. GitHub Releases publish only the signed and notarized macOS DMGs for Apple Silicon and Intel, plus the Windows x64 installer.

## Release Notes

macOS release builds are signed with a Developer ID certificate and notarized by Apple. Windows builds are currently unsigned and may show a SmartScreen warning on first launch.

See [docs/release.md](docs/release.md) for the detailed release workflow.

## License

PicTrim is released under the [MIT License](LICENSE). Bundled dependency notices are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
