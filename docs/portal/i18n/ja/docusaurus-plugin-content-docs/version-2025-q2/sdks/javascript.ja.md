---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3d65200b16bfc42048f91fe0ac2c6f57aa0ff1092b462684028e85f0182cb9d
source_last_modified: "2026-01-30T17:50:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c677012c88133bce76df1704fb9c23a98f4843d891cf13f48b5cebbe2d898ce6
source_last_modified: "2026-01-30T13:34:37+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: JavaScript SDK quickstart
description: Build transactions, stream events, and drive Connect previews with `@iroha/iroha-js`.
slug: /sdks/javascript
---

`@iroha/iroha-js` is the canonical Node.js package for interacting with Torii. It
bundles Norito builders, Ed25519 helpers, pagination utilities, and a resilient
HTTP/WebSocket client so you can mirror the CLI flows from TypeScript.

## Installation

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

The build step wraps `cargo build -p iroha_js_host`. Ensure the toolchain from
`rust-toolchain.toml` is available locally before running `npm run build:native`.

## Key management

```ts
import {
  generateKeyPair,
  publicKeyFromPrivate,
  signEd25519,
  verifyEd25519,
} from "@iroha/iroha-js";

const { publicKey, privateKey } = generateKeyPair();

const message = Buffer.from("hello iroha");
const signature = signEd25519(message, privateKey);

console.assert(verifyEd25519(message, signature, publicKey));

const derived = publicKeyFromPrivate(privateKey);
console.assert(Buffer.compare(derived, publicKey) === 0);
```

## Build transactions

Norito instruction builders normalise identifiers, metadata, and quantities so
encoded transactions match the Rust/CLI payloads.

```ts
import {
  buildMintAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
} from "@iroha/iroha-js";

const mint = buildMintAssetInstruction({
  assetId: "rose#wonderland#alice",
  quantity: "10",
});

const transfer = buildTransferAssetInstruction({
  sourceAssetId: "rose#wonderland#alice",
  destinationAccountId: "ih58...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "ih58...",
  mint: { assetId: "rose#wonderland#alice", quantity: "10" },
  transfers: [{ destinationAccountId: "ih58...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Torii client configuration

`ToriiClient` accepts retry/timeout knobs that mirror `iroha_config`. Use
`resolveToriiClientConfig` to merge a camelCase config object (normalize
`iroha_config` first), env overrides, and inline options.

```ts
import { ToriiClient, resolveToriiClientConfig } from "@iroha/iroha-js";
import fs from "node:fs";

const rawConfig = JSON.parse(fs.readFileSync("./iroha_config.json", "utf8"));
const config = rawConfig?.torii
  ? {
      ...rawConfig,
      torii: {
        ...rawConfig.torii,
        apiTokens: rawConfig.torii.api_tokens ?? rawConfig.torii.apiTokens,
      },
    }
  : rawConfig;
const clientConfig = resolveToriiClientConfig({
  config,
  overrides: { timeoutMs: 2_000, maxRetries: 5 },
});

const torii = new ToriiClient(
  config?.torii?.address ?? "http://localhost:8080",
  {
    config,
    timeoutMs: clientConfig.timeoutMs,
    maxRetries: clientConfig.maxRetries,
  },
);
```

Environment variables for local dev:

| Variable | Purpose |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Request timeout (milliseconds). |
| `IROHA_TORII_MAX_RETRIES` | Maximum retry attempts. |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Initial retry backoff. |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Exponential backoff multiplier. |
| `IROHA_TORII_MAX_BACKOFF_MS` | Maximum retry delay. |
| `IROHA_TORII_RETRY_STATUSES` | Comma-separated HTTP status codes to retry. |
| `IROHA_TORII_RETRY_METHODS` | Comma-separated HTTP methods to retry. |
| `IROHA_TORII_API_TOKEN` | Adds `X-API-Token`. |
| `IROHA_TORII_AUTH_TOKEN` | Adds `Authorization: Bearer …` header. |

Retry profiles mirror Android defaults and are exported for parity checks:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`. See `docs/source/sdk/js/torii_retry_policy.md`
for the endpoint-to-profile mapping and the parameters governance audits during
JS4/JS7.

## Iterable lists & pagination

Pagination helpers mirror the Python SDK ergonomics for `/v1/accounts`,
`/v1/domains`, `/v1/assets/definitions`, NFTs, balances, asset holders, and the
account transaction history.

