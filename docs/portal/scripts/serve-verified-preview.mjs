#!/usr/bin/env node

/**
 * Serve the built docs portal only after verifying the checksum manifest.
 *
 * Usage: node scripts/serve-verified-preview.mjs [docusaurus serve args...]
 */

import {spawnSync} from 'node:child_process';
import {existsSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const portalDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const buildDir = path.join(portalDir, 'build');
const checksumManifest = path.join(buildDir, 'checksums.sha256');

function fail(message) {
  console.error(`[serve-verified-preview] ${message}`);
  process.exit(1);
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: portalDir,
    stdio: 'inherit',
    shell: false,
  });
  if (result.error) {
    fail(result.error.message);
  }
  if (typeof result.status === 'number' && result.status !== 0) {
    process.exit(result.status);
  } else if (result.status === null && result.signal) {
    process.exit(1);
  }
}

if (!existsSync(buildDir)) {
  fail(`build directory not found at ${buildDir}. Run "npm run build" first.`);
}

if (!existsSync(checksumManifest)) {
  fail(
    `checksum manifest missing at ${checksumManifest}. ` +
      'The postbuild hook should write it; rerun "npm run build".',
  );
}

run('bash', ['scripts/preview_verify.sh', '--build-dir', buildDir]);

const serveArgs = process.argv.slice(2);
run('npx', ['docusaurus', 'serve', ...serveArgs]);
