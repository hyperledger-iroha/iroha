import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  OfflineCounterJournal,
  OfflineCounterJournalError,
} from "../src/offlineCounterJournal.js";

const FIXTURE_ALICE_I105 =
  "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";

async function withTempDir(prefix, fn) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), prefix));
  try {
    await fn(dir);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

test("offline counter journal persists summary hashes", async () => {
  await withTempDir("iroha-js-counters-", async (dir) => {
    const journalPath = path.join(dir, "journal.json");
    const apple = { "key-1": 2, "key-2": 5 };
    const android = { "series-1": 7 };
    const summaryHash = OfflineCounterJournal.computeSummaryHashHex(apple, android).toLowerCase();
    const payload = {
      items: [
        {
          certificate_id_hex: "DEADBEEF",
          controller_id: FIXTURE_ALICE_I105,
          controller_display: "Alice",
          summary_hash_hex: summaryHash,
          apple_key_counters: apple,
          android_series_counters: android,
        },
      ],
      total: 1,
    };

    const journal = new OfflineCounterJournal({ storagePath: journalPath });
    await journal.upsert(payload, { recordedAtMs: 1_000 });

    const reloaded = new OfflineCounterJournal({ storagePath: journalPath });
    const checkpoint = await reloaded.checkpoint("deadbeef");
    assert.ok(checkpoint);
    assert.equal(checkpoint.summaryHashHex, summaryHash);
    assert.equal(checkpoint.appleKeyCounters["key-1"], 2);
  });
});

test("offline counter journal rejects counter jumps", async () => {
  const journal = new OfflineCounterJournal({ storage: "memory" });
  await journal.updateCounter({
    certificateIdHex: "deadbeef",
    controllerId: FIXTURE_ALICE_I105,
    platform: "apple_key",
    scope: "key-1",
    counter: 1,
    recordedAtMs: 1,
  });

  await assert.rejects(
    () =>
      journal.updateCounter({
        certificateIdHex: "deadbeef",
        controllerId: FIXTURE_ALICE_I105,
        platform: "apple_key",
        scope: "key-1",
        counter: 3,
        recordedAtMs: 2,
      }),
    (err) => err instanceof OfflineCounterJournalError && err.code === "counter_jump",
  );
});

test("offline counter journal rejects fractional recordedAtMs", async () => {
  const journal = new OfflineCounterJournal({ storage: "memory" });
  await assert.rejects(
    () =>
      journal.updateCounter({
        certificateIdHex: "deadbeef",
        controllerId: FIXTURE_ALICE_I105,
        platform: "apple_key",
        scope: "key-1",
        counter: 1,
        recordedAtMs: 1.5,
      }),
    (err) =>
      err instanceof OfflineCounterJournalError && err.code === "invalid_recorded_at",
  );
});

test("offline counter journal validates summary hash parity", async () => {
  const journal = new OfflineCounterJournal({ storage: "memory" });
  const payload = {
    items: [
      {
        certificate_id_hex: "DEADBEEF",
        controller_id: FIXTURE_ALICE_I105,
        controller_display: "Alice",
        summary_hash_hex: "00".repeat(32),
        apple_key_counters: { "key-1": 1 },
        android_series_counters: {},
      },
    ],
    total: 1,
  };

  await assert.rejects(
    () => journal.upsert(payload, { recordedAtMs: 1_000 }),
    (err) =>
      err instanceof OfflineCounterJournalError &&
      err.code === "summary_hash_mismatch",
  );
});
