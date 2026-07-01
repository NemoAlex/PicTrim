<p>
  <img src="assets/pictrim-logo.png" alt="PicTrim - 초고속 이미지 처리 도구" width="360">
</p>

[English](../../README.md) | [中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

PicTrim은 고성능 이미지 일괄 처리 도구입니다. 이미지 일괄 리사이즈, 압축, 회전, 형식 변환을 지원합니다.

![PicTrim screenshot](assets/pictrim-screenshot-en.png)

## 기능

- 이미지 파일과 폴더를 함께 입력할 수 있고, 드래그 앤 드롭도 지원합니다.
- 긴 변, 지정 너비/높이, 고정 크기 자르기 등 다양한 방식으로 일괄 리사이즈합니다.
- JPG, PNG, WebP로 내보내거나 원본 형식을 유지할 수 있습니다.
- 품질, 회전, 동시 작업 수, 확대 여부, 기존 파일 처리 방식을 설정할 수 있습니다.
- 폴더 구조를 유지하며 진행률, 통계, 로그, 실패 목록을 실시간으로 표시합니다.

## 성능

- PicTrim은 매우 빠른 이미지 일괄 처리 도구를 목표로 합니다.
- 핵심 처리에는 Rust와 libvips를 사용하여 빠르고 메모리 사용량이 낮습니다.
- 스트리밍 작업 큐와 멀티스레드 처리로 매우 큰 이미지 모음도 다룰 수 있습니다.
- 기존 출력 파일을 건너뛸 수 있어 반복 실행과 증분 처리에 유용합니다.

## 사용 방법

1. 이미지 파일 또는 폴더를 추가합니다.
2. 출력 폴더를 선택합니다.
3. 크기, 형식, 품질, 일괄 처리 옵션을 설정합니다.
4. "Start"를 클릭합니다.

## 플랫폼 지원

- macOS
- Windows 10 / Windows 11 64-bit

현재 릴리스 빌드는 macOS와 Windows x64를 대상으로 합니다. Linux 패키지는 아직 제공하지 않습니다.

## 개발

```bash
npm install
npm run dev
```

프런트엔드 빌드 확인:

```bash
npm run build
```

Rust 부분 확인:

```bash
cd src-tauri
cargo check
```

Rust 바이너리를 빌드하거나 링크하려면 로컬에 libvips가 설치되어 있어야 합니다.

macOS:

```bash
brew install vips
```

Windows에서는 libvips 공식 사전 빌드 패키지를 사용하고, 압축을 푼 폴더를 `VIPS_DIR`로 설정하세요:

```powershell
$env:VIPS_DIR = "C:\path\to\vips-dev-8.x"
$env:Path = "$env:VIPS_DIR\bin;$env:Path"
```

[libvips 공식 설치 가이드](https://www.libvips.org/install.html)도 참고할 수 있습니다.

## 빌드

```bash
npm install
npm run tauri:build
npm run release:package
```

빌드가 끝나면 `release/PicTrim/`에 포터블 빌드가 생성됩니다. Windows 빌드는 NSIS 설치 프로그램도 생성하여 `release/`에 복사합니다. `npm run release:package`는 GitHub Releases용 포터블 zip과 `SHA256SUMS.txt`를 생성합니다.

## 릴리스 노트

현재 GitHub Releases는 서명되지 않은 바이너리를 제공합니다. macOS 또는 Windows에서 처음 실행할 때 보안 경고가 표시될 수 있습니다. 프로젝트 Releases 페이지에서 다운로드하고, 함께 제공되는 `SHA256SUMS.txt`로 파일 무결성을 확인하세요.

자세한 릴리스 절차는 [docs/release.md](../release.md)를 참고하세요.
