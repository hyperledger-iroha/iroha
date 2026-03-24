---
lang: mn
direction: ltr
source: docs/source/sdk/js/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 137a43a6ef4f6bfba104aba151e73785aab912fe4df34cb52ee59540c47e6fec
source_last_modified: "2026-01-30T18:06:03.356414+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Iroha JS SDK Quickstart

The `@iroha/iroha-js` package provides a Node.js interface for building and
submitting Norito transactions, managing Ed25519 keys, and interacting with
Torii. This guide walks through environment setup, configuration, and common
workflows such as signing and querying.

## Installation

```bash
npm install @iroha/iroha-js
# Required once after install so native bindings are built
npm run build:native
```

The build step wraps `cargo build -p iroha_js_host`. Ensure the Rust toolchain
specified in `rust-toolchain.toml` is available in your environment.

## Key Management

```js
import {
  generateKeyPair,
  publicKeyFromPrivate,
  signEd25519,
  verifyEd25519,
} from "@iroha/iroha-js";

const { publicKey, privateKey } = generateKeyPair();

const message = Buffer.from("hello iroha");
const signature = signEd25519(message, privateKey);

console.assert(
  verifyEd25519(message, signature, publicKey),
  "signature must verify",
);

// Derive the same public key from the private key (seed)
const derived = publicKeyFromPrivate(privateKey);
console.assert(
  Buffer.compare(derived, publicKey) === 0,
  "public key mismatch",
);
```

## Building Transactions

Use the instruction builders to compose Norito payloads. The helpers normalise
IDs, quantities, and metadata so the encoded transaction matches Rust parity.

```js
import {
  buildMintAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
} from "@iroha/iroha-js";

const mintInstruction = buildMintAssetInstruction({
  assetId: "norito:4e52543000000001",
  quantity: "10",
});

const transferInstruction = buildTransferAssetInstruction({
  sourceAssetId: "norito:4e52543000000001",
  destinationAccountId: "i105...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "i105...",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "i105...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Norito encode/decode helpers

The Norito helpers mirror the Rust codecs so payloads stay deterministic across SDKs. They also
share fixtures with the Rust goldens to guard against drift.

```js
import {
  noritoEncodeInstruction,
  noritoDecodeInstruction,
  buildRegisterDomainInstruction,
  buildRegisterAccountInstruction,
  buildTransferAssetInstruction,
  buildTransaction,
} from "@iroha/iroha-js";

const registerDomain = noritoEncodeInstruction(
  buildRegisterDomainInstruction({ domainId: "wonderland" }),
);
const registerAccount = buildRegisterAccountInstruction({ accountId: "i105..." });
const transfer = buildTransferAssetInstruction({
  sourceAssetId: "norito:4e52543000000001",
  destinationAccountId: "i105...",
  quantity: "5",
});

const tx = buildTransaction({
  chainId: "demo-chain",
  authority: "i105...",
  instructions: [registerAccount, transfer],
  privateKey: Buffer.alloc(32, 0x42),
});
console.log(noritoDecodeInstruction(registerDomain).Register.Domain.id);
console.log(tx.hash.toString("hex")); // deterministic hash bytes
```

See `javascript/iroha_js/test/instructionBuilders.test.js` and
`javascript/iroha_js/test/transactionFixturesParity.test.js` for the parity fixtures.

## Torii Client Configuration

`ToriiClient` now accepts retry/timeout parameters that mirror `iroha_config`.
The helper `resolveToriiClientConfig` merges defaults with a config object
(normalize `iroha_config` into camelCase first), environment overrides, and
inline options.

```js
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
  overrides: { timeoutMs: 2000, maxRetries: 5 },
});

const torii = new ToriiClient(config?.torii?.address ?? "http://localhost:8080", {
  config,
  timeoutMs: clientConfig.timeoutMs,
  maxRetries: clientConfig.maxRetries,
});
console.log(clientConfig.retryProfiles.pipeline.maxRetries); // 5 by default
```

### NFT and account-asset iterators

Use `requirePermissions` to ensure credentialed access before hitting Torii. NFT filters only allow
`id` equality/exists predicates; account-asset queries support quantity comparisons.

```js
const torii = new ToriiClient("https://torii.example", { authToken: process.env.TORII_AUTH_TOKEN });

const nftPage = await torii.listNfts({
  requirePermissions: true,
  limit: 2,
  sort: [{ key: "id", order: "asc" }],
});
console.log("nfts:", nftPage.items.map((it) => it.id));

