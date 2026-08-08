<p>
  <img src="assets/pictrim-logo.png" alt="PicTrim - 超快图片处理工具" width="360">
</p>

[English](../../README.md) | [中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

PicTrim 是一个开源免费的高性能图片批处理工具，支持批量缩放、压缩、旋转和格式转换。除了普通图片，也可以对 PDF 文件进行瘦身，对于图片量较大的 PDF 文件有非常明显的效果。

## 界面截图

![PicTrim 界面截图](assets/pictrim-screenshot-zh-CN.png)

## 功能

- 采用流式处理，轻松应对千万级数量文件。
- 支持按最长边、指定宽高、固定裁剪等方式批量缩放。
- 支持输出为 JPG、PNG、WebP 格式，也可以选择保持原格式。
- 支持质量、旋转、并发数、是否放大、是否跳过已有文件等常用选项。
- 支持 PDF 文件，可以选择保留原 PDF 结构，压缩其中的图片，也可以选择输出图片格式。

## PDF 文件处理

- **输入**：可以直接添加 PDF 文件或包含 PDF 的文件夹，PicTrim 会处理 PDF 页面内容中的内嵌图片。
- **选择保持原格式**：输出仍为 PDF，仅替换其中的图片，其他内容保持不变。
- **选择输出为 JPG / PNG / WebP**：将 PDF 中的内嵌图片输出为独立文件，保存在 `<PDF文件名>/page-0001-image-0001.jpg` 这样的路径下。

## 性能

- 底层使用 Rust 和 libvips，处理速度快、内存占用低。
- 使用流式任务队列 + 多线程并发处理，轻松处理千万级数量的图片。
- 支持跳过已存在的输出文件，便于重复运行和增量处理。

## 使用

1. 添加图片文件、PDF 文件或文件夹。
2. 选择输出目录。
3. 设置尺寸、格式、质量和批处理选项。
4. 点击“开始处理”。

## 系统支持

- macOS
- Windows 10 / Windows 11 64 位

当前发布构建面向 macOS 和 Windows x64。Linux 暂未提供打包支持。

## 开发

```bash
npm install
npm run dev
```

检查前端构建：

```bash
npm run build
```

检查 Rust 部分：

```bash
cd src-tauri
cargo check
```

编译或链接 Rust 二进制文件需要在本机安装 libvips。QPDF 12.3.2、zlib 1.3.1 和 IJG libjpeg 9f 已固定版本并采用静态构建，因此不需要另行安装 QPDF、CMake 或 libclang。

macOS：

```bash
brew install vips
```

Windows 推荐使用 libvips 官方预编译包，并设置 `VIPS_DIR` 指向解压后的目录：

```powershell
$env:VIPS_DIR = "C:\path\to\vips-dev-8.x"
$env:Path = "$env:VIPS_DIR\bin;$env:Path"
```

也可以参考 [libvips 官方安装说明](https://www.libvips.org/install.html)。

## 构建

```bash
npm install
npm run tauri:build
```

构建完成后，`release/PicTrim/` 目录包含本地便携版产物。GitHub Releases 只发布适用于 Apple Silicon 和 Intel 的已签名、公证 macOS DMG，以及 Windows x64 安装程序。

## 发布说明

macOS 发布构建已使用 Developer ID 证书签名并通过 Apple 公证。Windows 构建目前尚未签名，首次打开时可能显示 SmartScreen 警告。

更详细的发布步骤见 [../release.md](../release.md)。

## 许可证

PicTrim 基于 [MIT License](../../LICENSE) 发布。内置依赖的声明见 [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md)。
