#!/usr/bin/env node
/**
 * Verifies that docs/portal/static/openapi/versions.json matches the on-disk
 * Torii OpenAPI specs and manifests. CI calls this from ci/check_openapi_spec.sh.
 */
import {createHash} from 'node:crypto';
import {readdir, readFile} from 'node:fs/promises';
import {dirname, isAbsolute, join, relative, resolve, sep} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const defaultOutputDir = join(__dirname, '..', 'static', 'openapi');
const defaultVersionsDir = join(defaultOutputDir, 'versions');
const defaultVersionsFile = join(defaultOutputDir, 'versions.json');
const staleHint = "Run 'npm run sync-openapi -- --latest' from docs/portal/ to refresh the version manifest.";

export async function verifyOpenApiVersions(options = {}) {
  const outputDir = options.outputDir ?? defaultOutputDir;
  const versionsDir = options.versionsDir ?? defaultVersionsDir;
  const versionsFile = options.versionsFile ?? defaultVersionsFile;

  const manifest = await readJsonFile(versionsFile, `versions manifest ${versionsFile} is missing. ${staleHint}`);
  validateManifestStructure(manifest);

  const entries = manifest.entries;
  const latestEntry = requireEntry(entries, 'latest');
  const currentEntry = requireEntry(entries, 'current');
  ensureVersionsList(manifest.versions, entries);
  await ensureDirectoryCoverage(versionsDir, entries);
  ensureLatestAndCurrentAligned(latestEntry, currentEntry);

  for (const entry of entries) {
    await verifyEntry(entry, {outputDir});
  }
}

async function ensureDirectoryCoverage(versionsDir, entries) {
  const labels = new Set(entries.map((entry) => entry.label));
  let dirEntries;
  try {
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
    if (!dirent.isDirectory()) {
      continue;
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
  const digest = computeSha256Hex(specBuffer);
  const recordedSha = entry.sha256;
  if (!equalsIgnoreCase(recordedSha, digest)) {
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

  if (entry.manifestPath) {
    const manifestPath = ensurePathWithinOutputDir(
      context.outputDir,
      entry.manifestPath,
      entry.label,
      'manifestPath',
    );
    await verifyManifest(entry, manifestPath, context.outputDir, {
      specPath,
      specSha: digest,
      specBytes: specBuffer.length,
    });
  } else if (entry.signed) {
    throw new Error(
      `versions.json entry ${entry.label} is marked as signed but has no manifestPath.`,
    );
  }
}

async function verifyManifest(entry, manifestPath, outputDir, specContext) {
  const manifest = await readJsonFile(
    manifestPath,
    `manifest ${manifestPath} referenced by ${entry.label} is missing. ${staleHint}`,
  );
  if (typeof manifest.version !== 'number') {
    throw new Error(`manifest ${manifestPath} is missing a numeric version. ${staleHint}`);
  }
  if (typeof manifest.generated_unix_ms !== 'number') {
    throw new Error(`manifest ${manifestPath} is missing generated_unix_ms. ${staleHint}`);
  }
  if (!isNonEmptyString(manifest.generator_commit)) {
    throw new Error(`manifest ${manifestPath} is missing generator_commit. ${staleHint}`);
  }
  const artifact = manifest?.artifact;
  if (!artifact || typeof artifact.path !== 'string') {
    throw new Error(
      `manifest ${manifestPath} is missing artifact.path. ${staleHint}`,
    );
  }
  const expectedArtifactPath = toPosix(relative(outputDir, specContext.specPath));
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
  if (!equalsIgnoreCase(artifact.sha256_hex, specContext.specSha)) {
    throw new Error(
      `manifest ${manifestPath} sha256 (${artifact.sha256_hex}) does not match the spec (${specContext.specSha}). ${staleHint}`,
    );
  }
  const recordedSignature = artifact.signature;
  const manifestSigned = Boolean(
    recordedSignature &&
      isNonEmptyString(recordedSignature.algorithm) &&
      isNonEmptyString(recordedSignature.public_key_hex) &&
      isNonEmptyString(recordedSignature.signature_hex),
  );
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
}

function ensurePathWithinOutputDir(outputDir, relativePath, label, fieldName) {
  if (!isNonEmptyString(relativePath)) {
    throw new Error(`versions.json entry ${label} is missing ${fieldName}. ${staleHint}`);
  }
  if (isAbsolute(relativePath)) {
    throw new Error(
      `versions.json entry ${label} ${fieldName} must be relative to the OpenAPI output directory. ${staleHint}`,
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
  if (!equalsIgnoreCase(recorded ?? null, expected ?? null)) {
    throw new Error(`${label} mismatch (${recorded} vs ${expected}). ${staleHint}`);
  }
}

function ensureVersionsList(versionsList, entries) {
  if (!Array.isArray(versionsList)) {
    throw new Error(`versions.json is missing the versions array. ${staleHint}`);
  }
  const expected = canonicalize(entries.filter((entry) => entry.label !== 'latest').map((entry) => entry.label));
  const recorded = canonicalize(versionsList);
  if (!arraysEqual(recorded, expected)) {
    throw new Error(
      `versions.json versions array (${versionsList.join(', ')}) does not match entries (${expected.join(', ')}). ${staleHint}`,
    );
  }
}

function canonicalize(values) {
  const seen = new Set();
  return values
    .filter((value) => typeof value === 'string' && value.trim().length > 0)
    .map((value) => value.trim())
    .filter((value) => {
      if (seen.has(value)) {
        return false;
      }
      seen.add(value);
      return true;
    })
    .sort();
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
  if (!equalsIgnoreCase(latestEntry.sha256, currentEntry.sha256) || latestEntry.bytes !== currentEntry.bytes) {
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
}

async function readJsonFile(path, missingMessage) {
  const text = await readFileSafe(path, missingMessage);
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`Failed to parse ${path}: ${error?.message ?? error}`);
  }
}

async function readFileSafe(path, missingMessage) {
  try {
    return await readFile(path, 'utf8');
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      throw new Error(missingMessage);
    }
    throw error;
  }
}

async function readBinaryFile(path, missingMessage) {
  try {
    return await readFile(path);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      throw new Error(missingMessage);
    }
    throw error;
  }
}

async function validateManifestStructure(manifest) {
  if (!manifest || typeof manifest !== 'object') {
    throw new Error(`versions.json is malformed. ${staleHint}`);
  }
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

function equalsIgnoreCase(a, b) {
  if (a == null && b == null) {
    return true;
  }
  if (typeof a !== 'string' || typeof b !== 'string') {
    return false;
  }
  return a.toLowerCase() === b.toLowerCase();
}

function toPosix(pathValue) {
  return pathValue.split('\\').join('/');
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

async function runCli() {
  await verifyOpenApiVersions();
}

const invokedUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : undefined;
if (invokedUrl === import.meta.url) {
  runCli().catch((error) => {
    console.error(error.message ?? error);
    process.exit(1);
  });
}
