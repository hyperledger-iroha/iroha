import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const NATIVE_IMPLEMENTATIONS = [
  ["source", await import("../src/native.js")],
  ["package dist", await import("../dist/native.js")],
];

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

for (const [implementationName, implementation] of NATIVE_IMPLEMENTATIONS) {
  const {
    __resetNativeStateForTests,
    __snapshotNativeBindingForTests,
    getNativeBinding,
    verifyNativeBinding,
  } = implementation;
  const variantTest = (name, run) => test(`${implementationName}: ${name}`, run);

variantTest("verifyNativeBinding succeeds when checksum matches manifest entry", async () => {
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

variantTest("getNativeBinding rejects checksum mismatches", async () => {
  __resetNativeStateForTests();
  const previousOverride = process.env.IROHA_JS_NATIVE_DIR;

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
      assert.throws(
        () => getNativeBinding(),
        (error) => {
          assert.match(error.message, /checksum mismatch/u);
          assert.equal(error.code, "ERR_IROHA_NATIVE_BINDING");
          assert.equal(error.nativeStatus, "hash_mismatch");
          return true;
        },
      );
    });
  } finally {
    if (previousOverride === undefined) {
      delete process.env.IROHA_JS_NATIVE_DIR;
    } else {
      process.env.IROHA_JS_NATIVE_DIR = previousOverride;
    }
    __resetNativeStateForTests();
  }
});

variantTest("verifyNativeBinding fails closed when its manifest is missing", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    await fs.writeFile(bindingPath, "missing-manifest");

    const strict = verifyNativeBinding(bindingPath, { manifestPath });
    assert.equal(strict.ok, false);
    assert.equal(strict.status, "missing_manifest");
  });
});

variantTest("verification rehashes files and reloads manifests on every call", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    const platformKey = `${process.platform}-${process.arch}`;
    const original = Buffer.from("original-native");
    const replacement = Buffer.from("replacement-native");
    await fs.writeFile(bindingPath, original);
    await fs.writeFile(
      manifestPath,
      `${JSON.stringify(
        { entries: { [platformKey]: { sha256: sha256(original) } } },
        null,
        2,
      )}\n`,
    );

    assert.equal(verifyNativeBinding(bindingPath, { manifestPath }).ok, true);
    await fs.writeFile(bindingPath, replacement);
    const replacedBinding = verifyNativeBinding(bindingPath, { manifestPath });
    assert.equal(replacedBinding.ok, false);
    assert.equal(replacedBinding.status, "hash_mismatch");

    await fs.writeFile(bindingPath, original);
    assert.equal(verifyNativeBinding(bindingPath, { manifestPath }).ok, true);
    await fs.writeFile(
      manifestPath,
      `${JSON.stringify(
        { entries: { [platformKey]: { sha256: sha256(replacement) } } },
        null,
        2,
      )}\n`,
    );
    const replacedManifest = verifyNativeBinding(bindingPath, { manifestPath });
    assert.equal(replacedManifest.ok, false);
    assert.equal(replacedManifest.status, "hash_mismatch");
  });
});

variantTest("verified snapshots retain the authenticated bytes across path substitution", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    const platformKey = `${process.platform}-${process.arch}`;
    const authenticated = Buffer.from("authenticated-native-bytes");
    const substituted = Buffer.from("substituted-after-verification");
    await fs.writeFile(bindingPath, authenticated);
    await fs.writeFile(
      manifestPath,
      `${JSON.stringify(
        { entries: { [platformKey]: { sha256: sha256(authenticated) } } },
        null,
        2,
      )}\n`,
    );

    const snapshot = __snapshotNativeBindingForTests(
      bindingPath,
      { manifestPath },
      () => writeFileSync(bindingPath, substituted),
    );
    try {
      assert.equal(snapshot.ok, true);
      assert.deepEqual(readFileSync(snapshot.path), authenticated);
      assert.deepEqual(readFileSync(bindingPath), substituted);
      assert.equal(sha256(readFileSync(snapshot.path)), snapshot.sha256);
      assert.match(path.basename(snapshot.path), new RegExp(`^${snapshot.sha256}\\.node$`, "u"));
      if (process.platform !== "win32") {
        assert.equal(statSync(snapshot.directory).mode & 0o777, 0o700);
        assert.equal(statSync(snapshot.path).mode & 0o777, 0o500);
      }
    } finally {
      await fs.rm(snapshot.directory, { recursive: true, force: true });
    }
  });
});

variantTest("verification rejects malformed checksum manifests and entries", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    const platformKey = `${process.platform}-${process.arch}`;
    const validHash = sha256(Buffer.from("native-stub"));
    await fs.writeFile(bindingPath, "native-stub");

    for (const manifest of [
      { [platformKey]: { sha256: "0".repeat(64) } },
      { entries: [] },
      { entries: { [platformKey]: { sha256: "A".repeat(64) } } },
      { entries: { [platformKey]: { sha256: "0".repeat(64), extra: true } } },
      {
        entries: {
          [platformKey]: { sha256: validHash },
          "other-x64": { sha256: "not-a-checksum" },
        },
      },
      {
        entries: {
          [platformKey]: { sha256: validHash },
          [platformKey.toUpperCase()]: { sha256: validHash },
        },
      },
      { entries: { "MixedCase-x64": { sha256: validHash } } },
      { entries: { invalid: { sha256: validHash } } },
    ]) {
      await fs.writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
      const result = verifyNativeBinding(bindingPath, { manifestPath });
      assert.equal(result.ok, false);
      assert.equal(result.status, "manifest_error");
    }
  });
});

variantTest("explicit expected checksums are validated independently per call", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const contents = Buffer.from("native-stub");
    const platformKey = `${process.platform}-${process.arch}`;
    await fs.writeFile(bindingPath, contents);

    const verified = verifyNativeBinding(bindingPath, {
      platformKey,
      expectedChecksums: {
        [platformKey]: { sha256: sha256(contents) },
      },
    });
    assert.equal(verified.ok, true);

    const rejected = verifyNativeBinding(bindingPath, {
      platformKey,
      expectedChecksums: {
        [platformKey]: { sha256: "0".repeat(64) },
      },
    });
    assert.equal(rejected.ok, false);
    assert.equal(rejected.status, "hash_mismatch");

    const malformed = verifyNativeBinding(bindingPath, {
      platformKey,
      expectedChecksums: [],
    });
    assert.equal(malformed.ok, false);
    assert.equal(malformed.status, "manifest_error");
  });
});

variantTest("getNativeBinding throws when binding is missing", async () => {
  __resetNativeStateForTests();
  const previousOverride = process.env.IROHA_JS_NATIVE_DIR;

  try {
    await withTempDir(async (dir) => {
      process.env.IROHA_JS_NATIVE_DIR = dir;
      assert.throws(
        () => getNativeBinding(),
        (error) => {
          assert.match(error.message, /binding missing/u);
          assert.equal(error.code, "ERR_IROHA_NATIVE_BINDING");
          assert.equal(error.nativeStatus, "missing_file");
          return true;
        },
      );
    });
  } finally {
    if (previousOverride === undefined) {
      delete process.env.IROHA_JS_NATIVE_DIR;
    } else {
      process.env.IROHA_JS_NATIVE_DIR = previousOverride;
    }
    __resetNativeStateForTests();
  }
});
}
