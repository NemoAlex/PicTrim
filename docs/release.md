# 发布指南

PicTrim 的 GitHub Release 只提供三个面向用户的安装包：

- `PicTrim-<version>-macOS-arm64.dmg`
- `PicTrim-<version>-macOS-x64.dmg`
- `PicTrim-<version>-Windows-x64-Setup.exe`

macOS DMG 使用 Developer ID 证书签名并通过 Apple 公证。Windows 安装程序目前尚未签名。

## 发布

发布前保持 `package.json` 和 `src-tauri/tauri.conf.json` 中的版本号一致，然后推送对应的 `v<version>` 标签。Release 工作流会：

1. 在 Apple Silicon、Intel Mac 和 Windows x64 runner 上分别构建。
2. 执行前端检查、Rust 测试和 PDF 固定样本 smoke test。
3. 检查最终程序不存在动态 QPDF 依赖；macOS 继续验证完整 Mach-O 动态库闭包。
4. 签名并公证两个 macOS App 与 DMG。
5. 只上传两个 DMG 和一个 Windows 安装程序。
6. 使用 GitHub 自动生成的 Release Notes。

也可以在 Actions 页面手动运行 Release 工作流并填写版本号。重新发布已有版本时，工作流会替换三个安装包，并删除该 Release 中不再生成的旧附件。

## macOS 签名配置

仓库需要配置以下 Actions Secrets：

- `MAC_CSC_LINK`
- `MAC_CSC_KEY_PASSWORD`
- `APPLE_ID`
- `APPLE_APP_SPECIFIC_PASSWORD`
- `APPLE_TEAM_ID`

可以在持有 Developer ID Application 证书的 Mac 上运行以下脚本进行配置：

```bash
./scripts/configure-macos-signing-secrets.sh NemoAlex/PicTrim
```

签名、公证或 Gatekeeper 验证失败时，macOS 构建会直接失败，不会发布未签名的 DMG。

## 本地构建

```bash
npm install
npm run tauri:build
```

本地构建仍会在 `release/PicTrim/` 生成便携目录，但 GitHub Release 不会上传 portable ZIP 或 `SHA256SUMS.txt`。

## PDF 发布门槛

- `cargo test --lib` 必须覆盖重复引用、嵌套 Form、inline image、JPEG、Flate Gray/RGB、CMYK、Indexed、ICCBased、SMask、签名、加密、损坏流和不支持编码样本。
- 安装 Poppler 后，本地测试会额外使用 `pdftotext -bbox`、`pdfinfo` 和 `pdftoppm` 比较处理前后的文字位置、页数、页面尺寸并渲染页面。
- 不支持或损坏的图片必须使整份 PDF 失败，并且正式输出位置不能出现部分文件。
- macOS arm64、macOS x64、Windows x64 最终二进制不得动态依赖 QPDF；QPDF 12.3.2、zlib 1.3.1 和 IJG libjpeg 9f 由仓库固定源码静态构建。
- PDF 视觉测试通过不替代真实签名阅读器、加密兼容性和大文件压力验证；发布前仍应抽样打开签名 PDF、空密码 PDF 和高分辨率扫描件。
