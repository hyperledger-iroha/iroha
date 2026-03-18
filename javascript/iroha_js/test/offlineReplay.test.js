import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { ConnectDirection, ConnectQueueJournal } from "../src/index.js";
import {
  appendConnectQueueMetric,
  deriveConnectSessionDirectory,
  exportConnectQueueEvidence,
  readConnectQueueSnapshot,
  updateConnectQueueSnapshot,
} from "../src/connectQueueDiagnostics.js";

async function withTempDir(prefix, callback) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), prefix));
  try {
    await callback(dir);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

const SESSION_ID = "AQIDBAUG"; // base64url for 0x01..0x06

function bytesFor(records) {
  return records.reduce((sum, record) => sum + record.ciphertext.length, 0);
}

test("offline Connect queue replay + evidence bundle", async () => {
  await withTempDir("iroha-js-offline-", async (rootDir) => {
    const journal = new ConnectQueueJournal(SESSION_ID, { storage: "memory", maxRecordsPerQueue: 8 });
    const nowMs = 1_700_000_000_000;

    await journal.append(
      ConnectDirection.APP_TO_WALLET,
      1,
      Buffer.from("app->wallet:seed"),
      { receivedAtMs: nowMs },
    );
    await journal.append(
      ConnectDirection.APP_TO_WALLET,
      2,
      Buffer.from("app->wallet:followup"),
      { receivedAtMs: nowMs + 10 },
    );
    await journal.append(
      ConnectDirection.WALLET_TO_APP,
      1,
      Buffer.from("wallet->app:ack"),
      { receivedAtMs: nowMs + 20 },
    );

    const initialApp = await journal.records(ConnectDirection.APP_TO_WALLET, { nowMs });
    const initialWallet = await journal.records(ConnectDirection.WALLET_TO_APP, { nowMs });
    assert.equal(initialApp.length, 2);
    assert.equal(initialWallet.length, 1);
    assert.equal(initialApp[0].sequence, 1n);
    assert.equal(initialWallet[0].sequence, 1n);

    const popped = await journal.popOldest(ConnectDirection.APP_TO_WALLET, 1, { nowMs });
    assert.equal(popped.length, 1);
    assert.equal(Buffer.from(popped[0].ciphertext).toString("utf8"), "app->wallet:seed");

    const remainingApp = await journal.records(ConnectDirection.APP_TO_WALLET, { nowMs });
    assert.equal(remainingApp.length, 1);
    assert.equal(remainingApp[0].sequence, 2n);

    const snapshot = await updateConnectQueueSnapshot(
      SESSION_ID,
      (current) => ({
        ...current,
        state: "enabled",
        warning_watermark: 0.55,
        drop_watermark: 0.9,
        app_to_wallet: {
          ...current.app_to_wallet,
          depth: remainingApp.length,
          bytes: bytesFor(remainingApp),
          oldest_sequence: Number(remainingApp[0].sequence),
          newest_sequence: Number(remainingApp[remainingApp.length - 1].sequence),
        },
        wallet_to_app: {
          ...current.wallet_to_app,
          depth: initialWallet.length,
          bytes: bytesFor(initialWallet),
          oldest_sequence: Number(initialWallet[0].sequence),
          newest_sequence: Number(initialWallet[initialWallet.length - 1].sequence),
        },
      }),
      { rootDir },
    );
    assert.equal(snapshot.state, "enabled");
    assert.equal(snapshot.app_to_wallet.depth, 1);
    assert.equal(snapshot.wallet_to_app.depth, 1);

    await appendConnectQueueMetric(
      SESSION_ID,
      {
        state: snapshot.state,
        app_to_wallet_depth: snapshot.app_to_wallet.depth,
        wallet_to_app_depth: snapshot.wallet_to_app.depth,
      },
      { rootDir },
    );

    const sessionDir = deriveConnectSessionDirectory({ sid: SESSION_ID, rootDir });
    await fs.mkdir(sessionDir, { recursive: true });
    await fs.writeFile(path.join(sessionDir, "app_to_wallet.queue"), "queue-a", "utf8");
    await fs.writeFile(path.join(sessionDir, "wallet_to_app.queue"), "queue-b", "utf8");

    const exportDir = path.join(rootDir, "bundle");
    const { manifest } = await exportConnectQueueEvidence(SESSION_ID, exportDir, { rootDir });
    assert.equal(manifest.snapshot.state, "enabled");
    assert.equal(manifest.files.app_queue_filename, "app_to_wallet.queue");
    assert.equal(manifest.files.wallet_queue_filename, "wallet_to_app.queue");

    const manifestDisk = JSON.parse(
      await fs.readFile(path.join(exportDir, "manifest.json"), "utf8"),
    );
    assert.equal(manifestDisk.files.metrics_filename, "metrics.ndjson");

    const { snapshot: roundTripSnapshot } = await readConnectQueueSnapshot({ sid: SESSION_ID, rootDir });
    assert.equal(roundTripSnapshot.state, "enabled");
  });
});
