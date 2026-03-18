import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  __resetNativeStateForTests,
  getNativeBinding,
  verifyNativeBinding,
} from "../src/native.js";

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

async function withTempDir(run) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "iroha-js-native-"));
  try {
    return await run(dir);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

test("verifyNativeBinding succeeds when checksum matches manifest entry", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    const contents = Buffer.from("native-stub");
    await fs.writeFile(bindingPath, contents);
    const digest = sha256(contents);
    const platformKey = `${process.platform}-${process.arch}`;
    await fs.writeFile(
      manifestPath,
      `${JSON.stringify({ entries: { [platformKey]: { sha256: digest } } }, null, 2)}\n`,
    );

    const result = verifyNativeBinding(bindingPath, { manifestPath });
    assert.equal(result.ok, true);
    assert.equal(result.status, "verified");
    assert.equal(result.sha256, digest);
    assert.equal(result.expectedSha256, digest);
  });
});

test("getNativeBinding falls back to JS when checksum mismatches", async () => {
  __resetNativeStateForTests();
  const originalWarn = console.warn;
  const warnings = [];
  console.warn = (...args) => warnings.push(args.join(" "));
  const previousOverride = process.env.IROHA_JS_NATIVE_DIR;
  const previousAllowUnverified = process.env.IROHA_JS_ALLOW_UNVERIFIED_NATIVE;
  const previousDisable = process.env.IROHA_JS_DISABLE_NATIVE;

  try {
    await withTempDir(async (dir) => {
      const bindingPath = path.join(dir, "iroha_js_host.node");
      const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
      await fs.writeFile(bindingPath, "tampered");
      const platformKey = `${process.platform}-${process.arch}`;
      await fs.writeFile(
        manifestPath,
        `${JSON.stringify(
          { entries: { [platformKey]: { sha256: sha256(Buffer.from("expected")) } } },
          null,
          2,
        )}\n`,
      );
      process.env.IROHA_JS_NATIVE_DIR = dir;
      delete process.env.IROHA_JS_DISABLE_NATIVE;
      delete process.env.IROHA_JS_ALLOW_UNVERIFIED_NATIVE;
      const binding = getNativeBinding();
      assert.equal(binding, null);
      assert(warnings.some((line) => line.includes("checksum mismatch")));
    });
  } finally {
    process.env.IROHA_JS_NATIVE_DIR = previousOverride;
    if (previousAllowUnverified === undefined) {
      delete process.env.IROHA_JS_ALLOW_UNVERIFIED_NATIVE;
    } else {
      process.env.IROHA_JS_ALLOW_UNVERIFIED_NATIVE = previousAllowUnverified;
    }
    if (previousDisable === undefined) {
      delete process.env.IROHA_JS_DISABLE_NATIVE;
    } else {
      process.env.IROHA_JS_DISABLE_NATIVE = previousDisable;
    }
    console.warn = originalWarn;
    __resetNativeStateForTests();
  }
});

test("verifyNativeBinding can allow missing manifests when requested", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    await fs.writeFile(bindingPath, "missing-manifest");

    const strict = verifyNativeBinding(bindingPath, { manifestPath });
    assert.equal(strict.ok, false);
    assert.equal(strict.status, "missing_manifest");

    const lax = verifyNativeBinding(bindingPath, { manifestPath, allowMissingExpected: true });
    assert.equal(lax.ok, true);
    assert.equal(lax.status, "missing_manifest");
  });
});

test("getNativeBinding honours IROHA_JS_DISABLE_NATIVE opt-out", async () => {
  __resetNativeStateForTests();
  const originalWarn = console.warn;
  const warnings = [];
  console.warn = (...args) => warnings.push(args.join(" "));
  const previousDisable = process.env.IROHA_JS_DISABLE_NATIVE;
  const previousOverride = process.env.IROHA_JS_NATIVE_DIR;

  try {
    process.env.IROHA_JS_DISABLE_NATIVE = "1";
    delete process.env.IROHA_JS_NATIVE_DIR;
    const binding = getNativeBinding();
    assert.equal(binding, null);
    assert(
      warnings.some((line) =>
        line.includes("native binding disabled via IROHA_JS_DISABLE_NATIVE"),
      ),
    );
  } finally {
    if (previousDisable === undefined) {
      delete process.env.IROHA_JS_DISABLE_NATIVE;
    } else {
      process.env.IROHA_JS_DISABLE_NATIVE = previousDisable;
    }
    if (previousOverride === undefined) {
      delete process.env.IROHA_JS_NATIVE_DIR;
    } else {
      process.env.IROHA_JS_NATIVE_DIR = previousOverride;
    }
    console.warn = originalWarn;
    __resetNativeStateForTests();
  }
});

test("getNativeBinding throws when IROHA_JS_FORCE_NATIVE is set and binding missing", async () => {
  __resetNativeStateForTests();
  const previousForce = process.env.IROHA_JS_FORCE_NATIVE;
  const previousDisable = process.env.IROHA_JS_DISABLE_NATIVE;
  const previousOverride = process.env.IROHA_JS_NATIVE_DIR;

  try {
    await withTempDir(async (dir) => {
      process.env.IROHA_JS_FORCE_NATIVE = "1";
      delete process.env.IROHA_JS_DISABLE_NATIVE;
      process.env.IROHA_JS_NATIVE_DIR = dir;
      assert.throws(() => getNativeBinding(), /IROHA_JS_FORCE_NATIVE/);
    });
  } finally {
    if (previousForce === undefined) {
      delete process.env.IROHA_JS_FORCE_NATIVE;
    } else {
      process.env.IROHA_JS_FORCE_NATIVE = previousForce;
    }
    if (previousDisable === undefined) {
      delete process.env.IROHA_JS_DISABLE_NATIVE;
    } else {
      process.env.IROHA_JS_DISABLE_NATIVE = previousDisable;
    }
    if (previousOverride === undefined) {
      delete process.env.IROHA_JS_NATIVE_DIR;
    } else {
      process.env.IROHA_JS_NATIVE_DIR = previousOverride;
    }
    __resetNativeStateForTests();
  }
});

test("getNativeBinding rejects conflicting force/disable flags", () => {
  __resetNativeStateForTests();
  const previousForce = process.env.IROHA_JS_FORCE_NATIVE;
  const previousDisable = process.env.IROHA_JS_DISABLE_NATIVE;

  try {
    process.env.IROHA_JS_FORCE_NATIVE = "1";
    process.env.IROHA_JS_DISABLE_NATIVE = "1";
    assert.throws(() => getNativeBinding(), /cannot be combined/);
  } finally {
    if (previousForce === undefined) {
      delete process.env.IROHA_JS_FORCE_NATIVE;
    } else {
      process.env.IROHA_JS_FORCE_NATIVE = previousForce;
    }
    if (previousDisable === undefined) {
      delete process.env.IROHA_JS_DISABLE_NATIVE;
    } else {
      process.env.IROHA_JS_DISABLE_NATIVE = previousDisable;
    }
    __resetNativeStateForTests();
  }
});
