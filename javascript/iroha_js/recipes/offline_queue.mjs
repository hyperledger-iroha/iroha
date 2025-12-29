import { Buffer } from "node:buffer";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { ConnectQueueJournal } from "../src/connectQueueJournal.js";
import { ConnectDirection } from "../src/connectJournalRecord.js";
import {
  appendConnectQueueMetric,
  deriveConnectSessionDirectory,
  exportConnectQueueEvidence,
  writeConnectQueueSnapshot,
} from "../src/connectQueueDiagnostics.js";

const DEFAULT_DEMO_SESSION_ID = "AQIDBAUGBwgJAAECAwQFBgcICQ"; // base64url for deterministic demo bytes
const DEFAULT_DEMO_TIMESTAMP_MS = Number(
  process.env.CONNECT_NOW_MS ?? 1_735_680_000_000,
); // 2025-01-01T00:00:00Z

function encodeFrames(records) {
  if (!records || records.length === 0) {
    return "";
  }
  return `${records
    .map((record) => Buffer.from(record.encode()).toString("base64"))
    .join("\n")}\n`;
}

/**
 * Build a deterministic offline Connect queue bundle: append a few frames to the in-memory
 * journal, write a snapshot/metrics bundle to disk, and export a manifest for later replay.
 * Returns the manifest plus the replayable records so tests and scripts can assert behaviour.
 * @param {{
 *   rootDir?: string;
 *   sessionId?: string;
 *   nowMs?: number;
 * }} [options]
 */
export async function runOfflineQueueDemo(options = {}) {
  const nowMs = options.nowMs ?? DEFAULT_DEMO_TIMESTAMP_MS;
  const rootDir =
    options.rootDir ??
    (await fs.mkdtemp(path.join(os.tmpdir(), "iroha-js-offline-demo-")));
  const sessionId = options.sessionId ?? DEFAULT_DEMO_SESSION_ID;
  const journal = new ConnectQueueJournal(sessionId, {
    storage: "memory",
    retentionMs: 4 * 60 * 60 * 1000,
  });

  const samples = [
    {
      direction: ConnectDirection.APP_TO_WALLET,
      sequence: 1n,
      payload: Buffer.from(
        process.env.CONNECT_APP_PAYLOAD ?? "app->wallet:seed-1",
        "utf8",
      ),
    },
    {
      direction: ConnectDirection.APP_TO_WALLET,
      sequence: 2n,
      payload: Buffer.from(
        process.env.CONNECT_APP_PAYLOAD_FOLLOW ?? "app->wallet:seed-2",
        "utf8",
      ),
    },
    {
      direction: ConnectDirection.WALLET_TO_APP,
      sequence: 1n,
      payload: Buffer.from(
        process.env.CONNECT_WALLET_PAYLOAD ?? "wallet->app:ack-1",
        "utf8",
      ),
    },
  ];

  for (const sample of samples) {
    await journal.append(sample.direction, sample.sequence, sample.payload, {
      receivedAtMs: nowMs + Number(sample.sequence),
      retentionMs: 4 * 60 * 60 * 1000,
    });
  }

  const appToWalletRecords = await journal.records(ConnectDirection.APP_TO_WALLET, {
    nowMs,
  });
  const walletToAppRecords = await journal.records(ConnectDirection.WALLET_TO_APP, {
    nowMs,
  });

  const sessionDir = deriveConnectSessionDirectory({ sid: sessionId, rootDir });
  await fs.mkdir(sessionDir, { recursive: true });
  const sessionIdBase64 = path.basename(sessionDir);

  const appBytes = appToWalletRecords.reduce(
    (sum, record) => sum + record.encodedLength,
    0,
  );
  const walletBytes = walletToAppRecords.reduce(
    (sum, record) => sum + record.encodedLength,
    0,
  );

  const snapshot = {
    schema_version: 1,
    session_id_base64: sessionIdBase64,
    state: "enabled",
    reason: null,
    warning_watermark: 0.6,
    drop_watermark: 0.85,
    last_updated_ms: nowMs,
    app_to_wallet: {
      depth: appToWalletRecords.length,
      bytes: appBytes,
      oldest_sequence: appToWalletRecords[0]
        ? Number(appToWalletRecords[0].sequence)
        : null,
      newest_sequence: appToWalletRecords.at(-1)
        ? Number(appToWalletRecords.at(-1).sequence)
        : null,
      oldest_timestamp_ms: appToWalletRecords[0]?.receivedAtMs ?? null,
      newest_timestamp_ms: appToWalletRecords.at(-1)?.receivedAtMs ?? null,
    },
    wallet_to_app: {
      depth: walletToAppRecords.length,
      bytes: walletBytes,
      oldest_sequence: walletToAppRecords[0]
        ? Number(walletToAppRecords[0].sequence)
        : null,
      newest_sequence: walletToAppRecords.at(-1)
        ? Number(walletToAppRecords.at(-1).sequence)
        : null,
      oldest_timestamp_ms: walletToAppRecords[0]?.receivedAtMs ?? null,
      newest_timestamp_ms: walletToAppRecords.at(-1)?.receivedAtMs ?? null,
    },
  };

  await writeConnectQueueSnapshot(snapshot, { rootDir, sid: sessionId });

  await appendConnectQueueMetric(
    sessionId,
    {
      timestamp_ms: nowMs,
      state: snapshot.state,
      app_to_wallet_depth: snapshot.app_to_wallet.depth,
      wallet_to_app_depth: snapshot.wallet_to_app.depth,
    },
    { rootDir },
  );

  await fs.writeFile(
    path.join(sessionDir, "app_to_wallet.queue"),
    encodeFrames(appToWalletRecords),
    "utf8",
  );
  await fs.writeFile(
    path.join(sessionDir, "wallet_to_app.queue"),
    encodeFrames(walletToAppRecords),
    "utf8",
  );

  const exportDir = path.join(rootDir, "bundle");
  const { manifest } = await exportConnectQueueEvidence(sessionId, exportDir, { rootDir });

  return {
    rootDir,
    exportDir,
    sessionDir,
    manifest,
    replay: {
      appToWallet: appToWalletRecords,
      walletToApp: walletToAppRecords,
    },
  };
}

if (import.meta.main) {
  runOfflineQueueDemo()
    .then(({ exportDir, replay }) => {
      const manifestPath = path.join(exportDir, "manifest.json");
      const summary = {
        exportDir,
        manifestPath,
        appFrames: replay.appToWallet.length,
        walletFrames: replay.walletToApp.length,
      };
      // eslint-disable-next-line no-console
      console.log(JSON.stringify(summary, null, 2));
    })
    .catch((error) => {
      // eslint-disable-next-line no-console
      console.error("offline queue demo failed", error);
      process.exit(1);
    });
}
