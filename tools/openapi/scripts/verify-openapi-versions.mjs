#!/usr/bin/env node
/**
 * Verifies that artifacts/openapi/versions.json matches the on-disk
 * Torii OpenAPI specs and manifests. CI calls this from ci/check_openapi_spec.sh.
 */
import {createHash} from 'node:crypto';
import {lstat, readdir} from 'node:fs/promises';
import {dirname, isAbsolute, join, relative, resolve, sep} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

import {
  validateReleaseOpenApiDocumentBytes,
} from './lib/openapi-provenance.mjs';
import {
  OPENAPI_MANIFEST_VERSION,
  parseOpenApiManifestV2Json,
  scanJsonRejectDuplicateKeys,
  verifyOpenApiManifestV2,
} from './lib/openapi-manifest-v2.mjs';
import {readOpenApiStableFile} from './lib/openapi-safe-file.mjs';
import {
  rejectUnknownOpenApiVersionsIndexFields,
  requireOpenApiVersionsIndexFields,
} from './lib/openapi-versions-index.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const defaultOutputDir = resolve(
  __dirname,
  '..',
  '..',
  '..',
  'artifacts',
  'openapi',
);
const defaultVersionsDir = join(defaultOutputDir, 'versions');
const defaultVersionsFile = join(defaultOutputDir, 'versions.json');
const staleHint =
  "Run 'npm --prefix tools/openapi run sync-openapi -- --latest' to refresh the version manifest.";
const OPENAPI_SPEC_MAX_BYTES = 64 * 1024 * 1024;
const OPENAPI_MANIFEST_MAX_BYTES = 64 * 1024;
const OPENAPI_VERSIONS_MAX_BYTES = 1024 * 1024;
const GIT_SHA1_HEX = /^[0-9a-f]{40}$/;

export async function verifyOpenApiVersions(options = {}) {
  const outputDir = options.outputDir ?? defaultOutputDir;
  const versionsDir = options.versionsDir ?? defaultVersionsDir;
  const versionsFile = options.versionsFile ?? defaultVersionsFile;
  const expectedGeneratorCommit = validateExpectedGeneratorCommit(
    options.expectedGeneratorCommit,
  );

  const manifest = await readJsonFile(versionsFile, `versions manifest ${versionsFile} is missing. ${staleHint}`);
  validateManifestStructure(manifest);

  const entries = manifest.entries;
  const latestEntry = requireEntry(entries, 'latest');
  const currentEntry = requireEntry(entries, 'current');
  ensureVersionsList(manifest.versions, entries);
  await ensureDirectoryCoverage(versionsDir, entries);
  ensureLatestAndCurrentAligned(latestEntry, currentEntry);

  const verifiedEntries = new Map();
  for (const entry of entries) {
    const verified = await verifyEntry(entry, {
      outputDir,
      allowDirtyUnsigned: options.allowUnsigned === true,
      expectedGeneratorCommit:
        entry.label === 'latest' || entry.label === 'current'
          ? expectedGeneratorCommit
          : undefined,
    });
    if (entry.label === 'latest' || entry.label === 'current') {
      verifiedEntries.set(entry.label, verified);
    }
  }
  ensureMutableAliasPaths(latestEntry, currentEntry);
  ensureLatestAndCurrentCopiesAligned(verifiedEntries);
}

async function ensureDirectoryCoverage(versionsDir, entries) {
  const labels = new Set(entries.map((entry) => entry.label));
  let dirEntries;
  try {
    const rootMetadata = await lstat(versionsDir);
    if (rootMetadata.isSymbolicLink() || !rootMetadata.isDirectory()) {
      throw new Error(`OpenAPI versions directory ${versionsDir} must be a regular directory.`);
    }
    dirEntries = await readdir(versionsDir, {withFileTypes: true});
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      throw new Error(
        `OpenAPI versions directory ${versionsDir} not found. ${staleHint}`,
      );
    }
    throw error;
  }

  for (const dirent of dirEntries) {
    if (dirent.isSymbolicLink() || !dirent.isDirectory()) {
      throw new Error(
        `OpenAPI versions directory contains a non-directory entry ${dirent.name}.`,
      );
    }
    if (!labels.has(dirent.name)) {
      throw new Error(
        `versions.json is missing an entry for ${dirent.name}. ${staleHint}`,
      );
    }
  }
}

