#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0

import {createHash} from 'node:crypto';
import {
  copyFile,
  mkdir,
  readFile,
  stat,
  writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import {parseArgs} from 'node:util';
import {fileURLToPath} from 'node:url';

import {verifySpecDigest} from './tryit-proxy-lib.mjs';

const PORTAL_ROOT = fileURLToPath(new URL('../', import.meta.url));
const DEFAULT_FILES = Object.freeze([
  'scripts/tryit-proxy.mjs',
  'scripts/tryit-proxy-lib.mjs',
  'scripts/tryit-proxy-probe.mjs',
  'scripts/tryit-proxy-rollback.mjs',
]);

function nowIso() {
  return new Date().toISOString();
}

function defaultLabel() {
  return nowIso().replace(/[:.]/g, '-');
}

async function sha256Hex(filePath) {
  const contents = await readFile(filePath);
  return createHash('sha256').update(contents).digest('hex');
}

async function describeFile(filePath, relativeName) {
  const fileStat = await stat(filePath);
  return {
    name: relativeName,
    bytes: fileStat.size,
    sha256_hex: await sha256Hex(filePath),
  };
}

async function loadOpenApiManifest(manifestPath) {
  const raw = await readFile(manifestPath, {encoding: 'utf8'});
  const parsed = JSON.parse(raw);
  if (!parsed.artifact || !parsed.artifact.sha256_hex) {
    throw new Error(
      `openapi manifest ${manifestPath} is missing the expected artifact digest`,
    );
  }
  const specTarget = parsed.artifact.path ?? 'torii.json';
  const specPath = path.isAbsolute(specTarget)
    ? specTarget
    : path.join(path.dirname(manifestPath), specTarget);
  const specBuffer = await readFile(specPath);
  await verifySpecDigest({manifestPath, specPath, specBuffer});
  return {manifest: parsed, specPath};
}

/**
 * Build a deterministic Try It proxy release bundle containing the proxy
 * sources, a signed OpenAPI manifest digest reference, and checksum metadata.
 *
 * @param {object} options
 * @param {string} options.outDir
 * @param {string | undefined} options.label
 * @param {string | undefined} options.targetHint
 * @param {boolean} options.allowClientAuth
 * @param {string | undefined} options.openapiManifestPath
 * @param {string[]} options.files
 * @returns {Promise<object>} release summary
 */
export async function buildTryItRelease({
  outDir,
  label,
  targetHint,
  allowClientAuth = false,
  openapiManifestPath,
  files = DEFAULT_FILES,
} = {}) {
  if (!outDir || outDir.trim() === '') {
    throw new Error('outDir is required');
  }
  const releaseDir = path.resolve(outDir);
  await mkdir(releaseDir, {recursive: true});

  const manifestPath =
    openapiManifestPath ??
    path.join(PORTAL_ROOT, 'static', 'openapi', 'manifest.json');
  const {manifest, specPath} = await loadOpenApiManifest(manifestPath);

  const copied = [];
  for (const relPath of files) {
    const trimmed = (relPath ?? '').trim();
    if (trimmed === '') {
      continue;
    }
    const source = path.join(PORTAL_ROOT, trimmed);
    const destination = path.join(releaseDir, path.basename(trimmed));
    await copyFile(source, destination);
    copied.push(await describeFile(destination, path.basename(trimmed)));
  }

  const release = {
    label: label && label.trim() !== '' ? label : defaultLabel(),
    generated_at: nowIso(),
    target_hint: targetHint ?? null,
    allow_client_auth: Boolean(allowClientAuth),
    entrypoint: 'tryit-proxy.mjs',
    openapi: {
      manifest_path: manifestPath,
      spec_path: specPath,
      sha256_hex: manifest.artifact.sha256_hex,
      blake3_hex: manifest.artifact.blake3_hex ?? null,
      bytes: manifest.artifact.bytes ?? null,
      generator_commit: manifest.generator_commit ?? null,
      signature_public_key_hex:
        manifest.artifact.signature?.public_key_hex ?? null,
      signature_hex: manifest.artifact.signature?.signature_hex ?? null,
    },
    files: copied,
  };

  const releasePath = path.join(releaseDir, 'release.json');
  await writeFile(releasePath, JSON.stringify(release, null, 2) + '\n', {
    encoding: 'utf8',
  });

  const checksumEntries = [];
  for (const file of copied) {
    checksumEntries.push(`${file.sha256_hex}  ${file.name}`);
  }
  const releaseDigest = await sha256Hex(releasePath);
  checksumEntries.push(`${releaseDigest}  release.json`);
  const checksumPath = path.join(releaseDir, 'checksums.sha256');
  await writeFile(checksumPath, checksumEntries.join('\n') + '\n', {
    encoding: 'utf8',
  });

  return {
    release,
    releasePath,
    checksumPath,
  };
}

async function main() {
  const {values} = parseArgs({
    options: {
      out: {type: 'string'},
      label: {type: 'string'},
      target: {type: 'string'},
      'allow-client-auth': {type: 'boolean', default: false},
      'openapi-manifest': {type: 'string'},
    },
  });

  const outDir =
    values.out ??
    process.env.TRYIT_RELEASE_OUT ??
    path.join(PORTAL_ROOT, '..', '..', 'artifacts', 'tryit-proxy', defaultLabel());

  const {release, releasePath, checksumPath} = await buildTryItRelease({
    outDir,
    label: values.label ?? process.env.TRYIT_RELEASE_LABEL,
    targetHint: values.target ?? process.env.TRYIT_RELEASE_TARGET,
    allowClientAuth:
      values['allow-client-auth'] ??
      process.env.TRYIT_RELEASE_ALLOW_CLIENT_AUTH === '1',
    openapiManifestPath:
      values['openapi-manifest'] ?? process.env.TRYIT_RELEASE_OPENAPI_MANIFEST,
  });

  console.log(
    `[tryit-proxy-release] wrote release manifest to ${releasePath} (files=${release.files.length})`,
  );
  console.log(
    `[tryit-proxy-release] wrote checksum manifest to ${checksumPath}`,
  );
  if (release.target_hint) {
    console.log(
      `[tryit-proxy-release] target hint recorded: ${release.target_hint}`,
    );
  }
}

const isCli =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isCli) {
  main().catch((error) => {
    console.error('[tryit-proxy-release] fatal error', error);
    process.exitCode = 1;
  });
}
