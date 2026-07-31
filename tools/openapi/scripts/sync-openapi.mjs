#!/usr/bin/env node
/**
 * Generate the Torii OpenAPI spec in artifacts/openapi/torii.json.
 *
 * This script runs the workspace `xtask` binary from the repository root. The
 * `package.json` `sync-openapi` script orchestrates the call.
 */
import {spawn} from 'node:child_process';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {dirname, join, relative, resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {cp, lstat, mkdtemp, mkdir, readdir, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';

import {
  validateReleaseOpenApiDocumentBytes,
} from './lib/openapi-provenance.mjs';
import {
  OPENAPI_MANIFEST_VERSION,
  parseOpenApiManifestV2Json,
  scanJsonRejectDuplicateKeys,
  verifyOpenApiManifestV2,
} from './lib/openapi-manifest-v2.mjs';
import {
  readOpenApiStableFile,
  writeOpenApiAtomicFile,
} from './lib/openapi-safe-file.mjs';
import {isAllowedSigner, loadAllowedSigners} from './lib/openapi-signers.mjs';
import {
  rejectUnknownOpenApiVersionsIndexFields,
  requireOpenApiVersionsIndexFields,
} from './lib/openapi-versions-index.mjs';
import {checkOpenApiSignatures} from './check-openapi-signatures.mjs';
import {verifyOpenApiVersions} from './verify-openapi-versions.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

export const defaultRepoRoot = resolve(__dirname, '..', '..', '..');
const defaultOutputDir = join(defaultRepoRoot, 'artifacts', 'openapi');
const defaultVersionsDir = join(defaultOutputDir, 'versions');
const MANIFEST_SOURCE_BYTES = Symbol('openapi-manifest-source-bytes');
const OPENAPI_SPEC_MAX_BYTES = 64 * 1024 * 1024;
const OPENAPI_MANIFEST_MAX_BYTES = 64 * 1024;
const OPENAPI_VERSIONS_MAX_BYTES = 1024 * 1024;

export function parseArgs(argv) {
  const options = {
    version: 'current',
    latest: false,
    mirrors: [],
    requireSigned: true,
  };

  for (const arg of argv) {
    if (arg.startsWith('--version=')) {
      options.version = arg.slice('--version='.length);
    } else if (arg === '--latest') {
      options.latest = true;
    } else if (arg.startsWith('--mirror=')) {
      const mirror = arg.slice('--mirror='.length);
      if (!mirror) {
        throw new Error('mirror label must not be empty');
      }
      options.mirrors.push(mirror);
    } else if (arg === '--allow-unsigned') {
      options.requireSigned = false;
    } else if (arg === '--require-signed') {
      options.requireSigned = true;
    } else if (arg.startsWith('--allowed-signers=')) {
      const allowedSignersFile = arg.slice('--allowed-signers='.length).trim();
      if (!allowedSignersFile) {
        throw new Error('allowed signers path must not be empty');
      }
      options.allowedSignersFile = resolve(allowedSignersFile);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  validateVersionLabel(options.version, 'version');
  for (const mirror of options.mirrors) {
    validateVersionLabel(mirror, 'mirror');
  }
  if (collectTargetLabels(options).includes('current') && !options.latest) {
    throw new Error("updating the 'current' OpenAPI version requires --latest");
  }

  return options;
}

function validateVersionLabel(label, kind) {
  if (!label) {
    throw new Error(`${kind} must not be empty`);
  }
  if (label === 'latest') {
    throw new Error(`${kind} label "latest" is reserved for the canonical spec alias`);
  }
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(label)) {
    throw new Error(
      `${kind} label must start with an alphanumeric character and contain only alphanumerics, dots, underscores, or hyphens`,
    );
  }
}

function runCargo(repoRoot, args) {
  return new Promise((resolve) => {
    const child = spawn('cargo', args, {
      cwd: repoRoot,
      stdio: 'inherit',
      env: {
        ...process.env,
        NORITO_SKIP_BINDINGS_SYNC: '1'
      }
    });
    child.on('close', (code) => resolve(code ?? 1));
  });
}

const defaultContext = {
  repoRoot: defaultRepoRoot,
  outputDir: defaultOutputDir,
  versionsDir: defaultVersionsDir,
  allowedSignersFile: join(defaultOutputDir, 'allowed_signers.json'),
  async generateSpec(repoRoot, outputFile) {
    const code = await runCargo(repoRoot, [
      'run',
      '--locked',
      '--offline',
      '-p',
      'xtask',
      '--bin',
      'xtask',
      '--',
      'openapi',
      '--output',
      outputFile,
    ]);
    if (code !== 0) {
      throw new Error(`OpenAPI generation failed (cargo exit code ${code})`);
    }
  },
};

export async function syncOpenApi(options, context = defaultContext) {
  const {
    repoRoot,
    outputDir,
    versionsDir,
    generateSpec,
    allowedSignersFile: contextAllowedSignersFile = join(outputDir, 'allowed_signers.json'),
  } = context;
  const requireSignedManifest = options.requireSigned !== false;
  const allowedSignersFile = options.allowedSignersFile ?? contextAllowedSignersFile;

  validateVersionLabel(options.version, 'version');
  for (const mirror of options.mirrors ?? []) {
    validateVersionLabel(mirror, 'mirror');
  }
  const targetLabels = collectTargetLabels(options);
  if (targetLabels.includes('current') && !options.latest) {
    throw new Error("updating the 'current' OpenAPI version requires latest=true");
  }

  const stagingDir = await mkdtemp(join(tmpdir(), 'iroha-openapi-sync-'));
  const generatedSpecPath = join(stagingDir, 'generated-torii.json');
  const stagedOutputDir = join(stagingDir, 'openapi');
  const stagedVersionsDir = join(stagedOutputDir, 'versions');

  try {
    await generateSpec(repoRoot, generatedSpecPath);
    await assertRegularFile(generatedSpecPath, 'generated OpenAPI spec');
    const specBytes = await readOpenApiStableFile(generatedSpecPath, {
      label: 'generated OpenAPI spec',
      maxBytes: OPENAPI_SPEC_MAX_BYTES,
    });
    validateReleaseOpenApiDocumentBytes(specBytes, {
      label: `generated OpenAPI spec ${generatedSpecPath}`,
    });
    await assertRegularTree(outputDir);
    const manifestTemplate = await prepareManifestTemplate(
      join(outputDir, 'manifest.json'),
      specBytes,
      {requireSigned: requireSignedManifest, allowedSignersFile},
    );

    await assertMutableTargets(targetLabels, versionsDir);
    const previousIndex = await readVersionIndexOptional(join(outputDir, 'versions.json'));

    // Build the complete prospective OpenAPI tree away from tracked files.
    // Every historical file and the resulting index are read and validated
    // before publication starts, so validation failures cannot leave a mixed
    // latest/current/index state behind.
    await assertRegularTree(outputDir);
    await cp(outputDir, stagedOutputDir, {recursive: true});
    await assertRegularTree(stagedOutputDir);
    for (const label of targetLabels) {
      await stageVersion(
        join(stagedVersionsDir, label),
        stagedOutputDir,
        specBytes,
        manifestTemplate,
      );
    }
    if (options.latest) {
      await writeFile(join(stagedOutputDir, 'torii.json'), specBytes);
    }

    const prospectiveIndex = await buildVersionIndex(
      stagedVersionsDir,
      stagedOutputDir,
      join(stagedOutputDir, 'torii.json'),
      join(stagedOutputDir, 'manifest.json'),
      {
        previousIndex,
        generatedUnixMs: manifestTemplate.generated_unix_ms,
        allowedSignersFile,
      },
    );
    const prospectiveIndexBytes = Buffer.from(
      serializeJson(prospectiveIndex),
      'utf8',
    );
    await writeFile(join(stagedOutputDir, 'versions.json'), prospectiveIndexBytes);
    const unsignedLabels = prospectiveIndex.entries
      .filter((entry) => !entry.signed)
      .map((entry) => entry.label);
    await verifyOpenApiVersions({
      outputDir: stagedOutputDir,
      versionsDir: stagedVersionsDir,
      versionsFile: join(stagedOutputDir, 'versions.json'),
      allowUnsigned: unsignedLabels.length > 0,
    });
    await checkOpenApiSignatures({
      staticDir: stagedOutputDir,
      versionsFile: join(stagedOutputDir, 'versions.json'),
      allowedSignersFile,
      allowUnsigned: unsignedLabels,
    });

    // Publication contains writes only; every fallible content/index check is
    // complete at this point.
    const manifestBytes =
      manifestTemplate[MANIFEST_SOURCE_BYTES] ??
      Buffer.from(serializeJson(manifestTemplate), 'utf8');
    for (const label of targetLabels) {
      const destinationDir = join(versionsDir, label);
      await mkdir(destinationDir, {recursive: true});
      await writeOpenApiAtomicFile(
        join(destinationDir, 'torii.json'),
        specBytes,
        {label: `OpenAPI version ${label} specification`},
      );
      await writeOpenApiAtomicFile(
        join(destinationDir, 'manifest.json'),
        manifestBytes,
        {label: `OpenAPI version ${label} manifest`},
      );
      console.log(`Torii OpenAPI spec refreshed at ${join(destinationDir, 'torii.json')}`);
    }
    if (options.latest) {
      await writeOpenApiAtomicFile(
        join(outputDir, 'torii.json'),
        specBytes,
        {label: 'latest OpenAPI specification'},
      );
      console.log(`Latest spec pointer updated at ${join(outputDir, 'torii.json')}`);
    }
    await writeOpenApiAtomicFile(
      join(outputDir, 'versions.json'),
      prospectiveIndexBytes,
      {label: 'OpenAPI versions index'},
    );
  } finally {
    await rm(stagingDir, {recursive: true, force: true});
  }
}

function collectTargetLabels(options) {
  const labels = [];
  const seen = new Set();
  for (const label of [options.version, ...(options.mirrors ?? [])]) {
    if (!seen.has(label)) {
      seen.add(label);
      labels.push(label);
    }
  }
  return labels;
}

async function assertMutableTargets(labels, versionsDirPath) {
  for (const label of labels) {
    if (label === 'current') {
      continue;
    }
    if (await pathExists(join(versionsDirPath, label))) {
      throw new Error(
        `historical OpenAPI version '${label}' already exists and is immutable; create a new version label instead`,
      );
    }
  }
}

async function stageVersion(versionDir, outputDir, specBytes, manifestTemplate) {
  await mkdir(versionDir, {recursive: true});
  await writeFile(join(versionDir, 'torii.json'), specBytes);
  const versionManifest = manifestForVersion(
    versionDir,
    outputDir,
    manifestTemplate,
  );
  await writeFile(
    join(versionDir, 'manifest.json'),
    versionManifest[MANIFEST_SOURCE_BYTES] ?? serializeJson(versionManifest),
  );
}

async function buildVersionIndex(
  versionsDirPath = defaultVersionsDir,
  outputDirPath = defaultOutputDir,
  latestSpecPath = join(defaultOutputDir, 'torii.json'),
  latestManifestPath = join(defaultOutputDir, 'manifest.json'),
  {previousIndex = null, generatedUnixMs, allowedSignersFile} = {},
) {
  const entries = await readdir(versionsDirPath, {withFileTypes: true});
  const versionEntries = [];
  const previousEntries = new Map(
    (previousIndex?.entries ?? []).map((entry) => [entry.label, entry]),
  );

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const label = entry.name;
    validateVersionLabel(label, 'stored version');
    const versionDir = join(versionsDirPath, label);
    const specPath = join(versionDir, 'torii.json');
    const manifestPath = join(versionDir, 'manifest.json');
    const metadata = await loadVersionMetadata(label, specPath, manifestPath, outputDirPath, {
      previousEntry: previousEntries.get(label),
      allowedSignersFile,
    });
    if (!metadata) {
      throw new Error(`OpenAPI version directory '${label}' is missing torii.json`);
    }
    versionEntries.push(metadata);
  }

  const latestEntry = await loadVersionMetadata(
    'latest',
    latestSpecPath,
    latestManifestPath,
    outputDirPath,
    {
      previousEntry: previousEntries.get('latest'),
      allowedSignersFile,
    },
  );
  if (latestEntry) {
    versionEntries.push(latestEntry);
  }

  versionEntries.sort((a, b) => {
    if (a.label === 'latest') {
      return -1;
    }
    if (b.label === 'latest') {
      return 1;
    }
    return a.label.localeCompare(b.label);
  });

  const versions = versionEntries
    .filter((entry) => entry.label !== 'latest')
    .map((entry) => entry.label);
  const unchanged = indexEntriesEqual(previousIndex, versions, versionEntries);
  const manifest = {
    versions,
    generatedAt: unchanged
      ? previousIndex.generatedAt
      : isoTimestampFromUnixMs(generatedUnixMs, 'canonical manifest generated_unix_ms'),
    entries: versionEntries,
  };
  validateVersionIndexShape(manifest);
  return manifest;
}

async function prepareManifestTemplate(
  manifestPath,
  specBytes,
  {requireSigned = false, allowedSignersFile} = {},
) {
  try {
    const text = await readOpenApiStableFile(manifestPath, {
      label: `OpenAPI manifest ${manifestPath}`,
      maxBytes: OPENAPI_MANIFEST_MAX_BYTES,
      encoding: 'utf8',
    });
    const manifest = parseOpenApiManifestV2Json(text, {
      label: `manifest ${manifestPath}`,
    });
    verifyOpenApiManifestV2({
      manifest,
      artifactBytes: specBytes,
      label: `manifest ${manifestPath}`,
      expectedArtifactPath: 'torii.json',
      requireSignature: requireSigned,
      requireClean: requireSigned,
    });
    const recorded = manifest?.artifact?.sha256_hex;
    if (typeof recorded !== 'string') {
      throw new Error(
        `manifest ${manifestPath} is missing artifact.sha256_hex; regenerate the canonical manifest before syncing.`,
      );
    }
    const computed = computeSha256Hex(specBytes);
    if (recorded !== computed) {
      throw new Error(
        `manifest ${manifestPath} sha256 (${recorded}) does not match the freshly generated spec (${computed}); ` +
        'regenerate the canonical manifest and re-run sync-openapi.',
      );
    }
    const recordedBytes = manifest?.artifact?.bytes;
    if (!Number.isSafeInteger(recordedBytes) || recordedBytes < 0) {
      throw new Error(
        `manifest ${manifestPath} is missing a valid artifact.bytes value; regenerate the canonical manifest before syncing.`,
      );
    }
    if (recordedBytes !== specBytes.byteLength) {
      throw new Error(
        `manifest ${manifestPath} artifact.bytes (${recordedBytes}) does not match the freshly generated spec (${specBytes.byteLength}); regenerate the canonical manifest and re-run sync-openapi.`,
      );
    }

    const signature = manifest?.artifact?.signature;
    if (requireSigned && !hasSignature(signature)) {
      throw new Error(
        `manifest ${manifestPath} is missing signature fields; emit \`--unsigned-manifest --signing-payload <path>\`, sign that exact V2 payload with the release HSM, then attach it with \`--signature-envelope <path>\` before publishing.`,
      );
    }
    if (signature != null) {
      if (!hasSignature(signature)) {
        throw new Error(`manifest ${manifestPath} has incomplete signature fields`);
      }
      if (!allowedSignersFile) {
        throw new Error('an allowed signers file is required for signed OpenAPI manifests');
      }
      const allowedSigners = await loadAllowedSigners(allowedSignersFile);
      if (
        !isAllowedSigner(allowedSigners, {
          algorithm: signature.algorithm,
          publicKey: signature.public_key_hex,
        })
      ) {
        throw new Error(
          `manifest ${manifestPath} signer is not present in ${allowedSignersFile}`,
        );
      }
    }
    Object.defineProperty(manifest, MANIFEST_SOURCE_BYTES, {
      value: Buffer.from(text, 'utf8'),
      enumerable: false,
      writable: false,
    });
    return manifest;
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      throw new Error(
        `manifest ${manifestPath} not found; generate the canonical manifest before syncing.`,
      );
    }
    throw error;
  }
}

function manifestForVersion(versionDir, outputDir, manifestTemplate) {
  void versionDir;
  void outputDir;
  return manifestTemplate;
}

function computeSha256Hex(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

function toPosix(pathValue) {
  return pathValue.split('\\').join('/');
}

function hasSignature(signature) {
  return Boolean(
    signature &&
      isNonEmptyString(signature.algorithm) &&
      isNonEmptyString(signature.public_key_hex) &&
      isNonEmptyString(signature.signature_hex),
  );
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

async function loadVersionMetadata(
  label,
  specPath,
  manifestPath,
  outputDirPath,
  {previousEntry = null, allowedSignersFile} = {},
) {
  const specBuffer = await readFileOptional(specPath, {
    label: `OpenAPI version ${label}`,
    maxBytes: OPENAPI_SPEC_MAX_BYTES,
  });
  if (!specBuffer) {
    return null;
  }
  validateReleaseOpenApiDocumentBytes(specBuffer, {
    label: `OpenAPI version ${label} at ${specPath}`,
  });
  const specSha = computeSha256Hex(specBuffer);
  const manifestDetails = await loadManifestDetails(manifestPath, outputDirPath, {
    specPath,
    specSha,
    specBytes: specBuffer.length,
    specBuffer,
    allowedSignersFile,
  });
  const entry = {
    label,
    path: toPosix(relative(outputDirPath, specPath)),
    bytes: specBuffer.length,
    sha256: specSha,
    blake3: manifestDetails.blake3,
    signed: manifestDetails.signed,
    manifestPath: manifestDetails.path,
    signatureAlgorithm: manifestDetails.signatureAlgorithm,
    signaturePublicKeyHex: manifestDetails.signaturePublicKeyHex,
    signatureHex: manifestDetails.signatureHex,
  };
  entry.updatedAt = versionEntryIdentityEqual(previousEntry, entry)
    ? previousEntry.updatedAt
    : isoTimestampFromUnixMs(
        manifestDetails.generatedUnixMs,
        `${label} manifest generated_unix_ms`,
      );
  return entry;
}

async function readFileOptional(path, options) {
  try {
    return await readOpenApiStableFile(path, options);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function pathExists(path) {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return false;
    }
    throw error;
  }
}

async function assertRegularFile(path, label) {
  const metadata = await lstat(path);
  if (!metadata.isFile()) {
    throw new Error(`${label} must be a regular file`);
  }
  if (metadata.nlink !== 1) {
    throw new Error(`${label} must have exactly one hard link`);
  }
}

async function assertRegularTree(path) {
  const metadata = await lstat(path);
  if (metadata.isFile()) {
    if (metadata.nlink !== 1) {
      throw new Error(`OpenAPI artifact tree contains a hard-linked file: ${path}`);
    }
    return;
  }
  if (!metadata.isDirectory()) {
    throw new Error(`OpenAPI artifact tree contains a non-regular path: ${path}`);
  }
  const entries = await readdir(path, {withFileTypes: true});
  for (const entry of entries) {
    await assertRegularTree(join(path, entry.name));
  }
}

async function loadManifestDetails(manifestPath, outputDirPath, specContext = null) {
  try {
    const text = await readOpenApiStableFile(manifestPath, {
      label: `OpenAPI manifest ${manifestPath}`,
      maxBytes: OPENAPI_MANIFEST_MAX_BYTES,
      encoding: 'utf8',
    });
    const manifest = parseOpenApiManifestV2Json(text, {
      label: `manifest ${manifestPath}`,
    });
    const artifact = manifest?.artifact;
    if (specContext && !manifestMatchesSpec(artifact, specContext, outputDirPath)) {
      throw new Error(`manifest ${manifestPath} does not match its OpenAPI spec`);
    }
    if (manifest.version !== OPENAPI_MANIFEST_VERSION) {
      throw new Error(
        `manifest ${manifestPath} has unsupported version ${manifest.version}; expected ${OPENAPI_MANIFEST_VERSION}`,
      );
    }
    const generatedUnixMs = manifest.generated_unix_ms;
    if (!Number.isSafeInteger(generatedUnixMs) || generatedUnixMs < 0) {
      throw new Error(`manifest ${manifestPath} has invalid generated_unix_ms`);
    }
    const signature = artifact?.signature;
    if (signature != null && !hasSignature(signature)) {
      throw new Error(`manifest ${manifestPath} has incomplete signature fields`);
    }
    verifyOpenApiManifestV2({
      manifest,
      artifactBytes: specContext.specBuffer,
      label: `manifest ${manifestPath}`,
      expectedArtifactPath: toPosix(relative(dirname(manifestPath), specContext.specPath)),
      requireSignature: signature != null,
      requireClean: signature != null,
    });
    if (signature != null) {
      const allowedSigners = await loadAllowedSigners(specContext.allowedSignersFile);
      if (
        !isAllowedSigner(allowedSigners, {
          algorithm: signature.algorithm,
          publicKey: signature.public_key_hex,
        })
      ) {
        throw new Error(`manifest ${manifestPath} signer is not allowed`);
      }
    }
    return {
      signed: signature != null,
      signatureAlgorithm: signature?.algorithm ?? null,
      signatureHex: signature?.signature_hex ?? null,
      signaturePublicKeyHex: signature?.public_key_hex ?? null,
      path: toPosix(relative(outputDirPath, manifestPath)),
      blake3: artifact?.blake3_hex ?? null,
      generatedUnixMs,
    };
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return unsignedManifestDetails();
    }
    throw error;
  }
}

function manifestMatchesSpec(artifact, specContext, outputDirPath) {
  void outputDirPath;
  if (!artifact || typeof artifact !== 'object') {
    return false;
  }
  const expectedPath = toPosix(
    relative(dirname(specContext.specPath), specContext.specPath),
  );
  return (
    artifact.path === expectedPath &&
    artifact.bytes === specContext.specBytes &&
    artifact.sha256_hex === specContext.specSha
  );
}

function unsignedManifestDetails() {
  return {
    signed: false,
    signatureAlgorithm: null,
    signatureHex: null,
    signaturePublicKeyHex: null,
    path: null,
    blake3: null,
    generatedUnixMs: null,
  };
}

async function readVersionIndexOptional(path) {
  const bytes = await readFileOptional(path, {
    label: 'OpenAPI versions index',
    maxBytes: OPENAPI_VERSIONS_MAX_BYTES,
  });
  if (!bytes) {
    return null;
  }
  const text = bytes.toString('utf8');
  scanJsonRejectDuplicateKeys(text, `OpenAPI versions index ${path}`);
  let index;
  try {
    index = JSON.parse(text);
  } catch (error) {
    throw new Error(`failed to parse ${path}: ${error.message ?? error}`);
  }
  validateVersionIndexShape(index);
  return index;
}

function validateVersionIndexShape(index) {
  if (!index || !Array.isArray(index.versions) || !Array.isArray(index.entries)) {
    throw new Error('OpenAPI versions index must contain versions and entries arrays');
  }
  rejectUnknownOpenApiVersionsIndexFields(index);
  requireOpenApiVersionsIndexFields(index);
  assertIsoTimestamp(index.generatedAt, 'OpenAPI versions index generatedAt');
  const labels = new Set();
  for (const entry of index.entries) {
    if (!entry || !isNonEmptyString(entry.label) || labels.has(entry.label)) {
      throw new Error('OpenAPI versions index contains an invalid or duplicate label');
    }
    labels.add(entry.label);
    assertIsoTimestamp(entry.updatedAt, `OpenAPI versions entry ${entry.label} updatedAt`);
  }
}

function assertIsoTimestamp(value, label) {
  if (!isNonEmptyString(value)) {
    throw new Error(`${label} must be an ISO-8601 timestamp`);
  }
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime()) || parsed.toISOString() !== value) {
    throw new Error(`${label} must be an ISO-8601 timestamp`);
  }
}

