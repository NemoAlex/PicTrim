# PicTrim

PicTrim 是一款便携的桌面工具，专为处理大量图片而设计——可一次性处理上百万张图片的批量缩放。

应用使用 Tauri v2 构建图形界面，图像处理则由 Rust + libvips 完成。

## 默认设置

- 最长边：`2000`
- 压缩质量：`85`
- 并发数：CPU 核心数
- 输出格式：`保持原格式`
- 跳过已存在文件：开启
- 复制非图片文件：关闭

## 开发

```bash
npm install
npm run dev
```

`npm run dev` 将启动桌面开发模式并打开 PicTrim 界面。

如需生产环境前端构建和 Rust 检查：

```bash
npm run build
cd src-tauri
cargo check
```

编译或链接 Rust 二进制文件需要在本机安装 libvips。

**macOS：**

```bash
brew install vips
```

**Windows：**

推荐使用 [vcpkg](https://github.com/microsoft/vcpkg) 安装 libvips：

```powershell
vcpkg install libvips:x64-windows
```

安装后确保 vcpkg 的工具链集成到构建环境中：

```powershell
vcpkg integrate install
```

或者从 [libvips 官方](https://www.libvips.org/install.html) 下载预编译的 Windows 二进制包，并手动将 `bin` 目录加入 `PATH`。

## 便携版构建说明

请在目标平台上构建对应的发布产物。

1. 安装 Rust 稳定版、Node.js 以及目标平台的 Tauri 前置依赖。
2. 为目标平台安装 libvips（参见上方 macOS / Windows 安装说明）。
3. 执行：

```bash
npm install
npm run tauri:build
```

构建完成后，`release/PicTrim/` 目录即为自包含的便携版产物，包含应用二进制文件及所有必需的 libvips 动态库，可直接打包分发，无需手动拷贝。

如需安装包，可在 `src-tauri/tauri.conf.json` 中添加平台特定的打包目标。

### Windows 构建注意事项

- 需安装 [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（含 MSVC 及 Windows SDK）。
- 需安装 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)（Tauri v2 依赖）。
- 构建脚本会自动从 `VCPKG_ROOT` 或 `PATH` 中查找 libvips DLL 并复制到发布目录。如未找到，请确保 `VCPKG_ROOT` 环境变量已设置，或 vips.dll 所在目录已加入 `PATH`。

## 验证

本工作区中已验证：

- `npm run build`
- `cargo check`

`cargo test` 在本 macOS 机器上到达链接阶段后失败，原因是未安装系统 libvips（`ld: library 'vips' not found`）。请在本地安装 libvips，或在已安装 libvips 的构建机器上运行测试。