for await (const holding of torii.iterateAccountAssetsQuery("i105...", {
  requirePermissions: true,
  pageSize: 2,
  filter: { Gte: ["quantity", 1] },
  sort: [{ key: "quantity", order: "desc" }],
})) {
  console.log(`${holding.asset_id} => ${holding.quantity}`);
}
```

### Auth headers and TLS guardrails

`ToriiClient` and `NoritoRpcClient` refuse to send credentials over insecure `http`/`ws` or to
absolute URLs outside the configured base. Opt into `allowInsecure: true` only for local testing;
the telemetry hook fires whenever the escape hatch is used.

```js
import { NoritoRpcClient } from "@iroha/iroha-js";

const torii = new ToriiClient("http://localhost:8080", {
  authToken: "dev-token",
  allowInsecure: true,
  insecureTransportTelemetryHook: (event) => console.warn("insecure", event),
});
await torii.getStatusSnapshot(); // emits telemetry; errors when allowInsecure is false

const rpc = new NoritoRpcClient("https://torii.example", { apiToken: "abc" });
const payload = new Uint8Array([0x01]);
await rpc.call("/v1/pipeline/submit", payload); // throws on host/protocol mismatch
```

See `javascript/iroha_js/test/transportSecurity.test.js` for the guarded cases.

Environment variables are available for local development:

| Variable | Purpose |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Request timeout (milliseconds) |
| `IROHA_TORII_MAX_RETRIES` | Maximum retry attempts |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Initial backoff delay |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Backoff multiplier |
| `IROHA_TORII_MAX_BACKOFF_MS` | Maximum backoff delay |
| `IROHA_TORII_RETRY_STATUSES` | Comma separated HTTP status codes |
| `IROHA_TORII_RETRY_METHODS` | Comma separated HTTP methods |
| `IROHA_TORII_API_TOKEN` | Adds `X-API-Token` header |
| `IROHA_TORII_AUTH_TOKEN` | Adds `Authorization: Bearer …` header |

The resolver also surfaces retry profiles that mirror the Torii roadmap:

- `pipeline` — used for `/v1/pipeline/transactions` + `/v1/pipeline/transactions/status`. POST
  submissions are safe to retry because the payload hash deduplicates requests, so the profile adds
  `POST` to the allowed methods, lowers the initial backoff to 250 ms, and raises the attempt cap to 5.
  A `404` from the status endpoint is treated as pending, so pollers keep waiting after Torii restarts.
- `streaming` — used for SSE endpoints (`/v1/events/sse`, `/v1/sumeragi/status/sse`,
  `/v1/kaigi/relays/events`). It prefers longer retry windows so event feeds reconnect automatically.

Override a profile by passing `retryProfiles` to `resolveToriiClientConfig` or the `ToriiClient`
constructor:

```js
const clientConfig = resolveToriiClientConfig({
  config: rawConfig,
  overrides: {
    retryProfiles: {
      pipeline: { maxRetries: 8, backoffInitialMs: 150 },
    },
  },
});
const torii = new ToriiClient(baseUrl, {
  config: rawConfig,
  retryProfiles: clientConfig.retryProfiles,
});
```

For the detailed retry/error policy (profile defaults, telemetry hooks, and
incident evidence expectations) see {doc}`torii_retry_policy`.

## Minimal Torii client usage

`ToriiClient` enforces TLS whenever `authToken`/`apiToken` headers are present.
Use `allowInsecure: true` only for localhost development; otherwise HTTP bases
throw a descriptive error before any credentials are sent.

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example", {
  authToken: process.env.TORII_AUTH_TOKEN,
});

try {
  const { items } = await torii.listDomains({ limit: 1 });
  console.log("first domain", items[0]?.id);
} catch (error) {
  console.error("Torii request failed", error);
}
```

When you must target a plain HTTP devnet, pass `allowInsecure: true` explicitly:

```js
const torii = new ToriiClient("http://127.0.0.1:8080", {
  authToken: "local-token",
  allowInsecure: true,
});
```

### Canonical request headers

App-facing JSON endpoints accept optional `X-Iroha-Account` / `X-Iroha-Signature`
headers. Provide `canonicalAuth` to sign requests on the fly:

```js
import { ToriiClient, generateKeyPair } from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { privateKey } = generateKeyPair({ seed: Buffer.alloc(32, 7) });

const { items } = await torii.listAccountAssets("i105...", {
  limit: 10,
  canonicalAuth: { accountId: "i105...", privateKey },
});
```

When constructing ad-hoc HTTP calls, reuse `buildCanonicalRequestHeaders` to
render the two headers from a method/path/query/body tuple.

## Iterable Lists & Pagination

Use the new helpers to mirror the Python SDK’s ergonomics for `/v1/accounts`,
`/v1/domains`, `/v1/assets/definitions`, NFTs, account balances, asset holders,
and account transaction history. Each method accepts the same filter/sort
envelope as the Torii JSON API and returns `{ items, total }`. When you want to
exhaust a dataset, the corresponding `iterate*` method advances the offset
automatically.

```js
const { items, total } = await torii.listDomains({
  limit: 25,
  sort: [{ key: "id", order: "asc" }],
});
console.log(`first page out of ${total}`, items);

for await (const account of torii.iterateAccounts({ pageSize: 50, maxItems: 200 })) {
  console.log(account.id);
}

const defs = await torii.queryAssetDefinitions({
  filter: { Eq: ["metadata.display_name", "Ticket"] },
  sort: [{ key: "metadata.display_name", order: "desc" }],
  fetchSize: 64,
});
console.log("filtered definitions", defs.items);

const perms = await torii.listAccountPermissions("i105...", {
  limit: 10,
});
console.log("direct permissions", perms.items);
for await (const perm of torii.iterateAccountPermissions("i105...", {
  pageSize: 5,
})) {
  console.log("iterated permission", perm.name);
}
const holdings = await torii.listAccountAssets("i105...", {
  limit: 5,
  assetId: "norito:4e52543000000001",
});
console.log("asset holdings", holdings.items);
const holders = await torii.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
  limit: 5,
  assetId: "norito:4e52543000000001",
});
console.log("top holders", holders.items.map((entry) => entry.account_id));
const txs = await torii.listAccountTransactions("i105...", {
  limit: 3,
  assetId: "norito:4e52543000000001",
});
console.log("recent hashes", txs.items.map((tx) => tx.entrypoint_hash));

for await (const nft of torii.iterateNfts({
  pageSize: 10,
  filter: { Eq: ["id.definition_id", "5Pz9SwdN9eXPbiXPX9HRCpzCcE3o"] },
  sort: [{ key: "id", order: "asc" }],
})) {
  console.log("nft:", nft.id);
}

for await (const holding of torii.iterateAccountAssetsQuery("i105...", {
  pageSize: 8,
  filter: { Eq: ["asset_id.definition_id", "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"] },
  select: [{ Fields: ["asset_id", "quantity"] }],
})) {
  console.log("compressed holding", holding.asset_id, "->", holding.quantity);
}
```

### NFT and account-asset iterators

`iterateNfts` and `iterateAccountAssets` wrap the same Norito filter/sort
envelopes as the POST query endpoints while handling pagination for you. Pass
`pageSize`/`maxItems` to bound the iteration. Responses use canonical I105 account identifiers. Torii returns permission errors as
`ToriiHttpError` (status/`code`/`message`); catch them to surface deny reasons
in UI flows.

```js
const assets = [];
for await (const holding of torii.iterateAccountAssets("i105...", {
  pageSize: 2,
  maxItems: 10,
  sort: [{ key: "quantity", order: "desc" }],
})) {
  assets.push({ id: holding.asset_id, qty: holding.quantity });
}
console.log("top holdings", assets);

const nftIds = [];
for await (const nft of torii.iterateNftsQuery({
  pageSize: 3,
  maxItems: 6,
  filter: { Contains: ["id", "ticket#"] },
})) {
  nftIds.push(nft.id);
}
console.log("matching NFTs", nftIds);

try {
  await torii.listNfts({ limit: 1 });
} catch (error) {
  if (error instanceof ToriiHttpError && error.code === "permission_denied") {
    console.warn("missing NFT read permission", error.errorMessage);
  } else {
    throw error;
  }
}

try {
  await torii.listAccountAssets("i105...", { limit: 1 });
} catch (error) {
  if (error instanceof ToriiHttpError && error.code === "permission_denied") {
    console.warn("missing asset read permission", error.errorMessage);
  } else {
    throw error;
  }
}

const ownedNfts = await torii.listAccountNfts("i105...", {
  domainId: "wonderland",
  limit: 5,
});
console.log("alice NFTs", ownedNfts.items.map((entry) => entry.id));

for await (const nft of torii.iterateAccountNfts("i105...", {
  domainId: "wonderland",
  pageSize: 10,
  maxItems: 20,
})) {
  console.log("owned nft", nft.id, nft.metadata);
}
// The runnable recipe `recipes/assets_iterators.mjs` wraps the same helpers with
// env-driven pagination/filters so you can smoke-test permissions against a live Torii.
```

