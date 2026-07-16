#!/usr/bin/env node
// Post-build: assembles a portable release directory with all required shared libraries.

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';
import { copyWindowsVipsDlls } from './libvips-windows.mjs';
import {
  bundleMacOSDylibs,
  findMacOSAppExecutable,
} from './macos-dylibs.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const targetDir = path.join(root, 'src-tauri', 'target', 'release');
const releaseDir = path.join(root, 'release');
const outDir = path.join(root, 'release', 'PicTrim');
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });

if (process.platform === 'win32') {
  buildWindows();
} else if (process.platform === 'darwin') {
  buildMacOS();
} else {
  console.error(`Unsupported platform: ${process.platform}`);
  process.exit(1);
}

console.log(`\nPortable build ready: ${outDir}`);

// ── Windows ──────────────────────────────────────────────────────────

function buildWindows() {
  const exe = path.join(targetDir, 'PicTrim.exe');
  if (!fs.existsSync(exe)) {
    console.error('PicTrim.exe not found. Run "npm run tauri:build" first.');
    process.exit(1);
  }
  fs.copyFileSync(exe, path.join(outDir, 'PicTrim.exe'));

  try {
    const { count, sourceDir } = copyWindowsVipsDlls(outDir);
    console.log(`Copied ${count} DLLs from ${sourceDir}`);
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  }

  copyWindowsInstaller();
}

function copyWindowsInstaller() {
  const nsisDir = path.join(targetDir, 'bundle', 'nsis');
  if (!fs.existsSync(nsisDir)) {
    console.error(`NSIS bundle directory not found: ${nsisDir}`);
    process.exit(1);
  }

  const installers = fs.readdirSync(nsisDir)
    .filter(file => file.toLowerCase().endsWith('.exe'))
    .map(file => ({
      file,
      path: path.join(nsisDir, file),
      mtimeMs: fs.statSync(path.join(nsisDir, file)).mtimeMs,
    }))
    .sort((a, b) => b.mtimeMs - a.mtimeMs);

  if (installers.length === 0) {
    console.error(`No NSIS installer found in: ${nsisDir}`);
    process.exit(1);
  }

  const installer = installers[0];
  const dest = path.join(releaseDir, `PicTrim-${version}-Windows-${process.arch}-Setup.exe`);
  fs.copyFileSync(installer.path, dest);
  console.log(`Installer ready: ${dest}`);
}

// ── macOS ────────────────────────────────────────────────────────────

function buildMacOS() {
  const appSrc = path.join(targetDir, 'bundle', 'macos', 'PicTrim.app');
  if (fs.existsSync(appSrc)) {
    execSync(`cp -R "${appSrc}" "${outDir}/"`);
  } else {
    const bin = path.join(targetDir, 'PicTrim');
    if (!fs.existsSync(bin)) {
      console.error('Build output not found. Run "npm run tauri:build" first.');
      process.exit(1);
    }
    fs.copyFileSync(bin, path.join(outDir, 'PicTrim'));
  }

  const isApp = fs.existsSync(path.join(outDir, 'PicTrim.app'));
  const binary = isApp
    ? findMacOSAppExecutable(path.join(outDir, 'PicTrim.app'))
    : path.join(outDir, 'PicTrim');
  const frameworksDir = isApp
    ? path.join(outDir, 'PicTrim.app', 'Contents', 'Frameworks')
    : outDir;

  const count = bundleMacOSDylibs(binary, frameworksDir);
  console.log(`Copied, relinked, and verified ${count} external dylibs`);
}
