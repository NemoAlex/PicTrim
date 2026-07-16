import fs from 'fs';
import path from 'path';
import { execFileSync } from 'child_process';

const SYSTEM_LIBRARY_PREFIXES = [
  '/System/Library/',
  '/usr/lib/',
  '/Library/Apple/System/',
];

export function findMacOSAppExecutable(appPath) {
  const plistPath = path.join(appPath, 'Contents', 'Info.plist');
  const executableName = execFileSync(
    '/usr/libexec/PlistBuddy',
    ['-c', 'Print :CFBundleExecutable', plistPath],
    { encoding: 'utf8' },
  ).trim();
  const executablePath = path.join(appPath, 'Contents', 'MacOS', executableName);
  if (!fs.existsSync(executablePath)) {
    throw new Error(`macOS application executable not found: ${executablePath}`);
  }
  return executablePath;
}

export function bundleMacOSDylibs(binaryPath, frameworksDir) {
  fs.mkdirSync(frameworksDir, { recursive: true });
  ensureRPath(binaryPath, '@executable_path/../Frameworks');

  const executableDir = path.dirname(binaryPath);
  const queue = [{ targetPath: binaryPath, sourcePath: binaryPath }];
  const processedTargets = new Set();
  const sourcesByName = new Map();
  let copiedCount = 0;

  while (queue.length > 0) {
    const { targetPath, sourcePath } = queue.shift();
    if (processedTargets.has(targetPath)) continue;
    processedTargets.add(targetPath);

    for (const dependency of getDependencies(targetPath)) {
      if (isSystemDependency(dependency)) continue;
      if (dependency === `@rpath/${path.basename(targetPath)}`) continue;

      const dependencySource = resolveSourceDependency(
        dependency,
        sourcePath,
        executableDir,
      );
      if (!dependencySource) {
        throw new Error(`Cannot resolve dynamic library ${dependency} referenced by ${sourcePath}`);
      }

      const name = path.basename(dependency);
      const realSourcePath = fs.realpathSync(dependencySource);
      const previousSource = sourcesByName.get(name);
      if (previousSource && previousSource !== realSourcePath) {
        throw new Error(`Dynamic library name collision for ${name}: ${previousSource} and ${realSourcePath}`);
      }
      sourcesByName.set(name, realSourcePath);

      const destination = path.join(frameworksDir, name);
      if (!fs.existsSync(destination)) {
        fs.copyFileSync(realSourcePath, destination);
        fs.chmodSync(destination, fs.statSync(realSourcePath).mode);
        execFileSync('install_name_tool', ['-id', `@rpath/${name}`, destination]);
        queue.push({ targetPath: destination, sourcePath: realSourcePath });
        copiedCount++;
      }

      execFileSync('install_name_tool', [
        '-change',
        dependency,
        `@rpath/${name}`,
        targetPath,
      ]);
    }
  }

  verifyMacOSDylibs(binaryPath, frameworksDir);
  return copiedCount;
}

export function verifyMacOSDylibs(binaryPath, frameworksDir) {
  const executableDir = path.dirname(binaryPath);
  const targets = [binaryPath, ...walkMachOFiles(frameworksDir)];
  const errors = [];

  for (const target of targets) {
    for (const dependency of getDependencies(target)) {
      if (isSystemDependency(dependency)) continue;

      const resolvedPath = resolveBundledDependency(
        dependency,
        target,
        executableDir,
        frameworksDir,
      );
      if (!resolvedPath) {
        errors.push(`${target} references external library ${dependency}`);
      } else if (!fs.existsSync(resolvedPath)) {
        errors.push(`${target} references missing bundled library ${dependency}`);
      }
    }
  }

  if (errors.length > 0) {
    throw new Error(`macOS bundle is not self-contained:\n${errors.join('\n')}`);
  }

  return targets.length;
}

function ensureRPath(binaryPath, rpath) {
  const output = execFileSync('otool', ['-l', binaryPath], { encoding: 'utf8' });
  if (output.includes(`path ${rpath} (`)) return;
  execFileSync('install_name_tool', ['-add_rpath', rpath, binaryPath]);
}

function getDependencies(target) {
  const output = execFileSync('otool', ['-L', target], { encoding: 'utf8' });
  return output
    .split('\n')
    .slice(1)
    .map(line => line.trim().replace(/ \(compatibility version.*$/, ''))
    .filter(Boolean);
}

function isSystemDependency(dependency) {
  return SYSTEM_LIBRARY_PREFIXES.some(prefix => dependency.startsWith(prefix));
}

function resolveSourceDependency(dependency, sourcePath, executableDir) {
  if (path.isAbsolute(dependency)) {
    return fs.existsSync(dependency) ? dependency : null;
  }

  const suffix = dependency.replace(/^@(rpath|loader_path|executable_path)\//, '');
  const candidates = [];
  if (dependency.startsWith('@loader_path/')) {
    candidates.push(path.resolve(path.dirname(sourcePath), suffix));
  } else if (dependency.startsWith('@executable_path/')) {
    candidates.push(path.resolve(executableDir, suffix));
  } else if (dependency.startsWith('@rpath/')) {
    for (const rpath of getRPaths(sourcePath)) {
      const expandedRPath = rpath
        .replace(/^@loader_path/, path.dirname(sourcePath))
        .replace(/^@executable_path/, executableDir);
      if (path.isAbsolute(expandedRPath)) {
        candidates.push(path.resolve(expandedRPath, suffix));
      }
    }
    candidates.push(path.resolve(path.dirname(sourcePath), suffix));
  } else {
    return null;
  }

  return candidates.find(candidate => fs.existsSync(candidate)) ?? null;
}

function getRPaths(target) {
  const output = execFileSync('otool', ['-l', target], { encoding: 'utf8' });
  const rpaths = [];
  const lines = output.split('\n');
  for (let index = 0; index < lines.length; index++) {
    if (lines[index].trim() !== 'cmd LC_RPATH') continue;
    for (let detailIndex = index + 1; detailIndex < Math.min(index + 6, lines.length); detailIndex++) {
      const match = lines[detailIndex].trim().match(/^path (.+) \(offset \d+\)$/);
      if (match) {
        rpaths.push(match[1]);
        break;
      }
    }
  }
  return rpaths;
}

function resolveBundledDependency(dependency, target, executableDir, frameworksDir) {
  if (dependency.startsWith('@rpath/')) {
    return path.join(frameworksDir, dependency.slice('@rpath/'.length));
  }
  if (dependency.startsWith('@loader_path/')) {
    return path.resolve(path.dirname(target), dependency.slice('@loader_path/'.length));
  }
  if (dependency.startsWith('@executable_path/')) {
    return path.resolve(executableDir, dependency.slice('@executable_path/'.length));
  }
  return null;
}

function walkMachOFiles(directory) {
  if (!fs.existsSync(directory)) return [];
  const files = [];
  const queue = [directory];
  while (queue.length > 0) {
    const current = queue.shift();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const entryPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        queue.push(entryPath);
      } else if (entry.isFile() && isMachO(entryPath)) {
        files.push(entryPath);
      }
    }
  }
  return files;
}

function isMachO(filePath) {
  const output = execFileSync('file', ['-b', filePath], { encoding: 'utf8' });
  return output.includes('Mach-O');
}
