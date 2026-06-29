# 发布指南

PicTrim 当前通过 GitHub Releases 发布未签名二进制文件。这适合开源项目的早期版本，但 macOS 和 Windows 可能在首次打开时显示安全提示。

## 构建

在每个目标平台分别构建：

```bash
npm install
npm run tauri:build
npm run release:package
```

上传 `release/` 目录中的文件到 GitHub Release：

- `PicTrim-<version>-<platform>-<arch>-portable.zip`
- Windows 构建时生成的 NSIS 安装包
- `SHA256SUMS.txt`

如果重复构建同一个版本，发布前建议清理旧的 `release/` 目录，避免上传或校验旧产物。

## Release 文案模板

```markdown
## 下载

- Windows：下载 installer 或 portable zip。
- macOS：下载 portable zip。

## 安全提示

这些构建未进行代码签名。macOS 或 Windows 首次打开时可能显示安全提示。

如需校验下载文件，请对照本 release 附带的 `SHA256SUMS.txt`。

Windows 用户可能需要在 SmartScreen 中选择“更多信息”，然后选择“仍要运行”。
macOS 用户可能需要首次尝试打开后，到“系统设置 > 隐私与安全性”中允许打开。
```

## 注意事项

- 保持 `package.json` 和 `src-tauri/tauri.conf.json` 中的版本号一致。
- Windows 安装包需要在 Windows 上构建，以便打包 libvips DLL。
- macOS 应用需要在 macOS 上构建，以便打包 libvips dylib。
