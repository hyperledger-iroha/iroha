// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

test("packed privacy-x509 subpath resolves its installed dependency exports", () => {
  const packageRoot = fileURLToPath(new URL("..", import.meta.url));
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "iroha-x509-package-"));
  try {
    const packed = spawnSync("npm", ["pack", "--ignore-scripts", "--json", "--pack-destination", temporary],
      { cwd: packageRoot, encoding: "utf8" });
    assert.equal(packed.status, 0, packed.stderr);
    const archive = path.join(temporary, JSON.parse(packed.stdout)[0].filename);
    const extracted = spawnSync("tar", ["-xzf", archive, "-C", temporary], { encoding: "utf8" });
    assert.equal(extracted.status, 0, extracted.stderr);
    const installed = path.join(temporary, "node_modules/@iroha/iroha-js");
    fs.mkdirSync(path.dirname(installed), { recursive: true });
    fs.renameSync(path.join(temporary, "package"), installed);
    fs.symlinkSync(path.join(packageRoot, "node_modules"), path.join(installed, "node_modules"), "dir");
    const entry = path.join(temporary, "consumer.mjs");
    fs.writeFileSync(entry, [
      'import assert from "node:assert/strict";',
      'import * as x509 from "@iroha/iroha-js/privacy-x509";',
      'assert.equal(typeof x509.PrivacyX509DirectTransactionV1, "function");',
      'assert.equal(typeof x509.preparePrivacyX509DirectTransactionV1, "function");',
      'assert.equal(typeof x509.buildPrivacyX509DirectTransactionV1, "function");',
    ].join("\n"));
    const consumed = spawnSync(process.execPath, [entry], {
      cwd: temporary, encoding: "utf8", env: { ...process.env, IROHA_JS_NATIVE_WARN: "0" },
    });
    assert.equal(consumed.status, 0, `packed privacy-x509 import failed:\n${consumed.stderr}`);
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
});