```ts
const { items, total } = await torii.listDomains({
  limit: 25,
  sort: [{ key: "id", order: "asc" }],
});
console.log(`first page out of ${total}`, items);

for await (const account of torii.iterateAccounts({
  pageSize: 50,
  maxItems: 200,
})) {
  console.log(account.id);
}

const defs = await torii.queryAssetDefinitions({
  filter: { Eq: ["metadata.display_name", "Ticket"] },
  sort: [{ key: "metadata.display_name", order: "desc" }],
  fetchSize: 64,
});
console.log("filtered definitions", defs.items);

const assetId = "rose#wonderland#alice@test";
const balances = await torii.listAccountAssets("alice@test", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("alice@test", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("rose#wonderland", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## Offline allowances & verdict metadata

Offline allowance responses expose the enriched ledger metadata up-front —
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex`, and `remaining_amount` are returned alongside the raw
record so dashboards don’t have to decode the embedded Norito payloads. The new
countdown helpers (`deadline_kind`, `deadline_state`, `deadline_ms`,
`deadline_ms_remaining`) highlight the next expiring deadline (refresh → policy
→ certificate) so UI badges can warn operators whenever an allowance has
<24 h remaining. The SDK
mirrors the REST filters exposed by `/v1/offline/allowances`:
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`, `attestationNonceHex`, `refreshBeforeMs/AfterMs`, and the
`requireVerdict` / `onlyMissingVerdict` booleans. Invalid combinations (for
example `onlyMissingVerdict` + `verdictIdHex`) are rejected locally before Torii
is called.

```ts
const { items: allowances } = await torii.listOfflineAllowances({
  limit: 25,
  policyExpiresBeforeMs: Date.now() + 86_400_000,
  requireVerdict: true,
});

for (const entry of allowances) {
  console.log(
    entry.controller_display,
    entry.remaining_amount,
    entry.verdict_id_hex,
    entry.refresh_at_ms,
  );
}
```

## Offline top-ups (issue + register)

Use the top-up helpers when you want to issue a certificate and immediately
register it on-ledger. The SDK verifies the issued and registered certificate
IDs match before returning, and the response includes both payloads. There is
no dedicated top-up endpoint; the helper chains the issue + register calls. If
you already have a signed certificate, call `registerOfflineAllowance` (or
`renewOfflineAllowance`) directly.

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "alice@wonderland",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "alice@wonderland",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## Torii queries & streaming (WebSockets)

Query helpers expose status, Prometheus metrics, telemetry snapshots, and event
streams using the Norito filter grammar. Streaming automatically upgrades to
WebSockets and resumes when the retry budget allows.

```ts
const status = await torii.getSumeragiStatus();
console.log(status?.leader_index);

const metrics = await torii.getMetrics({ asText: true });
console.log(metrics.split("\n").slice(0, 5));

const abort = new AbortController();
for await (const event of torii.streamEvents({
  filter: { Pipeline: { Block: {} } },
  signal: abort.signal,
})) {
  console.log(event.id, event.data);
  break;
}
abort.abort(); // closes the underlying WebSocket cleanly
```

Use `streamBlocks`, `streamTransactions`, or `streamTelemetry` for the other
WebSocket endpoints. All streaming helpers surface retry attempts, so hook the
`onReconnect` callback to feed dashboards and alerting.

## Explorer snapshots & QR payloads

Explorer telemetry provides typed helpers for the `/v1/explorer/metrics` and
`/v1/explorer/accounts/{account_id}/qr` endpoints so dashboards can replay the
same snapshots that power the portal. `getExplorerMetrics()` normalises the
payload and returns `null` when the route is disabled. Pair it with
`getExplorerAccountQr()` whenever you need IH58 (preferred)/sora (second-best) literals plus inline
SVG for share buttons.

```ts
import { promises as fs } from "node:fs";

const snapshot = await torii.getExplorerMetrics();
if (!snapshot) {
  console.warn("explorer metrics unavailable");
} else {
  console.log("peers:", snapshot.peers);
  console.log("last block:", snapshot.blockHeight, snapshot.blockCreatedAt);
  console.log("avg commit ms:", snapshot.averageCommitTimeMs ?? "n/a");
}

const qr = await torii.getExplorerAccountQr("ih58...", {
  addressFormat: "compressed",
});
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

Passing `addressFormat: "compressed"` mirrors Explorer’s default compressed
selectors; omit the override for the preferred IH58 output or request `ih58_qr`
when you need the QR-safe variant. The compressed literal is the second-best
Sora-only option for UX. The helper always returns the canonical identifier,
the selected literal, and metadata (network prefix, QR version/modules, error
correction tier, and inline SVG), so CI/CD can publish the same payloads that
the Explorer surfaces without calling bespoke converters.

