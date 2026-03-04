#!/usr/bin/env node
/**
 * Generate the Torii OpenAPI spec in docs/portal/static/openapi/torii.json.
 *
 * This script runs `cargo xtask openapi` from the repository root. The `package.json`
 * `sync-openapi` script orchestrates the call.
 */
import {spawn} from 'node:child_process';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {dirname, join, relative} from 'node:path';
import {createHash} from 'node:crypto';
import {mkdir, readFile, readdir, stat, writeFile} from 'node:fs/promises';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const defaultRepoRoot = join(__dirname, '..', '..');
const defaultOutputDir = join(__dirname, '..', 'static', 'openapi');
const defaultVersionsDir = join(defaultOutputDir, 'versions');

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
    }
  }

  if (!options.version) {
    throw new Error('version must not be empty');
  }

  return options;
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
  async generateSpec(repoRoot, outputFile) {
    const primaryCode = await runCargo(repoRoot, ['xtask', 'openapi', '--output', outputFile]);
    if (primaryCode === 0) {
      return;
    }

    console.warn(
      `cargo xtask openapi exited with code ${primaryCode}; falling back to 'cargo run -p xtask --bin xtask -- openapi ...'`
    );
    const fallbackCode = await runCargo(repoRoot, [
      'run',
      '-p',
      'xtask',
      '--bin',
      'xtask',
      '--',
      'openapi',
      '--output',
      outputFile,
    ]);
    if (fallbackCode !== 0) {
      throw new Error(`OpenAPI generation failed (primary=${primaryCode}, fallback=${fallbackCode})`);
    }
  },
};

export async function syncOpenApi(options, context = defaultContext) {
  const {
    repoRoot,
    outputDir,
    versionsDir,
    generateSpec,
  } = context;
  const requireSignedManifest = options.requireSigned !== false;

  const versionDir = join(versionsDir, options.version);
  const outputFile = join(versionDir, 'torii.json');

  await mkdir(versionDir, {recursive: true});
  await generateSpec(repoRoot, outputFile);

  const data = await readFile(outputFile, 'utf8');
  const specBytes = Buffer.from(data, 'utf8');
  const manifestTemplate = await prepareManifestTemplate(
    join(outputDir, 'manifest.json'),
    specBytes,
    {requireSigned: requireSignedManifest},
  );
  await maybeCopyManifest(versionDir, outputDir, manifestTemplate);
  console.log(`Torii OpenAPI spec refreshed at ${outputFile}`);

  const seenMirrors = new Set();
  for (const mirror of options.mirrors ?? []) {
    if (!mirror || mirror === options.version || seenMirrors.has(mirror)) {
      continue;
    }
    seenMirrors.add(mirror);
    const mirrorDir = join(versionsDir, mirror);
    await mkdir(mirrorDir, {recursive: true});
    const mirrorFile = join(mirrorDir, 'torii.json');
    await writeFile(mirrorFile, data, 'utf8');
    console.log(`Mirrored spec to ${mirrorFile}`);
    await maybeCopyManifest(mirrorDir, outputDir, manifestTemplate);
  }

  if (options.latest) {
    const latestFile = join(outputDir, 'torii.json');
    await writeFile(latestFile, data, 'utf8');
    console.log(`Latest spec pointer updated at ${latestFile}`);
  }

  const latestSpecPath = join(outputDir, 'torii.json');
  const latestManifestPath = join(outputDir, 'manifest.json');
  await updateVersionIndex(versionsDir, outputDir, latestSpecPath, latestManifestPath);
}

async function updateVersionIndex(
  versionsDirPath = defaultVersionsDir,
  outputDirPath = defaultOutputDir,
  latestSpecPath = join(defaultOutputDir, 'torii.json'),
  latestManifestPath = join(defaultOutputDir, 'manifest.json')
) {
  await mkdir(versionsDirPath, {recursive: true});
  const entries = await readdir(versionsDirPath, {withFileTypes: true});
  const versionEntries = [];

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const label = entry.name;
    const versionDir = join(versionsDirPath, label);
    const specPath = join(versionDir, 'torii.json');
    const manifestPath = join(versionDir, 'manifest.json');
    const metadata = await loadVersionMetadata(label, specPath, manifestPath, outputDirPath);
    if (metadata) {
      versionEntries.push(metadata);
    }
  }

  const latestEntry = await loadVersionMetadata('latest', latestSpecPath, latestManifestPath, outputDirPath);
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

  const manifest = {
    versions: versionEntries.filter((entry) => entry.label !== 'latest').map((entry) => entry.label),
    generatedAt: new Date().toISOString(),
    entries: versionEntries,
  };

  await writeFile(
    join(outputDirPath, 'versions.json'),
    JSON.stringify(manifest, null, 2),
    'utf8'
  );
}

