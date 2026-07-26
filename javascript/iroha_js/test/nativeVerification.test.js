import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  machOSigningIndependentSHA256,
  peSigningIndependentSHA256,
} from "../src/nativeArtifactHash.js";

const NATIVE_IMPLEMENTATIONS = [
  ["source", await import("../src/native.js")],
  ["package dist", await import("../dist/native.js")],
];

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

function syntheticSignedMachO({ signatureByte, signatureSize, codeByte = 0x42 }) {
  const headerBytes = 32;
  const segmentCommandBytes = 72;
  const signatureCommandBytes = 16;
  const commandBytes = segmentCommandBytes + signatureCommandBytes;
  const codeBytes = 16;
  const signatureOffset = headerBytes + commandBytes + codeBytes;
  const bytes = Buffer.alloc(signatureOffset + signatureSize, 0);
  bytes.writeUInt32LE(0xfeedfacf, 0);
  bytes.writeUInt32LE(0x0100000c, 4);
  bytes.writeUInt32LE(8, 12);
  bytes.writeUInt32LE(2, 16);
  bytes.writeUInt32LE(commandBytes, 20);

  const segment = headerBytes;
  bytes.writeUInt32LE(0x19, segment);
  bytes.writeUInt32LE(segmentCommandBytes, segment + 4);
  bytes.write("__LINKEDIT", segment + 8, "ascii");
  bytes.writeBigUInt64LE(BigInt(signatureSize + codeBytes), segment + 32);
  bytes.writeBigUInt64LE(BigInt(headerBytes + commandBytes), segment + 40);
  bytes.writeBigUInt64LE(BigInt(signatureSize + codeBytes), segment + 48);

  const signatureCommand = segment + segmentCommandBytes;
  bytes.writeUInt32LE(0x1d, signatureCommand);
  bytes.writeUInt32LE(signatureCommandBytes, signatureCommand + 4);
  bytes.writeUInt32LE(signatureOffset, signatureCommand + 8);
  bytes.writeUInt32LE(signatureSize, signatureCommand + 12);
  bytes.fill(codeByte, headerBytes + commandBytes, signatureOffset);
  bytes.fill(signatureByte, signatureOffset);
  return bytes;
}

function syntheticUnsignedPe({ codeByte = 0x42 } = {}) {
  const peOffset = 64;
  const coffBytes = 20;
  const optionalBytes = 240;
  const optionalOffset = peOffset + 4 + coffBytes;
  const bytes = Buffer.alloc(optionalOffset + optionalBytes + 17, codeByte);
  bytes.fill(0, 0, optionalOffset + optionalBytes);
  bytes.write("MZ", 0, "ascii");
  bytes.writeUInt32LE(peOffset, 0x3c);
  bytes.write("PE\0\0", peOffset, "binary");
  bytes.writeUInt16LE(optionalBytes, peOffset + 4 + 16);
  bytes.writeUInt16LE(0x20b, optionalOffset);
  bytes.writeUInt32LE(16, optionalOffset + 108);
  return bytes;
}

function authenticodeSignPe(unsigned, { certificateByte = 0x77, certificateBytes = 64 } = {}) {
  const padding = (8 - (unsigned.length % 8)) % 8;
  const certificateOffset = unsigned.length + padding;
  const signed = Buffer.alloc(certificateOffset + certificateBytes);
  unsigned.copy(signed);
  signed.writeUInt32LE(0x1234_5678, 64 + 4 + 20 + 64);
  const certificateDirectory = 64 + 4 + 20 + 112 + 4 * 8;
  signed.writeUInt32LE(certificateOffset, certificateDirectory);
  signed.writeUInt32LE(certificateBytes, certificateDirectory + 4);
  signed.fill(certificateByte, certificateOffset);
  signed.writeUInt32LE(certificateBytes, certificateOffset);
  signed.writeUInt16LE(0x0200, certificateOffset + 4);
  signed.writeUInt16LE(0x0002, certificateOffset + 6);
  return signed;
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

variantTest("getNativeBinding rejects a checksum-valid dirty-source artifact", async () => {
  __resetNativeStateForTests();
  const previousNativeDir = process.env.IROHA_JS_NATIVE_DIR;
  try {
    await withTempDir(async (dir) => {
      const bindingPath = path.join(dir, "iroha_js_host.node");
      const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
      const contents = Buffer.from("dirty-source-native-stub");
      await fs.writeFile(bindingPath, contents);
      await fs.writeFile(
        manifestPath,
        `${JSON.stringify({
          entries: {
            [`${process.platform}-${process.arch}`]: {
              sha256: sha256(contents),
              source_git_revision: "a".repeat(40),
              source_tree_clean: false,
            },
          },
        })}\n`,
      );
      process.env.IROHA_JS_NATIVE_DIR = dir;
      assert.throws(
        () => getNativeBinding(),
        (error) => {
          assert.match(error.message, /dirty source tree/u);
          assert.equal(error.code, "ERR_IROHA_NATIVE_BINDING");
          assert.equal(error.nativeStatus, "source_provenance_error");
          return true;
        },
      );
    });
  } finally {
    if (previousNativeDir === undefined) {
      delete process.env.IROHA_JS_NATIVE_DIR;
    } else {
      process.env.IROHA_JS_NATIVE_DIR = previousNativeDir;
    }
    __resetNativeStateForTests();
  }
});

variantTest("Darwin verification accepts only signing-container changes", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    const original = syntheticSignedMachO({ signatureByte: 0x11, signatureSize: 64 });
    const resigned = syntheticSignedMachO({ signatureByte: 0x77, signatureSize: 96 });
    const stableDigest = machOSigningIndependentSHA256(original);
    assert.equal(stableDigest, machOSigningIndependentSHA256(resigned));
    await fs.writeFile(bindingPath, resigned);
    await fs.writeFile(
      manifestPath,
      `${JSON.stringify({
        entries: {
          "darwin-arm64": {
            sha256: sha256(original),
            mach_o_signing_independent_sha256: stableDigest,
          },
        },
      })}\n`,
    );

    const verified = verifyNativeBinding(bindingPath, {
      manifestPath,
      platformKey: "darwin-arm64",
    });
    assert.equal(verified.ok, true);
    assert.equal(verified.status, "verified_resigned_macho");
    assert.equal(verified.machOSigningIndependentSha256, stableDigest);

    resigned[32 + 72 + 16] ^= 1;
    await fs.writeFile(bindingPath, resigned);
    const tampered = verifyNativeBinding(bindingPath, {
      manifestPath,
      platformKey: "darwin-arm64",
    });
    assert.equal(tampered.ok, false);
    assert.equal(tampered.status, "hash_mismatch");
  });
});

