import { createHash, randomUUID } from "node:crypto";
import {
  closeSync,
  cpSync,
  existsSync,
  openSync,
  lstatSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  renameSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = resolve(__filename, "..");
const ROOT = process.env.IROHA_JS_BUILD_DIST_ROOT
  ? resolve(process.env.IROHA_JS_BUILD_DIST_ROOT)
  : resolve(__dirname, "..");
const DIST = join(ROOT, "dist");
const SRC = join(ROOT, "src");
const LOCK = join(ROOT, ".build-dist.lock");
const LOCK_TIMEOUT_MS = 60_000;
const STALE_LOCK_MS = 5 * 60_000;
const REQUIRED_OUTPUTS = ["address.js", "curveRegistry.js", "toriiClient.js", "kotodamaCompiler/index.js"];

const delay = (milliseconds) => new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));

async function acquireBuildLock() {
  const startedAt = Date.now();
  while (true) {
    try {
      const descriptor = openSync(LOCK, "wx", 0o600);
      try {
        writeFileSync(descriptor, `${process.pid}\n`, { encoding: "utf8" });
        return descriptor;
      } catch (error) {
        closeSync(descriptor);
        rmSync(LOCK, { force: true });
        throw error;
      }
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
    }

    try {
      if (Date.now() - statSync(LOCK).mtimeMs > STALE_LOCK_MS) {
        unlinkSync(LOCK);
        continue;
      }
    } catch (error) {
      if (error?.code === "ENOENT") continue;
      throw error;
    }

    if (Date.now() - startedAt >= LOCK_TIMEOUT_MS) {
      throw new Error(`build:dist timed out waiting for ${LOCK}`);
    }
    await delay(50);
  }
}

function validateOutputs(directory) {
  for (const fileName of REQUIRED_OUTPUTS) {
    if (!existsSync(join(directory, fileName))) {
      throw new Error(`build:dist missing expected output: ${fileName}`);
    }
  }
}

function directoryDigest(directory) {
  const hash = createHash("sha256");
  const visit = (current, relative) => {
    const entries = readdirSync(current, { withFileTypes: true }).sort((left, right) =>
      left.name.localeCompare(right.name),
    );
    for (const entry of entries) {
      const entryRelative = relative ? `${relative}/${entry.name}` : entry.name;
      const entryPath = join(current, entry.name);
      const metadata = lstatSync(entryPath);
      if (metadata.isDirectory()) {
        hash.update(`d:${entryRelative}\0`);
        visit(entryPath, entryRelative);
      } else if (metadata.isSymbolicLink()) {
        hash.update(`l:${entryRelative}:${readlinkSync(entryPath)}\0`);
      } else if (metadata.isFile()) {
        hash.update(`f:${entryRelative}:${metadata.mode & 0o777}\0`);
        hash.update(readFileSync(entryPath));
        hash.update("\0");
      } else {
        throw new Error(`build:dist cannot publish unsupported entry: ${entryPath}`);
      }
    }
  };
  visit(directory, "");
  return hash.digest("hex");
}

async function main() {
  const staging = join(ROOT, `.dist-stage-${process.pid}-${randomUUID()}`);
  let descriptor;
  try {
    descriptor = await acquireBuildLock();
    cpSync(SRC, staging, { recursive: true, errorOnExist: true });
    validateOutputs(staging);
    if (existsSync(DIST) && directoryDigest(staging) === directoryDigest(DIST)) {
      return;
    }
    rmSync(DIST, { recursive: true, force: true });
    renameSync(staging, DIST);
    validateOutputs(DIST);
  } finally {
    rmSync(staging, { recursive: true, force: true });
    if (descriptor !== undefined) {
      closeSync(descriptor);
      try {
        unlinkSync(LOCK);
      } catch (error) {
        if (error?.code !== "ENOENT") throw error;
      }
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