async function verifyEntry(entry, context) {
  if (!entry || typeof entry.label !== 'string' || entry.label.trim().length === 0) {
    throw new Error('versions.json entry is missing a label.');
  }
  if (typeof entry.path !== 'string' || entry.path.trim().length === 0) {
    throw new Error(`versions.json entry ${entry.label} is missing a path.`);
  }
  ensureIsoTimestamp(entry.updatedAt, `versions.json entry ${entry.label} updatedAt`);

  const specPath = ensurePathWithinOutputDir(context.outputDir, entry.path, entry.label, 'path');
  const specBuffer = await readBinaryFile(
    specPath,
    `OpenAPI spec ${specPath} referenced by ${entry.label} is missing. ${staleHint}`,
  );
  validateReleaseOpenApiDocumentBytes(specBuffer, {
    label: `OpenAPI spec ${specPath} referenced by ${entry.label}`,
  });
  const digest = computeSha256Hex(specBuffer);
  const recordedSha = entry.sha256;
  if (recordedSha !== digest) {
    throw new Error(
      `versions.json sha256 for ${entry.label} (${recordedSha}) does not match ${digest}. ${staleHint}`,
    );
  }
  const recordedBytes = entry.bytes;
  if (typeof recordedBytes !== 'number' || recordedBytes !== specBuffer.length) {
    throw new Error(
      `versions.json bytes for ${entry.label} (${recordedBytes}) do not match the spec (${specBuffer.length}). ${staleHint}`,
    );
  }

  let manifestBytes = null;
  if (entry.manifestPath) {
    const manifestPath = ensurePathWithinOutputDir(
      context.outputDir,
      entry.manifestPath,
      entry.label,
      'manifestPath',
    );
    manifestBytes = await verifyManifest(entry, manifestPath, context.outputDir, {
      specPath,
      specSha: digest,
      specBytes: specBuffer.length,
      specBuffer,
      allowDirtyUnsigned: context.allowDirtyUnsigned,
      expectedGeneratorCommit: context.expectedGeneratorCommit,
    });
  } else if (entry.signed) {
    throw new Error(
      `versions.json entry ${entry.label} is marked as signed but has no manifestPath.`,
    );
  }
  return {specBuffer, manifestBytes};
}

