// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import {
  buildBundleSummary,
  formatBytes,
} from "../scripts/report-bundle-size.mjs";

test("formatBytes scales values", () => {
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(2048), "2 KB");
  assert.equal(formatBytes(4096), "4 KB");
  assert.equal(formatBytes(1048576), "1 MB");
});

test("buildBundleSummary sorts the largest files", () => {
  const summary = buildBundleSummary(
    {
      id: "@iroha/iroha-js@0.0.2",
      name: "@iroha/iroha-js",
      version: "0.0.2",
      entryCount: 3,
      files: [
        { path: "src/a.js", size: 100 },
        { path: "src/b.js", size: 400 },
        { path: "src/c.js", size: 50 },
      ],
    },
    {
      recordedAt: "2026-02-01T00:00:00.000Z",
      tarballBytes: 1234,
      tarballSha256: "abc",
      tarballPath: "tmp/iroha-js.tgz",
    },
  );

  assert.equal(summary.files.count, 3);
  assert.equal(summary.files.totalBytes, 550);
  assert.equal(summary.tarball.bytes, 1234);
  assert.equal(summary.tarball.sha256, "abc");
  assert.equal(summary.files.topFiles[0].path, "src/b.js");
  assert.equal(summary.files.topFiles[0].percent, Number(((400 / 550) * 100).toFixed(2)));
  assert.equal(summary.files.topFiles[1].path, "src/a.js");
  assert.equal(summary.files.topFiles[2].path, "src/c.js");
});