async function prepareManifestTemplate(manifestPath, specBytes, {requireSigned = false} = {}) {
  try {
    const text = await readFile(manifestPath, 'utf8');
    const manifest = JSON.parse(text);
    const recorded = manifest?.artifact?.sha256_hex;
    if (typeof recorded !== 'string') {
      const message = `manifest ${manifestPath} is missing artifact.sha256_hex; rerun 'cargo xtask openapi --sign <key>' to refresh the signed manifest.`;
      if (requireSigned) {
        throw new Error(message);
      }
      console.warn(`${message} Skipping versioned manifest copy.`);
      return null;
    }
    const computed = computeSha256Hex(specBytes);
    if (!equalsIgnoreCase(recorded, computed)) {
      const message =
        `manifest ${manifestPath} sha256 (${recorded}) does not match the freshly generated spec (${computed}); ` +
        "re-run `cargo xtask openapi --sign <key>` and re-run sync-openapi to propagate signed manifests.";
      if (requireSigned) {
        throw new Error(message);
      }
      console.warn(message);
      return null;
    }
    if (requireSigned && !hasSignature(manifest?.artifact?.signature)) {
      throw new Error(
        `manifest ${manifestPath} is missing signature fields; sign the spec with \`cargo xtask openapi --sign <key>\` before publishing.`,
      );
    }
    return manifest;
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      const message =
        `manifest ${manifestPath} not found; generate a signed manifest with \`cargo xtask openapi --sign <key>\` before syncing.`;
      if (requireSigned) {
        throw new Error(message);
      }
      console.warn(`${message} Versioned manifests will be unsigned until the spec is signed.`);
      return null;
    }
    if (requireSigned) {
      throw error;
    }
    console.warn(`failed to load manifest ${manifestPath}: ${error?.message ?? error}`);
    return null;
  }
}

async function maybeCopyManifest(versionDir, outputDir, manifestTemplate) {
  if (!manifestTemplate) {
    return;
  }
  const specPath = join(versionDir, 'torii.json');
  const manifestPath = join(versionDir, 'manifest.json');
  const relativePath = toPosix(relative(outputDir, specPath));
  const copy = {
    ...manifestTemplate,
    artifact: {
      ...manifestTemplate.artifact,
      path: relativePath || manifestTemplate.artifact.path,
    },
  };
  await writeFile(manifestPath, JSON.stringify(copy, null, 2), 'utf8');
  console.log(`Copied manifest to ${manifestPath}`);
}

function computeSha256Hex(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

function equalsIgnoreCase(a, b) {
  return typeof a === 'string' && typeof b === 'string' && a.toLowerCase() === b.toLowerCase();
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

async function loadVersionMetadata(label, specPath, manifestPath, outputDirPath) {
  const specBuffer = await readFileOptional(specPath);
  if (!specBuffer) {
    return null;
  }
  const stats = await statOptional(specPath);
  const manifestDetails = await loadManifestDetails(manifestPath, outputDirPath);
  return {
    label,
    path: toPosix(relative(outputDirPath, specPath)),
    bytes: specBuffer.length,
    sha256: computeSha256Hex(specBuffer),
    blake3: manifestDetails.blake3,
    updatedAt: stats?.mtime?.toISOString?.() ?? null,
    signed: manifestDetails.signed,
    manifestPath: manifestDetails.path,
    signatureAlgorithm: manifestDetails.signatureAlgorithm,
    signaturePublicKeyHex: manifestDetails.signaturePublicKeyHex,
    signatureHex: manifestDetails.signatureHex,
  };
}

async function readFileOptional(path) {
  try {
    return await readFile(path);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function statOptional(path) {
  try {
    return await stat(path);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function loadManifestDetails(manifestPath, outputDirPath) {
  try {
    const text = await readFile(manifestPath, 'utf8');
    const manifest = JSON.parse(text);
    const signature = manifest?.artifact?.signature;
    return {
      signed: Boolean(signature),
      signatureAlgorithm: signature?.algorithm ?? null,
      signatureHex: signature?.signature_hex ?? null,
      signaturePublicKeyHex: signature?.public_key_hex ?? null,
      path: toPosix(relative(outputDirPath, manifestPath)),
      blake3: manifest?.artifact?.blake3_hex ?? null,
    };
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return {
        signed: false,
        signatureAlgorithm: null,
        signatureHex: null,
        signaturePublicKeyHex: null,
        path: null,
        blake3: null,
      };
    }
    console.warn(`failed to load manifest ${manifestPath}: ${error?.message ?? error}`);
    return {
      signed: false,
      signatureAlgorithm: null,
      signatureHex: null,
      signaturePublicKeyHex: null,
      path: null,
      blake3: null,
    };
  }
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