async function verifyManifest(entry, manifestPath, outputDir, specContext) {
  const manifestText = await readFileSafe(
    manifestPath,
    `manifest ${manifestPath} referenced by ${entry.label} is missing. ${staleHint}`,
    {
      label: `OpenAPI manifest ${manifestPath}`,
      maxBytes: OPENAPI_MANIFEST_MAX_BYTES,
      encoding: 'utf8',
    },
  );
  const manifest = parseOpenApiManifestV2Json(manifestText, {
    label: `manifest ${manifestPath}`,
  });
  if (manifest.version !== OPENAPI_MANIFEST_VERSION) {
    throw new Error(
      `manifest ${manifestPath} must use version ${OPENAPI_MANIFEST_VERSION}. ${staleHint}`,
    );
  }
  if (typeof manifest.generated_unix_ms !== 'number') {
    throw new Error(`manifest ${manifestPath} is missing generated_unix_ms. ${staleHint}`);
  }
  const recordedSignature = manifest?.artifact?.signature;
  const manifestSigned = Boolean(
    recordedSignature &&
      isNonEmptyString(recordedSignature.algorithm) &&
      isNonEmptyString(recordedSignature.public_key_hex) &&
      isNonEmptyString(recordedSignature.signature_hex),
  );
  verifyOpenApiManifestV2({
    manifest,
    artifactBytes: specContext.specBuffer,
    label: `manifest ${manifestPath}`,
    expectedArtifactPath: toPosix(
      relative(dirname(manifestPath), specContext.specPath),
    ),
    requireSignature: Boolean(entry.signed),
    requireClean:
      specContext.expectedGeneratorCommit !== undefined ||
      !specContext.allowDirtyUnsigned,
  });
  if (
    specContext.expectedGeneratorCommit !== undefined &&
    manifest.generator_commit !== specContext.expectedGeneratorCommit
  ) {
    throw new Error(
      `manifest ${manifestPath} generator_commit (${String(manifest.generator_commit)}) does not match expected source commit ${specContext.expectedGeneratorCommit}. ${staleHint}`,
    );
  }
  const artifact = manifest?.artifact;
  if (!artifact || typeof artifact.path !== 'string') {
    throw new Error(
      `manifest ${manifestPath} is missing artifact.path. ${staleHint}`,
    );
  }
  const expectedArtifactPath = toPosix(
    relative(dirname(manifestPath), specContext.specPath),
  );
  if (artifact.path !== expectedArtifactPath) {
    throw new Error(
      `manifest ${manifestPath} references ${artifact.path} but versions.json lists ${entry.path}. ${staleHint}`,
    );
  }
  if (typeof artifact.bytes !== 'number' || artifact.bytes !== specContext.specBytes) {
    throw new Error(
      `manifest ${manifestPath} bytes (${artifact.bytes}) do not match the spec (${specContext.specBytes}). ${staleHint}`,
    );
  }
  if (typeof entry.bytes === 'number' && artifact.bytes !== entry.bytes) {
    throw new Error(
      `manifest ${manifestPath} bytes (${artifact.bytes}) do not match versions.json (${entry.bytes}). ${staleHint}`,
    );
  }
  if (!isNonEmptyString(artifact.sha256_hex)) {
    throw new Error(
      `manifest ${manifestPath} is missing artifact.sha256_hex. ${staleHint}`,
    );
  }
  if (artifact.sha256_hex !== specContext.specSha) {
    throw new Error(
      `manifest ${manifestPath} sha256 (${artifact.sha256_hex}) does not match the spec (${specContext.specSha}). ${staleHint}`,
    );
  }
  if (Boolean(entry.signed) !== manifestSigned) {
    throw new Error(
      `versions.json entry ${entry.label} signed=${entry.signed} disagrees with manifest ${manifestPath}. ${staleHint}`,
    );
  }
  const manifestBlake3 = artifact.blake3_hex ?? null;
  if (manifestSigned) {
    if (!isNonEmptyString(entry.blake3)) {
      throw new Error(
        `versions.json entry ${entry.label} is signed but missing blake3. ${staleHint}`,
      );
    }
    if (!isNonEmptyString(manifestBlake3)) {
      throw new Error(
        `manifest ${manifestPath} is signed but missing artifact.blake3_hex. ${staleHint}`,
      );
    }
  }
  if (manifestSigned) {
    compareHexField(
      entry.signatureAlgorithm,
      recordedSignature.algorithm,
      `signature algorithm for ${entry.label}`,
    );
    compareHexField(
      entry.signaturePublicKeyHex,
      recordedSignature.public_key_hex,
      `signature public key for ${entry.label}`,
    );
    compareHexField(
      entry.signatureHex,
      recordedSignature.signature_hex,
      `signature hex for ${entry.label}`,
    );
  }
  compareHexField(entry.blake3, manifestBlake3, `BLAKE3 digest for ${entry.label}`);
  return Buffer.from(manifestText, 'utf8');
}

function ensurePathWithinOutputDir(outputDir, relativePath, label, fieldName) {
  if (!isNonEmptyString(relativePath)) {
    throw new Error(`versions.json entry ${label} is missing ${fieldName}. ${staleHint}`);
  }
  const segments = relativePath.split('/');
  if (isAbsolute(relativePath) || /^[A-Za-z]:/.test(relativePath)) {
    throw new Error(
      `versions.json entry ${label} ${fieldName} must be relative to the OpenAPI output directory. ${staleHint}`,
    );
  }
  if (segments.some((segment) => segment === '..')) {
    throw new Error(
      `versions.json entry ${label} ${fieldName} escapes the OpenAPI output directory. ${staleHint}`,
    );
  }
  if (
    relativePath.trim() !== relativePath ||
    relativePath.includes('\\') ||
    segments.some((segment) => segment === '' || segment === '.')
  ) {
    throw new Error(
      `versions.json entry ${label} ${fieldName} must use canonical forward-slash segments. ${staleHint}`,
    );
  }
  const resolvedRoot = resolve(outputDir);
  const resolvedTarget = resolve(resolvedRoot, relativePath);
  const relativeToRoot = relative(resolvedRoot, resolvedTarget);
  if (
    relativeToRoot.startsWith('..') ||
    relativeToRoot.startsWith(`..${sep}`) ||
    isAbsolute(relativeToRoot)
  ) {
    throw new Error(
      `versions.json entry ${label} ${fieldName} escapes the OpenAPI output directory. ${staleHint}`,
    );
  }
  return resolvedTarget;
}

