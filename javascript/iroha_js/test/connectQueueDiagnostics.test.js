import test from "node:test";
import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import fs from "node:fs/promises";
import {
  appendConnectQueueMetric,
  exportConnectQueueEvidence,
  defaultConnectQueueRoot,
  deriveConnectSessionDirectory,
  readConnectQueueSnapshot,
  updateConnectQueueSnapshot,
} from "../src/connectQueueDiagnostics.js";

async function withTempDir(prefix, fn) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), prefix));
  try {
    await fn(dir);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

test("connect queue diagnostics snapshot + evidence", async () => {
  await withTempDir("iroha-js-connect-", async (rootDir) => {
    const sid = "AQIDBA"; // base64url for 0x01020304
    const updated = await updateConnectQueueSnapshot(
      sid,
      (snapshot) => {
        const next = { ...snapshot };
        next.state = "throttled";
        next.reason = "disk_watermark";
        next.app_to_wallet = {
          ...next.app_to_wallet,
          depth: 4,
          bytes: 2048,
        };
        return next;
      },
      { rootDir },
    );
    assert.equal(updated.state, "throttled");
    const { snapshot } = await readConnectQueueSnapshot({ sid, rootDir });
    assert.equal(snapshot.app_to_wallet.depth, 4);
    await appendConnectQueueMetric(
      sid,
      { state: "throttled", app_to_wallet_depth: 4, wallet_to_app_depth: 0 },
      { rootDir },
    );
    const exportDir = path.join(rootDir, "bundle");
    const { manifest } = await exportConnectQueueEvidence(sid, exportDir, { rootDir });
    assert.equal(manifest.snapshot.state, "throttled");
    assert.equal(manifest.files.metrics_filename, "metrics.ndjson");
    const manifestPath = path.join(exportDir, "manifest.json");
    const manifestDisk = JSON.parse(await fs.readFile(manifestPath, "utf8"));
    assert.equal(manifestDisk.schema_version, manifest.schema_version);
  });
});

test("connect queue root resolves config before env and gates env usage", () => {
  const originalEnvRoot = process.env.IROHA_CONNECT_QUEUE_ROOT;
  const originalNodeEnv = process.env.NODE_ENV;
  try {
    process.env.IROHA_CONNECT_QUEUE_ROOT = path.join(os.tmpdir(), "iroha-js-env-root");
    process.env.NODE_ENV = "production";

    const defaultRoot = path.join(os.homedir(), ".iroha", "connect");
    assert.equal(defaultConnectQueueRoot(), path.resolve(defaultRoot));

    const configRoot = path.join(os.tmpdir(), "iroha-js-config-root");
    const resolvedFromConfig = defaultConnectQueueRoot({
      connectConfig: { connect: { queue: { root: configRoot } } },
    });
    assert.equal(resolvedFromConfig, path.resolve(configRoot));

    const envRoot = path.resolve(process.env.IROHA_CONNECT_QUEUE_ROOT);
    const withEnvAllowed = defaultConnectQueueRoot({ allowEnvOverride: true });
    assert.equal(withEnvAllowed, envRoot);

    process.env.NODE_ENV = "test";
    const withTestEnv = defaultConnectQueueRoot();
    assert.equal(withTestEnv, path.resolve(defaultRoot));
    const testEnvAllowed = defaultConnectQueueRoot({ allowEnvOverride: true });
    assert.equal(testEnvAllowed, envRoot);

    const sessionDir = deriveConnectSessionDirectory({
      sid: "AQIDBA",
      connectConfig: { connect: { queue: { root: configRoot } } },
    });
    assert.ok(sessionDir.startsWith(path.resolve(configRoot)));
  } finally {
    process.env.IROHA_CONNECT_QUEUE_ROOT = originalEnvRoot;
    if (originalNodeEnv === undefined) {
      delete process.env.NODE_ENV;
    } else {
      process.env.NODE_ENV = originalNodeEnv;
    }
  }
});

test("connect queue diagnostics fallback encodes non-base64 session ids as utf8", () => {
  const rootDir = path.join(os.tmpdir(), "iroha-js-connect-fallback");
  const sid = "session*id";
  const sessionDir = deriveConnectSessionDirectory({ sid, rootDir });
  const expected = Buffer.from(sid, "utf8")
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
  assert.equal(path.basename(sessionDir), expected);
});

test("connect queue diagnostics rejects empty binary session ids", () => {
  const rootDir = path.join(os.tmpdir(), "iroha-js-connect-empty");
  assert.throws(
    () => deriveConnectSessionDirectory({ sid: new Uint8Array(), rootDir }),
    (error) => error instanceof TypeError,
  );
});

test("connect queue diagnostics rejects non-byte session ids", () => {
  const rootDir = path.join(os.tmpdir(), "iroha-js-connect-invalid");
  assert.throws(
    () => deriveConnectSessionDirectory({ sid: [256], rootDir }),
    (error) => error instanceof TypeError,
  );
});

test("connect queue diagnostics accepts array-like session ids", () => {
  const rootDir = path.join(os.tmpdir(), "iroha-js-connect-array");
  const sessionDir = deriveConnectSessionDirectory({ sid: [1, 2, 3, 4], rootDir });
  const expected = Buffer.from([1, 2, 3, 4])
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
  assert.equal(path.basename(sessionDir), expected);
});
