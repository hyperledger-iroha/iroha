#!/usr/bin/env node

/**
 * Generate a sha256 manifest for the Docusaurus build output.
 *
 * Writes `build/checksums.sha256` containing `sha256sum`-compatible entries
 * so CI and reviewers can verify preview artifacts without re-running the
 * site generator.
 */

import { createHash } from 'node:crypto';
import { promises as fs } from 'node:fs';
import { join, relative, resolve } from 'node:path';

const BUILD_DIR = resolve(process.cwd(), 'build');
const MANIFEST_NAME = 'checksums.sha256';
const RELEASE_INFO_NAME = 'release.json';

async function main() {
  const exists = await directoryExists(BUILD_DIR);
  if (!exists) {
    throw new Error(
      `build directory not found at ${BUILD_DIR}; run "npm run build" first.`,
    );
  }

  await writeReleaseInfo(BUILD_DIR);
  const entries = await walkForChecksums(BUILD_DIR);
  entries.sort((a, b) => a.path.localeCompare(b.path));

  const manifestLines = entries.map(
    ({ hash, path: filePath }) => `${hash}  ${filePath}`,
  );
  const manifestPath = join(BUILD_DIR, MANIFEST_NAME);
  await fs.writeFile(manifestPath, manifestLines.join('\n') + '\n', 'utf8');
}

async function writeReleaseInfo(buildDir) {
  const info = {
    tag: process.env.DOCS_RELEASE_TAG ?? process.env.GIT_COMMIT ?? 'dev',
    generated_at: new Date().toISOString(),
    source: process.env.DOCS_RELEASE_SOURCE ?? 'local',
  };
  const releasePath = join(buildDir, RELEASE_INFO_NAME);
  await fs.writeFile(releasePath, JSON.stringify(info, null, 2) + '\n', 'utf8');
}

async function directoryExists(path) {
  try {
    const stat = await fs.stat(path);
    return stat.isDirectory();
  } catch {
    return false;
  }
}

async function walkForChecksums(root) {
  const queue = [root];
  const files = [];

  while (queue.length > 0) {
    const current = queue.pop();
    const entries = await fs.readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const resolved = join(current, entry.name);
      if (entry.isDirectory()) {
        queue.push(resolved);
      } else if (entry.isFile()) {
        if (
          entry.name === MANIFEST_NAME ||
          entry.name.startsWith('.DS_Store')
        ) {
          continue;
        }
        const hash = await sha256File(resolved);
        const relativePath = relative(root, resolved);
        files.push({ hash, path: relativePath.replace(/\\/g, '/') });
      }
    }
  }

  return files;
}

async function sha256File(path) {
  const data = await fs.readFile(path);
  const hash = createHash('sha256');
  hash.update(data);
  return hash.digest('hex');
}

main().catch((error) => {
  console.error(`[write-checksums] ${error.message}`);
  process.exitCode = 1;
});