Offline allowance responses expose the enriched ledger metadata up-front —
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex`, and `remaining_amount` sit alongside the embedded
record so dashboards do not have to drill into the raw payloads to understand
refresh cadence or verdict bindings. `integrity_metadata` mirrors the Android
policy slug (`policy`) for quick filtering; when `policy === "provisioned"`
the helper adds the inspector key plus manifest schema/version/TTL/digest under
`integrity_metadata.provisioned` so auditors can confirm which manifest profile
was authorised. The new countdown helpers
(`deadline_kind`, `deadline_state`, `deadline_ms`, `deadline_ms_remaining`)
surface the next expiring deadline (refresh → policy → certificate) so UIs can
render “warning” badges whenever an allowance has <24 h remaining.

Offline transfer entries now expose `platform_policy` plus
`platform_token_snapshot.policy`/`.attestation_jws_b64`, which mirrors the
Android policy label (`marker_key`, `play_integrity`, `hms_safety_detect`,
`provisioned`) and bundles the inspector-issued attestation JWS. Dashboards can
split queues by policy source or export the attestation token directly to
reporting tooling without parsing the raw Norito record. When you need the full
manifest metadata, inspect `integrity_metadata`: the helper mirrors the policy
slug and, for Provisioned allowances, surfaces the inspector key, schema,
version, TTL, and optional digest under `integrity_metadata.provisioned`.

`listOfflineAllowances` accepts the same convenience filters exposed via
`/v1/offline/allowances`: pass `assetId`, `certificateExpiresBeforeMs/AfterMs`,
`policyExpiresBeforeMs/AfterMs`, `verdictIdHex`, `requireVerdict`, or
`onlyMissingVerdict` to avoid building JSON predicates for common reports.

To top up an offline allowance, issue a certificate and register it on-ledger.
`topUpOfflineAllowance` performs both steps in one call and returns the issued
certificate plus registration response. There is no dedicated top-up endpoint;
the helper simply chains the issue + register calls.

```js
const topUp = await torii.topUpOfflineAllowance({
  authority: "i105:...",
  privateKey: "ed25519:...",
  certificate: draft,
});
console.log("certificate id", topUp.registration.certificate_id_hex);
```

For renewals, call `topUpOfflineAllowanceRenewal` with the existing certificate id:

```js
const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "i105:...",
    privateKey: "ed25519:...",
    certificate: draft,
  },
);
console.log("renewed id", renewed.registration.certificate_id_hex);
```

If you already have a signed certificate (for example, issued out-of-band),
call `registerOfflineAllowance` or `renewOfflineAllowance` directly instead of
the top-up helpers.

### Norito encoding and transaction helpers

Use the Norito helpers when you need canonical instruction bytes or to build and
sign transactions without hand-rolled JSON.

```js
import {
  noritoEncodeInstruction,
  noritoDecodeInstruction,
  buildRegisterDomainTransaction,
  submitSignedTransaction,
  hashSignedTransaction,
} from "@iroha/iroha-js";

const instructionJson = { RegisterDomain: { id: "docs", metadata: null } };
const encoded = noritoEncodeInstruction(instructionJson);
const decoded = noritoDecodeInstruction(encoded); // => normalized JSON object

// Build and sign a transaction using the native helper (requires `npm run build:native`)
const { signedTransaction, hash } = buildRegisterDomainTransaction({
  chainId: "sora-mainnet",
  authority: "i105...",
  domainId: "docs",
  privateKey: Buffer.alloc(32, 1), // replace with a real seed
});
console.log("tx hash (bytes)", hash.toString("hex"));

// Submit over Norito RPC (Torii pipeline)
await submitSignedTransaction(torii, signedTransaction, {
  retryProfile: "pipeline",
});