## Connect sessions & queueing

The Connect helpers mirror `docs/source/connect_architecture_strawman.md`. The
fastest path to a preview-ready session is `bootstrapConnectPreviewSession`,
which stitches together deterministic SID/URI generation and the Torii
registration call.

```ts
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { preview, session, tokens } = await bootstrapConnectPreviewSession(
  torii,
  {
    chainId: "sora-mainnet",
    node: "https://torii.nexus.example",
    sessionOptions: { node: "https://torii.backup.example" },
  },
);

console.log("wallet QR", preview.walletUri);
console.log("Connect tokens", tokens?.wallet, tokens?.app);
```

- Pass `register: false` when you only need deterministic URIs for QR/deeplink
  previews.
- `generateConnectSid` stays available when you need to derive session ids
  without minting URIs.
- Directional keys and ciphertext envelopes come from the native bridge; when
  unavailable the SDK falls back to the JSON codec and throws
  `ConnectQueueError.bridgeUnavailable`.
- Offline buffers are stored as Norito `.to` blobs in IndexedDB. Monitor queue
  state via the emitted `ConnectQueueError.overflow(limit)` /
  `.expired(ttlMs)` errors and feed `connect.queue_depth` telemetry as outlined
  in the roadmap.

### Connect registry & policy snapshots

Platform operators can introspect and update the Connect registry without
leaving Node.js. `iterateConnectApps()` pages through the registry, while
`getConnectStatus()` and `getConnectAppPolicy()` expose the runtime counters and
current policy envelope. `updateConnectAppPolicy()` accepts camelCase fields,
so you can stage the same JSON payload that Torii expects.

```ts
const status = await torii.getConnectStatus();
console.log("connect enabled:", status?.enabled ?? false);
console.log("active sessions:", status?.sessionsActive ?? 0);
console.log("buffered bytes:", status?.totalBufferBytes ?? 0);

for await (const app of torii.iterateConnectApps({ limit: 100 })) {
  console.log(app.appId, app.namespaces, app.policy?.relayEnabled ? "relay" : "wallet-only");
}

const policy = await torii.getConnectAppPolicy();
if ((policy.wsPerIpMaxSessions ?? 0) < 5) {
  await torii.updateConnectAppPolicy({
    wsPerIpMaxSessions: 5,
    pingIntervalMs: policy.pingIntervalMs ?? 30_000,
    pingMissTolerance: policy.pingMissTolerance ?? 3,
  });
}
```

Always capture the latest `getConnectStatus()` snapshot before applying
mutations—the governance checklist requires evidence that policy updates start
from the fleet’s current limits.

### Connect WebSocket dialling

`ToriiClient.openConnectWebSocket()` assembles the canonical
`/v1/connect/ws` URL (including `sid`, `role`, and token parameters), upgrades
`http→ws` / `https→wss`, and hands the final URL to whichever WebSocket
implementation you supply. Browsers automatically reuse the global
`WebSocket`. Node.js callers should pass a constructor such as `ws`:

```ts
import WebSocket from "ws";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.IROHA_TORII_URL ?? "https://torii.nexus.example");
const preview = await torii.createConnectSessionPreview({ chainId: "sora-mainnet" });
const session = await torii.createConnectSession({ sid: preview.sidBase64Url });

const socket = torii.openConnectWebSocket({
  sid: session.sid,
  role: "wallet",
  token: session.token_wallet,
  WebSocketImpl: WebSocket,
  protocols: ["iroha-connect"],
});

socket.addEventListener("message", (event) => {
  console.log("Connect payload", event.data);
});
socket.addEventListener("close", () => {
  console.log("Connect socket closed");
});

socket.binaryType = "arraybuffer";
socket.addEventListener("message", (event) => {
  if (typeof event.data === "string") {
    const control = JSON.parse(event.data);
    console.log("[ws] control", control.kind);
    return;
  }
  pendingFrames.enqueue(new Uint8Array(event.data));
});
```

When you only need the URL, call `torii.buildConnectWebSocketUrl(params)` or the
top-level `buildConnectWebSocketUrl(baseUrl, params)` helper and reuse the
resulting string in a custom transport/queue.

Looking for a complete CLI-oriented sample? The
[Connect preview recipe](./recipes/javascript-connect-preview.md) includes a
runnable script plus telemetry guidance that mirrors the roadmap deliverable for
documenting the Connect queue + WebSocket flow.

### Queue telemetry & alerting

Wire queue metrics directly into the helper surfaces so dashboards can mirror
the roadmap KPIs.