function compareHexField(recorded, expected, label) {
  if ((recorded ?? null) !== (expected ?? null)) {
    throw new Error(`${label} mismatch (${recorded} vs ${expected}). ${staleHint}`);
  }
}

function ensureVersionsList(versionsList, entries) {
  if (!Array.isArray(versionsList)) {
    throw new Error(`versions.json is missing the versions array. ${staleHint}`);
  }
  const expected = canonicalize(
    entries.filter((entry) => entry.label !== 'latest').map((entry) => entry.label),
    'versions.json entries',
  );
  const recorded = canonicalize(versionsList, 'versions.json versions');
  if (!arraysEqual(recorded, expected)) {
    throw new Error(
      `versions.json versions array (${versionsList.join(', ')}) does not match entries (${expected.join(', ')}). ${staleHint}`,
    );
  }
}

function canonicalize(values, label) {
  const seen = new Set();
  const canonical = [];
  for (const [index, value] of values.entries()) {
    if (typeof value !== 'string' || value === '') {
      throw new Error(`${label}[${index}] must be a non-empty string. ${staleHint}`);
    }
    if (value.trim() !== value) {
      throw new Error(`${label}[${index}] must not contain surrounding whitespace. ${staleHint}`);
    }
    if (seen.has(value)) {
      throw new Error(`${label}[${index}] duplicates ${value}. ${staleHint}`);
    }
    seen.add(value);
    canonical.push(value);
  }
  return canonical.sort();
}

function arraysEqual(a, b) {
  if (a.length !== b.length) {
    return false;
  }
  return a.every((value, index) => value === b[index]);
}

function requireEntry(entries, label) {
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error(`versions.json has no entries. ${staleHint}`);
  }
  const entry = entries.find((candidate) => candidate.label === label);
  if (!entry) {
    throw new Error(`versions.json is missing the '${label}' entry. ${staleHint}`);
  }
  return entry;
}

function ensureLatestAndCurrentAligned(latestEntry, currentEntry) {
  if (latestEntry.sha256 !== currentEntry.sha256 || latestEntry.bytes !== currentEntry.bytes) {
    throw new Error(
      `versions.json latest entry must match current entry for digest and size. ${staleHint}`,
    );
  }
  if (Boolean(latestEntry.signed) !== Boolean(currentEntry.signed)) {
    throw new Error(
      `versions.json latest entry signed=${latestEntry.signed} disagrees with current signed=${currentEntry.signed}. ${staleHint}`,
    );
  }
  compareHexField(latestEntry.blake3, currentEntry.blake3, 'latest/current blake3 digest');
  compareHexField(
    latestEntry.signatureAlgorithm,
    currentEntry.signatureAlgorithm,
    'latest/current signature algorithm',
  );
  compareHexField(
    latestEntry.signaturePublicKeyHex,
    currentEntry.signaturePublicKeyHex,
    'latest/current signature public key',
  );
  compareHexField(latestEntry.signatureHex, currentEntry.signatureHex, 'latest/current signature');
  if (latestEntry.updatedAt !== currentEntry.updatedAt) {
    throw new Error(
      `versions.json latest updatedAt must match current updatedAt. ${staleHint}`,
    );
  }
}

function ensureMutableAliasPaths(latestEntry, currentEntry) {
  for (const [entry, expectedPath, expectedManifestPath] of [
    [latestEntry, 'torii.json', 'manifest.json'],
    [
      currentEntry,
      'versions/current/torii.json',
      'versions/current/manifest.json',
    ],
  ]) {
    if (entry.path !== expectedPath) {
      throw new Error(
        `versions.json ${entry.label} path must be exactly ${expectedPath}. ${staleHint}`,
      );
    }
    if (entry.manifestPath !== expectedManifestPath) {
      throw new Error(
        `versions.json ${entry.label} manifestPath must be exactly ${expectedManifestPath}. ${staleHint}`,
      );
    }
  }
}