// Derive the hash later if needed
const hashHex = hashSignedTransaction(signedTransaction);
console.log("tx hash (derived)", hashHex);
```

For ad hoc instruction payloads, `noritoEncodeInstruction` also accepts JSON
strings and base64/hex payloads and will normalise account/asset identifiers
before encoding. The native binding falls back to pure JS automatically when
unavailable.
Invalid combinations (for example, `verdictIdHex` + `onlyMissingVerdict`) are
rejected locally before Torii is called.

```js
const { items: allowances } = await torii.listOfflineAllowances({ limit: 10 });
console.log(
  "first certificate",
  allowances[0]?.controller_display,
  allowances[0]?.remaining_amount,
  allowances[0]?.verdict_id_hex,
);
```

## Torii Queries & Streaming

```js
const status = await torii.getSumeragiStatus();
console.log(status?.leader_index);

const metrics = await torii.getMetrics({ asText: true });
console.log(metrics.split("\n").slice(0, 5));

for await (const event of torii.streamEvents({
  filter: { Pipeline: { Block: {} } },
})) {
  console.log(event.id, event.data);
  break;
}
```

## Explorer snapshots & QR payloads

Explorer telemetry surfaces two helper endpoints so SDKs can capture the same replay
snapshots and QR payloads exposed in the portal. `getExplorerMetrics()` fetches
`/v1/explorer/metrics`, normalises the payload, and returns `null` when the route is
disabled or gated. Pair it with `getExplorerAccountQr()` when you need a share-ready
preferred I105 literal (or the I105 literal) plus inline SVG for QR buttons.

```js
import { promises as fs } from "node:fs";

const snapshot = await torii.getExplorerMetrics();
if (!snapshot) {
  console.warn("explorer metrics unavailable");
} else {
  console.log("peers:", snapshot.peers);
  console.log("latest block:", snapshot.blockHeight, snapshot.blockCreatedAt);
  console.log("avg commit ms:", snapshot.averageCommitTimeMs ?? "n/a");
}

const qr = await torii.getExplorerAccountQr("i105...");
console.log("explorer literal", qr.literal);
await fs.promises.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

`getExplorerAccountQr()` returns canonical I105 output. The helper trims invalid
combinations locally and always returns the canonical account identifier,
selected literal, and QR metadata (version, error correction level, module count,
network prefix, and inline SVG) so automation can cache or embed the same payloads the
Explorer renders.

## Connect Sessions & Queueing

The Connect helpers mirror the strawman documented in
`docs/source/connect_architecture_strawman.md`. The easiest way to stand up a
preview-friendly session is to call `bootstrapConnectPreviewSession`, which
wires deterministic SID/URI generation together with the Torii registration API:

```js
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { preview, session, tokens } = await bootstrapConnectPreviewSession(torii, {
  chainId: "sora-mainnet",
  node: "https://torii.nexus.example",
  // optional: override the Torii node used for registration
  sessionOptions: { node: "https://torii.backup.example" },
});

console.log("wallet QR", preview.walletUri);
console.log("Connect tokens", tokens?.wallet, tokens?.app);
```

- `bootstrapConnectPreviewSession` defaults to registering the session with Torii. Pass
  `register: false` when you only need deterministic URIs for QR/deeplink previews.
- Under the hood the helper calls `createConnectSessionPreview` +
  `ToriiClient.createConnectSession`, so you can still orchestrate multi-step flows if you
  need custom retries or storage.
- `generateConnectSid` remains available when you need deterministic session ids without
  also minting URIs (for example when persisting SIDs in a CI harness).
- Directional keys and ciphertext envelopes are produced by the native bridge. When the
  bridge is unavailable the SDK falls back to the JSON codec and throws
  `ConnectError.bridgeUnavailable`.
- Offline buffers are stored as Norito `.to` blobs in IndexedDB. Monitor queue state via
  the emitted `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` exceptions, and hook
  the telemetry feed to emit `connect.queue_depth`, `connect.replay_success_total`, and
  `connect.resume_latency_ms` per the roadmap.

### Connect registry & policy snapshots

Operators can inspect the Connect registry and policy knobs directly through the client.
`iterateConnectApps()` walks the registry with cursor-based pagination, `getConnectStatus()`
provides the fleet-wide aggregate counters, and `getConnectAppPolicy()` / `updateConnectAppPolicy()`
surface the mutable policy envelope so SDKs stay in lockstep with relay/session constraints.

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");

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

`updateConnectAppPolicy()` accepts camelCase fields, so dashboards and automation can reuse the
same snippet when staging approvals. Always check the current `policy` snapshot returned from
`getConnectStatus()` before applying updates; governance reviews expect evidence that the queued
change starts from the fleet’s latest limits.

