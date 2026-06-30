<p>
  <img src="docs/pictrim-logo.png" alt="PicTrim - Ultra-fast image processing tool" width="360">
</p>

PicTrim 是一个高性能的图片批处理工具。支持对图片进行批量缩放、压缩、旋转和格式转换。

![PicTrim 界面截图](docs/pictrim-screenshot.png)

## 功能

- 支持图片文件和目录混合输入，也可以直接拖拽。
- 支持按最长边、指定宽高、固定裁剪等方式批量缩放。
- 支持 JPG、PNG、WebP 输出，也可以保持原格式。
- 支持质量、旋转、并发数、是否放大、是否跳过已有文件等常用选项。
- 保留目录结构，实时显示进度、统计和失败列表。

## 性能

- 本项目的核心目标是打造最高性能的图片批量处理工具。
- 底层使用 Rust 和 libvips，处理速度快、内存占用低。
- 使用流式任务队列 + 多线程并发处理，轻松处理千万级数量的图片。
- 支持跳过已存在的输出文件，便于重复运行和增量处理。

## 使用

1. 添加图片文件或文件夹。
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

编译或链接 Rust 二进制文件需要在本机安装 libvips。

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
npm run release:package
```

构建完成后，`release/PicTrim/` 目录即为便携版产物。Windows 会额外生成 NSIS 安装包并复制到 `release/` 目录。`npm run release:package` 会生成便携版 zip 和 `SHA256SUMS.txt`，用于 GitHub Releases。

## 发布说明

当前 GitHub Releases 提供未签名的二进制文件。macOS 或 Windows 首次打开时可能显示安全提示。请从项目 Releases 页面下载，并使用随 release 附带的 `SHA256SUMS.txt` 校验文件完整性。

更详细的发布步骤见 [docs/release.md](docs/release.md)。