```ts
import { bootstrapConnectPreviewSession, ConnectQueueError } from "@iroha/iroha-js";

async function dialWithTelemetry(client: ToriiClient) {
  try {
    const { session } = await bootstrapConnectPreviewSession(client, { chainId: "sora-mainnet" });
    queueDepthGauge.record(session.queue_depth ?? 0);
    // …open the WebSocket here…
  } catch (error) {
    if (error instanceof ConnectQueueError) {
      if (error.kind === ConnectQueueError.KIND.OVERFLOW) {
        queueOverflowCounter.add(1, { limit: error.limit ?? 0 });
      } else if (error.kind === ConnectQueueError.KIND.EXPIRED) {
        queueExpiryCounter.add(1, { ttlMs: error.ttlMs ?? 0 });
      }
      return;
    }
    throw error;
  }
}
```

`ConnectQueueError#toConnectError()` converts queue failures into the generic
`ConnectError` taxonomy so shared HTTP/WebSocket interceptors can emit the
standard `connect.queue_depth`, `connect.queue_overflow_total`, and
`connect.queue_expired_total` metrics referenced throughout the roadmap.

## Streaming watchers & event cursors

`ToriiClient.streamEvents()` exposes `/v1/events/sse` as an async iterator with automatic
retries, so Node/Bun CLIs can tail pipeline activity the same way the Rust CLI does.
Persist the `Last-Event-ID` cursor alongside your runbook artefacts so operators can
resume a stream without skipping events when a process restarts.

```ts
import fs from "node:fs/promises";
import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080");
const cursorFile = process.env.STREAM_CURSOR_FILE ?? ".cache/torii.cursor";
const resumeId = await fs
  .readFile(cursorFile, "utf8")
  .then((value) => value.trim())
  .catch(() => null);
const controller = new AbortController();

process.once("SIGINT", () => controller.abort());
process.once("SIGTERM", () => controller.abort());

for await (const event of torii.streamEvents({
  filter: { Pipeline: { Transaction: { status: "Committed" } } },
  lastEventId: resumeId || undefined,
  signal: controller.signal,
})) {
  if (event.id) {
    await fs.writeFile(cursorFile, `${event.id}\n`, "utf8");
  }
  const status = event.data ? extractPipelineStatusKind(event.data) : null;
  console.log(`[${event.event}] id=${event.id ?? "∅"} status=${status ?? "n/a"}`);
}
```

- Switch `PIPELINE_STATUS` (for example `Pending`, `Applied`, or `Approved`) or set
  `STREAM_FILTER_JSON` to replay the same filters the CLI accepts.
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` keeps the iterator alive until a
  signal is received; pass `STREAM_MAX_EVENTS=25` when you only need the first few events
  for a smoke test.
- `ToriiClient.streamSumeragiStatus()` mirrors the same interface for
  `/v1/sumeragi/status/sse` so consensus telemetry can be tailed separately, and the
  iterator honours `Last-Event-ID` the same way.
- See `javascript/iroha_js/recipes/streaming.mjs` for a turnkey CLI (cursor persistence,
  env-var filter overrides, and `extractPipelineStatusKind` logging) used in the JS4
  streaming/WebSocket roadmap deliverable.

## UAID portfolios & Space Directory

The Space Directory APIs surface the Universal Account ID (UAID) lifecycle. The
helpers accept `uaid:<hex>` literals or raw 64-hex digests (LSB=1) and
canonicalise them before submitting requests:

- `getUaidPortfolio(uaid, { assetId })` aggregates balances per dataspace,
  grouping asset holdings by canonical account IDs; pass `assetId` to filter the
  portfolio down to a single asset instance.
- `getUaidBindings(uaid, { addressFormat })` enumerates every dataspace ↔ account
  binding (`addressFormat: "compressed"` returns the `sora…` literals).
- `getUaidManifests(uaid, { dataspaceId })` returns each capability manifest,
  lifecycle status, and bound accounts for auditing.

For operator evidence packs, manifest publish/revoke flows, and SDK migration
guidance, follow the Universal Account Guide (`docs/source/universal_accounts_guide.md`)
alongside these client helpers so the portal and source documentation remain in sync.

```ts
import { promises as fs } from "node:fs";

const uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaid, {
  assetId: "cash#global::holder@global",
});
portfolio.dataspaces.forEach((entry) => {
  console.log(entry.dataspace_alias ?? entry.dataspace_id, entry.accounts.length);
});

const bindings = await torii.getUaidBindings(uaid, { addressFormat: "compressed" });
console.log("bindings", bindings.dataspaces);

