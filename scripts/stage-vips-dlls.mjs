#!/usr/bin/env node
// Stages libvips DLLs next to the Windows release executable before Tauri bundles it.

import path from 'path';
import { fileURLToPath } from 'url';
import { copyWindowsVipsDlls } from './libvips-windows.mjs';

if (process.platform !== 'win32') {
  process.exit(0);
}

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const targetDir = path.join(root, 'src-tauri', 'target', 'release');

try {
  const { count, sourceDir } = copyWindowsVipsDlls(targetDir);
  console.log(`Staged ${count} libvips DLLs from ${sourceDir}`);
} catch (err) {
  console.error(err instanceof Error ? err.message : String(err));
  process.exit(1);
}