### Connect WebSocket dialling

`ToriiClient.openConnectWebSocket()` builds the canonical `/v1/connect/ws` URL (converting
`http→ws` / `https→wss`, adding `sid`, `role`, and `token`) and hands it to whichever
WebSocket implementation you provide. Browsers automatically reuse the global `WebSocket`;
Node.js callers should pass a constructor such as `ws`:

```js
import WebSocket from "ws";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.IROHA_TORII_URL);
const session = await torii.createConnectSession({ sid: preview.sidBase64Url });

const socket = torii.openConnectWebSocket({
  sid: session.sid,
  role: "wallet",
  token: session.token_wallet,
  WebSocketImpl: WebSocket,
  protocols: ["iroha-connect"],
});
```

Most wallets set `binaryType = "arraybuffer"` to distinguish encrypted payloads from control
events and fan out the frames to a shared decoder:

```js
socket.binaryType = "arraybuffer";

socket.addEventListener("message", (event) => {
  if (typeof event.data === "string") {
    const control = JSON.parse(event.data);
    console.log("[ws] control", control.kind, control.sid);
    return;
  }

  const ciphertext = new Uint8Array(event.data);
  pendingFrames.enqueue(ciphertext);
});
```

When you only need the URL, call `ToriiClient.buildConnectWebSocketUrl()` or the top-level
`buildConnectWebSocketUrl(baseUrl, params)` helper and reuse the resulting string inside a
custom transport/queue. Tokens are sent via `Authorization` headers by default; set
`allowInsecure: true` only when dialing `http://` Torii bases in dev environments.

For a turnkey CLI sample (including queue telemetry hooks), see
`docs/portal/docs/sdks/recipes/javascript-connect-preview.md`, which now hosts the script called
out in the roadmap for documenting the Connect preview + WebSocket walkthrough.

### Queue telemetry & alerting

Tie queue metrics directly to the helpers exposed in this section so the roadmap-mandated
telemetry stays deterministic.

```js
import {
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

async function dialWithTelemetry(client) {
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

`ConnectQueueError#toConnectError()` converts queue failures into the general `ConnectError`
taxonomy when you need to forward them to shared HTTP/WebSocket interceptors, and emitting the
metrics above satisfies the `connect.queue_depth`, `connect.queue_overflow_total`, and
`connect.queue_expired_total` dashboards referenced throughout the JS roadmap. For offline queue
snapshot/evidence helpers, see {doc}`offline`.

## Streaming watchers & event cursors

`ToriiClient.streamEvents()` exposes `/v1/events/sse` as an async iterator with automatic
retries, so Node/Bun CLIs can tail pipeline activity the same way the Rust CLI does.
Persist the `Last-Event-ID` cursor alongside your runbook artefacts so operators can
resume a stream without skipping events when a process restarts.

