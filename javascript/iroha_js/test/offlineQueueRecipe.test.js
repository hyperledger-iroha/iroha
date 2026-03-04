import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { runOfflineQueueDemo } from "../recipes/offline_queue.mjs";

async function withTempDir(prefix, fn) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), prefix));
  try {
    await fn(dir);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

test("offline queue recipe exports an evidence bundle with replay metadata", async () => {
  await withTempDir("iroha-js-offline-recipe-", async (rootDir) => {
    const nowMs = 1_735_680_000_123;
    const { exportDir, manifest, replay, sessionDir } = await runOfflineQueueDemo({
      rootDir,
      nowMs,
    });
    assert.equal(manifest.schema_version, 1);
    assert.equal(manifest.snapshot.state, "enabled");
    assert.equal(manifest.snapshot.app_to_wallet.depth, 2);
    assert.equal(manifest.snapshot.wallet_to_app.depth, 1);
    assert.equal(manifest.files.app_queue_filename, "app_to_wallet.queue");
    assert.equal(manifest.files.wallet_queue_filename, "wallet_to_app.queue");
    assert.equal(manifest.files.metrics_filename, "metrics.ndjson");

    assert.equal(replay.appToWallet.length, 2);
    assert.equal(replay.walletToApp.length, 1);
    assert.ok(
      replay.appToWallet.every((record) => record.receivedAtMs >= nowMs),
      "app->wallet records must be stamped with demo timestamp",
    );

    const manifestDisk = JSON.parse(
      await fs.readFile(path.join(exportDir, "manifest.json"), "utf8"),
    );
    assert.equal(manifestDisk.snapshot.session_id_base64, manifest.snapshot.session_id_base64);

    const queueText = await fs.readFile(path.join(sessionDir, "app_to_wallet.queue"), "utf8");
    assert(queueText.trim().length > 0, "spool must contain encoded frames");
  });
});