function ensureLatestAndCurrentCopiesAligned(verifiedEntries) {
  const latest = verifiedEntries.get('latest');
  const current = verifiedEntries.get('current');
  if (!latest.specBuffer.equals(current.specBuffer)) {
    throw new Error(
      `checked-in latest and current OpenAPI specs must be byte-identical. ${staleHint}`,
    );
  }
  if (
    latest.manifestBytes === null ||
    current.manifestBytes === null ||
    !latest.manifestBytes.equals(current.manifestBytes)
  ) {
    throw new Error(
      `checked-in latest and current OpenAPI manifests must be byte-identical. ${staleHint}`,
    );
  }
}

function validateExpectedGeneratorCommit(value) {
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== 'string' || !GIT_SHA1_HEX.test(value)) {
    throw new Error(
      'expectedGeneratorCommit must be exactly 40 lowercase hexadecimal characters',
    );
  }
  return value;
}

async function readJsonFile(path, missingMessage) {
  const text = await readFileSafe(path, missingMessage, {
    label: `OpenAPI versions manifest ${path}`,
    maxBytes: OPENAPI_VERSIONS_MAX_BYTES,
    encoding: 'utf8',
  });
  scanJsonRejectDuplicateKeys(text, `OpenAPI versions manifest ${path}`);
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`Failed to parse ${path}: ${error?.message ?? error}`);
  }
}

async function readFileSafe(path, missingMessage, options) {
  try {
    return await readOpenApiStableFile(path, options);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      throw new Error(missingMessage);
    }
    throw error;
  }
}

async function readBinaryFile(path, missingMessage) {
  try {
    return await readOpenApiStableFile(path, {
      label: `OpenAPI specification ${path}`,
      maxBytes: OPENAPI_SPEC_MAX_BYTES,
    });
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      throw new Error(missingMessage);
    }
    throw error;
  }
}

function validateManifestStructure(manifest) {
  if (!manifest || typeof manifest !== 'object') {
    throw new Error(`versions.json is malformed. ${staleHint}`);
  }
  rejectUnknownOpenApiVersionsIndexFields(manifest, {
    label: 'versions.json',
  });
  requireOpenApiVersionsIndexFields(manifest, {
    label: 'versions.json',
  });
  ensureIsoTimestamp(manifest.generatedAt, 'versions.json generatedAt');
  if (!Array.isArray(manifest.entries) || manifest.entries.length === 0) {
    throw new Error(`versions.json has no entries. ${staleHint}`);
  }
}

function ensureIsoTimestamp(value, label) {
  if (!isIsoTimestamp(value)) {
    throw new Error(`${label} must be an ISO-8601 timestamp. ${staleHint}`);
  }
}

export function isIsoTimestamp(value) {
  if (!isNonEmptyString(value)) {
    return false;
  }
  const parsed = Date.parse(value);
  const hasTimezone = /T.+(Z|[+-]\d{2}:?\d{2})$/.test(value);
  return !Number.isNaN(parsed) && hasTimezone;
}

function computeSha256Hex(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

function toPosix(pathValue) {
  return pathValue.split('\\').join('/');
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

async function runCli() {
  const args = process.argv.slice(2);
  let allowUnsigned = false;
  let expectedGeneratorCommit;
  const unknown = [];
  for (const arg of args) {
    if (arg === '--allow-unsigned') {
      allowUnsigned = true;
    } else if (arg.startsWith('--expected-generator-commit=')) {
      if (expectedGeneratorCommit !== undefined) {
        throw new Error(
          'verify-openapi-versions accepts --expected-generator-commit only once',
        );
      }
      expectedGeneratorCommit = arg.slice(
        '--expected-generator-commit='.length,
      );
    } else {
      unknown.push(arg);
    }
  }
  if (unknown.length > 0) {
    throw new Error(`unknown verify-openapi-versions option: ${unknown.join(', ')}`);
  }
  await verifyOpenApiVersions({allowUnsigned, expectedGeneratorCommit});
}

const invokedUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : undefined;
if (invokedUrl === import.meta.url) {
  runCli().catch((error) => {
    console.error(error.message ?? error);
    process.exit(1);
  });
}
