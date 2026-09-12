import {
  closeSync, constants, fsyncSync, lstatSync, openSync, readdirSync,
  renameSync, rmdirSync, unlinkSync,
} from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { acquireDistLock, assertDistLockOwnership, releaseDistLock, syncDirectory } from "./build-dist.mjs";

const GENERATED_FILES = Object.freeze([
  "iroha_js_codec_wasm.js", "iroha_js_codec_wasm.d.ts",
  "iroha_js_codec_wasm_bg.wasm", "iroha_js_codec_wasm_bg.wasm.d.ts",
].sort());

function snapshotGeneration(directory) {
  const metadata = lstatSync(directory);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(`browser codec generation must be a regular directory: ${directory}`);
  }
  const names = readdirSync(directory).sort();
  if (JSON.stringify(names) !== JSON.stringify(GENERATED_FILES)) {
    throw new Error(`browser codec generation must contain exactly the four generated files: ${directory}`);
  }
  return [metadata, ...names.map((name) => {
    const entry = lstatSync(join(directory, name));
    if (!entry.isFile() || entry.isSymbolicLink() || entry.nlink !== 1) {
      throw new Error(`browser codec generation contains an unsupported file: ${name}`);
    }
    return entry;
  })];
}

function assertGeneration(directory, expected) {
  const actual = snapshotGeneration(directory);
  if (actual.some((entry, index) => entry.dev !== expected[index].dev
    || entry.ino !== expected[index].ino || entry.size !== expected[index].size
    || entry.mtimeMs !== expected[index].mtimeMs)) {
    throw new Error(`browser codec generation changed during publication: ${directory}`);
  }
}

function pathExists(path) {
  try { lstatSync(path); return true; } catch (error) {
    if (error.code === "ENOENT") return false;
    throw error;
  }
}

/** Publish one complete generated pair under the same lock as its dist reader. */
export async function publishBrowserCodecGeneration({ staging, output }) {
  if (!isAbsolute(staging) || !isAbsolute(output)) {
    throw new TypeError("browser codec publication requires absolute paths");
  }
  staging = resolve(staging);
  output = resolve(output);
  if (staging === output || staging.startsWith(`${output}/`) || output.startsWith(`${staging}/`)) {
    throw new TypeError("browser codec staging and output must be separate directories");
  }
  const root = dirname(output);
  const backup = `${output}.previous`;
  const lock = await acquireDistLock({ root });
  try {
    if (pathExists(backup)) {
      throw new Error(`browser codec previous generation requires recovery: ${backup}`);
    }
    const incoming = snapshotGeneration(staging);
    const previous = pathExists(output) ? snapshotGeneration(output) : undefined;
    for (const name of GENERATED_FILES) {
      const fd = openSync(join(staging, name), constants.O_RDONLY);
      try { fsyncSync(fd); } finally { closeSync(fd); }
    }
    syncDirectory(staging);
    assertGeneration(staging, incoming);
    assertDistLockOwnership(lock);
    if (previous) {
      assertGeneration(output, previous);
      renameSync(output, backup);
      syncDirectory(root);
    }
    try {
      renameSync(staging, output);
    } catch (error) {
      if (previous) {
        assertDistLockOwnership(lock);
        assertGeneration(backup, previous);
        if (pathExists(output)) throw new AggregateError([error], "browser codec output appeared during rollback; retained previous generation");
        renameSync(backup, output);
        syncDirectory(root);
      }
      throw error;
    }
    syncDirectory(root);
    syncDirectory(dirname(staging));
    assertGeneration(output, incoming);
    if (previous) {
      assertDistLockOwnership(lock);
      assertGeneration(backup, previous);
      // Only the authenticated, replaced four-file generation is retired.
      for (const name of GENERATED_FILES) unlinkSync(join(backup, name));
      rmdirSync(backup);
      syncDirectory(root);
    }
  } finally {
    releaseDistLock(lock);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const { values } = parseArgs({ options: { staging: { type: "string" }, output: { type: "string" } } });
    await publishBrowserCodecGeneration(values);
  } catch (error) {
    console.error(error);
    process.exitCode = 1;
  }
}
