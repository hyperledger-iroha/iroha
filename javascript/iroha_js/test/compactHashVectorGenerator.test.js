// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  checkCompactHashVectorFile,
  COMPACT_HASH_VECTOR_PATH,
  compactHashVectorSourceBundleSha256,
  createCompactHashVector,
  parseCompactHashVectorArguments,
  renderCompactHashVector,
  writeCompactHashVectorFile,
} from "../scripts/regenerate-compact-hash-vector.mjs";

test("compact vector is an exact deterministic ABI21 browser-codec artifact", () => {
  const vector = createCompactHashVector();
  assert.equal(vector["schema.version"], "2");
  assert.equal(vector["source.tag"], "abi21-browser-codec-source-bundle-v1");
  assert.equal(
    vector["source.bundle.sha256"],
    compactHashVectorSourceBundleSha256(),
  );
  assert.equal(vector["versioned.bytes"], "648");
  assert.equal(vector["bare.bytes"], "647");
  assert.equal(vector["compact.length.hex"], "f703");
  assert.equal(vector["canonical.prefix.hex"], "00000000f703");
  assert.equal(
    vector["canonical.hash"],
    "1bc7da85d038c52752ce4fd5fa79316e5f3e0af4e9b08fdbf225c0f1d1b249df",
  );
  assert.equal(
    vector["payload.prehash"],
    "aa8293edd3261028afae447b2fc9a3a6ac0cdd6d46dd133218b6b59492224983",
  );
  assert.equal(fs.readFileSync(COMPACT_HASH_VECTOR_PATH, "utf8"), renderCompactHashVector());
  checkCompactHashVectorFile();
});

test("compact vector source identity changes on generator codec and lockfile drift", () => {
  const baseline = compactHashVectorSourceBundleSha256();
  for (const suffix of [
    "scripts/regenerate-compact-hash-vector.mjs",
    "src/transactionCodec.js",
    "src/address.js",
    "package-lock.json",
  ]) {
    const drifted = compactHashVectorSourceBundleSha256({
      readFileSync(file) {
        const bytes = fs.readFileSync(file);
        return file.endsWith(suffix)
          ? Buffer.concat([bytes, Buffer.from("\n// adversarial drift\n")])
          : bytes;
      },
    });
    assert.notEqual(drifted, baseline, `${suffix} drift must alter source identity`);
    assert.throws(
      () =>
        checkCompactHashVectorFile(
          COMPACT_HASH_VECTOR_PATH,
          renderCompactHashVector({ sourceBundleSha256: drifted }),
        ),
      /compact vector drift/u,
      `${suffix} drift must fail the committed-vector check`,
    );
  }
});

test("compact vector source discovery rejects dynamic-import dependency escapes", () => {
  assert.throws(
    () =>
      compactHashVectorSourceBundleSha256({
        readFileSync(file) {
          const bytes = fs.readFileSync(file);
          return file.endsWith("src/transactionCodec.js")
            ? Buffer.concat([bytes, Buffer.from('\nvoid import("./escaped.js");\n')])
            : bytes;
        },
      }),
    /dynamic imports are not allowed/u,
  );
});

test("compact vector check fails closed on tampering and symlink substitution", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "iroha-compact-vector-"));
  try {
    const vector = path.join(directory, "vector.properties");
    writeCompactHashVectorFile(vector);
    checkCompactHashVectorFile(vector);
    fs.appendFileSync(vector, "canonical.hash=forged\n");
    assert.throws(() => checkCompactHashVectorFile(vector), /compact vector drift/u);

    const symlink = path.join(directory, "vector-link.properties");
    fs.symlinkSync(vector, symlink);
    assert.throws(
      () => checkCompactHashVectorFile(symlink),
      /regular non-symlink file/u,
    );
    assert.throws(
      () => writeCompactHashVectorFile(symlink),
      /regular non-symlink file/u,
    );
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("compact vector CLI rejects ambiguous and unknown arguments", () => {
  assert.throws(
    () => parseCompactHashVectorArguments(["--check", "--check"]),
    /only be specified once/u,
  );
  assert.throws(
    () => parseCompactHashVectorArguments(["--output"]),
    /requires a path/u,
  );
  assert.throws(
    () => parseCompactHashVectorArguments(["--output", "a", "--output", "b"]),
    /only be specified once/u,
  );
  assert.throws(
    () => parseCompactHashVectorArguments(["--forged"]),
    /unknown argument/u,
  );
});