const manifests = await torii.getUaidManifests(uaid, { dataspaceId: 11 });
console.log("manifests", manifests.manifests[0].manifest.entries.length);
```

Operators can also rotate manifests or execute emergency deny-wins flows without
dropping to the CLI. Both helpers accept an optional `{ signal }` object so
long-running submissions can be cancelled with `AbortController`; non-object
options or non-`AbortSignal` inputs raise a synchronous `TypeError` before the
request hits Torii:

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "ih58...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "ih58...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` accepts either raw manifest JSON (matching the
fixtures under `fixtures/space_directory/`) or any object that serialises to the
same structure. `privateKey`, `privateKeyHex`, or `privateKeyMultihash` map to
the `ExposedPrivateKey` field Torii expects and default to the `ed25519`
algorithm when no prefix is supplied. Both requests return once Torii enqueues
the instruction (`202 Accepted`), at which point the ledger will emit the
matching `SpaceDirectoryEvent`.

## Governance & ISO bridge

`ToriiClient` exposes the governance APIs for inspecting contracts, staging
proposals, submitting ballots (plain or ZK), rotating the council, and calling
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` without hand-written DTOs. ISO&nbsp;20022 helpers
follow the same pattern via `buildPacs008Message`/`buildPacs009Message` and the
`submitIso*`/`waitForIsoMessageStatus` trio.

See the [governance & ISO bridge recipe](./recipes/javascript-governance-iso.md)
for CLI-ready samples plus pointers back to the full field guide in
`docs/source/sdk/js/governance_iso_examples.md`.

## RBC sampling & delivery evidence

The JS roadmap also requires Roadrunner Block Commitment (RBC) sampling so operators can
prove that the block they fetched through Sumeragi matches the chunk proofs they verify.
Use the built-in helpers instead of building payloads by hand:

1. `getSumeragiRbcSessions()` mirrors `/v1/sumeragi/rbc/sessions`, and
   `findRbcSamplingCandidate()` auto-selects the first delivered session with a block hash
   (the integration suite falls back to it whenever
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` is unset).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` normalises `{blockHash,height,view}`
   plus optional `{count,seed,apiToken}` overrides so malformed hex or negative integers never
   reach Torii.
3. `sampleRbcChunks()` POSTs the request to `/v1/sumeragi/rbc/sample`, returning chunk proofs
   and Merkle paths (`samples[].chunkHex`, `chunkRoot`, `payloadHash`) you should archive with
   the rest of your adoption evidence.
4. `getSumeragiRbcDelivered(height, view)` captures the cohort’s delivery metadata so auditors
   can replay the proof end-to-end.

```js
import assert from "node:assert";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080", {
  apiToken: process.env.TORII_API_TOKEN,
});

const candidate =
  (await torii.findRbcSamplingCandidate().catch(() => null)) ??
  (await torii.getSumeragiRbcSessions()).items.find((session) => session.delivered);
if (!candidate) {
  throw new Error("no delivered RBC session available; set IROHA_TORII_INTEGRATION_RBC_SAMPLE");
}

const request = ToriiClient.buildRbcSampleRequest(candidate, {
  count: Number(process.env.RBC_SAMPLE_COUNT ?? 2),
  seed: Number(process.env.RBC_SAMPLE_SEED ?? 0),
  apiToken: process.env.RBC_SAMPLE_API_TOKEN ?? process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks(request);
sample.samples.forEach((chunk) => {
  assert.ok(Buffer.from(chunk.chunkHex, "hex").length > 0, "chunk must be hex");
});

const delivery = await torii.getSumeragiRbcDelivered(sample.height, sample.view);
console.log(
  `rbc height=${sample.height} view=${sample.view} chunks=${sample.samples.length} delivered=${delivery?.delivered}`,
);
```

Persist both responses under the artefact root you submit to governance. Override the
auto-selected session via `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`
whenever you need to probe a specific block, and treat failures to fetch RBC snapshots as a
pre-flight gating error rather than silently downgrading to direct mode.

## Testing & CI

1. Cache cargo and npm artifacts.
2. Run `npm run build:native`.
3. Execute `npm test` (or `node --test` for smoke jobs).

The reference GitHub Actions workflow lives in
`docs/source/examples/iroha_js_ci.md`.

## Next steps

- Review the generated types in `javascript/iroha_js/index.d.ts`.
- Explore the recipes under `javascript/iroha_js/recipes/`.
- Pair `ToriiClient` with the Norito quickstart to inspect payloads alongside
  SDK calls.
