---
lang: ar
direction: rtl
source: docs/source/sdk/js/offline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2b6d8b5edd0e8a1b1740c51d76b4fb4344e4273ae547aad999ee203d4096c28
source_last_modified: "2026-01-23T20:25:52.728869+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Offline envelopes and Connect queues

JavaScript mirrors the offline workflows documented for Swift (`docs/source/sdk/swift/offline.md`)
and Android (`docs/source/sdk/android/offline_signing.md`). This guide covers two surfaces:

1. Packaging signed transactions into deterministic offline envelopes for replay.
2. Inspecting Connect queues and exporting evidence bundles.

## Transaction envelopes (pipeline)

Build and persist envelopes with `buildOfflineEnvelope`. The helper normalises metadata, enforces
non-empty hashes/schema/key aliases, and computes the canonical `hashHex` with
`hashSignedTransaction` when omitted.

```js
import {
  buildMintAssetInstruction,
  buildOfflineEnvelope,
  buildTransaction,
  writeOfflineEnvelopeFile,
} from "@iroha/iroha-js";

const { signedTransaction } = buildTransaction({
  chainId: "offline-demo",
  authority: "soraカタカナ...",
  instructions: [
    buildMintAssetInstruction({
      assetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      quantity: "10",
    }),
  ],
  metadata: { purpose: "offline-demo" },
  privateKey: Buffer.alloc(32, 0x11),
});

const envelope = buildOfflineEnvelope({
  signedTransaction,
  keyAlias: "alice-key",
  metadata: { purpose: "offline-demo" },
});
await writeOfflineEnvelopeFile("./artifacts/js/offline/envelope.json", envelope);
```

Replay envelopes against Torii with the built-in poller (override `intervalMs`/`timeoutMs` with
non-negative integers as needed):

```js
import { ToriiClient, readOfflineEnvelopeFile, replayOfflineEnvelope } from "@iroha/iroha-js";

const envelope = await readOfflineEnvelopeFile("./artifacts/js/offline/envelope.json");
const torii = new ToriiClient("https://torii.devnet.example");
await replayOfflineEnvelope(torii, envelope, { intervalMs: 250, timeoutMs: 15_000 });
```

For a deterministic, self-contained flow, run:

```bash
npm run example:offline:pipeline
```

The recipe spins up a mock Torii server, replays the envelope, and logs both the locally computed
hash and what the mock observes. The JSON payload uses base64 payloads and typed fields so you can
hand envelopes between devices without losing deterministic hashes.

## Offline counter journals

Use `OfflineCounterJournal` to persist platform counters and enforce
`summary_hash_hex` parity plus monotonic increments before submitting bundles.
`recordedAtMs` must be a non-negative integer timestamp in milliseconds.
The journal defaults to `~/.iroha/offline_counters/journal.json` and supports
`storage: "memory"` for deterministic tests.

```js
import { OfflineCounterJournal } from "@iroha/iroha-js";

const journal = new OfflineCounterJournal();
const summaries = await torii.listOfflineSummaries({ limit: 50 });
await journal.upsert(summaries, { recordedAtMs: Date.now() });

await journal.advanceCounterFromProof({
  certificateIdHex: summaries.items[0].certificate_id_hex,
  controllerId: summaries.items[0].controller_id,
  platformProof: receipt.platform_proof,
});
```

## Connect offline queues, journals, and evidence bundles

Connect sessions record their buffered frames to per-session queues so wallets and apps can retry
while offline and export an audit trail later. This section covers the storage layout, journal APIs,
and the evidence bundle helpers shipped in the JS SDK.

### Storage layout and defaults

- **Root directory:** `~/.iroha/connect` by default, driven by
  `connect.queue.root` in `client.toml`. JS helpers accept `connectConfig` to
  resolve this setting and only honour `IROHA_CONNECT_QUEUE_ROOT` when
  `allowEnvOverride: true` (intended for dev/tests); otherwise pass an explicit
  `rootDir`.
- **Session directory:** base64url of the Connect SID (for example,
  `~/.iroha/connect/AQIDBAUGBwgJAAECAwQFBgcICQ/`).
- **Files:** `state.json` (schema_version=1, warning_watermark=0.6, drop_watermark=0.85),
  optional `app_to_wallet.queue` and `wallet_to_app.queue`, and `metrics.ndjson` for time-series
  samples. `deriveConnectSessionDirectory()` computes the canonical path, and `read/write/update`
  helpers keep the schema/version in sync.

### Capture snapshots and export evidence

