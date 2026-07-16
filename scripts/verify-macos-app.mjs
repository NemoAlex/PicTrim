#!/usr/bin/env node

import path from 'path';
import {
  findMacOSAppExecutable,
  verifyMacOSDylibs,
} from './macos-dylibs.mjs';

const appPath = process.argv[2];
if (!appPath) {
  console.error('Usage: node scripts/verify-macos-app.mjs /path/to/PicTrim.app');
  process.exit(1);
}

const resolvedAppPath = path.resolve(appPath);
const binaryPath = findMacOSAppExecutable(resolvedAppPath);
const frameworksDir = path.join(resolvedAppPath, 'Contents', 'Frameworks');
const machOCount = verifyMacOSDylibs(binaryPath, frameworksDir);

console.log(`Verified self-contained macOS app with ${machOCount} Mach-O files: ${resolvedAppPath}`);
