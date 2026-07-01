<p>
  <img src="assets/pictrim-logo.png" alt="PicTrim - ultraschnelles Bildverarbeitungstool" width="360">
</p>

[English](../../README.md) | [中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

PicTrim ist ein leistungsstarkes Tool zur Stapelverarbeitung von Bildern. Es unterstützt das massenhafte Skalieren, Komprimieren, Drehen und Konvertieren von Bildern.

![PicTrim screenshot](assets/pictrim-screenshot-en.png)

## Funktionen

- Bilddateien und Ordner können gemischt als Eingabe verwendet oder direkt in die App gezogen werden.
- Stapelweise Skalierung nach längster Seite, Begrenzungsrahmen, festem Zuschnitt, Breite oder Höhe.
- Export als JPG, PNG oder WebP oder Beibehaltung des Originalformats.
- Einstellungen für Qualität, Drehung, Parallelität, Vergrößerung und vorhandene Dateien.
- Ordnerstruktur bleibt erhalten, mit Live-Fortschritt, Statistiken, Protokollen und Fehlerliste.

## Leistung

- PicTrim ist als extrem schnelles Tool zur Bild-Stapelverarbeitung ausgelegt.
- Der Kern nutzt Rust und libvips für hohen Durchsatz und geringen Speicherverbrauch.
- Eine Streaming-Aufgabenwarteschlange und Multithreading machen sehr große Bildmengen praktikabel.
- Bereits vorhandene Ausgabedateien können übersprungen werden, praktisch für wiederholte oder inkrementelle Läufe.

## Verwendung

1. Bilddateien oder Ordner hinzufügen.
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

Zum Bauen oder Linken des Rust-Binaries muss libvips lokal installiert sein.

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
npm run release:package
```

Nach Abschluss des Builds enthält `release/PicTrim/` die portable Version. Windows-Builds erzeugen zusätzlich einen NSIS-Installer und kopieren ihn nach `release/`. `npm run release:package` erzeugt die portable zip-Datei und `SHA256SUMS.txt` für GitHub Releases.

## Release-Hinweise

Aktuelle GitHub Releases enthalten unsignierte Binärdateien. macOS oder Windows können beim ersten Start eine Sicherheitswarnung anzeigen. Laden Sie die Dateien von der Releases-Seite des Projekts herunter und prüfen Sie die Integrität mit der enthaltenen Datei `SHA256SUMS.txt`.

Details zum Veröffentlichungsablauf finden Sie in [docs/release.md](../release.md).