function isoTimestampFromUnixMs(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label} must be a non-negative safe integer`);
  }
  return new Date(value).toISOString();
}

function versionEntryIdentityEqual(previous, next) {
  if (!previous) {
    return false;
  }
  for (const field of [
    'label',
    'path',
    'bytes',
    'sha256',
    'blake3',
    'signed',
    'manifestPath',
    'signatureAlgorithm',
    'signaturePublicKeyHex',
    'signatureHex',
  ]) {
    if (previous[field] !== next[field]) {
      return false;
    }
  }
  return true;
}

function indexEntriesEqual(previousIndex, versions, entries) {
  if (
    !previousIndex ||
    JSON.stringify(previousIndex.versions) !== JSON.stringify(versions) ||
    previousIndex.entries.length !== entries.length
  ) {
    return false;
  }
  const fields = [
    'label',
    'path',
    'bytes',
    'sha256',
    'blake3',
    'updatedAt',
    'signed',
    'manifestPath',
    'signatureAlgorithm',
    'signaturePublicKeyHex',
    'signatureHex',
  ];
  return entries.every((entry, index) =>
    fields.every((field) => previousIndex.entries[index]?.[field] === entry[field]),
  );
}

function serializeJson(value) {
  return JSON.stringify(value, null, 2);
}

async function runCli() {
  const options = parseArgs(process.argv.slice(2));
  await syncOpenApi(options);
}

const invokedUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : undefined;
if (invokedUrl === import.meta.url) {
  runCli().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