variantTest("Darwin signing-independent fallback rejects malformed Mach-O bounds", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    const original = syntheticSignedMachO({ signatureByte: 0x11, signatureSize: 64 });
    const malformed = Buffer.from(original);
    malformed.writeUInt32LE(0xffff_ffff, 20);
    await fs.writeFile(bindingPath, malformed);
    await fs.writeFile(
      manifestPath,
      `${JSON.stringify({
        entries: {
          "darwin-arm64": {
            sha256: sha256(original),
            mach_o_signing_independent_sha256:
              machOSigningIndependentSHA256(original),
          },
        },
      })}\n`,
    );
    const result = verifyNativeBinding(bindingPath, {
      manifestPath,
      platformKey: "darwin-arm64",
    });
    assert.equal(result.ok, false);
    assert.equal(result.status, "hash_error");
  });
});

variantTest("Windows verification accepts only a final bounded Authenticode region", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    const original = syntheticUnsignedPe();
    const signed = authenticodeSignPe(original);
    const stableDigest = peSigningIndependentSHA256(original);
    assert.equal(
      stableDigest,
      peSigningIndependentSHA256(signed, original.length, { requireSigned: true }),
    );
    await fs.writeFile(bindingPath, signed);
    await fs.writeFile(
      manifestPath,
      `${JSON.stringify({
        entries: {
          "win32-x64": {
            sha256: sha256(original),
            pe_signing_independent_sha256: stableDigest,
            pe_unsigned_size: original.length,
            source_git_revision: "a".repeat(40),
            source_tree_clean: true,
          },
        },
      })}\n`,
    );
    const verified = verifyNativeBinding(bindingPath, {
      manifestPath,
      platformKey: "win32-x64",
    });
    assert.equal(verified.ok, true);
    assert.equal(verified.status, "verified_resigned_pe");
    assert.equal(verified.peSigningIndependentSha256, stableDigest);
    assert.equal(verified.expectedPeUnsignedSize, original.length);
    assert.equal(verified.sourceGitRevision, "a".repeat(40));
    assert.equal(verified.sourceTreeClean, true);

    signed[original.length - 1] ^= 1;
    await fs.writeFile(bindingPath, signed);
    const tampered = verifyNativeBinding(bindingPath, {
      manifestPath,
      platformKey: "win32-x64",
    });
    assert.equal(tampered.ok, false);
    assert.equal(tampered.status, "hash_mismatch");
  });
});

variantTest("Windows signing fallback rejects missing or malformed certificate bounds", async () => {
  __resetNativeStateForTests();
  await withTempDir(async (dir) => {
    const bindingPath = path.join(dir, "iroha_js_host.node");
    const manifestPath = path.join(dir, "iroha_js_host.checksums.json");
    const original = syntheticUnsignedPe();
    const malformed = authenticodeSignPe(original);
    malformed[original.length] = 1;
    await fs.writeFile(bindingPath, malformed);
    await fs.writeFile(
      manifestPath,
      `${JSON.stringify({
        entries: {
          "win32-x64": {
            sha256: sha256(original),
            pe_signing_independent_sha256: peSigningIndependentSHA256(original),
            pe_unsigned_size: original.length,
          },
        },
      })}\n`,
    );
    const malformedResult = verifyNativeBinding(bindingPath, {
      manifestPath,
      platformKey: "win32-x64",
    });
    assert.equal(malformedResult.ok, false);
    assert.equal(malformedResult.status, "hash_error");

    await fs.writeFile(bindingPath, original);
    original[original.length - 1] ^= 1;
    await fs.writeFile(bindingPath, original);
    const unsignedResult = verifyNativeBinding(bindingPath, {
      manifestPath,
      platformKey: "win32-x64",
    });
    assert.equal(unsignedResult.ok, false);
    assert.equal(unsignedResult.status, "hash_error");
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
          [platformKey]: {
            sha256: validHash,
            source_git_revision: "a".repeat(40),
          },
        },
      },
      {
        entries: {
          [platformKey]: {
            sha256: validHash,
            source_git_revision: "A".repeat(40),
            source_tree_clean: true,
          },
        },
      },
      {
        entries: {
          [platformKey]: {
            sha256: validHash,
            pe_signing_independent_sha256: "0".repeat(64),
          },
        },
      },
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
