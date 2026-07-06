<p>
  <img src="assets/pictrim-logo.png" alt="PicTrim - 超高速画像処理ツール" width="360">
</p>

[English](../../README.md) | [中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

PicTrim は、高性能な画像バッチ処理ツールです。画像の一括リサイズ、圧縮、回転、形式変換に対応しています。

![PicTrim screenshot](assets/pictrim-screenshot-ja.png)

## 機能

- 画像ファイルとフォルダーを混在して入力でき、ドラッグ＆ドロップにも対応。
- 長辺、指定幅・高さ、固定クロップなどの方法で一括リサイズ。
- JPG、PNG、WebP で出力、または元の形式を維持。
- 品質、回転、並列数、拡大可否、既存ファイルの扱いなどを設定可能。
- フォルダー構造を保持し、進捗、統計、ログ、失敗リストをリアルタイム表示。

## パフォーマンス

- PicTrim は非常に高速な画像バッチ処理ツールを目指しています。
- コアには Rust と libvips を使用し、高速かつ低メモリで処理します。
- ストリーミングタスクキューとマルチスレッド処理により、大量の画像にも対応できます。
- 既存の出力ファイルをスキップできるため、繰り返し実行や差分処理に便利です。

## 使い方

1. 画像ファイルまたはフォルダーを追加します。
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

Rust バイナリのビルドまたはリンクには、ローカルに libvips が必要です。

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
npm run release:package
```

ビルド完了後、`release/PicTrim/` にポータブル版が生成されます。Windows ビルドでは NSIS インストーラーも生成され、`release/` にコピーされます。`npm run release:package` は GitHub Releases 用のポータブル zip と `SHA256SUMS.txt` を生成します。

## リリースノート

現在の GitHub Releases では未署名のバイナリを提供しています。macOS または Windows では初回起動時にセキュリティ警告が表示される場合があります。プロジェクトの Releases ページからダウンロードし、同梱の `SHA256SUMS.txt` でファイルの整合性を確認してください。

詳しいリリース手順は [../release.md](../release.md) を参照してください。

## ライセンス

PicTrim は [MIT License](../../LICENSE) の下で公開されています。
