#!/usr/bin/env node
/**
 * Generate a preview descriptor that records the checksum manifest,
 * archive metadata, and GitHub context so governance tooling can audit
 * which artefacts were produced for a given run.
 *
 * Usage:
 *   node scripts/generate-preview-descriptor.mjs \
 *     --manifest artifacts/checksums.sha256 \
 *     --archive artifacts/preview-site.tar.gz \
 *     --out artifacts/preview-descriptor.json
 */

import { createHash } from 'node:crypto';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { basename, resolve } from 'node:path';

function parseArgs() {
  const args = process.argv.slice(2);
  const options = {};
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i];
    const value = args[i + 1];
    if (!value || value.startsWith('--')) {
      throw new Error(`missing value for ${key}`);
    }
    switch (key) {
      case '--manifest':
        options.manifest = value;
        break;
      case '--archive':
        options.archive = value;
        break;
      case '--out':
        options.out = value;
        break;
      default:
        throw new Error(`unknown argument ${key}`);
    }
  }
  if (!options.manifest || !options.archive || !options.out) {
    throw new Error('usage: --manifest <path> --archive <path> --out <path>');
  }
  return options;
}

async function sha256(path) {
  const buffer = await readFile(path);
  const hash = createHash('sha256');
  hash.update(buffer);
  return hash.digest('hex');
}

function githubContext() {
  const commit = process.env.GITHUB_SHA ?? null;
  const ref = process.env.GITHUB_HEAD_REF || process.env.GITHUB_REF || null;
  const repo = process.env.GITHUB_REPOSITORY ?? null;
  const server = process.env.GITHUB_SERVER_URL ?? 'https://github.com';
  const runId = process.env.GITHUB_RUN_ID ?? null;
  const runAttempt = process.env.GITHUB_RUN_ATTEMPT ?? null;

  let runUrl = null;
  if (repo && runId) {
    runUrl = `${server}/${repo}/actions/runs/${runId}`;
    if (runAttempt) {
      runUrl += `?attempt=${runAttempt}`;
    }
  }

  return { commit, ref, repository: repo, run_url: runUrl };
}

async function main() {
  const { manifest, archive, out } = parseArgs();

  const manifestPath = resolve(manifest);
  const archivePath = resolve(archive);
  const outPath = resolve(out);

  const [manifestSha, archiveSha, archiveStats] = await Promise.all([
    sha256(manifestPath),
    sha256(archivePath),
    stat(archivePath),
  ]);

  const descriptor = {
    version: 1,
    generated_at: new Date().toISOString(),
    context: githubContext(),
    checksums_manifest: {
      path: manifestPath,
      filename: basename(manifestPath),
      sha256: manifestSha,
    },
    archive: {
      path: archivePath,
      filename: basename(archivePath),
      sha256: archiveSha,
      size_bytes: archiveStats.size,
    },
  };

  await writeFile(outPath, `${JSON.stringify(descriptor, null, 2)}\n`, 'utf8');
}

main().catch((error) => {
  console.error(`failed to generate preview descriptor: ${error.message}`);
  process.exit(1);
});