Use the diagnostics helpers to atomically write snapshots, append metrics, and emit an evidence
bundle for incident reviews:

```js
import {
  writeConnectQueueSnapshot,
  appendConnectQueueMetric,
  exportConnectQueueEvidence,
  deriveConnectSessionDirectory,
  defaultConnectQueueRoot,
} from "@iroha/iroha-js";

const sid = "AQIDBAUGBwgJAAECAwQFBgcICQ"; // base64url Connect session id
const clientConfig = {
  connect: { queue: { root: "/tmp/iroha-connect-demo" } }, // mirror connect.queue.root
};
const rootDir = defaultConnectQueueRoot({
  connectConfig: clientConfig,
  allowEnvOverride: true, // optional: enable IROHA_CONNECT_QUEUE_ROOT in dev/test harnesses
});
const sessionDir = deriveConnectSessionDirectory({ sid, rootDir });

await writeConnectQueueSnapshot(
  {
    schema_version: 1,
    session_id_base64: sid,
    state: "enabled",
    warning_watermark: 0.6,
    drop_watermark: 0.85,
    last_updated_ms: 1_735_680_000_000,
    app_to_wallet: { depth: 2, bytes: 2048 },
    wallet_to_app: { depth: 1, bytes: 512 },
  },
  { rootDir },
);
await appendConnectQueueMetric(
  sid,
  { state: "enabled", app_to_wallet_depth: 2, wallet_to_app_depth: 1 },
  { rootDir },
);
const { manifest, targetDir } = await exportConnectQueueEvidence(sid, `${sessionDir}/bundle`, {
  rootDir,
});
console.log(targetDir, manifest.files); // manifest.json + optional queues/metrics copied in
```

The `javascript/iroha_js/recipes/offline_queue.mjs` recipe stitches these steps together with
deterministic fixtures; run `npm run example:offline` or inspect
`javascript/iroha_js/test/offlineQueueRecipe.test.js` to see the expected manifest layout.

### Journal append/replay

`ConnectQueueJournal` keeps short-lived queues for both directions (`app_to_wallet`,
`wallet_to_app`) and prunes by depth, byte count, and TTL (defaults: 32 records, 1 MiB, 24 hours).
When IndexedDB is available (browsers), the journal uses it automatically; in Node.js it falls back
to in-memory storage or `storage: "memory"` when you want predictable behaviour during tests.

```js
import { ConnectQueueJournal, ConnectDirection } from "@iroha/iroha-js";

const journal = new ConnectQueueJournal("AQIDBA", { storage: "memory", retentionMs: 600_000 });
await journal.append(
  ConnectDirection.APP_TO_WALLET,
  1,
  Buffer.from("app->wallet:seed"),
  { receivedAtMs: Date.now() },
);
const pending = await journal.records(ConnectDirection.APP_TO_WALLET);
console.log(pending[0].sequence); // bigint sequence number

// replay oldest frame(s) and continue
await journal.popOldest(ConnectDirection.APP_TO_WALLET, 1);
```

Pair the journal with the snapshot/evidence helpers above to materialise offline queues on disk, and
ship the resulting bundle to operators for replay or audits.

## QR stream handoff

For device-to-device transfers, use the QR stream helpers from `offlineQrStream.js`:

- **Encode frames:** `OfflineQrStreamEncoder.encodeFrames(...)` produces header/data/parity frames.
- **Scan loop:** `scanQrStreamFrames(...)` accepts an async iterable of frames (strings or bytes)
  and feeds them into `OfflineQrStreamScanSession`.
- **Text fallback:** wrap frame bytes as `iroha:qr1:<base64(frame_bytes)>` using
  `encodeQrFrameText`, and decode with `decodeQrFrameText`.
- **Playback skins:** use `sakuraQrStreamSkin` for default animation,
  `sakuraQrStreamReducedMotionSkin` for accessibility, and `sakuraQrStreamLowPowerSkin` for
  battery-friendly playback.

## Petal stream handoff (custom scanner)

Petal stream renders the same QR stream frames as a sakura petal field and requires a custom
scanner. Use the helpers from `offlinePetalStream.js`:

- **Encode grids:** `OfflinePetalStreamEncoder.encodeGrids(frameBytes[])` returns a consistent
  `gridSize` and per-frame grids.
- **Sampling:** `samplePetalStreamGridFromRgba(image, gridSize)` converts RGBA frames into
  luminance samples.
- **Scan session:** `OfflinePetalStreamScanSession` ingests sample grids and returns the
  `OfflineQrStreamDecodeResult` for progress + payload recovery.
