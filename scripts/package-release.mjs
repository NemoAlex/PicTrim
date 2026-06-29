#!/usr/bin/env node
// Packages unsigned release artifacts and writes SHA256SUMS.txt for GitHub Releases.

import crypto from 'crypto';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { execFileSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const releaseDir = path.join(root, 'release');
const portableDir = path.join(releaseDir, 'PicTrim');
const version = readVersion();
const platform = process.platform;
const arch = normalizeArch(os.arch());
const platformName = platform === 'win32' ? 'windows' : platform === 'darwin' ? 'macos' : platform;

if (!['win32', 'darwin'].includes(platform)) {
  console.error(`Unsupported release platform: ${platform}`);
  process.exit(1);
}

if (!fs.existsSync(portableDir)) {
  console.error(`Portable release directory not found: ${portableDir}`);
  console.error('Run "npm run tauri:build" before packaging release artifacts.');
  process.exit(1);
}

fs.mkdirSync(releaseDir, { recursive: true });

const portableZip = path.join(releaseDir, `PicTrim-${version}-${platformName}-${arch}-portable.zip`);
fs.rmSync(portableZip, { force: true });
createPortableZip(portableZip);

const artifacts = collectArtifacts();
const checksumFile = path.join(releaseDir, 'SHA256SUMS.txt');
const checksumLines = artifacts
  .map(file => `${sha256(file)}  ${path.basename(file)}`)
  .sort((a, b) => a.localeCompare(b));

fs.writeFileSync(checksumFile, `${checksumLines.join('\n')}\n`);

console.log('Release artifacts ready:');
for (const file of artifacts) {
  console.log(`- ${file}`);
}
console.log(`- ${checksumFile}`);

function readVersion() {
  const packageJson = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
  return packageJson.version;
}

function normalizeArch(value) {
  if (value === 'x64') return 'x64';
  if (value === 'arm64') return 'arm64';
  return value;
}

function createPortableZip(dest) {
  if (platform === 'darwin') {
    execFileSync('ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', 'PicTrim', dest], {
      cwd: releaseDir,
      stdio: 'inherit',
    });
    return;
  }

  const command = [
    '$ErrorActionPreference = "Stop";',
    `Compress-Archive -Path ${psQuote(path.join(releaseDir, 'PicTrim'))} -DestinationPath ${psQuote(dest)} -Force`,
  ].join(' ');

  execFileSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', command], {
    stdio: 'inherit',
  });
}

function collectArtifacts() {
  const files = fs.readdirSync(releaseDir)
    .filter(name => {
      const lower = name.toLowerCase();
      return lower.endsWith('.zip') || lower.endsWith('.exe') || lower.endsWith('.dmg');
    })
    .map(name => path.join(releaseDir, name))
    .filter(file => fs.statSync(file).isFile());

  if (files.length === 0) {
    console.error(`No release artifacts found in: ${releaseDir}`);
    process.exit(1);
  }

  return files;
}

function sha256(file) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(file));
  return hash.digest('hex');
}

function psQuote(value) {
  return `'${value.replaceAll("'", "''")}'`;
}
