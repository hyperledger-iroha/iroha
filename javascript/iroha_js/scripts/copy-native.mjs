#!/usr/bin/env node
/**
 * Copy the compiled `iroha_js_host` dynamic library into a `.node` artefact
 * that Node.js can load via `require`.
 */
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { join, dirname, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(scriptDir, "..", "..", "..");
const configuredTarget = process.env.CARGO_TARGET_DIR;
const targetRoot = configuredTarget
  ? isAbsolute(configuredTarget)
    ? configuredTarget
    : join(repoRoot, configuredTarget)
  : join(repoRoot, "target");
const targetDir = join(targetRoot, "debug");

const platform = process.platform;
const libName = platform === "win32"
  ? "iroha_js_host.dll"
  : `libiroha_js_host.${platform === "darwin" ? "dylib" : "so"}`;

const source = join(targetDir, libName);

if (!existsSync(source)) {
  throw new Error(
    `Native module not found at ${source}. Ensure ` +
      "`cargo build -p iroha_js_host` ran successfully.",
  );
}

const destDir = join(repoRoot, "javascript", "iroha_js", "native");
mkdirSync(destDir, { recursive: true });
const dest = join(destDir, "iroha_js_host.node");
const checksumManifestPath = join(destDir, "iroha_js_host.checksums.json");

copyFileSync(source, dest);
console.log(`Copied native module to ${dest}`);

const sha256 = createHash("sha256")
  .update(readFileSync(dest))
  .digest("hex");
const platformKey = `${process.platform}-${process.arch}`.toLowerCase();
writeFileSync(
  checksumManifestPath,
  `${JSON.stringify(
    {
      entries: {
        [platformKey]: {
          sha256,
        },
      },
    },
    null,
    2,
  )}\n`,
);
console.log(`Wrote checksum manifest to ${checksumManifestPath}`);