```js
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

## Governance & ISO Bridge

`ToriiClient` exposes the governance surfaces needed to inspect contract
instances, draft deployment proposals, submit ballots (plain or ZK), rotate the
council, and call `governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped` without rolling your own DTOs. The ISO bridge uses the same
patterns: build XML with `buildPacs008Message`/`buildPacs009Message`, submit it via
`submitIsoPacs008AndWait` or `submitIsoPacs009AndWait`, or call
`waitForIsoMessageStatus` when you already have a message identifier. A dedicated guide
with runnable snippets lives in {doc}`sdk/js/governance_iso_examples`, the README +
recipes (`recipes/governance.mjs`, `recipes/iso_bridge.mjs`) provide copy-paste scripts
for CI jobs, and the portal recipe (`docs/portal/docs/sdks/recipes/javascript-governance-iso.md`)
packages both flows with downloadable samples to satisfy the JS5 documentation requirement.
The guide now documents the exact environment variables and command
invocations (`GOV_SUBMIT`, `GOV_FETCH`, `ISO_MESSAGE_KIND`, `ISO_MESSAGE_ID`, polling
interval overrides, and the required Torii/AUTHORITY/PRIVATE_KEY_HEX triplet) so the same
scripts can be dropped into staging rehearals or release workflows without bespoke glue.

### Contract deployment recipe (JS-06)

JS-06 also requires a deterministic way to publish bytecode/manifest payloads without
rewriting curl invocations. The new `javascript/iroha_js/recipes/contracts.mjs` helper
reads bytecode from `CONTRACT_CODE_PATH`, optional manifest JSON from either
`CONTRACT_MANIFEST_PATH` or `CONTRACT_MANIFEST_JSON`, and runs `/v1/contracts/deploy`
followed by `/v1/contracts/instance` when `CONTRACT_STAGE` includes `instance` (default:
`both`). Supply the credentials and namespace context via:

```
TORII_URL=https://torii.devnet.example \
AUTHORITY=i105... \
PRIVATE_KEY_HEX=$(cat ~/.iroha/keys/alice.hex) \
CONTRACT_CODE_PATH=./artifacts/demo_contract.to \
CONTRACT_MANIFEST_PATH=./artifacts/demo_manifest.json \
CONTRACT_NAMESPACE=apps \
CONTRACT_ID=demo.contract \
node javascript/iroha_js/recipes/contracts.mjs
```

Set `CONTRACT_STAGE=register` to upload the manifest/bytecode without activating a
namespace binding (useful when governance controls activation), or `CONTRACT_STAGE=instance`
when bytecode already lives on-chain and you only need to bind an `ActivateContractInstance`.
`TORII_AUTH_TOKEN`/`TORII_API_TOKEN` propagate directly to `ToriiClient`, and private keys are
validated whether you pass `PRIVATE_KEY=ed25519:<hex>` or a raw `PRIVATE_KEY_HEX` string, so
the script can run inside CI without bespoke wrappers.

## RBC sampling & delivery evidence

Roadmap item **JS-08** also requires Roadrunner Block Commitment (RBC) sampling proof so
operators can show that Sumeragi delivered the block whose chunks they fetched. The JS SDK
exposes the same helpers as the Rust CLI:

1. `getSumeragiRbcSessions()` mirrors `/v1/sumeragi/rbc/sessions`, while
   `findRbcSamplingCandidate()` auto-selects the first delivered session with a block hash
   (used by the integration suite when `IROHA_TORII_INTEGRATION_RBC_SAMPLE` is unset).
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` guards the `{blockHash,height,view}`
   triple and optional `{count,seed,apiToken}` overrides before Torii receives the payload.
3. `sampleRbcChunks()` POSTs the request to `/v1/sumeragi/rbc/sample` and returns chunk proofs
   (`samples[].chunkHex`, Merkle paths, `chunkRoot`, `payloadHash`) that you should persist
   alongside the height/view identifiers.
4. `getSumeragiRbcDelivered(height, view)` captures the delivery metadata so the sampling log
   includes both the proofs and the cohort that accepted them.

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
sample.samples.forEach((chunk, index) => {
  assert.ok(Buffer.from(chunk.chunkHex, "hex").length > 0, `chunk ${index} must be hex`);
  assert.ok(
    chunk.proof.auditPath.every((entry) => entry === null || /^[0-9a-f]+$/i.test(entry)),
    `chunk ${index} proof must contain hex audit path entries`,
  );
});

const delivery = await torii.getSumeragiRbcDelivered(sample.height, sample.view);
console.log(
  `rbc height=${sample.height} view=${sample.view} chunks=${sample.samples.length} delivered=${delivery?.delivered}`,
);
```

- Persist the JSON emitted by `sampleRbcChunks()` and `getSumeragiRbcDelivered()` with the
  same artefacts you submit to governance so auditors can replay the proofs.
- Override the auto-selected session via `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'`
  (or reuse `IROHA_TORII_INTEGRATION_RBC_SAMPLE`) whenever you want to probe a specific block.
- `findRbcSamplingCandidate()` accepts an `AbortSignal`; treat failures to fetch RBC snapshots
  as a pre-flight gating error and fall back to direct-mode captures only when governance
  approves the downgrade.

## Testing & CI

See `docs/source/examples/iroha_js_ci.md` for a complete GitHub Actions
workflow. The key steps are:

1. Cache cargo and npm artifacts.
2. Run `npm run build:native`.
3. Execute `npm test` (or `node --test` for smoke jobs).

## Next Steps

- Review the API documentation in `javascript/iroha_js/index.d.ts` for the full
  surface area.
- Explore the recipes directory (`javascript/iroha_js/recipes/`) for additional
  batching examples.
- Integrate `resolveToriiClientConfig` in production environments to keep Node
  clients aligned with operational policy.

## Additional Resources

- Sample config-aware client script: `javascript/iroha_js/recipes/configured-client.mjs`.
