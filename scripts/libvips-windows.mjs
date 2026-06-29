import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';

export function copyWindowsVipsDlls(outDir) {
  const vipsBin = findVipsOnWindows();
  if (!vipsBin) {
    throw new Error('Could not find libvips DLLs. Set VIPS_DIR/VCPKG_ROOT, or add the libvips bin directory to PATH.');
  }

  fs.mkdirSync(outDir, { recursive: true });

  const dlls = fs.readdirSync(vipsBin).filter(f => f.toLowerCase().endsWith('.dll'));
  if (dlls.length === 0) {
    throw new Error(`No DLL files found in libvips bin directory: ${vipsBin}`);
  }

  for (const dll of dlls) {
    fs.copyFileSync(path.join(vipsBin, dll), path.join(outDir, dll));
  }

  assertWindowsBundleComplete(outDir);
  return { count: dlls.length, sourceDir: vipsBin };
}

export function findVipsOnWindows() {
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
    throw new Error(`Portable bundle is missing required DLLs: ${missing.join(', ')}\nChecked directory: ${dir}`);
  }
}
