#!/usr/bin/env node
// Post-build: assembles a portable release directory with all required shared libraries.

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const targetDir = path.join(root, 'src-tauri', 'target', 'release');
const outDir = path.join(root, 'release', 'PicTrim');

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

  const vipsBin = findVipsOnWindows();
  if (!vipsBin) {
    console.error('Could not find libvips DLLs. Set VIPS_DIR/VCPKG_ROOT, or add the libvips bin directory to PATH.');
    process.exit(1);
  }
  const dlls = fs.readdirSync(vipsBin).filter(f => f.toLowerCase().endsWith('.dll'));
  if (dlls.length === 0) {
    console.error(`No DLL files found in libvips bin directory: ${vipsBin}`);
    process.exit(1);
  }
  for (const dll of dlls) {
    fs.copyFileSync(path.join(vipsBin, dll), path.join(outDir, dll));
  }
  assertWindowsBundleComplete(outDir);
  console.log(`Copied ${dlls.length} DLLs from ${vipsBin}`);
}

function findVipsOnWindows() {
  if (process.env.VIPS_DIR) {
    const dir = path.join(process.env.VIPS_DIR, 'bin');
    if (isVipsBinDir(dir)) return dir;
  }
  if (process.env.VCPKG_ROOT) {
    const dir = path.join(process.env.VCPKG_ROOT, 'installed', 'x64-windows', 'bin');
    if (isVipsBinDir(dir)) return dir;
  }
  try {
    const libdir = execSync('pkg-config --variable=libdir vips', { encoding: 'utf-8' }).trim();
    const bindir = path.join(path.dirname(libdir), 'bin');
    if (isVipsBinDir(bindir)) return bindir;
  } catch {}
  for (const p of (process.env.PATH || '').split(path.delimiter)) {
    if (p && isVipsBinDir(p)) return p;
  }
  return null;
}

function isVipsBinDir(dir) {
  if (!dir || !fs.existsSync(dir)) return false;
  const files = fs.readdirSync(dir).map(f => f.toLowerCase());
  return files.includes('vips.dll') || files.some(f => f.startsWith('libvips') && f.endsWith('.dll'));
}

function assertWindowsBundleComplete(dir) {
  const files = fs.readdirSync(dir).map(f => f.toLowerCase());
  const required = [
    ['libvips', f => f === 'vips.dll' || (f.startsWith('libvips') && f.endsWith('.dll'))],
    ['glib', f => f.includes('glib-2.0') && f.endsWith('.dll')],
    ['gobject', f => f.includes('gobject-2.0') && f.endsWith('.dll')],
  ];
  const missing = required.filter(([, matches]) => !files.some(matches)).map(([name]) => name);
  if (missing.length > 0) {
    console.error(`Portable bundle is missing required DLLs: ${missing.join(', ')}`);
    console.error(`Checked directory: ${dir}`);
    process.exit(1);
  }
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

  const vipsLib = findVipsOnMacOS();
  if (!vipsLib) {
    console.warn('Warning: could not find libvips libraries.');
    return;
  }

  const isApp = fs.existsSync(path.join(outDir, 'PicTrim.app'));
  const binary = isApp
    ? path.join(outDir, 'PicTrim.app', 'Contents', 'MacOS', 'PicTrim')
    : path.join(outDir, 'PicTrim');
  const frameworksDir = isApp
    ? path.join(outDir, 'PicTrim.app', 'Contents', 'Frameworks')
    : outDir;

  if (!fs.existsSync(frameworksDir)) fs.mkdirSync(frameworksDir, { recursive: true });

  // Add rpath pointing to the Frameworks directory
  const rpath = isApp ? '@loader_path/../Frameworks' : '@loader_path';
  addRpath(binary, rpath);

  const count = collectDylibs(binary, frameworksDir, vipsLib, rpath);
  console.log(`Copied and fixed ${count} dylibs`);
}

function findVipsOnMacOS() {
  try {
    return execSync('pkg-config --variable=libdir vips', { encoding: 'utf-8' }).trim();
  } catch {}
  for (const p of ['/opt/homebrew/lib', '/usr/local/lib']) {
    if (fs.existsSync(path.join(p, 'libvips.dylib'))) return p;
  }
  return null;
}

function addRpath(binary, rpath) {
  try {
    const output = execSync(`otool -l "${binary}"`, { encoding: 'utf-8' });
    if (output.includes(rpath)) return;
  } catch {}
  execSync(`install_name_tool -add_rpath "${rpath}" "${binary}"`);
}

function collectDylibs(binary, outDir, libDir, rpath) {
  const seen = new Set();
  const queue = [binary];
  let count = 0;

  while (queue.length > 0) {
    const target = queue.shift();
    let output;
    try {
      output = execSync(`otool -L "${target}"`, { encoding: 'utf-8' });
    } catch { continue; }

    for (const line of output.split('\n').slice(1)) {
      const lib = line.trim().split(' ')[0];
      if (seen.has(lib) || !lib.startsWith(libDir)) continue;
      seen.add(lib);

      const name = path.basename(lib);
      const dest = path.join(outDir, name);
      if (!fs.existsSync(dest)) {
        fs.copyFileSync(lib, dest);
        count++;
      }

      // Fix reference in the target
      execSync(`install_name_tool -change "${lib}" "@rpath/${name}" "${target}"`);
      // Fix id of the copied dylib
      execSync(`install_name_tool -id @rpath/${name} "${dest}"`);

      // Scan transitive dependencies
      queue.push(dest);
    }
  }
  return count;
}
