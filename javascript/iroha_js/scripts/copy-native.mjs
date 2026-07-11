#!/usr/bin/env node
/**
 * Copy the compiled `iroha_js_host` dynamic library into a `.node` artefact
 * that Node.js can load via `require`.
 */
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { join, dirname, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { verifyNativeBinding } from "../src/native.js";

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

const configuredDestDir = process.env.IROHA_JS_NATIVE_DIR;
const destDir = configuredDestDir
  ? isAbsolute(configuredDestDir)
    ? configuredDestDir
    : join(repoRoot, configuredDestDir)
  : join(repoRoot, "javascript", "iroha_js", "native");
mkdirSync(destDir, { recursive: true });
const dest = join(destDir, "iroha_js_host.node");
const checksumManifestPath = join(destDir, "iroha_js_host.checksums.json");
const stagingDir = mkdtempSync(join(destDir, ".iroha-js-host-"));
const stagedNative = join(stagingDir, "iroha_js_host.node");
const stagedManifest = join(stagingDir, "iroha_js_host.checksums.json");

try {
  copyFileSync(source, stagedNative);

  if (platform === "darwin") {
    const sign = spawnSync("codesign", ["--force", "--sign", "-", stagedNative], {
      cwd: repoRoot,
      stdio: "inherit",
      env: process.env,
    });
    if (sign.status !== 0) {
      throw new Error(
        `Failed to ad-hoc sign ${stagedNative}; macOS requires a valid signature for Node.js native addons.`,
      );
    }
  }

  const sha256 = createHash("sha256")
    .update(readFileSync(stagedNative))
    .digest("hex");
  const platformKey = `${process.platform}-${process.arch}`.toLowerCase();
  writeFileSync(
    stagedManifest,
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
    { flag: "wx", mode: 0o600 },
  );

  // Both files are fully staged and authenticated before either public path
  // changes. A process interruption between the two renames can only produce
  // a checksum mismatch, which the loader rejects closed.
  renameSync(stagedNative, dest);
  renameSync(stagedManifest, checksumManifestPath);

  const verification = verifyNativeBinding(dest, {
    manifestPath: checksumManifestPath,
    platformKey,
  });
  if (!verification.ok) {
    throw new Error(
      `Published native binding failed checksum verification (${verification.status}).`,
    );
  }
  console.log(`Published verified native module to ${dest}`);
  console.log(`Wrote checksum manifest to ${checksumManifestPath}`);
} finally {
  rmSync(stagingDir, { recursive: true, force: true });
}
