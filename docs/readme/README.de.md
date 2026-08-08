<p>
  <img src="assets/pictrim-logo.png" alt="PicTrim - ultraschnelles Bildverarbeitungstool" width="360">
</p>

[English](../../README.md) | [中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

PicTrim ist ein kostenloses, quelloffenes und leistungsstarkes Tool zur Stapelverarbeitung von Bildern. Es unterstützt das massenhafte Skalieren, Komprimieren, Drehen und Konvertieren. Neben normalen Bildern kann es auch PDF-Dateien verkleinern, was sich besonders bei PDFs mit vielen Bildern deutlich bemerkbar macht.

## Bildschirmfoto

![PicTrim screenshot](assets/pictrim-screenshot-de.png)

## Funktionen

- Durch Streaming-Verarbeitung lassen sich problemlos mehrere zehn Millionen Dateien bewältigen.
- Stapelweise Skalierung nach längster Seite, Begrenzungsrahmen, festem Zuschnitt, Breite oder Höhe.
- Ausgabe als JPG, PNG oder WebP oder Beibehaltung des Originalformats.
- Gängige Optionen für Qualität, Drehung, Parallelität, Vergrößerung und das Überspringen vorhandener Dateien.
- Unterstützung für PDF-Dateien: Die ursprüngliche PDF-Struktur kann beibehalten und die enthaltenen Bilder können komprimiert oder in einem Bildformat ausgegeben werden.

## Verarbeitung von PDF-Dateien

- **Eingabe**: PDF-Dateien oder Ordner mit PDF-Dateien können direkt hinzugefügt werden. PicTrim verarbeitet die in den Seiteninhalten eingebetteten Bilder.
- **Originalformat beibehalten**: Die Ausgabe bleibt eine PDF-Datei. Nur die Bilder werden ersetzt; alle übrigen Inhalte bleiben unverändert.
- **Als JPG / PNG / WebP ausgeben**: Eingebettete PDF-Bilder werden als einzelne Dateien unter Pfaden wie `<PDF-Name>/page-0001-image-0001.jpg` ausgegeben.

## Leistung

- Der Kern nutzt Rust und libvips für hohen Durchsatz und geringen Speicherverbrauch.
- Eine Streaming-Aufgabenwarteschlange und Multithreading ermöglichen die problemlose Verarbeitung von mehreren zehn Millionen Bildern.
- Bereits vorhandene Ausgabedateien können übersprungen werden, praktisch für wiederholte oder inkrementelle Läufe.

## Verwendung

1. Bilddateien, PDF-Dateien oder Ordner hinzufügen.
2. Ausgabeordner auswählen.
3. Größe, Format, Qualität und Stapeloptionen einstellen.
4. Auf „Start“ klicken.

## Plattformunterstützung

- macOS
- Windows 10 / Windows 11 64-bit

Aktuelle Release-Builds richten sich an macOS und Windows x64. Linux-Pakete werden noch nicht bereitgestellt.

## Entwicklung

```bash
npm install
npm run dev
```

Frontend-Build prüfen:

```bash
npm run build
```

Rust-Teil prüfen:

```bash
cd src-tauri
cargo check
```

Zum Bauen oder Linken des Rust-Binaries muss libvips lokal installiert sein. QPDF 12.3.2, zlib 1.3.1 und IJG libjpeg 9f sind auf feste Versionen gesetzt und werden statisch gebaut; QPDF, CMake und libclang müssen daher nicht separat installiert werden.

macOS:

```bash
brew install vips
```

Unter Windows verwenden Sie das offizielle vorgefertigte libvips-Paket und setzen `VIPS_DIR` auf den entpackten Ordner:

```powershell
$env:VIPS_DIR = "C:\path\to\vips-dev-8.x"
$env:Path = "$env:VIPS_DIR\bin;$env:Path"
```

Sie können auch die [offizielle libvips-Installationsanleitung](https://www.libvips.org/install.html) lesen.

## Build

```bash
npm install
npm run tauri:build
```

Nach Abschluss des Builds enthält `release/PicTrim/` die lokale portable Version. GitHub Releases veröffentlichen ausschließlich signierte und von Apple notarisierte macOS-DMGs für Apple Silicon und Intel sowie den Windows-x64-Installer.

## Release-Hinweise

macOS-Release-Builds sind mit einem Developer-ID-Zertifikat signiert und von Apple notarisiert. Windows-Builds sind derzeit nicht signiert und können beim ersten Start eine SmartScreen-Warnung anzeigen.

Details zum Veröffentlichungsablauf finden Sie in [../release.md](../release.md).

## Lizenz

PicTrim wird unter der [MIT License](../../LICENSE) veröffentlicht. Hinweise zu den enthaltenen Abhängigkeiten stehen in [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md).
