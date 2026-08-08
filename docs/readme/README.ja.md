<p>
  <img src="assets/pictrim-logo.png" alt="PicTrim - 超高速画像処理ツール" width="360">
</p>

[English](../../README.md) | [中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

PicTrim は、オープンソースで無料の高性能な画像バッチ処理ツールです。画像の一括リサイズ、圧縮、回転、形式変換に対応します。通常の画像だけでなく PDF ファイルも軽量化でき、画像が多い PDF ほど大きな効果が得られます。

## スクリーンショット

![PicTrim screenshot](assets/pictrim-screenshot-ja.png)

## 機能

- ストリーミング処理により、数千万規模のファイルも容易に扱えます。
- 長辺、指定幅・高さ、固定クロップなどの方法で一括リサイズできます。
- JPG、PNG、WebP 形式で出力するか、元の形式を維持できます。
- 品質、回転、並列数、拡大可否、既存ファイルのスキップなどの一般的なオプションを設定できます。
- PDF に対応し、元の PDF 構造を維持したまま内部の画像を圧縮するか、画像形式で出力できます。

## PDF ファイルの処理

- **入力**：PDF ファイル、または PDF を含むフォルダーを直接追加できます。PicTrim は PDF ページ内容に埋め込まれた画像を処理します。
- **元の形式を保持**：出力は引き続き PDF で、画像だけを置換し、その他の内容は変更しません。
- **JPG / PNG / WebP で出力**：PDF 内の画像を個別のファイルとして、`<PDF名>/page-0001-image-0001.jpg` のようなパスに出力します。

## パフォーマンス

- コアには Rust と libvips を使用し、高速かつ低メモリで処理します。
- ストリーミングタスクキューとマルチスレッド処理により、数千万規模の画像も容易に処理できます。
- 既存の出力ファイルをスキップできるため、繰り返し実行や差分処理に便利です。

## 使い方

1. 画像ファイル、PDF ファイル、またはフォルダーを追加します。
2. 出力フォルダーを選択します。
3. サイズ、形式、品質、バッチ処理オプションを設定します。
4. 「Start」をクリックします。

## 対応プラットフォーム

- macOS
- Windows 10 / Windows 11 64-bit

現在のリリースビルドは macOS と Windows x64 向けです。Linux パッケージはまだ提供していません。

## 開発

```bash
npm install
npm run dev
```

フロントエンドのビルド確認:

```bash
npm run build
```

Rust 側の確認:

```bash
cd src-tauri
cargo check
```

Rust バイナリのビルドまたはリンクには、ローカルに libvips が必要です。QPDF 12.3.2、zlib 1.3.1、IJG libjpeg 9f はバージョンを固定して静的にビルドされるため、QPDF、CMake、libclang を別途インストールする必要はありません。

macOS:

```bash
brew install vips
```

Windows では libvips 公式のプリビルドパッケージを使用し、展開先を `VIPS_DIR` に設定してください:

```powershell
$env:VIPS_DIR = "C:\path\to\vips-dev-8.x"
$env:Path = "$env:VIPS_DIR\bin;$env:Path"
```

[libvips 公式インストールガイド](https://www.libvips.org/install.html) も参照できます。

## ビルド

```bash
npm install
npm run tauri:build
```

ビルド完了後、`release/PicTrim/` にローカル用のポータブル版が生成されます。GitHub Releases では、Apple Silicon／Intel 向けの署名・公証済み macOS DMG と Windows x64 インストーラーのみを公開します。

## リリースノート

macOS リリースビルドは Developer ID 証明書で署名され、Apple の公証を受けています。Windows ビルドは現在未署名のため、初回起動時に SmartScreen の警告が表示される場合があります。

詳しいリリース手順は [../release.md](../release.md) を参照してください。

## ライセンス

PicTrim は [MIT License](../../LICENSE) の下で公開されています。組み込み依存関係の通知は [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md) に記載しています。
