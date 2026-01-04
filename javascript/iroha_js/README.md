# Iroha JS SDK (Preview)

`@iroha/iroha-js` is an experimental JavaScript/TypeScript SDK for interacting
with Hyperledger Iroha nodes from Node.js runtimes. The initial focus mirrors
the Python helper coverage so developers can manage attachments, prover
reports, Ed25519 signing, and Norito payloads while we thread manifest builders
and gRPC transports into future milestones.

TypeScript consumers can import the bundled `index.d.ts` definitions for the
SDK surface.

Run the native build (wrapping `cargo build -p iroha_js_host`) before
importing:

```bash
npm install
npm run build:native
```

When publishing or testing the packaged layout, build the ESM dist tree:

```bash
npm run build:dist
```

Native bindings load only after verifying the platform-specific SHA-256 recorded
in `native/iroha_js_host.checksums.json`. When the checksum is missing or
mismatched, the SDK logs the failure and falls back to the pure-JS
implementations; it no longer tries to auto-build the native module at runtime.
Run `npm run build:native` explicitly after installing the Rust toolchain to
enable the native path. Set `IROHA_JS_NATIVE_DIR` to point at an alternate
`native/` folder for test harness overrides; pass `IROHA_JS_NATIVE_WARN=0` to
silence verification warnings in noisy CI logs. For local builds where the
checksum manifest lags the compiled artifact, `IROHA_JS_ALLOW_UNVERIFIED_NATIVE=1`
lets the loader accept the current hash while keeping verification enabled
elsewhere.

When you only need the pure JS layer (for example, on machines where the native
build is unavailable), run tests with `npm run test:js`, which sets
`IROHA_JS_DISABLE_NATIVE=1`. The default `npm test` still exercises the native
binding when present.

In JS-only mode `noritoEncodeInstruction`/`noritoDecodeInstruction` operate on
canonicalised JSON strings (UTF-8 bytes) instead of the binary Norito codec so
tooling can keep running without the Rust bridge. For canonical Norito payloads
build the native module first.

> **ESM-only:** The package ships as pure ESM. Use dynamic `import()` from
> CommonJS (`const { ToriiClient } = await import("@iroha/iroha-js/torii");`)
> when migrating existing CJS callers.

Common subpath imports for lighter bundling (ESM):

```js
import { ToriiClient } from "@iroha/iroha-js/torii";
import { noritoEncodeInstruction } from "@iroha/iroha-js/norito";
import { generateKeyPair } from "@iroha/iroha-js/crypto";
import { buildOfflineEnvelope } from "@iroha/iroha-js/offline";
```

You can also use namespaced exports when you prefer grouped imports:

```js
import { Torii, Norito, Crypto, Offline } from "@iroha/iroha-js";

const torii = new Torii.ToriiClient("https://torii.example");
const encoded = Norito.noritoEncodeInstruction({ Register: { Domain: { id: "wonderland" } } });
const keys = Crypto.generateKeyPair();
const offline = Offline.buildOfflineEnvelope({ txBytes: encoded, metadata: {} });
```

> **Key storage:** Store Ed25519 seed material in dedicated key vaults or
> platform keystores whenever possible. The helpers shown below accept raw
> buffers for developer convenience, but production code should hydrate keys
> from secure storage and avoid logging them. Use
> `deriveConfidentialKeyset()` when building confidential workflows so all
> derived keys share the same handling guarantees.

```js
import { AccountAddress } from "@iroha/iroha-js";

const address = AccountAddress.fromAccount({
  domain: "default",
  publicKey: new Uint8Array(32),
});
console.log(address.canonicalHex());
console.log(address.toIH58(753));
console.log(address.toCompressedSora());

const formats = address.displayFormats(753);
console.log(formats.ih58);
console.log(formats.compressed);
console.log(formats.compressedWarning);
console.log(formats.domainSummary.selector.label); // "default"
console.log(formats.domainSummary.selector.registryId); // registry id when Global selectors are used
```

> ℹ️ When showing addresses in wallets, explorers, or SDK samples, follow the
> dual-format UX checklist captured in
> [`docs/source/sns/address_display_guidelines.md`](../../docs/source/sns/address_display_guidelines.md):
> IH58 remains the default copy/share target, compressed strings need an inline
> warning, and QR codes should always encode the IH58 value.

## Multisig TTL preview and enforcement

```js
import { MultisigSpecBuilder, buildProposeMultisigInstruction } from "@iroha/iroha-js";

const spec = new MultisigSpecBuilder()
  .setQuorum(3)
  .setTransactionTtlMs(86_400_000)
  .addSignatory("alice@wonderland", 2)
  .addSignatory("bob@wonderland", 1)
  .build();

// Preview the effective TTL (clamped to the policy cap) and expiry time
const preview = spec.enforceProposalTtl({ requestedTtlMs: 90_000, nowMs: Date.now() });
console.log(preview.effectiveTtlMs, preview.expiresAtMs, preview.wasCapped);

// Build a multisig proposal while enforcing the policy TTL cap client-side
const propose = buildProposeMultisigInstruction({
  accountId: "controller@wonderland",
  spec,
  instructions: [{ Log: { Level: "INFO", message: "hello" } }],
  transactionTtlMs: 45_000, // throws if above spec.transaction_ttl_ms
});

// Register the multisig controller with an explicit (non-derived) account id
const register = buildRegisterMultisigTransaction({
  chainId: "wonderland",
  authority: "alice@wonderland",
  accountId: "controller@wonderland",
  spec,
  privateKey: generateKeyPair().privateKey, // controller key is NOT used for signing
});
```

`enforceProposalTtl` rejects TTL overrides above the registered policy
(`transaction_ttl_ms`) before submitting a multisig proposal so client UX can
surface the same error Torii would return. Use `previewProposalExpiry` when you
only need a non-throwing preview for relayer TTL hints.

> Multisig controllers must never use derived keys. Supply an explicit account id in the
> signatory domain (random keys are fine; private halves should be discarded). Nodes will reject
> derived multisig ids at admission.

```js
import {
  ToriiClient,
  NoritoRpcClient,
  generateKeyPair,
  signEd25519,
  verifyEd25519,
  deriveConfidentialKeyset,
  noritoEncodeInstruction,
  noritoDecodeInstruction,
  buildRegisterDomainTransaction,
  buildTransaction,
  buildMintAssetInstruction,
  buildMintAssetTransaction,
  buildBurnAssetTransaction,
  buildBurnTriggerTransaction,
  buildMintAndTransferTransaction,
  buildRegisterDomainAndMintTransaction,
  buildRegisterAccountAndTransferTransaction,
  buildRegisterAssetDefinitionAndMintTransaction,
  buildRegisterDomainInstruction,
  buildRegisterAccountInstruction,
  buildTransferAssetInstruction,
  buildTransferAssetTransaction,
  submitSignedTransaction,
  normalizeAccountId,
  normalizeAssetId,
} from "@iroha/iroha-js";

const { publicKey, privateKey } = generateKeyPair();
const authorityInput =
  "ed0120ce7fa46c9dce7ea4b125e2e36bdb63ea33073e7590ac92816ae1e861b7048b03@wonderland";
const newAccountIdInput =
  "ed0120bc35289d74af3796470409268afd7adda7bb2a4627b10c24be17864d1116da31@wonderland";
const authority = normalizeAccountId(authorityInput);
const newAccountId = normalizeAccountId(newAccountIdInput);
const roseAssetId = normalizeAssetId(`rose##${authorityInput}`);
// Normalise human-supplied identifiers once and reuse the canonical forms below.
const message = Buffer.from("test");
const signature = signEd25519(message, privateKey);
console.log(verifyEd25519(message, signature, publicKey)); // true

const confidential = deriveConfidentialKeyset(Buffer.alloc(32, 0x42));
console.log(confidential.nkHex); // cb7149cc...

const torii = new ToriiClient("http://localhost:8080");
const meta = await torii.uploadAttachment(Buffer.from("{}"), {
  contentType: "application/json",
});
console.log(meta.id);
console.log(meta.contentType, meta.size, meta.createdMs);
// Attachment helpers validate both the payload type and contentType locally so
// malformed inputs fail fast before hitting Torii.

When you pass `authToken` or `apiToken` credentials, prefer an `https://` Torii base URL; the
client will reject insecure schemes unless you opt into `allowInsecure: true` for local/dev use.

const reportsResult = await torii.listProverReports({
  failedOnly: true,
  hasTag: "PROF",
  limit: 5,
});
if (reportsResult.kind === "reports") {
  for (const report of reportsResult.reports) {
    console.log(report.id, report.error, report.latency_ms);
  }
} else if (reportsResult.kind === "ids") {
  console.log("report ids:", reportsResult.ids);
} else {
  // messages_only projection
  console.log("failed messages:", reportsResult.messages);
}
// listProverReports/countProverReports accept ToriiProverReportFilters to keep the
// available query flags (failedOnly, hasTag, sinceMs, order, etc.) fully typed.
// Pass an AbortSignal as the second argument to the prover helpers to cancel
// long-running queries before Torii responds.
for await (const report of torii.iterateProverReports({ failedOnly: true }, { pageSize: 2 })) {
  // If idsOnly/messagesOnly are provided, the iterator yields strings or message summaries.
  console.log(report);
}

const instruction = buildRegisterDomainInstruction({
  domainId: "wonderland",
  metadata: { key: "value" },
});
const encoded = noritoEncodeInstruction(instruction);
const decoded = noritoDecodeInstruction(encoded);
console.log(decoded.Register.Domain.id); // "wonderland"
// Note: `noritoDecodeInstruction` throws when the payload cannot be decoded
// (for example, current builds reject Kaigi relay manifests until the runtime
// canonicalises them), so wrap it in a try/catch in production code.

const registerAccountInstruction = buildRegisterAccountInstruction({
  accountId: newAccountId,
  metadata: { nickname: "alice" },
});
console.log(noritoDecodeInstruction(registerAccountInstruction).Register.Account.id);

await torii.submitTransaction(encoded);
const sampleHashHex = "ab".repeat(32); // 32-byte transaction hash as lowercase hex
const status = await torii.getTransactionStatus(sampleHashHex);
console.log(status?.content.status.kind); // e.g. "Committed"

// Normalised helper exposes canonical fields (`kind`, `hashHex`, `status.kind`, etc.)
const typedStatus = await torii.getTransactionStatusTyped(sampleHashHex);
console.log(typedStatus?.status?.kind); // e.g. "Committed"

// The wait helpers also ship normalised variants if you prefer structured DTOs
await torii.waitForTransactionStatusTyped(sampleHashHex, { intervalMs: 500 });
await torii.submitTransactionAndWaitTyped(encoded, { hashHex: sampleHashHex });
// Note: the polling helpers require `options` to be a plain object. intervalMs/timeoutMs
// must be non-negative (use timeoutMs: null to disable the deadline), maxAttempts must
// be a positive integer when provided, and onStatus must be a function.

// Submit while re-signing with a fresh private key (mutating buffer supported)
await submitSignedTransaction(torii, encoded, { privateKey });

// Inspect the deterministic pipeline recovery sidecar for a given block height.
const recovery = await torii.getPipelineRecoveryTyped(42);
if (recovery) {
  console.log(
    `dag fingerprint: ${recovery.dag.fingerprintHex}, tx count=${recovery.txs.length}`,
  );
}

### Iterating NFTs and account assets

The iterable helpers accept `requirePermissions` to fail fast when credentials are missing. NFT
filters only admit `id` equality/exists checks, while account-asset queries allow quantity
comparisons.

```js
const torii = new ToriiClient("https://torii.example", {
  authToken: process.env.TORII_AUTH_TOKEN,
});

const nftPage = await torii.listNfts({
  requirePermissions: true,
  limit: 3,
  sort: [{ key: "id", order: "asc" }],
});
console.log("first nft page:", nftPage.items.map((it) => it.id));

for await (const holding of torii.iterateAccountAssetsQuery("alice@wonderland", {
  requirePermissions: true,
  pageSize: 2,
  filter: { Gte: ["quantity", 1] },
  sort: [{ key: "quantity", order: "desc" }],
  addressFormat: "ih58",
})) {
  console.log(`${holding.asset_id} => ${holding.quantity}`);
}
```

See `recipes/assets_iterators.mjs` and `recipes/nft_account_iteration.mjs` for runnable examples.

### Auth headers and TLS guardrails

`ToriiClient` and `NoritoRpcClient` reject sending `Authorization`/`X-API-Token`
credentials over insecure `http`/`ws` or to mismatched hosts. Opt into
`allowInsecure: true` only for local testing; both clients emit an
`insecureTransportTelemetryHook` event when the escape hatch is used so audits
can flag leaked tokens.

```js
const torii = new ToriiClient("http://localhost:8080", {
  authToken: "dev-token",
  allowInsecure: true,
  insecureTransportTelemetryHook: (event) => console.warn("insecure", event),
});
await torii.getStatusSnapshot(); // emits telemetry; throws if allowInsecure is false

const rpc = new NoritoRpcClient("https://torii.example", { apiToken: "abc" });
const payload = new Uint8Array([0x01]);
await rpc.call("/v1/pipeline/submit", payload); // throws on host/protocol mismatch
```

Error strings stay stable (`ToriiClient: refusing to send credentials over insecure protocol …`)
and the regression suite in `javascript/iroha_js/test/transportSecurity.test.js` covers the
allowed permutations so dApps can mirror the same checks.

### Offline envelopes and Connect queue evidence

Package signed transactions for replay with the deterministic envelope helpers and mirror the
Connect offline queue diagnostics shipped on Swift/Android:

```js
import {
  ToriiClient,
  buildOfflineEnvelope,
  writeOfflineEnvelopeFile,
  readOfflineEnvelopeFile,
  replayOfflineEnvelope,
  ConnectQueueJournal,
  ConnectDirection,
  exportConnectQueueEvidence,
} from "@iroha/iroha-js";

const signedTransaction = Buffer.from("norito-payload"); // replace with a real signed transaction
const envelope = buildOfflineEnvelope({
  signedTransaction, // Norito bytes (Buffer/Uint8Array)
  keyAlias: "alice-key",
  metadata: { purpose: "offline-demo" },
});
await writeOfflineEnvelopeFile("./artifacts/js/offline/envelope.json", envelope);
const stored = await readOfflineEnvelopeFile("./artifacts/js/offline/envelope.json");
await replayOfflineEnvelope(new ToriiClient("https://torii.example"), stored);

const journal = new ConnectQueueJournal("AQIDBAUG", { storage: "memory" });
await journal.append(ConnectDirection.APP_TO_WALLET, 1n, Buffer.from("frame"), {
  receivedAtMs: Date.now(),
});
const { manifest } = await exportConnectQueueEvidence(
  "AQIDBAUG",
  "./artifacts/js/connect_bundle",
);
console.log(manifest.snapshot.state); // "enabled"
```

Recipes mirror the same flows with deterministic fixtures: `npm run example:offline` (Connect queues)
and `npm run example:offline:pipeline` (pipeline envelope + mock Torii). See
`docs/source/sdk/js/offline.md` and the tests in `javascript/iroha_js/test/offline*.test.js` for the
expected schema and layout.

### Norito helpers and fixtures

The Norito encode/decode helpers mirror the Rust codecs. Instruction builders cover domain/account
registration and asset transfers; fixtures in `javascript/iroha_js/test/instructionBuilders.test.js`
and `javascript/iroha_js/test/transactionFixturesParity.test.js` keep the payloads aligned with the
Rust goldens.

```js
const registerDomain = noritoEncodeInstruction(
  buildRegisterDomainInstruction({ domainId: "wonderland" }),
);
const registerAccount = buildRegisterAccountInstruction({
  accountId: "alice@wonderland",
});
const transfer = buildTransferAssetInstruction({
  sourceAssetId: "rose#wonderland#alice",
  destinationAccountId: "bob@wonderland",
  quantity: "5",
});

const transferTx = buildTransaction({
  chainId: "demo-chain",
  authority: "alice@wonderland",
  instructions: [transfer],
  privateKey,
});
console.log(noritoDecodeInstruction(registerDomain).Register.Domain.id);
console.log(transferTx.signedTransaction.length); // deterministic Norito bytes
```

### Validation errors

Input guards exposed by helpers such as `normalizeAccountId()`, `normalizeAssetId()`,
and the instruction builders now throw `ValidationError` instances. They extend
`TypeError` while providing a deterministic `code` and `path` that automation can
key on.

```js
import {
  ValidationError,
  ValidationErrorCode,
  normalizeAccountId,
} from "@iroha/iroha-js";

try {
  normalizeAccountId("invalid-account");
} catch (error) {
  if (error instanceof ValidationError) {
    console.log(error.code); // e.g. ERR_INVALID_ACCOUNT_ID
    console.log(error.path); // supplied parameter name
  }
  throw error;
}
```

The `ValidationErrorCode` enum covers common categories (`ERR_INVALID_STRING`,
`ERR_INVALID_ACCOUNT_ID`, `ERR_INVALID_NUMERIC`, etc.) so dashboards and CI checks
can count violations without parsing human-readable messages.

### Bundle size reports

Roadmap JS-04 tracks bundle-size impact whenever validation helpers evolve.
Run the reporting helper to generate a JSON summary backed by an actual
`npm pack` tarball:

```bash
npm run report:bundle-size
```

The script writes the report to
`artifacts/js-sdk-bundle-size/bundle-size-<timestamp>.json` by default and
prints the top contributors to stdout:

```
[bundle-size] @iroha/iroha-js@0.0.2
  files: 46 (total 1 MB)
  tarball: 229 KB (b4ee…)
  top files:
     1. src/toriiClient.js — 494 KB (41.5% of total)
     2. src/instructionBuilders.js — 60 KB (5.0% of total)
```

Pass `-- --out /tmp/report.json` to control the output path or
`-- --keep-tarball` to retain the generated `.tgz` for manual inspection. The
JSON artifact stores the same metadata used in release reviews, so attaching it
to roadmap evidence or a PR comment satisfies the “bundle-size impact report”
gate without requiring a full publish.

// Build a fresh RegisterDomain transaction using the native builder helper
const built = buildRegisterDomainTransaction({
  chainId: "test-chain",
  authority,
  domainId: "wonderland",
  metadata: { key: "value" },
  creationTimeMs: Date.now(),
  ttlMs: 60_000,
  nonce: 1,
  privateKey,
});
console.log(Buffer.from(built.hash).toString("hex"));

const mint = buildMintAssetInstruction({
  assetId: roseAssetId,
  quantity: "10",
});
const transfer = buildTransferAssetInstruction({
  sourceAssetId: roseAssetId,
  quantity: "5",
  destinationAccountId: authority,
});
console.log(noritoDecodeInstruction(mint)); // structured JSON

const mintTx = buildMintAssetTransaction({
  chainId: "test-chain",
  authority,
  assetId: roseAssetId,
  quantity: "10",
  privateKey,
});

const burnTx = buildBurnAssetTransaction({
  chainId: "test-chain",
  authority,
  assetId: roseAssetId,
  quantity: "2",
  privateKey,
});

const transferTx = buildTransferAssetTransaction({
  chainId: "test-chain",
  authority,
  sourceAssetId: roseAssetId,
  quantity: "5",
  destinationAccountId: authority,
  privateKey,
});

const mintAndTransferTx = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority,
  mint: { assetId: roseAssetId, quantity: "10" },
  transfers: [
    {
      quantity: "6",
      destinationAccountId: authority,
    },
    {
      sourceAssetId: roseAssetId,
      quantity: "1",
      destinationAccountId: authority,
    },
  ],
  privateKey,
});

const domainAndMintTx = buildRegisterDomainAndMintTransaction({
  chainId: "test-chain",
  authority,
  domain: { domainId: "garden_of_live_flowers", metadata: { key: "value" } },
  mints: [
    { assetId: roseAssetId, quantity: "5" },
    { assetId: normalizeAssetId(`lily##${authorityInput}`), quantity: "2" },
  ],
  privateKey,
});

const accountAndTransferTx = buildRegisterAccountAndTransferTransaction({
  chainId: "test-chain",
  authority,
  account: { accountId: newAccountId, metadata: { nickname: "alice" } },
  transfers: [
    {
      sourceAssetId: roseAssetId,
      quantity: "2",
      destinationAccountId: newAccountId,
    },
    {
      sourceAssetId: roseAssetId,
      quantity: "1",
      destinationAccountId: authority,
    },
  ],
  privateKey,
});

const assetDefinitionAndMintTx = buildRegisterAssetDefinitionAndMintTransaction({
  chainId: "test-chain",
  authority,
  assetDefinition: {
    assetDefinitionId: "rose#wonderland",
    metadata: { description: "Rose asset" },
    mintable: "Not",
    spec: { scale: 4 },
    confidentialPolicy: {
      vk_set_hash: "deadbeef",
      pending_transition: { stage: "Queued" },
    },
  },
  mints: [
    {
      accountId: newAccountId,
      quantity: "3",
    },
    {
      assetId: `rose#wonderland#${authority}`,
      quantity: "1",
    },
  ],
  privateKey,
});

const assetDefinitionMintAndTransferTx = buildRegisterAssetDefinitionMintAndTransferTransaction({
  chainId: "test-chain",
  authority,
  assetDefinition: {
    assetDefinitionId: "lily#wonderland",
    metadata: { description: "Lily asset" },
  },
  mints: [
    {
      accountId: newAccountId,
      quantity: "8",
    },
    {
      assetId: `lily#wonderland#${authority}`,
      quantity: "5",
    },
  ],
  transfers: [
    {
      quantity: "3",
      destinationAccountId: authority,
    },
    {
      sourceAssetId: `lily#wonderland#${authority}`,
      quantity: "3",
      destinationAccountId: newAccountId,
    },
  ],
  privateKey,
});

// Build an arbitrary transaction from instruction payloads (JSON strings or objects)
const genericTx = buildTransaction({
  chainId: "test-chain",
  authority,
  instructions: [mint, transfer],
  privateKey,
});

const kaigiCreateTx = buildCreateKaigiTransaction({
  chainId: "test-chain",
  authority,
  call: {
    id: { domainId: "wonderland", callName: "weekly-sync" },
    host: authority,
    gasRatePerMinute: 120,
    metadata: { topic: "roadmap" },
    relayManifest: {
      expiryMs: 1700111000000,
      hops: [
        {
          relayId: authority,
          hpkePublicKey: Buffer.from([1, 2, 3, 4]),
          weight: 3,
        },
      ],
    },
  },
  privateKey,
});

const kaigiJoinTx = buildJoinKaigiTransaction({
  chainId: "test-chain",
  authority,
  join: {
    callId: "wonderland:weekly-sync",
    participant: authority,
    commitment: {
      commitment: Buffer.alloc(32, 0x11),
      aliasTag: "host",
    },
    nullifier: {
      digest: Buffer.alloc(32, 0x22),
      issuedAtMs: 42,
    },
  },
  privateKey,
});

console.log(Buffer.from(mintTx.hash).toString("hex"));
console.log(Buffer.from(burnTx.hash).toString("hex"));
console.log(Buffer.from(transferTx.hash).toString("hex"));
console.log(Buffer.from(mintAndTransferTx.hash).toString("hex"));
console.log(Buffer.from(assetDefinitionAndMintTx.hash).toString("hex"));
console.log(Buffer.from(assetDefinitionMintAndTransferTx.hash).toString("hex"));
console.log(Buffer.from(genericTx.hash).toString("hex"));
console.log(Buffer.from(kaigiCreateTx.hash).toString("hex"));
console.log(Buffer.from(kaigiJoinTx.hash).toString("hex"));

// NOTE: Instruction coverage currently includes `Register` (domain/account/asset
// definition), `Mint::Asset`, `Mint::TriggerRepetitions`, `Transfer`
// variants for assets, asset definitions, domains, NFTs, and the Kaigi instruction
// family (create/join/leave/end/usage/relay). See the roadmap for upcoming extensions.
```

## Norito RPC client

The [`NoritoRpcClient`](./src/noritoRpcClient.js) mirrors the Python helper so
you can talk to the binary Norito-RPC surface without sprinkling manual fetch
calls throughout your code. It automatically sets the required
`Content-Type: application/x-norito` header, defaults `Accept` to the same
media type, and lets you provide shared headers (authorization tokens, custom
trace identifiers, etc.) when the client is constructed.

```js
import {
  NoritoRpcClient,
  noritoEncodeInstruction,
  buildRegisterDomainInstruction,
} from "@iroha/iroha-js";

const rpc = new NoritoRpcClient("https://localhost:8080", {
  defaultHeaders: { Authorization: `Bearer ${process.env.API_TOKEN ?? ""}` },
  timeoutMs: 5_000,
  // For http:// endpoints during local development, pass allowInsecure: true and attach
  // insecureTransportTelemetryHook to log the downgraded transport.
});

const payload = noritoEncodeInstruction(
  buildRegisterDomainInstruction({
    domainId: "wonderland",
    metadata: {},
  }),
);

// Returns the raw Norito bytes Torii responds with (Uint8Array).
const responseBytes = await rpc.call("/v1/pipeline/submit", payload);

// Override media type and append query parameters when needed.
await rpc.call("/v1/pipeline/status", payload, {
  params: { hash: "deadbeef" },
  accept: "application/json",
});
```

Use the exported `NoritoRpcError` to detect non-success responses:

```js
import { NoritoRpcClient, NoritoRpcError } from "@iroha/iroha-js";

try {
  await rpc.call("/v1/pipeline/submit", payload);
} catch (error) {
  if (error instanceof NoritoRpcError) {
    console.error(`status ${error.status}: ${error.body}`);
  }
  throw error;
}
```

Pass a custom `fetchImpl`, per-request headers, alternate HTTP methods, or an
AbortSignal when integrating with higher-level transports. The helper returns
`Uint8Array` so you can feed the response straight into the Norito decode
utilities or persist it for parity fixtures.

### Transport security and host validation

`ToriiClient` and `NoritoRpcClient` keep secrets bound to the client's base URL. When
`authToken`/`apiToken`/`Authorization` headers are present the request scheme and host must
match the client's base; absolute URL overrides are rejected, and insecure `http`/`ws` is allowed
only when you opt into `allowInsecure: true` (intended for local development). Cross-host calls
without credentials require an explicit `allowAbsoluteUrl: true` on the per-request options.
Attach `insecureTransportTelemetryHook` to record/alert whenever an insecure transport is used:

```js
const logInsecure = (event) => console.warn("[insecure-transport]", event);

const torii = new ToriiClient("http://127.0.0.1:8080", {
  authToken: process.env.IROHA_API_TOKEN ?? "",
  allowInsecure: true, // dev/local only
  insecureTransportTelemetryHook: logInsecure,
});

const rpc = new NoritoRpcClient("http://127.0.0.1:8080", {
  authToken: process.env.IROHA_API_TOKEN ?? "",
  allowInsecure: true, // dev/local only
  insecureTransportTelemetryHook: logInsecure,
});
```

For mock/testing targets without credentials, pass `allowAbsoluteUrl: true` to
`NoritoRpcClient.call` to intentionally reach a different host while keeping credentialled traffic
pinned to the configured base.

## Batching Best Practices

- Treat the instruction array passed to `buildTransaction` as authoritative: the
  order you supply becomes the exact Norito execution order on-chain. Keep
  dependent steps adjacent (for example, mint before transfer, transfer before
  burn) so later instructions can safely reference state written by earlier ones.
- Prefer the convenience helpers (`buildMintAndTransferTransaction`,
  `buildRegisterAssetDefinitionMintAndTransferTransaction`, etc.) when they fit
  your use case. They validate numeric quantities, asset IDs, and mutually
  exclusive options (`transfer` vs `transfers`) before serialisation.
- When assembling instructions manually, re-use the specific builders
  (`buildMintAssetInstruction`, `buildTransferAssetInstruction`,
  `buildBurnAssetInstruction`, `buildBurnTriggerRepetitionsInstruction`) so numeric
  inputs, metadata, and asset IDs are normalised identically to the convenience
  helpers. Pass the resulting objects directly to `buildTransaction`.
- Use string quantities (`"10"`) for `Numeric` values whenever you want to avoid
  JavaScript floating-point pitfalls; the builders accept `string | number |
  bigint` but always serialise through the canonical Norito string form.
- Keep asset IDs fully qualified (`rose#wonderland#ed0120…`) when chaining mint
  and transfer steps. The helpers do not guess missing suffixes, ensuring all
  peers derive the same destination.
- Reuse the exported `normalizeAccountId()` / `normalizeAssetId()` helpers when you
  accept human input. They canonicalise multihash identifiers into the uppercase
  format expected by the data model, preventing subtle casing mismatches before
  you hand values to the builders.
- During development, consider round-tripping instructions through
  `noritoEncodeInstruction`/`noritoDecodeInstruction` (as shown in
  `recipes/batching.mjs`) to confirm the payload shape matches your intent prior
  to signing or submitting transactions.

The `recipes/batching.mjs` script demonstrates these patterns end-to-end and
prints deterministic hashes for the batched transactions.

## SM2 Deterministic Fixture & Helpers

The JS SDK now ships higher-level SM2 helpers backed by the native host:

- `generateSm2KeyPair({ distid? })`
- `deriveSm2KeyPairFromSeed(seed, distid?)`
- `loadSm2KeyPair(privateKey, distid?)`
- `signSm2(message, privateKey, distid?)`
- `verifySm2(message, signature, publicKey, distid?)`
- `sm2PublicKeyMultihash(publicKey)`

All helpers default to the canonical distinguishing ID (`1234567812345678`)
and share the same deterministic policy as the Rust/Python SDKs. The
cross-SDK fixture lives in `fixtures/sm/sm2_fixture.json` and can be retrieved
via `sm2FixtureFromSeed(distid, seed, message)` for parity tests:

```js
import {
  generateSm2KeyPair,
  deriveSm2KeyPairFromSeed,
  loadSm2KeyPair,
  signSm2,
  verifySm2,
  sm2PublicKeyMultihash,
  sm2FixtureFromSeed,
} from "@iroha/iroha-js";

const generated = generateSm2KeyPair();
console.log(generated.distid); // "1234567812345678"
console.log(sm2PublicKeyMultihash(generated.publicKey));

const seed = Buffer.from("11".repeat(32), "hex");
const derived = deriveSm2KeyPairFromSeed(seed, "1234567812345678");
const message = Buffer.from("69726F686120736D2073646B2066697874757265", "hex");
const signature = signSm2(message, derived.privateKey, derived.distid);
console.log(verifySm2(message, signature, derived.publicKey, derived.distid)); // true

const loaded = loadSm2KeyPair(derived.privateKey, derived.distid);
console.log(Buffer.from(loaded.publicKey).equals(derived.publicKey));

const fixture = sm2FixtureFromSeed(derived.distid, seed, message);
console.log(fixture.signature); // 1877845D5F...
```

When the `iroha_js_host` native module is unavailable the fixture helper falls
back to the JSON reference, allowing tests to continue asserting deterministic
outputs without rebuilding native artifacts.

## ISO Bridge Alias Helpers

Alias resolution endpoints surface ISO bridge account bindings so operators can
cross-check IBAN attestations without building bespoke HTTP clients. The JS SDK
now mirrors the Python helper coverage:

```js
const torii = new ToriiClient("http://localhost:8080", {
  config: { torii: { apiTokens: ["bridge-token"] } },
});

const voprf = await torii.evaluateAliasVoprf("deadbeef");
console.log(voprf.backend); // "blake2b512-mock"
console.log(voprf.evaluated_element_hex); // hex digest

const resolved = await torii.resolveAlias("GB82 WEST 1234 5698 7654 32");
if (resolved) {
  console.log(`${resolved.alias} → ${resolved.account_id}`);
}

const indexed = await torii.resolveAliasByIndex(0);
console.log(indexed?.source); // "iso_bridge"
```

`resolveAlias*` returns `null` when the alias is missing and throws when the ISO
bridge runtime is disabled, matching Torii’s semantics.

> **Recipe:** run `node javascript/iroha_js/recipes/iso_alias.mjs` to exercise
> the VOPRF and lookup endpoints from the CLI. The script accepts
> `ISO_VOPRF_INPUT`, `ISO_ALIAS_LABEL`, and `ISO_ALIAS_INDEX` so ISO bridge gate
> jobs can hash blinded elements and confirm deterministic account bindings
> without writing bespoke tooling.

Sumeragi consensus status now exposes deterministic membership hashes. Inspecting
the `membership` block is a quick way to verify roster alignment across peers:

```js
const status = await torii.getSumeragiStatus();
if (status.membership) {
  const { height, view, epoch, view_hash: hash } = status.membership;
  console.log(`membership ${height}/${view}/${epoch} hash=${hash}`);
}

if (status.lane_governance) {
  for (const lane of status.lane_governance) {
    const manifest = lane.manifest_ready ? "ready" : "missing";
    console.log(`lane ${lane.alias} manifest ${manifest}; validators=${lane.validator_ids.join(",")}`);
    for (const commitment of lane.privacy_commitments) {
      if (commitment.scheme === "merkle" && commitment.merkle) {
        console.log(`  merkle commitment ${commitment.id} root=${commitment.merkle.root}`);
      } else if (commitment.scheme === "snark" && commitment.snark) {
        console.log(`  snark commitment ${commitment.id} circuit=${commitment.snark.circuit_id}`);
      }
    }
  }
}
if (typeof status.lane_governance_sealed_total === "number") {
  console.log(`sealed lanes remaining: ${status.lane_governance_sealed_total}`);
}
if (Array.isArray(status.lane_governance_sealed_aliases) && status.lane_governance_sealed_aliases.length > 0) {
  console.log(`sealed aliases: ${status.lane_governance_sealed_aliases.join(", ")}`);
}

if (status.lane_commitments) {
  for (const lane of status.lane_commitments) {
    console.log(
      `lane ${lane.lane_id} committed ${lane.teu_total} TEU across ${lane.tx_count} transactions`,
    );
  }
}

if (typeof status.da_reschedule_total === "number") {
  console.log(`DA reschedules so far: ${status.da_reschedule_total}`);
}

if (status.dataspace_commitments) {
  for (const dataspace of status.dataspace_commitments) {
    console.log(
      `lane ${dataspace.lane_id} dataspace ${dataspace.dataspace_id} accounted for ${dataspace.teu_total} TEU`,
    );
  }
}
```

All Sumeragi status helpers accept the standard `{signal}` option so you can
cancel a fetch when rolling the telemetry window:

```js
const abortController = new AbortController();
const status = await torii.getSumeragiStatus({ signal: abortController.signal });
```

When you prefer fully-normalized lane data (numeric IDs and the sealed-lane
summary), call `getSumeragiStatusTyped()` instead; it reuses the same endpoint
but runs the Nexus parsers internally:

```js
const typed = await torii.getSumeragiStatusTyped();
console.log(typed.lane_governance_sealed_total);
console.log(typed.lane_governance_sealed_aliases.join(", "));
```

## Advanced Sumeragi Telemetry

Torii exposes additional consensus observability endpoints. The JS SDK now
mirrors them so operators can inspect pacemaker timers, QC snapshots, collector
plans, and on-chain parameters without bespoke fetch plumbing:

```js
const pacemaker = await torii.getSumeragiPacemaker();
if (pacemaker) {
  console.log(`backoff=${pacemaker.backoff_ms}ms jitter=${pacemaker.jitter_ms}ms`);
}

const qc = await torii.getSumeragiQc();
console.log(`highest QC height=${qc.highest_qc.height} subject=${qc.highest_qc.subject_block_hash ?? "n/a"}`);

const phases = await torii.getSumeragiPhases();
console.log(`pipeline total=${phases.pipeline_total_ms}ms ema=${phases.ema_ms.pipeline_total_ms}ms`);

const blsKeys = await torii.getSumeragiBlsKeys();
console.log(`BLS-capable peers=${Object.values(blsKeys).filter(Boolean).length}`);

const leader = await torii.getSumeragiLeader();
console.log(`leader index=${leader.leader_index} epoch seed=${leader.prf.epoch_seed ?? "unset"}`);

const collectors = await torii.getSumeragiCollectors();
console.log(`collectors K=${collectors.collectors_k} redundant R=${collectors.redundant_send_r}`);

const params = await torii.getSumeragiParams();
console.log(`block time=${params.block_time_ms}ms next mode=${params.next_mode ?? "current"}`);

const telemetry = await torii.getSumeragiTelemetryTyped();
console.log(`availability votes=${telemetry.availability.total_votes_ingested}`);
console.log(`vrf epoch=${telemetry.vrf.epoch} finalized=${telemetry.vrf.finalized}`);
console.log(`pending RBC sessions=${telemetry.rbc_backlog.pending_sessions}`);

// Commit certificates and key lifecycle history
const commitCerts = await torii.listSumeragiCommitCertificates();
console.log(`latest commit cert height=${commitCerts[0]?.height ?? "none"}`);

const keyRecords = await torii.listSumeragiKeyLifecycle();
console.log(`latest key record status=${keyRecords[0]?.status ?? "none"}`);
```

All advanced helpers validate the Torii payloads and coerce numeric string
fields into numbers. If Torii returns malformed data (missing fields or invalid
types) the SDK raises a `TypeError`, ensuring broken telemetry never flows into
dashboards unnoticed.

`getSumeragiPacemaker` returns `null` when developer telemetry outputs are
disabled; the remaining helpers bubble up HTTP errors so dashboards can
distinguish network failures from gated endpoints.

Gateway telemetry also exposes peer metadata (connectivity, config facts, map
info) for operators pinning Torii relays. The SDK normalises these payloads via
`listTelemetryPeersInfo`:

```js
const peers = await torii.listTelemetryPeersInfo();
for (const peer of peers) {
  console.log(
    `${peer.url} connected=${peer.connected} telemetry=${
      peer.telemetryUnsupported ? "disabled" : "enabled"
    }`,
  );
  if (peer.config?.queueCapacity) {
    console.log(`  queue=${peer.config.queueCapacity} public_key=${peer.config.publicKey}`);
  }
  if (peer.location) {
    console.log(`  location=${peer.location.city}, ${peer.location.country}`);
  }
}
```

Torii status snapshots extend the base `/v1/status` payload with derived metrics:

```js
const snapshot = await torii.getStatusSnapshot();
console.log(
  `queue=${snapshot.status.queue_size} Δ=${snapshot.metrics.queue_delta} approvals=${snapshot.metrics.tx_approved_delta}`,
);
console.log(`DA reschedules this interval=${snapshot.metrics.da_reschedule_delta}`);
if (snapshot.status.governance) {
  const admission = snapshot.status.governance.manifest_admission;
  console.log(
    `governance checks=${admission.total_checks} runtime rejections=${admission.runtime_hook_rejected}`,
  );
}
```

### Capturing telemetry replay snapshots

Roadmap JS-04/JS-07 also call for deterministic telemetry replay artefacts. Use
`captureSumeragiTelemetrySnapshot` when you need an in-memory snapshot with a
stable timestamp, or `appendSumeragiTelemetrySnapshot` to build an NDJSON file
that dashboards and incident drills can replay later:

```js
import {
  ToriiClient,
  appendSumeragiTelemetrySnapshot,
} from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.IROHA_TORII_URL, {
  apiToken: process.env.IROHA_TORII_API_TOKEN,
});

await appendSumeragiTelemetrySnapshot(torii, "artifacts/sumeragi/latest.ndjson");
```

The repo also includes a CLI helper that wraps the same API and runs on a timer:

```bash
npm run telemetry:capture -- \
  --torii-url=https://torii.nexus.dev \
  --output=artifacts/sumeragi/telemetry.ndjson \
  --samples=10 \
  --interval-ms=2000
```

Every invocation appends a JSON line containing the capture timestamp and typed
telemetry payload so operators can feed the bundle into replay tooling or share
it with other SDKs.

## RBC Telemetry & Sampling

Reliable broadcast (RBC) observability surfaces under `/v1/sumeragi/rbc*` and
the authenticated sampling endpoint. `ToriiClient` exposes typed helpers so
SDK consumers can gather telemetry, inspect delivery status, or request chunk
samples without duplicating retry logic:

```js
const telemetry = await torii.getSumeragiRbc();
console.log(`active sessions=${telemetry?.sessionsActive ?? 0}`);

const sessions = await torii.getSumeragiRbcSessions();
for (const session of sessions?.items ?? []) {
  console.log(
    `height=${session.height} ready=${session.readyCount} delivered=${session.delivered}`,
  );
}

const delivered = await torii.getSumeragiRbcDelivered(42, 0);
if (delivered?.present) {
  console.log(`block ${delivered.blockHash} delivered=${delivered.delivered}`);
}

const candidate = await torii.findRbcSamplingCandidate();
if (!candidate) {
  throw new Error("no delivered RBC sessions available for sampling");
}
const sampleRequest = ToriiClient.buildRbcSampleRequest(candidate, {
  count: 3,
  apiToken: process.env.SUMERAGI_API_TOKEN,
});
const samples = await torii.sampleRbcChunks(sampleRequest);
samples?.samples.forEach(({ index, proof }) => {
  console.log(`chunk ${index} proof depth=${proof.depth}`);
});

// Abort long-running sampling requests if your UI needs to cancel.
const controller = new AbortController();
await torii.sampleRbcChunks({ ...sampleRequest, signal: controller.signal });
await torii.getSumeragiRbc({ signal: controller.signal });
await torii.getSumeragiRbcDelivered(candidate.height, candidate.view, {
  signal: controller.signal,
});

const evidence = await torii.listSumeragiEvidence({ limit: 20, kind: "DoublePrevote" });
console.log(`Observed ${evidence.total} evidence entries`);
const count = await torii.getSumeragiEvidenceCount();
await torii.submitSumeragiEvidence({
  evidence_hex: "deadbeef",
  apiToken: process.env.SUMERAGI_API_TOKEN,
});
```

Telemetry helpers return `null` when the node disables developer outputs. When
`apiToken` is set in the constructor or environment (`IROHA_TORII_API_TOKEN`)
the client automatically sends the required `X-API-Token` header for RBC
sampling; pass `apiToken` directly to `sampleRbcChunks` (or the helper) only
when overriding. Use the top-level `buildRbcSampleRequest` helper (or its
static counterpart on `ToriiClient`) to derive a sampling payload straight
from an active session so height/view/block-hash normalisation stays aligned
with the roadmap’s JS4 RBC helper requirements. When manual selection is
inconvenient, call `findRbcSamplingCandidate()` — it scans the RBC session
snapshot for the latest delivered block hash and returns the matching
`SumeragiRbcSession`, which can be passed directly into
`buildRbcSampleRequest`. RBC helpers validate option objects up front (including
unsupported keys) and accept `AbortSignal` inputs where applicable so typos and
canceled requests never reach Torii.

## SoraFS Storage Helpers

```js
const pinResult = await torii.pinSorafsManifest({
  manifest: fs.readFileSync("./manifest.norito"),
  payload: fs.readFileSync("./payload.bin"),
});
console.log(`manifest=${pinResult.manifest_id_hex} digest=${pinResult.payload_digest_hex}`);

const registerRequest = {
  authority: process.env.SORAFS_OPERATOR_ID ?? "alice@wonderland",
  privateKey: process.env.SORAFS_OPERATOR_KEY ?? "ed25519:deadbeef",
  manifestDigestHex: pinResult.manifest_id_hex,
  chunkDigestSha3_256Hex: process.env.SORAFS_CHUNK_DIGEST ?? "1".repeat(64),
  submittedEpoch: Date.now(),
  chunker: {
    profileId: 1,
    namespace: "sorafs",
    name: "sf1",
    semver: "1.0.0",
  },
  pinPolicy: { minReplicas: 3, storageClass: "Hot", retentionEpoch: 86_400 },
  alias: {
    namespace: "docs",
    name: "main",
    proof: fs.readFileSync("./artifacts/docs_alias.proof"),
  },
};
const registerResponse = await torii.registerSorafsPinManifest(registerRequest);
console.log("pin registry status:", registerResponse.status);

const registerTyped = await torii.registerSorafsPinManifestTyped(registerRequest);
console.log("registered manifest:", registerTyped.manifest_digest_hex, registerTyped.chunker_handle);

const range = await torii.fetchSorafsPayloadRange({
  manifestIdHex: pinResult.manifest_id_hex,
  offset: 0,
  length: 4096,
});
const firstChunk = Buffer.from(range.data_b64, "base64");

const storageState = await torii.getSorafsStorageState();
console.log(`pin queue depth=${storageState.pin_queue_depth}`);

const storedManifest = await torii.getSorafsManifest(pinResult.manifest_id_hex);
console.log(`profile=${storedManifest.chunk_profile_handle} chunks=${storedManifest.chunk_count}`);

const daBundle = await torii.getDaManifest("0x" + "aa".repeat(32));
console.log(`DA manifest lane=${daBundle.lane_id} chunkPlanChunks=${daBundle.chunk_plan?.chunks?.length}`);

const ingestResult = await torii.submitDaBlob({
  payload: fs.readFileSync("./artifacts/nexus_sidecar.car"),
  codec: "nexus_lane_sidecar",
  laneId: 7,
  epoch: 11,
  sequence: Date.now(),
  retentionPolicy: { storageClass: "Hot", governanceTag: "nexus.sidecars" },
  metadata: {
    "content-type": "application/car",
    "da.stream": {
      value: "governance",
      visibility: "Public",
    },
  },
  privateKeyHex: process.env.DA_SUBMITTER_PRIVATE_KEY,
});
if (ingestResult.receipt) {
  console.log(
    `storage ticket ${ingestResult.receipt.storage_ticket_hex} hash=${ingestResult.receipt.blob_hash_hex}`,
  );
}

const session = await torii.fetchDaPayloadViaGateway({
  storageTicketHex: ingestResult.receipt.storage_ticket_hex,
  gatewayProviders: [
    {
      name: "alpha",
      providerIdHex: process.env.SORAFS_PROVIDER_ID,
      baseUrl: "https://gateway.example.com/",
      streamTokenB64: process.env.SORAFS_STREAM_TOKEN,
    },
  ],
  fetchOptions: {
    maxPeers: 4,
    retryBudget: 5,
    scoreboard: {
      persist_path: "/tmp/scoreboard.json",
      telemetry_source_label: "ci-da-audit",
    },
  },
  proofSummary: {
    sampleCount: 12,
    sampleSeed: 99,
    leafIndexes: [0, 1, 2],
  },
});
console.log(`payload fetched (${session.gatewayResult.assembledBytes} bytes)`);
console.log(`proofs verified=${session.proofSummary?.proofs.every((proof) => proof.verified)}`);

// Derive handles manually when working with saved manifests:
const chunkerHandle = deriveDaChunkerHandle(session.manifest.manifest_bytes);
console.log(`resolved chunker handle=${chunkerHandle}`);

// You can also re-run the native helper on saved artefacts:
const summary = generateDaProofSummary(
  session.manifest.manifest_bytes,
  session.gatewayResult.payload,
  { sampleCount: 4, leafIndexes: [0, 5] },
);
console.log(`sample seed=${summary.sample_seed} proofCount=${summary.proof_count}`);

const artifact = buildDaProofSummaryArtifact(summary, {
  manifestPath: "./artifacts/manifest.to",
  payloadPath: "./artifacts/payload.car",
});
await emitDaProofSummaryArtifact({
  summary,
  manifestPath: artifact.manifest_path,
  payloadPath: artifact.payload_path,
  outputPath: "./artifacts/proof_summary.json",
});
console.log(`proof summary emitted to ${artifact.manifest_path} / proof_summary.json`);

// Mirror the CLI artefact layout without shelling out:
const manifestResult = await torii.getDaManifestToDir(
  ingestResult.receipt.storage_ticket_hex,
  { outputDir: "./artifacts/da/get_blob" },
);
const proveResult = await torii.proveDaAvailabilityToDir({
  storageTicketHex: ingestResult.receipt.storage_ticket_hex,
  gatewayProviders: [
    {
      name: "alpha",
      providerIdHex: process.env.SORAFS_PROVIDER_ID,
      baseUrl: "https://gateway.example.com/",
      streamTokenB64: process.env.SORAFS_STREAM_TOKEN,
    },
  ],
  fetchOptions: { allowSingleSourceFallback: true },
  proofSummary: { sampleCount: 4, leafIndexes: [0, 2] },
  outputDir: "./artifacts/da/prove_availability",
});
console.log("manifest paths:", manifestResult.paths);
console.log("payload saved:", proveResult.payloadPath);
console.log("scoreboard saved:", proveResult.scoreboardPath);
console.log("proof summary saved:", proveResult.proofSummaryPath);

`fetchDaPayloadViaGateway` automatically derives the chunker handle from the manifest bundle when you omit `chunkerHandle`, and the exported `deriveDaChunkerHandle` helper surfaces the same logic for bespoke tooling. `generateDaProofSummary` reuses the Norito + PoR logic from the CLI via the native binding so proofs remain identical across SDKs.

> **Multi-source enforcement:** the JS SDK now requires at least two gateway providers for every orchestrated fetch. This matches the SF-6c roadmap requirement and keeps `cargo xtask sorafs-adoption-check` green by default. When you must temporarily fall back to a single healthy provider (e.g., during an incident), pass `fetchOptions: { allowSingleSourceFallback: true }` and include the incident ticket in your release notes.

Every gateway fetch also exposes the orchestrator’s scoreboard metadata so you
can attach the same evidence bundle as the CLI. `gatewayResult.metadata`
includes the direct/gateway provider counts, the derived provider-mix label
(`"gateway-only"` for the JS bindings unless you deliberately mix in local
providers), policy override flags, manifest IDs/CIDs, and telemetry labels—the
new `telemetryRegion` field mirrors the `--telemetry-region` CLI flag so adoption
reports can prove which fleet produced the capture:

```js
const { metadata } = session.gatewayResult;
console.log(
  `provider mix=${metadata.providerMix} transport=${metadata.transportPolicy} manifest=${metadata.gatewayManifestId}`,
);
if (!metadata.gatewayManifestProvided) {
  throw new Error("Gateway fetches must include a signed manifest envelope.");
}
```

`submitDaBlob` computes the BLAKE3 digest via the native binding, so run `npm run build:native`
before calling it—and the gateway/proof helpers—in development environments. Pass
`artifactDir: "./artifacts/da/submission_<stamp>"` (and `noSubmit: true` for dry
runs) to mirror the CLI ingest artefacts without leaving Node.

const pinListing = await torii.listSorafsPinManifests({ status: "approved", limit: 25 });
console.log(`approved manifests returned=${pinListing.returned_count}`);

const aliases = await torii.listSorafsAliases({ namespace: "docs" });
console.log(`doc namespace aliases=${aliases.returned_count}`);

const replication = await torii.listSorafsReplicationOrders({ status: "pending" });
console.log(`pending replication orders=${replication.total_count}`);

for await (const manifest of torii.iterateSorafsPinManifests({ pageSize: 25 })) {
  console.log("manifest digest", manifest.digest_hex);
}
for await (const alias of torii.iterateSorafsAliases({ namespace: "docs", pageSize: 50 })) {
  console.log("alias entry", alias.alias);
}
for await (const order of torii.iterateSorafsReplicationOrders({ pageSize: 25 })) {
  console.log("replication order", order.order_id_hex);
}
```

> **Missing manifests:** `getSorafsPinManifest` now returns `null` when Torii
> responds with `404 Not Found`, allowing scripts to differentiate between a
> missing manifest and a malformed payload. `getSorafsPinManifestTyped`
> continues to throw when the digest is absent so automation that expects a
> manifest still fails fast.

Uptime telemetry and PoR automation helpers surface the raw endpoints so SDK
callers can publish probe samples, submit Norito-encoded challenges/proofs, and
retrieve the coordinator exports:

```js
await torii.submitSorafsUptimeObservation({ uptimeSecs: 540, observedSecs: 600 });
await torii.submitSorafsPorObservation({ success: true });
await torii.recordSorafsPorChallenge({ challenge: porChallengeBytes });
await torii.recordSorafsPorProof({ proof: porProofBytes });
await torii.recordSorafsPorVerdict({ verdict: porVerdictBytes });

const porStatuses = await torii.getSorafsPorStatus({ providerHex: providerIdHex });
const porExport = await torii.exportSorafsPorStatus({ startEpoch: 1024, endEpoch: 1032 });
const weeklyReport = await torii.getSorafsPorWeeklyReport("2026-W05");
```

`getSorafsPorStatus`, `exportSorafsPorStatus`, and `getSorafsPorWeeklyReport`
return Norito bytes (`Buffer` instances). Decode them with `norito::json`, the
Rust `norito` crate, or another Norito-compatible runtime before inspecting the
structured payloads.

## UAID Portfolios & Space Directory Manifests

Universal Account IDs (UAIDs) power the Nexus dataspace model. Torii exposes
three read-only endpoints (documented in
[`docs/source/torii/portfolio_api.md`](../../docs/source/torii/portfolio_api.md))
so SDKs can inspect aggregated balances, dataspace bindings, and the canonical
capability manifests tracked by the Space Directory. The JS SDK surfaces typed
helpers for all three surfaces:

```js
const uaidLiteral = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaidLiteral);
for (const ds of portfolio.dataspaces) {
  console.log(`dataspace ${ds.dataspace_alias ?? ds.dataspace_id} accounts=${ds.accounts.length}`);
  ds.accounts.forEach((account) => {
    account.assets.forEach((asset) => {
      console.log(`  ${asset.asset_definition_id} -> ${asset.quantity}`);
    });
  });
}

const bindings = await torii.getUaidBindings(uaidLiteral, { addressFormat: "compressed" });
bindings.dataspaces.forEach((entry) => {
  console.log(`${entry.dataspace_alias ?? entry.dataspace_id}: ${entry.accounts.join(", ")}`);
});

const manifests = await torii.getUaidManifests(uaidLiteral, { dataspaceId: 11 });
manifests.manifests.forEach((manifest) => {
  console.log(
    `manifest ${manifest.manifest_hash} status=${manifest.status} entries=${manifest.manifest.entries.length}`,
  );
});
```

Each helper normalises/validates the response payloads:

- `getUaidPortfolio` enforces numeric totals and returns the deterministically
  sorted dataspace/account tree.
- `getUaidBindings` mirrors the Space Directory bindings map so tooling can
  confirm which Torii account IDs are active per dataspace. Pass
  `addressFormat: "compressed"` to retrieve `snx1…@domain` literals when you
  need QR-friendly output; IH58 strings remain the default.
- `getUaidManifests` validates lifecycle metadata, manifest hashes, allow/deny
  entries, and optional dataspace filters (set `dataspaceId` to restrict the
  snapshot).

The helpers automatically canonicalise the UAID literal (`uaid:<hex>`) and throw
when the supplied identifier is malformed, ensuring automation scripts surface
clear diagnostics long before the request reaches Torii.

### Publishing & revoking manifests

Operators can stage capability rotations or emergency deny-wins decisions via
Torii as well. `publishSpaceDirectoryManifest()` posts the canonical manifest
JSON (or a structure parsed from the fixtures under
`fixtures/space_directory/capability/`) together with the authority’s private
key, while `revokeSpaceDirectoryManifest()` enqueues an immediate revocation
for a UAID/dataspace pair. Both helpers accept an optional `options.signal`
so callers can abort long-running submissions with `AbortController`. Passing
anything other than an object (or an `AbortSignal` instance on the `signal`
field) throws synchronously, keeping JS-04’s validation parity intact:

```js
import { promises as fs } from "node:fs";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "governance@sora",
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    manifest,
    reason: "Rotation to attester set v2",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "governance@sora",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid: "uaid:c2b61dd6bb73e91ee6d0949508d491bbc1b2a347a3f41b5cd35d733c1e751111",
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins trigger",
  },
  { signal: controller.signal },
);
```

`privateKey` accepts a raw 32-byte seed, `privateKeyHex` wraps 64-character hex
strings as `ed25519:<hex>`, and `privateKeyMultihash` allows callers to supply
preformatted multihash literals when hardware security modules emit the string
directly. Both helpers return `null` for HTTP `202 Accepted` responses; Torii
queues the underlying transaction and emits the corresponding
`SpaceDirectoryEvent` once consensus processes the manifest.

## SoraNet Puzzle & Token Service Client

The `SoranetPuzzleClient` helper talks to the optional
`soranet-puzzle-service` microservice so SDK consumers can mint Argon2 tickets,
inspect puzzle policy, and request ML-DSA admission tokens without reimplementing
the HTTP transport. The client mirrors the JSON schema described in
[`docs/source/soranet/puzzle_service_operations.md`](../../docs/source/soranet/puzzle_service_operations.md).

```js
import { SoranetPuzzleClient } from "@iroha/iroha-js";

const puzzle = new SoranetPuzzleClient("http://localhost:8088", {
  defaultHeaders: { Authorization: `Bearer ${process.env.SORANET_TOKEN}` },
  timeoutMs: 5_000,
});

const config = await puzzle.getPuzzleConfig();
if (config.required) {
  console.log(
    `difficulty=${config.difficulty} Argon2 lanes=${config.puzzle?.lanes ?? 0}`,
  );
}

const ticket = await puzzle.mintPuzzleTicket({
  ttlSecs: 90,
  signed: true,
  transcriptHashHex: "bb".repeat(32),
});
console.log(`ticket=${ticket.ticketB64} expires=${ticket.expiresAt}`);
if (ticket.signedTicketB64) {
  console.log(`signed ticket fingerprint=${ticket.signedTicketFingerprintHex}`);
}

const token = await puzzle.mintAdmissionToken("aa".repeat(32), {
  ttlSecs: 300,
  flags: 1,
});
console.log(`token id=${token.tokenIdHex} issuer=${token.issuerFingerprintHex}`);
```

`mintPuzzleTicket` accepts optional `transcriptHashHex` and `signed` flags to bind
tickets to a session transcript and request relay-signed credentials; signed
responses include a `signedTicketFingerprintHex` to help track replay cache
state across restarts.

`mintAdmissionToken` enforces 32-byte transcript hashes and clamps TTL, flag,
and issued-at overrides to the relay policy. Use `/v1/token/config` to display
the active issuer fingerprint and revocation window in operator tooling. Errors
from the service propagate as `SoranetPuzzleError` with `status`/`body`
accessors so callers can feed structured logs or retry policies easily.

## Kaigi Relay Telemetry

Relay operators and observability tooling can now inspect Kaigi health directly
from the SDK. The new helpers mirror the Torii endpoints so you can fetch
summaries, inspect a single relay, grab the aggregated health snapshot, or
stream the live registration/health SSE feed with domain/relay/kind filters:

```js
const relays = await torii.listKaigiRelays();
console.log(`registered relays: ${relays.total}`);
relays.items.forEach((relay) => {
  console.log(`${relay.relay_id} (${relay.domain}) status=${relay.status ?? "unknown"}`);
});

const detail = await torii.getKaigiRelay(relays.items[0]?.relay_id ?? "relay@kaigi");
if (detail?.metrics) {
  console.log(`${detail.metrics.domain} registrations=${detail.metrics.registrations_total}`);
}

const health = await torii.getKaigiRelaysHealth();
console.log(
  `healthy=${health.healthy_total} degraded=${health.degraded_total} unavailable=${health.unavailable_total}`,
);

for await (const event of torii.streamKaigiRelayEvents({
  domain: "kaigi",
  kind: ["registration", "health"],
})) {
  if (event.data?.kind === "health") {
    console.log(`${event.data.relay_id} reported ${event.data.status}`);
    break;
  }
}
```

`streamKaigiRelayEvents` yields strongly-typed SSE payloads so you can feed
operators dashboards without reimplementing filtering/normalisation logic.

## ISO 20022 Bridge

Submit ISO 20022 pacs.008 or pacs.009 payloads and poll their deterministic status
through the Torii bridge:

```js
const xml = `<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.10">
  <!-- ... -->
</Document>`;

const status = await torii.submitIsoPacs008AndWait(xml, {
  wait: {
    maxAttempts: 20,
    pollIntervalMs: 3_000,
    onPoll: ({ attempt, status }) => {
      const label = status?.status ?? "unknown";
      const hash = status?.transaction_hash ?? "<pending>";
      console.log(`[attempt ${attempt}] ${label} tx=${hash}`);
    },
  },
});
console.log(status.message_id, status.status, status.transaction_hash);
```

`submitIsoPacs008` and `submitIsoPacs009` accept strings or binary buffers and
enforce `application/xml` content-type by default. `submitIsoPacs008AndWait` /
`submitIsoPacs009AndWait` build on those helpers to poll `/v1/iso20022/status`
until the bridge reports a deterministic terminal state. Provide `wait` options
to customise the cadence, attach telemetry hooks, or opt into resolving as soon
as an `Accepted` status arrives (even before the Torii transaction hash is
available). If you already have a message identifier, call
`waitForIsoMessageStatus(messageId, waitOptions)` directly. Both helpers also
accept an `AbortSignal` so CI and long-running scripts can cancel pending
polls—pass `signal` inside `wait` options or call
`getIsoMessageStatus(id, { signal })` when you only need a single fetch.
Unknown fields inside `wait` options are rejected up front so mis-configured
automation fails before any network traffic is sent. Non-zero `pollIntervalMs`
values below 10 ms are rejected to avoid tight spin loops; keep `0` only for
deterministic unit tests and use sensible intervals in production flows.

`submitIsoMessage` combines the builders with the submission/wait helpers so callers can
provide structured ISO 20022 fields instead of hand-written XML. Pass `kind: "pacs.009"`
for PvP funding legs (defaults to pacs.008), and include `wait` options when you want the
helper to poll until Torii returns a deterministic status. The helper sets pragmatic MIME
types (`application/pacs008+xml` or `application/pacs009+xml`) and reuses the same
`AbortSignal` for both the submission and polling phases:

```js
const settlement = await torii.submitIsoMessage(
  {
    instructionId: "pvpfund-1",
    amount: { currency: "USD", value: "1250.50" },
    instigatingAgent: { bic: "BOFAUS3N" },
    instructedAgent: { bic: "DEUTDEFF" },
    purposeCode: "SECU",
  },
  {
    kind: "pacs.009",
    wait: { maxAttempts: 10, pollIntervalMs: 2_000 },
  },
);
console.log(settlement.status, settlement.transaction_hash);
```

Pass either `kind` or its alias `messageKind`; if both are provided they must match or the
helper will throw before any network requests are issued.

Pass a string `contentType` override when you submit namespaced XML (for
example, `application/pacs009+xml`). The helpers reject non-string or blank
overrides before issuing the HTTP request so CI jobs immediately surface
misconfigured headers instead of sending malformed traffic to Torii.

Attach a `retryProfile` when you need ISO submissions and status polls to ride a
custom retry budget (for example, an "iso" policy tuned for long-running bridge
workers). The same profile value flows into `waitForIsoMessageStatus`, so both
the submit and poll legs share the configured backoff.

All ISO bridge wait helpers throw `IsoMessageTimeoutError` when the message does
not reach a terminal status within the requested attempts.

### Generate camt.052 reports and camt.056 cancellation requests

The ISO helpers also cover the account-reporting (`camt.052`) and cancellation
(`camt.056`) schemas exercised by the ledger. Use the structured builders to avoid
manually stitching XML when you need to export statements or cancel pending transfers:

```js
import {
  buildCamt052Message,
  buildCamt056Message,
  buildSampleCamt052Message,
  buildSampleCamt056Message,
} from "@iroha/iroha-js";

const camt052 = buildCamt052Message({
  messageId: "report-20260305",
  creationDateTime: "2026-03-05T08:00:00Z",
  reportId: "report-20260305-page-1",
  pagination: { pageNumber: 1, lastPage: true },
  account: { otherId: "treasury-usd-001" },
  accountCurrency: "USD",
  balances: [
    {
      typeCode: "ITBD",
      amount: { currency: "USD", value: "950000.00" },
      creditDebitIndicator: "CRDT",
    },
  ],
  entries: [
    {
      amount: { currency: "USD", value: "5000.00" },
      creditDebitIndicator: "DBIT",
      status: "BOOK",
      reference: "pacs008-ffe5",
    },
  ],
});

const camt056 = buildCamt056Message({
  assignmentId: "cancel-ffe5",
  creationDateTime: "2026-03-05T10:00:00Z",
  cancellationId: "cancel-ffe5-tx",
  assignerAgent: { bic: "ALPHGB2L" },
  assigneeAgent: { bic: "OMEGGB2L" },
  debtorAgent: { bic: "ALPHGB2L" },
  creditorAgent: { bic: "OMEGGB2L" },
  originalMessageId: "pacs008-ffe5",
  originalMessageNameId: "pacs.008.001.10",
  interbankSettlementAmount: { currency: "USD", value: "5000.00" },
  interbankSettlementDate: "2026-03-05",
  originalInstructionId: "instr-ffe5",
  originalEndToEndId: "e2e-ffe5",
  originalTransactionId: "tx-ffe5",
});

// Sample helpers mirror the fixtures used in docs/tests.
const sampleReport = buildSampleCamt052Message();
const sampleCancellation = buildSampleCamt056Message();
```

Inputs are validated the same way as the pacs builders (BIC/IBAN/LEI checks,
ISO datetimes, CRDT/DBIT enumerations, pagination metadata), so malformed
reports or cancellation requests are rejected locally before hitting the Torii
bridge.

Bridge responses normalise `status` to `Pending`, `Accepted`, or `Rejected` and
ensure `pacs002_code` is one of `ACTC`, `ACSP`, `ACSC`, `ACWC`, `PDNG`, or
`RJCT`. Any other value raises a `TypeError` before the payload leaves the SDK,
so CI and operators catch unexpected bridge states immediately.

See `recipes/iso_bridge.mjs` for a runnable example that submits a sample
pacs.008 or pacs.009 payload, polls status with per-attempt logging, and shows
how to wire `ISO_POLL_ATTEMPTS`, `ISO_POLL_INTERVAL_MS`, `ISO_MESSAGE_ID`, and
`TORII_URL` through environment variables.

### ISO 20022 Message Builders

The SDK exports `buildPacs008Message` and `buildPacs009Message` helpers that map
structured inputs to standards-compliant XML while validating identifiers
described in the [ISO field mapping guide](../../docs/source/finance/settlement_iso_mapping.md).
Pass the required identifiers (BIC, amount, purpose code, etc.) and the helpers
emit deterministic XML payloads ready for submission via the Torii client. In
addition to the length/character checks, IBAN inputs must pass the canonical
mod-97 checksum so builders fail fast when an attestation hash contains a typo.
Creation timestamps must include a timezone offset (`Z` or `±HH:MM`) so the
payload is deterministic across hosts; timezone-less strings are rejected to
avoid lossy local-time conversions.

```js
import { buildPacs008Message, ToriiClient } from "@iroha/iroha-js";

const settlement = buildPacs008Message({
  messageId: "iso-demo-1",
  instructionId: "instr-1",
  settlementDate: "2026-02-10",
  amount: { currency: "EUR", value: "25.00" },
  instigatingAgent: { bic: "DEUTDEFF", lei: "529900ODI3047E2LIV03" },
  instructedAgent: { bic: "COBADEFF" },
  debtorAccount: { iban: "DE89370400440532013000" },
  creditorAccount: { otherId: "alice@wonderland" },
  purposeCode: "SECU",
  supplementaryData: { account_id: "alice@wonderland", leg: "delivery" },
});

const torii = new ToriiClient("http://localhost:8080");
const status = await torii.submitIsoPacs008AndWait(settlement, {
  wait: { maxAttempts: 15, pollIntervalMs: 3_000 },
});
console.log(status.status, status.transaction_hash);
```

For advanced flows, supply optional `debtorAgent`/`creditorAgent` (additional BIC/LEI pairs)
and `debtor`/`creditor` party metadata (legal name, LEI, proprietary IDs with custom scheme
codes). The builders insert those records as `DbtrAgt`/`CdtrAgt` and `Dbtr`/`Cdtr` elements so
PvP/RFQ pipelines can mirror the ISO 20022 guidance in
[`settlement_iso_mapping.md`](../../docs/source/finance/settlement_iso_mapping.md) without hand
crafting XML. Both builders also accept optional debtor/creditor accounts, purpose codes, and
structured supplementary JSON, making it trivial to carry Norito identifiers alongside the
standard MT-style fields.
Accounts may also carry proxy aliases (for example, phone-number or email handles) via
`proxy: { id, typeCode?, typeProprietary? }`. When present, the proxy is emitted under `Prxy`
alongside the IBAN, enforcing `Max2048Text` for the identifier and requiring either a 1-4
character type code or a proprietary label (but not both).
`buildPacs009Message` reuses the instruction id as both `MsgId` and `BizMsgIdr` when no explicit
message identifiers are provided and defaults `MsgDefIdr` to `pacs.009.001.10`, matching the
bridge’s canonical profile.
The `pacs.009` helper defaults `Purp` to `SECU` (the securities funding category purpose) but
accepts any valid ISO code when callers intentionally emit non-securities transfers; invalid
values still throw before submission so PvP funding flows stay aligned with the mapping guide
while other ISO scenarios remain supported.

Cash amounts are normalised to the correct ISO 4217 minor units before emission (for example,
`JPY` rejects fractional values, while `BHD` pads to three decimals). The helpers pad shorter
values with zeros and reject inputs that exceed the allowed precision so callers cannot produce
non-compliant interbank payloads.

The `recipes/iso_bridge_builder.mjs` example wires those helpers into a CLI that
derives sensible defaults, accepts overrides via environment variables or a JSON
config file, prints the generated XML, and optionally submits the message to
Torii when `ISO_SUBMIT=1`.

## Contract Deployment Helpers

Register manifests and bytecode directly from JavaScript without hand-crafting
Norito payloads. The SDK normalises hash literals, validates access-set hints,
and encodes code bytes as base64 strings before signing:

> Need a turnkey CLI instead of writing bespoke scripts? Use
> `javascript/iroha_js/recipes/contracts.mjs`. The helper reads your `.to`
> artifact (`CONTRACT_CODE_PATH`), optional manifest JSON
> (`CONTRACT_MANIFEST_PATH` or `CONTRACT_MANIFEST_JSON`), and dispatches the
> relevant REST calls. Flip `CONTRACT_STAGE` between `register`, `instance`, or
> `both` to mirror the JS-06 roadmap flows, and pass
> `TORII_AUTH_TOKEN`/`TORII_API_TOKEN` when the node is locked down.

```js
import { AccountAddress } from "@iroha/iroha-js";

const address = AccountAddress.fromAccount({
  domain: "default",
  publicKey: new Uint8Array(32),
});
console.log(address.canonicalHex());
console.log(address.toIH58(753));
console.log(address.toCompressedSora());

// Build a registry-backed selector when a global registry id is available.
const registryAddress = AccountAddress.fromAccount({
  registryId: 42,
  publicKey: new Uint8Array(32),
});
console.log(registryAddress.domainSummary()); // { kind: "global", warning: null }
```

```js
import {
  buildRegisterSmartContractCodeTransaction,
  buildRegisterSmartContractBytesTransaction,
  buildActivateContractInstanceInstruction,
} from "@iroha/iroha-js";
import fs from "node:fs";

const manifestTx = buildRegisterSmartContractCodeTransaction({
  chainId: "test-chain",
  authority,
  manifest: {
    codeHash: Buffer.alloc(32, 0xaa),
    abiHash: "hash:…",
    compilerFingerprint: "kotodama-1.2 rustc-1.79",
    accessSetHints: {
      readKeys: ["account:alice@wonderland"],
      writeKeys: ["contract:apps:ledger"],
    },
  },
  privateKey,
});

const codeTx = buildRegisterSmartContractBytesTransaction({
  chainId: "test-chain",
  authority,
  codeHash: Buffer.alloc(32, 0xaa),
  code: fs.readFileSync("./contract.to"),
  privateKey,
});

const activateInstruction = buildActivateContractInstanceInstruction({
  namespace: "apps",
  contractId: "ledger",
  codeHash: Buffer.alloc(32, 0xaa),
});

const deactivateTx = buildDeactivateContractInstanceTransaction({
  chainId: "test-chain",
  authority,
  namespace: "apps",
  contractId: "ledger",
  reason: "rotate after audit finding",
  privateKey,
});

const removeBytesTx = buildRemoveSmartContractBytesTransaction({
  chainId: "test-chain",
  authority,
  codeHash: Buffer.alloc(32, 0xaa),
  reason: "retire archived artifact",
  privateKey,
});
```

`buildRegisterSmartContractCodeInstruction/Transaction` accepts partial manifests
when governance stages code hashes separately. Bytecode helpers enforce the
32-byte hash length and accept `Buffer`, typed arrays, or base64 strings.
Activation helpers normalise namespace/contract IDs into the canonical
`ActivateContractInstance` shape so governance workflows can bind a manifest to
an `{namespace, contract_id}` tuple deterministically.

`buildDeactivateContractInstanceInstruction/Transaction` exposes the governance
kill-switch path with optional audit reasons, and
`buildRemoveSmartContractBytesInstruction/Transaction` wires the bytecode
reclamation ISI into CI/governance tooling. Both helpers reject empty namespace,
contract, or reason strings before submitting payloads so operators get fast
feedback during rehearsals.

The recipe mirrors the same validation rules: keys can be supplied as
`PRIVATE_KEY=ed25519:<hex>` or `PRIVATE_KEY_HEX=<hex>`, namespace/contract ids
must be non-empty strings, and manifest overrides use camelCase fields so CI
orchestrators can reuse governance artefacts directly.

### Contract calls via Torii

`ToriiClient.callContract` wraps `/v1/contracts/call`, preparing the JSON
payload that Torii expects (authority credentials, namespace/contract id,
optional entrypoint/payload/gas settings) and normalising all hash fields. The
helper returns the queued transaction hash along with the code/ABI hashes Torii
resolved for the call.

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.IROHA_TORII_URL, {
  authToken: process.env.IROHA_TORII_AUTH_TOKEN,
});

const response = await torii.callContract({
  authority: AUTHORITY_ACCOUNT_ID,
  privateKey: process.env.IROHA_TORII_PRIVATE_KEY,
  namespace: "apps",
  contractId: "ledger",
  entrypoint: "increment",
  payload: { amount: 1 },
  gasAssetId: "xor#wonderland",
  gasLimit: 50_000,
});

console.log("queued tx:", response.tx_hash_hex);
console.log("code hash:", response.code_hash_hex);
```

Any JSON-compatible payload is cloned before submission so callers can reuse the
object elsewhere without mutation. The helper rejects malformed entrypoint
selectors, negative gas limits, or empty namespace/contract identifiers before
the request reaches Torii.

## Governance Voting Helpers

The governance ISI builders mirror the Torii DTOs, handling hash/hex
normalisation, referendum windows, and ballot encoding:

```js
import { AccountAddress } from "@iroha/iroha-js";

const address = AccountAddress.fromAccount({
  domain: "default",
  publicKey: new Uint8Array(32),
});
console.log(address.canonicalHex());
console.log(address.toIH58(753));
console.log(address.toCompressedSora());
```

```js
import {
  buildProposeDeployContractTransaction,
  buildCastPlainBallotTransaction,
  buildCastZkBallotTransaction,
  buildEnactReferendumTransaction,
} from "@iroha/iroha-js";

const proposalTx = buildProposeDeployContractTransaction({
  chainId: "test-chain",
  authority,
  proposal: {
    namespace: "apps",
    contractId: "ledger",
    codeHash: Buffer.alloc(32, 0xaa),
    abiHash: "hash:…#…",
    abiVersion: "1",
    window: { lower: Date.now(), upper: Date.now() + 60000 },
    votingMode: "Plain",
  },
  privateKey,
});

const zkBallotTx = buildCastZkBallotTransaction({
  chainId: "test-chain",
  authority,
  ballot: {
    electionId: "referendum-1",
    proof: Buffer.from(proofBytes),
    publicInputs: { tally: "aye" },
  },
  privateKey,
});

const plainBallotTx = buildCastPlainBallotTransaction({
  chainId: "test-chain",
  authority,
  ballot: {
    referendumId: "ref-plain",
    owner: authority,
    amount: "5000",
    durationBlocks: 7200,
    direction: "aye",
  },
  privateKey,
});

const enactTx = buildEnactReferendumTransaction({
  chainId: "test-chain",
  authority,
  enactment: {
    referendumId: Buffer.alloc(32, 0xee),
    preimageHash: Buffer.alloc(32, 0xdd),
    window: { lower: 100, upper: 200 },
  },
  privateKey,
});
```

Helper inputs accept either strings or raw `Buffer`s for 32-byte hashes, ensure
referendum windows remain ordered, and convert ballot payloads to canonical
Norito JSON before signing.

See `recipes/governance.mjs` for an end-to-end script that assembles the common
governance transactions, prints deterministic hashes, and optionally submits
them to Torii (`GOV_SUBMIT=1`). Build the native binding first via
`npm run build:native` so `hashSignedTransaction` is available.

## Confidential Asset Helpers

The confidential ISIs ship in parity with the Rust builders so Node.js clients
can register shielded assets, schedule policy transitions, and issue
shield/transfer/unshield transactions without hand-writing Norito payloads.
Inputs accept byte arrays, Buffers, or base64 strings for commitments/nullifiers
and reuse the `ProofAttachmentInput` structure to describe verifier references.

```js
import {
  buildRegisterZkAssetTransaction,
  buildShieldTransaction,
  buildZkTransferTransaction,
} from "@iroha/iroha-js";

const registerTx = buildRegisterZkAssetTransaction({
  chainId: "test-chain",
  authority,
  registration: {
    assetDefinitionId: "rose#wonderland",
    mode: "Hybrid",
    transferVerifyingKey: "halo2/ipa:vk_transfer",
    unshieldVerifyingKey: { backend: "halo2/ipa", name: "vk_unshield" },
  },
  privateKey,
});

const encryptedPayload = {
  version: 1,
  ephemeralPublicKey: crypto.getRandomValues(new Uint8Array(32)),
  nonce: crypto.getRandomValues(new Uint8Array(24)),
  ciphertext: Buffer.from("sealed note bytes"),
};

const shieldTx = buildShieldTransaction({
  chainId: "test-chain",
  authority,
  shield: {
    assetDefinitionId: "rose#wonderland",
    fromAccountId: authority,
    amount: "10",
    noteCommitment: Buffer.alloc(32, 0xaa),
    encryptedPayload,
  },
  privateKey,
});

const transferTx = buildZkTransferTransaction({
  chainId: "test-chain",
  authority,
  transfer: {
    assetDefinitionId: "rose#wonderland",
    inputs: [Buffer.alloc(32, 0x01)],
    outputs: [Buffer.alloc(32, 0x02)],
    proof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof-bytes", "base64"),
      verifyingKeyRef: "halo2/ipa:vk_transfer",
    },
  },
  privateKey,
});
```

`ProofAttachmentInput` also accepts inline verifying keys via
`verifyingKeyInline`, plus optional `verifyingKeyCommitment` digests. Election
builders (`buildCreateElectionTransaction`, `buildSubmitBallotTransaction`, and
`buildFinalizeElectionTransaction`) share the same helpers so ballot ciphertexts
and Halo2 proofs stay canonical across SDKs. See `index.d.ts` for the
full set of confidential input shapes.

Verifying-key registry helpers mirror the Torii app API (`/v1/zk/vk/*`). Typed
helpers normalise casing and payload layouts so tests and automation can inspect
registry state without manual parsing:

```js
const torii = new ToriiClient("http://localhost:8080");
const list = await torii.listVerifyingKeysTyped({ backend: "halo2/ipa", status: "active" });
console.log(list[0]?.record?.commitment_hex);
for await (const item of torii.iterateVerifyingKeys({ backend: "halo2/ipa", pageSize: 1 })) {
  console.log(item.id.name);
}

const detail = await torii.getVerifyingKeyTyped("halo2/ipa", "vk_main");
console.log(detail.record.status); // "Active"

await torii.registerVerifyingKey({
  authority: "alice@wonderland",
  private_key: "ed0120…",
  backend: "halo2/ipa",
  name: "vk_main",
  version: 1,
  circuit_id: "halo2/ipa::transfer_v1",
  public_inputs_schema_hash_hex: "0x…",
  gas_schedule_id: "halo2_default",
  vk_bytes: Buffer.from("vk-bytes"),
});
```

## Connect Session Utilities

Connect overlays can now be bootstrapped directly from JS. The SDK exposes JSON
helpers alongside the existing WebSocket utilities so dApps can mint session
ids, preview deeplinks, request tokens, and read Connect status in the same code
path:

```js
import {
  ToriiClient,
  createConnectSessionPreview,
  bootstrapConnectPreviewSession,
} from "@iroha/iroha-js";

const torii = new ToriiClient("http://localhost:8080");
const connectStatus = await torii.getConnectStatus();
if (!connectStatus) {
  throw new Error("Connect is disabled on this node");
}
console.log(
  `connect sessions=${connectStatus.sessionsActive}/${connectStatus.sessionsTotal}`,
);
console.log(`relay status: ${connectStatus.policy?.relayEnabled ? "on" : "off"}`);

const preview = createConnectSessionPreview({
  chainId: "test-chain",
  node: "torii.devnet.example",
});
console.log(preview.walletUri); // iroha://connect?sid=...
console.log(preview.appUri); // iroha://connect/app?sid=...

const session = await torii.createConnectSession({
  sid: preview.sidHex,
  node: preview.node,
});

console.log(`tokens app=${session.token_app} wallet=${session.token_wallet}`);

// Or run the preview + registration flow in one step:
const { preview: bundledPreview, session: bundledSession } =
  await bootstrapConnectPreviewSession(torii, {
    chainId: "test-chain",
    node: "torii.devnet.example",
    // override Torii node used during registration if needed:
    sessionOptions: { node: "torii.devnet.backup" },
  });
console.log(bundledPreview.walletUri);
console.log(`Connect session registered with tokens:`, bundledSession?.token_wallet);
```

> **Note:** `sid` must encode exactly 32 bytes as either hexadecimal (with or without
> the `0x` prefix) or base64url per the Connect configuration. `createConnectSessionPreview`
> and `generateConnectSid` enforce the hashing rules described in `iroha_connect.md` so you
> don't need to hand-roll padding or domain separation.

### Connect registry administration

Platform teams can now manage Connect registry state directly from Node.js. The
client surfaces pagination helpers plus policy and manifest mutations so CI can
keep the overlay in sync with governance:

```js
const apps = await torii.listConnectApps({ limit: 10 });
const calc = apps.items.find((entry) => entry.appId === "calc.wallet");

const allAppIds = [];
for await (const app of torii.iterateConnectApps({ pageSize: 25 })) {
  allAppIds.push(app.appId);
}
console.log("connect registry apps:", allAppIds.join(", "));

await torii.registerConnectApp({
  appId: "calc.wallet",
  displayName: "Calc Wallet",
  namespaces: ["apps"],
  metadata: { website: "https://calc.example" },
  policy: { allow_guardian: true },
});

if (calc) {
  await torii.deleteConnectApp(calc.appId);
}

const policy = await torii.getConnectAppPolicy();
await torii.updateConnectAppPolicy({ ...policy, relayEnabled: true });

const manifest = await torii.getConnectAdmissionManifest();
await torii.setConnectAdmissionManifest({
  ...manifest,
  entries: manifest.entries.map((entry) => ({
    ...entry,
    namespaces: [...entry.namespaces, "preview"],
  })),
});
```

### Connect retry policy

`ConnectRetryPolicy` mirrors the Rust `connect_retry::policy` helper so browser and Node.js
clients share the same exponential back-off with full jitter (base 5 s, cap 60 s). Feed the
Connect session identifier into `delayMillis()` to derive deterministic jitter that matches
the Swift and Android SDKs:

```js
import { ConnectRetryPolicy } from "@iroha/iroha-js";

const sessionId = crypto.getRandomValues(new Uint8Array(32));
const retry = new ConnectRetryPolicy();
for (let attempt = 0; attempt < 5; attempt += 1) {
  const delayMs = retry.delayMillis(attempt, sessionId);
  await new Promise((resolve) => setTimeout(resolve, delayMs));
  await reconnect();
}
```

Using a shared seed/attempt sequence keeps telemetry, dashboards, and dApp behaviour aligned across SDKs.

### Connect WebSocket sessions

Once a session is registered you can dial `/v1/connect/ws` without hand-building the query
parameters. `ToriiClient.openConnectWebSocket()` derives the canonical URL (switching
`http→ws`/`https→wss`) and instantiates whichever WebSocket implementation you provide.
In browsers the global `WebSocket` is used automatically; in Node.js pass a constructor such as
[`ws`](https://github.com/websockets/ws):

```js
import WebSocket from "ws";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const session = await torii.createConnectSession({ sid: preview.sidHex });

const socket = torii.openConnectWebSocket({
  sid: session.sid,
  role: "wallet",
  token: session.token_wallet,
  protocols: ["iroha-connect"],
  // For Node/`ws`, headers are attached automatically; provide websocketOptions to add your own.
  websocketOptions: { headers: { "x-debug": "1" } },
  WebSocketImpl: WebSocket,
});

socket.on("open", () => console.log("Connect WS ready"));
socket.on("message", (data) => console.log("frame", data));
```

When you only need the canonical URL, call `ToriiClient.buildConnectWebSocketUrl()` (or the
top-level `buildConnectWebSocketUrl(baseUrl, { sid, role, token })`) and hand it to your own
WebSocket/queue implementation. Tokens are carried via `Authorization: Bearer` headers by default,
and browser clients automatically attach a `Sec-WebSocket-Protocol: iroha-connect.token.v1.<b64url(token)>`
marker so secrets stay out of referrers. The module also exports `openConnectWebSocket(options)` which accepts an
explicit `baseUrl` for cases where you are not holding a `ToriiClient` instance. Both helpers reject
endpoint host/protocol overrides when a token is present (to prevent cross-host leaks) and will only
dial insecure `ws://` URLs when `allowInsecure: true` is set. When you call
`torii.openConnectWebSocket()` the `allowInsecure` flag and `insecureTransportTelemetryHook` are
inherited from the client config; standalone calls can supply their own
`insecureTransportTelemetryHook` to log or alert on insecure opt-ins during local development. Notes:

- Keep endpoint hosts/schemes aligned with the Torii base; credentialed calls reject overrides.
- Enable telemetry hooks to detect accidental `ws://` usage during development.

### Offline pipeline envelopes

Build and persist deterministic Norito envelopes when Torii is unreachable. The helper normalises
metadata, enforces non-empty hashes/schema/key aliases, and computes `hashHex` with
`hashSignedTransaction` when omitted.

```js
import {
  ToriiClient,
  buildMintAssetInstruction,
  buildOfflineEnvelope,
  buildTransaction,
  readOfflineEnvelopeFile,
  replayOfflineEnvelope,
  writeOfflineEnvelopeFile,
} from "@iroha/iroha-js";

const { signedTransaction } = buildTransaction({
  chainId: "offline-demo",
  authority: "alice@wonderland",
  instructions: [
    buildMintAssetInstruction({
      assetId: "rose#wonderland#alice",
      quantity: "10",
    }),
  ],
  privateKey: Buffer.alloc(32, 0x11),
});

const envelope = buildOfflineEnvelope({
  signedTransaction,
  keyAlias: "alice-key",
  metadata: { purpose: "offline-demo" },
});
await writeOfflineEnvelopeFile("./artifacts/js/offline/envelope.json", envelope);

const stored = await readOfflineEnvelopeFile("./artifacts/js/offline/envelope.json");
const torii = new ToriiClient("https://torii.devnet.example");
await replayOfflineEnvelope(torii, stored, { intervalMs: 250, timeoutMs: 15_000 });
```

`npm run example:offline:pipeline` runs the same flow against an in-process mock Torii server and
logs both the computed hash and what the server observes for evidence capture.

### Offline Connect queueing and evidence

The SDK ships a bounded offline journal (`ConnectQueueJournal`) plus diagnostics
helpers for snapshotting queue depth/bytes and exporting evidence bundles when
wallets or Torii endpoints are unavailable. Queues prune expired frames, cap
record counts/bytes per direction, and prefer IndexedDB in browsers while
falling back to memory in Node.js. Diagnostics resolve `connect.queue.root`
from `client.toml` (defaulting to `~/.iroha/connect/<sid>/`) and only honour
`IROHA_CONNECT_QUEUE_ROOT` when `allowEnvOverride: true` (dev/test only); pass
`connectConfig` or `rootDir` explicitly to mirror the config.

```js
import {
  ConnectQueueJournal,
  ConnectDirection,
  appendConnectQueueMetric,
  exportConnectQueueEvidence,
  updateConnectQueueSnapshot,
} from "@iroha/iroha-js";

const sid = preview.sidHex;
const journal = new ConnectQueueJournal(sid, { storage: "memory" });
await journal.append(
  ConnectDirection.APP_TO_WALLET,
  1,
  Buffer.from("app->wallet payload"),
);
const pending = await journal.records(ConnectDirection.APP_TO_WALLET);
console.log("pending frames", pending.length);

const snapshot = await updateConnectQueueSnapshot(
  sid,
  (current) => ({
    ...current,
    state: "enabled",
    app_to_wallet: { ...current.app_to_wallet, depth: pending.length, bytes: 128 },
    wallet_to_app: { ...current.wallet_to_app, depth: 0, bytes: 0 },
  }),
  { rootDir: "./artifacts/js/connect_offline" },
);
await appendConnectQueueMetric(
  sid,
  {
    state: snapshot.state,
    app_to_wallet_depth: snapshot.app_to_wallet.depth,
    wallet_to_app_depth: snapshot.wallet_to_app.depth,
  },
  { rootDir: "./artifacts/js/connect_offline" },
);
const { manifest } = await exportConnectQueueEvidence(
  sid,
  "./artifacts/js/connect_bundle",
  { rootDir: "./artifacts/js/connect_offline" },
);
console.log("evidence files", manifest.files);
```

See `docs/source/sdk/js/offline.md` for the full snapshot layout and CLI usage
(`npm run example:offline` for Connect, `npm run example:offline:pipeline` for pipeline envelopes)
when you need deterministic replay/evidence capture.

### Connect error taxonomy

`ConnectError`, `ConnectQueueError`, and `connectErrorFrom()` mirror the shared taxonomy
documented in [`docs/source/connect_error_taxonomy.md`](../../docs/source/connect_error_taxonomy.md).
Wrap every failure that bubbles up from the Connect transport (WebSocket, fetch, codecs, queue)
before emitting telemetry so dashboards can rely on consistent `category`/`code` pairs:

```js
import {
  ConnectQueueError,
  connectErrorFrom,
} from "@iroha/iroha-js";
import { telemetry } from "./telemetry.js";

try {
  await queue.enqueue(frame);
} catch (error) {
  const connectError = connectErrorFrom(error);
  telemetry.emit("connect.error", connectError.telemetryAttributes({ fatal: true }));
  throw connectError;
}

const overflow = ConnectQueueError.overflow(256);
const attrs = overflow.toConnectError().telemetryAttributes();
console.log(attrs.category); // "queueOverflow"
console.log(attrs.code); // "queue.overflow"
```

`connectErrorFrom()` inspects HTTP status codes, Node.js error codes (TLS, socket, timeout),
`DOMException` names, and codec failures so Connect clients do not need bespoke switch statements.
If you implement a custom error type, expose `toConnectError()` and return a `ConnectError`
instance; the helper will pass it through unchanged.

### Connect queue journal

Use `ConnectQueueJournal` to persist Connect queue entries inside the browser.
The journal mirrors the Swift/Android file layout: entries are encoded as
`ConnectJournalRecordV1` Norito blobs, session identifiers are hashed with SHA-256,
and a background retention policy prunes expired or excess entries.

```js
import {
  ConnectDirection,
  ConnectQueueJournal,
} from "@iroha/iroha-js";

const journal = new ConnectQueueJournal(preview.sidBase64Url, {
  maxRecordsPerQueue: 32,
  maxBytesPerQueue: 1 << 20,
  storage: "auto", // use IndexedDB when available, fall back to memory otherwise
});

await journal.append(
  ConnectDirection.APP_TO_WALLET,
  frame.sequence,
  frame.ciphertext,
  { ttlMs: 60_000 },
);

const pending = await journal.records(ConnectDirection.APP_TO_WALLET);
const drained = await journal.popOldest(ConnectDirection.APP_TO_WALLET, 1);
```

When IndexedDB is unavailable (private browsing or unsupported browser),
the journal automatically falls back to an in-memory store; the last error is
exposed via `journal.fallbackError`. Applications can inspect `journal.sessionKey`
to derive deterministic evidence paths.

### Connect queue diagnostics

Queue diagnostics helpers mirror the new `iroha connect queue inspect` CLI workflow so Node.js
automation can persist the same telemetry/evidence bundles produced by Swift/Android tooling.

```js
import {
  appendConnectQueueMetric,
  exportConnectQueueEvidence,
  readConnectQueueSnapshot,
  updateConnectQueueSnapshot,
} from "@iroha/iroha-js";

const sid = preview.sidBase64Url;
await updateConnectQueueSnapshot(
  sid,
  (snapshot) => ({
    ...snapshot,
    state: "throttled",
    reason: "disk_watermark",
    app_to_wallet: { ...snapshot.app_to_wallet, depth: 12 },
  }),
);

await appendConnectQueueMetric(sid, {
  state: "throttled",
  app_to_wallet_depth: 12,
  wallet_to_app_depth: 3,
  reason: "disk_watermark",
});

const { manifest, targetDir } = await exportConnectQueueEvidence(sid, "./artifacts/connect-queue");
console.log(`Evidence bundle for ${manifest.session_id_base64} written to ${targetDir}`);
```

Operators can then run `iroha connect queue inspect --sid <sid> --root ~/.iroha/connect --metrics`
to print the same snapshot/telemetry summary captured above.

## Config Introspection

`extractToriiFeatureConfig()` normalises the ISO bridge, RBC sampling, and
Connect sections from a parsed `iroha_config`. It performs light validation,
renames fields into camelCase, and surfaces optional signer metadata so
dashboards or CLIs can display feature state without manual JSON parsing.

`extractConfidentialGasConfig()` returns the confidential verification gas schedule
(`proofBase`, `perPublicInput`, `perProofByte`, `perNullifier`, `perCommitment`) so tooling can
surface node gas policy without spelunking raw JSON.

```js
import { ToriiClient } from "@iroha/iroha-js";

const client = new ToriiClient("http://localhost:8080");
const gas = await client.getConfidentialGasSchedule();
if (gas) {
  console.log(`Proof base cost: ${gas.proofBase}`);
}
```

### Configuration snapshots

`getConfigurationTyped()` returns the `/v1/configuration` payload with typed fields so automation
can record logger/network queue settings without hand-parsing JSON.

```js
const torii = new ToriiClient("http://localhost:8080");
const snapshot = await torii.getConfigurationTyped();
if (snapshot) {
  console.log("Node key:", snapshot.publicKeyHex);
  console.log("Block gossip size:", snapshot.network.blockGossipSize);
  if (snapshot.queue) {
    console.log("Queue capacity:", snapshot.queue.capacity);
  }
  if (snapshot.confidentialGas) {
    console.log("Conf gas per nullifier:", snapshot.confidentialGas.perNullifier);
  }
  if (snapshot.transport?.streaming?.soranet) {
    console.log("SoraNet Norito exit:", snapshot.transport.streaming.soranet.exitMultiaddr);
  }
}
```

### Runtime and capability helpers

`ToriiClient` now covers the runtime capability endpoints so SDK consumers can surface ABI
versioning data without crafting raw HTTP calls. Use `getNodeCapabilities()` to inspect ABI
support and cryptography acceleration flags, `getRuntimeAbiActive()`/`getRuntimeAbiHash()` to
mirror the compiler guardrails, `getRuntimeMetrics()` for aggregate counters, and
`listRuntimeUpgrades()` to page through recorded manifests. The helper trio
`proposeRuntimeUpgrade()`, `activateRuntimeUpgrade()`, and `cancelRuntimeUpgrade()` post the
runtime JSON endpoints and return transaction skeletons (`wire_id` + payload hex) that you can
sign via `TxBuilder`, so rollout automation no longer needs bespoke HTTP clients.

```js
const torii = new ToriiClient("http://localhost:8080");
const caps = await torii.getNodeCapabilities();
console.log("Active ABI versions", caps.supportedAbiVersions);
console.log("Allowed curve IDs", caps.crypto.curves.allowedCurveIds);
console.log("Allowed curve bitmap", caps.crypto.curves.allowedCurveBitmap);

const abi = await torii.getRuntimeAbiActive();
console.log(`Default compile target: ABI v${abi.defaultCompileTarget}`);

const upgrades = await torii.listRuntimeUpgrades();
for (const item of upgrades) {
  console.log(`${item.idHex} -> ${item.record.status.kind}`);
}

const manifest = {
  name: "ABI v5",
  description: "Enable new syscalls",
  abiVersion: 5,
  abiHash: "0123...cdef",
  startHeight: 10_000,
  endHeight: 10_500,
};
const draft = await torii.proposeRuntimeUpgrade(manifest);
console.log(draft.tx_instructions);
```

### Network time helpers

`getNetworkTimeNow()` mirrors `/v1/time/now` so you can validate the network timestamp, offset,
and confidence window exposed by the node. `getNetworkTimeStatus()` wraps `/v1/time/status` and
returns the peer sampling plus RTT histogram that the NRPC/AND7 runbooks consume.

```js
const torii = new ToriiClient("http://localhost:8080");
const ntsNow = await torii.getNetworkTimeNow();
console.log(`cluster time=${ntsNow.timestampMs} offset=${ntsNow.offsetMs}ms`);

const status = await torii.getNetworkTimeStatus();
for (const sample of status.samples) {
  console.log(sample.peer, sample.lastOffsetMs, sample.lastRttMs, sample.count);
}
console.log("histogram", status.rtt.buckets);
```
```

```js
import { AccountAddress } from "@iroha/iroha-js";

const address = AccountAddress.fromAccount({
  domain: "default",
  publicKey: new Uint8Array(32),
});
console.log(address.canonicalHex());
console.log(address.toIH58(753));
console.log(address.toCompressedSora());
```

```js
import { extractToriiFeatureConfig } from "@iroha/iroha-js";

const config = JSON.parse(fs.readFileSync("iroha_config.json", "utf8"));
const features = extractToriiFeatureConfig({ config });

if (features.isoBridge?.enabled) {
  console.log(`Aliases: ${features.isoBridge.accountAliases.length}`);
}
if (features.rbcSampling?.enabled) {
  console.log(`RBC budget: ${features.rbcSampling.dailyByteBudget} bytes/day`);
}
```

## Continuous Integration

- See `docs/source/sdk/js/quickstart.md` for an expanded walkthrough covering key management, transaction assembly, Torii configuration, and CI tips.

- Cache both `npm` and `cargo` directories so native bindings rebuild quickly across matrix runs.
- Run `npm run lint:test` before the dockerised integration job. The script enforces ESLint with zero warnings, builds the native addon, and runs the Node test suite so the JS-10 gate matches what the publish workflow executes.
- Prefer Node LTS releases (currently 18 and 20) alongside the `rust-toolchain.toml` version to minimise drift across environments.
- Use `node --test` for quick smoke runs when native artifacts are already built (for example after `npm run build:native` in a cached workspace); keep `npm run lint:test` in CI to cover the full pipeline.
- Layer any project-specific linting or formatting checks on top of `npm run lint:test` if your monorepo enforces stricter policies.
- See `docs/source/examples/iroha_js_ci.md` for extended guidance and optional smoke-job templates.

```yaml
name: iroha-js-ci

on:
  push:
    branches: [ main ]
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        node-version: [18, 20]
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: npm

      - name: Set up Rust toolchain
        uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable

      - name: Cache cargo build artifacts
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - run: npm install
      - run: npm run build:native
      - run: npm test
```

## Integration Smoke Tests

`test/integrationTorii.test.js` exercises a live Torii node when the relevant
environment variables are set. The suite is skipped by default so CI can run
without provisioning infrastructure.

- `IROHA_TORII_INTEGRATION_URL` — Torii base URL (required to enable the test).
- `IROHA_TORII_INTEGRATION_API_TOKEN` — optional API token for secured nodes.
- `IROHA_TORII_INTEGRATION_AUTH_TOKEN` — optional bearer token for auth-protected deployments.
- `IROHA_TORII_INTEGRATION_CONFIG` — optional path to an `iroha_config` JSON file; when present the test asserts that `extractToriiFeatureConfig()` normalises ISO bridge, RBC sampling, and Connect settings.
- `IROHA_TORII_INTEGRATION_RBC_SAMPLE` — optional JSON string (e.g., `{"blockHash":"...","height":1,"view":0}`) forwarded to `sampleRbcChunks()`; when unset the integration suite auto-selects a delivered RBC session via `findRbcSamplingCandidate()`. Supplying an explicit payload forces the test to use that session instead.
- `IROHA_TORII_INTEGRATION_CONNECT_SESSION` — optional JSON string containing the payload for `createConnectSession()` (`{"sid":"<hex>","node":"torii.devnet.example"}` is a common pattern).
- `IROHA_TORII_INTEGRATION_CONNECT_PREVIEW` — optional JSON object consumed by the Connect preview bootstrapper test (`{"node":"torii.devnet.example","sessionOptions":{"node":"ingress.devnet.example"}}` is sufficient). When present and `IROHA_TORII_INTEGRATION_MUTATE=1`, the suite calls `bootstrapConnectPreviewSession()`, validates the deeplink URIs/tokens, and deletes the staged session.
- `IROHA_TORII_INTEGRATION_CONNECT_APP` — optional JSON object describing a Connect app registration payload (`{"appId":"demo","namespaces":["apps"],"metadata":{"suite":"ci"}}`); when present and `IROHA_TORII_INTEGRATION_MUTATE=1`, the suite registers the app, verifies that list/get/iterator APIs return it, and then deletes it.
- `IROHA_TORII_INTEGRATION_CONTRACT_CALL` — optional JSON object describing a contract call payload (for example: `{"namespace":"apps","contractId":"calc.v1","entrypoint":"ping","payload":{"value":1}}`). When supplied alongside `IROHA_TORII_INTEGRATION_MUTATE=1`, the suite invokes `ToriiClient.callContract`, waits for the resulting transaction status, and asserts success. The helper accepts camelCase keys plus overrides for `authority`, `privateKeyHex`, `gasAssetId`, and `gasLimit`.
- `IROHA_TORII_INTEGRATION_GOV_BALLOT` — optional JSON object ({`referendumId`,`owner`,`amount`,`durationBlocks`,`direction`} are the common keys) submitted via `governanceSubmitPlainBallot` when `IROHA_TORII_INTEGRATION_MUTATE=1`. Missing fields default to the configured `authority`/`chainId`, so the env var only needs to override vote-specific fields.
- `IROHA_TORII_INTEGRATION_CHAIN_ID` — optional override for the default devnet chain id (`00000000-0000-0000-0000-000000000000`).
- `IROHA_TORII_INTEGRATION_ACCOUNT_ID` / `IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX` — optional overrides for the default signer (`defaults/client.toml`); the defaults target `ed0120CE…@wonderland`.
- `IROHA_TORII_INTEGRATION_MUTATE` — set to `1` to enable mutation tests (registering disposable domains via the builder helpers). The docker harness described below enables this flag automatically.
- `IROHA_TORII_INTEGRATION_STREAM_ENABLED` — set to `1` (alongside `IROHA_TORII_INTEGRATION_MUTATE=1`) to exercise the event-stream coverage that waits for a `Pipeline.Block` SSE and asserts the typed payload mirrors Torii’s stream schema. Leave unset when SSE endpoints are disabled or proxied away.
- `IROHA_TORII_INTEGRATION_ISO_ENABLED` — set to `1` to exercise the ISO bridge smoke test (submits a tiny `pacs.008` payload and fetches its status). Leave unset/`0` to skip the ISO coverage when the bridge runtime is disabled.
- `IROHA_TORII_INTEGRATION_ISO_PACS008` — optional JSON object merged into the default ISO builder fields (useful for overriding BICs/amounts/message IDs when replaying production fixtures).
- `IROHA_TORII_INTEGRATION_ISO_PACS009` — optional JSON object merged into the default pacs.009 builder fields (same structure as the pacs.008 overrides; handy for replaying RTGS transfers with custom identifiers).
- `IROHA_TORII_INTEGRATION_ISO_ALIAS` — optional ISO alias (for example, `GB82 WEST 1234 5698 7654 32`) used by the alias-resolution integration test. Set alongside `IROHA_TORII_INTEGRATION_ISO_ENABLED=1` when the ISO runtime is active.
- `IROHA_TORII_INTEGRATION_ISO_ALIAS_INDEX` — optional deterministic index (integer) for exercising `resolveAliasByIndex`. Provide this when the target node exposes indexed alias metadata so the integration suite can cover both alias endpoints.
- `IROHA_TORII_INTEGRATION_ISO_VOPRF` — optional hex string forwarded to the alias VOPRF helper coverage (defaults to `deadbeef`); useful when replaying captured transcripts that require specific blinded elements.
- `IROHA_TORII_INTEGRATION_SORAFS_ENABLED` — set to `1` to run the optional SoraFS registry/storage smoke test (lists manifests/aliases/replication orders and fetches the storage state). Leave unset/`0` when SoraFS endpoints are disabled on the target node.
- `IROHA_TORII_INTEGRATION_SORAFS_POR_WEEK` — optional ISO week label such as `2026-W05`. When set alongside `IROHA_TORII_INTEGRATION_SORAFS_ENABLED=1`, the suite fetches the PoR weekly report for that week to exercise the Norito export path.
- `IROHA_TORII_INTEGRATION_UAID` — optional canonical UAID literal (for example `uaid:0f4d…`). When provided, the integration suite exercises the UAID portfolio/bindings/manifests endpoints so cross-dataspace APIs stay covered.
- `IROHA_TORII_INTEGRATION_UAID_DATASPACE` — optional dataspace id (non-negative integer) used to scope the UAID manifest request when `IROHA_TORII_INTEGRATION_UAID` is set. Leave unset to fetch manifests across every dataspace.
- `IROHA_TORII_INTEGRATION_SNS_SUFFIX` — optional SNS suffix id (u16) used to fetch the suffix policy snapshot. Supply alongside `IROHA_TORII_INTEGRATION_URL` to exercise the SNS policy smoke test.
- `IROHA_TORII_INTEGRATION_SNS_SELECTOR` — optional canonical name selector (for example `wonderland.sora`) used to fetch an SNS registration record.
- `IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_ENABLED` — set to `1` (alongside `IROHA_TORII_INTEGRATION_MUTATE=1`) to run the Space Directory manifest publish/revoke smoke tests. Supply a manifest JSON path via `IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_MANIFEST` (absolute or relative to the repo root; for example `fixtures/space_directory/capability/retail_dapp_access.manifest.json`). Optional overrides: `IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_REVOKE_EPOCH=<epoch>` to force the revoke call to use a specific epoch when your fixture omits `expiry_epoch`.
- `IROHA_TORII_INTEGRATION_DA_ENABLED` — set to `1` (and enable `IROHA_TORII_INTEGRATION_MUTATE=1`) to exercise the data-availability ingest smoke test (`submitDaBlob` + manifest polling). Leave unset when the DA ingest pipeline is disabled on the target Torii deployment.
- `IROHA_TORII_INTEGRATION_DA_TICKET` — optional hex-encoded storage ticket used to fetch an existing manifest bundle when DA endpoints are read-only or when you want to validate a production capture without submitting a new blob.
- `IROHA_TORII_INTEGRATION_DA_GATEWAYS` — optional JSON array describing the gateway providers used by `fetchDaPayloadViaGateway` (for example `[{"name":"gw-a","providerIdHex":"…","baseUrl":"https://gw-a.example","streamTokenB64":"..."}]`). Supply this alongside `IROHA_TORII_INTEGRATION_DA_TICKET` to stream proofs through the multi-source orchestrator.

Example invocation:

```bash
IROHA_TORII_INTEGRATION_URL=http://localhost:8080 \
IROHA_TORII_INTEGRATION_API_TOKEN=dev-token \
node --test javascript/iroha_js/test/integrationTorii.test.js
```

### Dockerised harness (`npm run test:integration`)

Use the bundled integration harness to spin up the single-node Docker Compose
topology, wait for `/status`, and run the mutation-enabled smoke suite:

```bash
npm run test:integration
```

`scripts/run_integration.mjs` performs the following steps:

1. Runs `npm ci` (skip via `JS_TORII_SKIP_INSTALL=1`) and rebuilds the native binding.
2. Starts `docker compose -f defaults/docker-compose.single.yml up -d irohad0`
   unless `--no-start` (or `JS_TORII_START=0`) is supplied.
3. Waits up to 90 s for `http://127.0.0.1:8080/status` (override via
   `--torii-url`/`--wait-seconds`/`IROHA_TORII_INTEGRATION_URL`).
4. Sets the mutation env vars (chain id, account id, private key) and runs
   `node --test test/integrationTorii.test.js`.
5. Tears the compose stack down (`down --remove-orphans`) on success or failure.

Flags/environment variables:

- `--compose-file` (or `JS_TORII_COMPOSE_FILE`) to point at a custom compose manifest.
- `--service` / `COMPOSE_SERVICE` to target a different service name.
- `--compose-bin` / `JS_TORII_COMPOSE_BIN` to use a non-default compose command.
- `--no-start` to reuse an existing node (the harness still waits for `/status`).
- Pass additional `node --test` arguments after `--`, for example:

  ```bash
  npm run test:integration -- -- --test-name-pattern=torii
  ```
- `--enable-iso` (or `JS_TORII_ENABLE_ISO=1`) to flip on the ISO bridge smoke tests without
  setting `IROHA_TORII_INTEGRATION_ISO_ENABLED` manually. Combine with
  `--iso-alias <alias>`/`--iso-alias-index <index>` to pre-populate the ISO alias inputs used by
  `resolveAlias`/`resolveAliasByIndex`, and `--iso-pacs008 <json-or-path>` /
  `--iso-pacs009 <json-or-path>` to feed override payloads into the builders. The JSON arguments
  accept inline objects or filesystem paths (absolute or relative to the repo root); the harness
  validates and forwards the resulting string to
  `IROHA_TORII_INTEGRATION_ISO_PACS008`/`IROHA_TORII_INTEGRATION_ISO_PACS009`.

Each run registers a fresh domain (prefixed `jsintegration-…`) so repeated
executions remain deterministic. Clean up by truncating the devnet database or
recreating the Docker stack.

With `IROHA_TORII_INTEGRATION_MUTATE=1`, the suite now:

1. Registers a disposable domain, account, and asset definition.
2. Mints and re-mints the asset, transfers balances via the iterator helpers,
   and queries the relevant lists through both `/list` and `/query` endpoints.
3. Optionally submits a `pacs.008` message (when `IROHA_TORII_INTEGRATION_ISO_ENABLED=1`)
   to verify the bridge pipeline end-to-end.
4. Optionally inspects the SoraFS pin registry (when `IROHA_TORII_INTEGRATION_SORAFS_ENABLED=1`),
   ensuring the alias/replication lists and storage state endpoints respond with typed payloads.
5. Optionally submits a DA ingest payload and polls the manifest endpoint (when
   `IROHA_TORII_INTEGRATION_DA_ENABLED=1`), and can stream multi-source fetch
   evidence when `IROHA_TORII_INTEGRATION_DA_GATEWAYS`/`IROHA_TORII_INTEGRATION_DA_TICKET`
   are set.
6. Optionally listens for a `Pipeline.Block` event (when
   `IROHA_TORII_INTEGRATION_STREAM_ENABLED=1`) to prove the streaming helpers stay in lockstep
   with Torii’s SSE payloads before the ISO/SoraFS/DA suites run.

## Iterable Lists & Pagination

`ToriiClient` now exposes helpers for the app-facing JSON list endpoints. They
mirror the Python SDK ergonomics: each `list*` method accepts `limit`, `offset`,
`filter`, and `sort` plus an optional `signal`, and returns `{ items, total }`.
The `iterate*` variants automatically advance the offset so you can traverse the
entire collection without manual bookkeeping. Every collection that also exposes
`/query` endpoints has a matching `iterate*Query` helper so you can apply
structured filters and projection rules without managing pagination cursors
yourself.

Alongside accounts/domains/asset definitions, the helpers now cover NFTs,
per-account asset balances, asset-definition holder lists, account
transaction history, the contract/gov instance registries, and both list/query
trigger surfaces so SDK consumers can reuse the same pagination ergonomics
across Torii's JSON endpoints (including query projections via
`iterateAccountsQuery`, `iterateDomainsQuery`, `iterateAssetDefinitionsQuery`,
`iterateNftsQuery`, `iterateAccountAssetsQuery`,
`iterateAccountTransactionsQuery`, `iterateAssetHoldersQuery`, and
`iterateTriggersQuery`).

When you need to pin iterator parity to specific Norito selectors, apply
structured filters against the NFT definition (`id.definition_id`) or asset
definition (`asset_id.definition_id`) fields and trim payloads with `select`
projections; see `recipes/nft_account_iteration.mjs` for a runnable example
that requests compressed account literals for downstream storage.

```js
const { items, total } = await torii.listAccounts({
  limit: 5,
  sort: [{ key: "id", order: "asc" }],
});
console.log("first five accounts", items.map((item) => item.id), "of", total);

const compressed = await torii.listAccounts({
  limit: 3,
  addressFormat: "compressed",
});
console.log("compressed literals", compressed.items.map((item) => item.id));

All iterable list/query helpers now require the `options` argument to be a
plain object. Passing primitives, arrays, or class instances throws a
`TypeError` before any HTTP call, keeping the JS-04 validation guarantees aligned
with the Rust/Python SDKs.

`addressFormat` accepts `"compressed"` to request the snx1 form or `"ih58"`/`"canonical"`
to stick with the multihash default; any other value is rejected before the HTTP call.

All pagination knobs (`limit`, `offset`, `pageSize`, `maxItems`, `fetchSize`) accept the
`NumericLike` inputs used across the transaction builders (`number`, `string`, or `bigint`).
They are normalised via the same unsigned-integer validators before any request fires, so
passing `"25"` or `10n` behaves exactly like `25` while still surfacing a `TypeError` when
the value is negative, NaN, or otherwise invalid.

Offline allowances and transfers reuse the same ergonomics. The helpers wrap the
`/v1/offline/allowances` and `/v1/offline/transfers` list/query endpoints so
monitoring agents can inspect registered wallet certificates and queued bundle
submissions without bespoke pagination logic.

Allowance entries surface the ledger metadata directly on each item —
`expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
`attestation_nonce_hex`, and `remaining_amount` — so dashboards do not have to
drill into the embedded record to understand refresh cadence or verdict
bindings. The new `integrity_metadata` field mirrors the Android policy slug
(`policy`) and, when Provisioned allowances are in play, surfaces the inspector
public key plus manifest schema/version/TTL/digest under
`integrity_metadata.provisioned`. Dashboards and telemetry exporters can now
bucket inventory by `policy` without parsing raw metadata maps.

Transfer rows expose the Android policy snapshot as top-level JSON so agents can
differentiate marker-key, Play Integrity, HMS Safety Detect, or Provisioned
allowances without decoding the raw Norito payload. Inspect `platform_policy`
or the new `integrity_metadata.policy` helper to bucket bundles by provider and
`platform_token_snapshot.attestation_jws_b64` when you need to persist or audit
the inspector-issued JWS backing a Provisioned manifest; when Provisioned is in
use the inspector metadata appears under
`integrity_metadata.provisioned`.

`status_transitions` mirrors validator lifecycle events (Settled, Archived,
Pruned) and each entry now carries the same `verdict_snapshot` captured during
settlement, so POS importers can correlate attestation metadata with every
state change without scraping the top-level record. When `verdict_snapshot`
appears on either the transition or the parent transfer item, the payload uses
the `hash:...` literal form emitted by Torii (matching the REST API).【crates/iroha_torii/src/routing.rs:26370】【javascript/iroha_js/src/toriiClient.js:12966】

`listOfflineAllowances` also exposes the roadmap-driven convenience filters from
`/v1/offline/allowances` via camelCase options. Set `controllerId`,
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`refreshBeforeMs/AfterMs`, `attestationNonceHex`, `verdictIdHex`,
`requireVerdict`, or `onlyMissingVerdict` to avoid crafting Norito filter
expressions for common reporting workflows; invalid combinations (`verdictIdHex`
+ `onlyMissingVerdict`, or `requireVerdict` + `onlyMissingVerdict`) are rejected
locally before hitting Torii. Transfers share the same ergonomics while adding
`receiverId`, `depositAccountId`, and `platformPolicy` to mirror the GET query
parameters on `/v1/offline/transfers`.

Offline counter summaries expose the aggregate App Attest / Android series map
for each certificate via `listOfflineSummaries`/`queryOfflineSummaries`. These
helpers share the iterable ergonomics described above and now validate filter
expressions before issuing requests — only `controller_id` and
`certificate_id_hex` support equality/membership/exists operators, mirroring the
Torii routing rules. Supplying any other field fails fast with a `TypeError`, so
misconfigured dashboards never reach the network.

Persist counters locally and enforce monotonic increments with
`OfflineCounterJournal`. The journal stores `summary_hash_hex` parity plus
per-scope counters under `~/.iroha/offline_counters/journal.json` by default
(use `storage: "memory"` for ephemeral tests).

```js
import { OfflineCounterJournal, OfflineCounterPlatform } from "@iroha/iroha-js";

const journal = new OfflineCounterJournal();
const summaries = await torii.listOfflineSummaries({ limit: 50 });
await journal.upsert(summaries, { recordedAtMs: Date.now() });

const summary = summaries.items[0];
const nextCounter = (summary.apple_key_counters["AAPLDEMO1"] ?? 0) + 1;
await journal.updateCounter({
  certificateIdHex: summary.certificate_id_hex,
  controllerId: summary.controller_id,
  platform: OfflineCounterPlatform.APPLE_KEY,
  scope: "AAPLDEMO1",
  counter: nextCounter,
});
```

```js
const { items: allowances } = await torii.listOfflineAllowances({ limit: 10 });
console.log(
  "first certificate",
  allowances[0]?.controller_display,
  allowances[0]?.remaining_amount,
  allowances[0]?.verdict_id_hex,
);

for await (const transfer of torii.iterateOfflineTransfersQuery({
  filter: { Eq: ["asset_id", "usd#wonderland"] },
  pageSize: 5,
})) {
  console.log("bundle", transfer.bundle_id_hex, "receipts", transfer.receipt_count);
}

const rejectionStats = await torii.getOfflineRejectionStats({
  telemetryProfile: "full",
});
if (rejectionStats) {
  console.log(
    "offline rejection total",
    rejectionStats.total,
    "top entry",
    rejectionStats.items[0],
  );
} else {
  console.log("telemetry profile forbids offline rejection stats");
}
```

Use `listOfflineRevocations`/`queryOfflineRevocations` (or their iterator
counterparts) to watch verdict revocation records in near real time. The SDK
surfaces the issuer display, revocation reason, optional notes/metadata, and the
raw ledger payload so operators can join the data with audit pipelines:

```js
const revocations = await torii.listOfflineRevocations({
  limit: 5,
  sort: "revoked_at_ms:desc",
  filter: { Eq: ["reason", "compromised_device"] },
});
for (const item of revocations.items) {
  console.log(
    `${item.verdict_id_hex} revoked at ${item.revoked_at_ms} by ${item.issuer_display}`,
    "note:",
    item.note ?? "<none>",
  );
}
```

for await (const assetDef of torii.iterateAssetDefinitions({
  pageSize: 50,
  maxItems: 120,
})) {
  console.log("asset definition:", assetDef.id);
}

const defs = await torii.queryAssetDefinitions({
  filter: { Eq: ["metadata.display_name", "Ticket"] },
  sort: [{ key: "metadata.display_name", order: "desc" }],
  fetchSize: 100,
});
console.log("filtered definitions", defs.items);

const perms = await torii.listAccountPermissions("alice@wonderland", {
  limit: 5,
});
console.log("direct permissions", perms.items.map((item) => item.name));
for await (const perm of torii.iterateAccountPermissions("alice@wonderland", {
  pageSize: 2,
})) {
  console.log("paged permission", perm.name);
}
const nfts = await torii.listNfts({ limit: 10 });
console.log("first NFT ids", nfts.items.map((nft) => nft.id));
const balances = await torii.listAccountAssets("alice@wonderland", { limit: 3 });
console.log("alice balances", balances.items);
const holders = await torii.listAssetHolders("rose#wonderland", { limit: 3 });
console.log("top holders", holders.items.map((entry) => entry.account_id));
const history = await torii.listAccountTransactions("alice@wonderland", { limit: 2 });
console.log(
  "recent hashes",
  history.items.map((tx) => tx.entrypoint_hash),
);

for await (const account of torii.iterateAccountsQuery({
  pageSize: 100,
  filter: { Eq: ["id", "snx1alicepayload@wonderland"] },
  select: [{ Fields: ["id", "metadata.display_name"] }],
})) {
  console.log("matching account", account.id);
}

for await (const balance of torii.iterateAccountAssetsQuery("alice@wonderland", {
  pageSize: 32,
  filter: { Eq: ["asset_id.definition_id", "rose#wonderland"] },
})) {
  console.log("filtered holding", balance.asset_id, balance.quantity);
}

for await (const instance of torii.iterateGovernanceInstances("apps", {
  pageSize: 25,
  contains: "calc",
})) {
  console.log("governance instance:", instance.contract_id);
}

for await (const trigger of torii.iterateTriggersQuery({
  pageSize: 50,
  filter: { Eq: ["object.authority", "alice@wonderland"] },
})) {
  console.log("trigger id:", trigger.id);
}

// Or mirror the same calls from the runnable recipe:
//   node ./recipes/nft_account_iteration.mjs \
//     TORII_URL=http://127.0.0.1:8080 \
//     ACCOUNT_ID=alice@wonderland \
//     ASSET_DEFINITION_ID=rose#wonderland \
//     NFT_DEFINITION_ID=art#wonderland
```

> **Roadmap ADDR-5a:** Account-scoped helpers (`listAccountAssets`, `listAccountPermissions`, `listAccountTransactions`, and their query/iterator variants) now accept canonical, IH58, or compressed literals and automatically percent-encode them when constructing `/v1/accounts/{account_id}/…` routes, so SDK callers can forward whatever selector they surface in wallets without hand-escaping.

Use the SNS helpers to manage Sora Name Service records without hand-crafting JSON:

```js
const policy = await torii.getSnsPolicy(1);
console.log(policy.suffix, policy.pricing.length);

const registration = await torii.registerSnsName({
  selector: { suffix_id: 1, label: "demo" },
  owner: "alice@wonderland",
  payment: {
    asset_id: "xor#sora",
    gross_amount: 120,
    net_amount: 120,
    settlement_tx: { tx: "hash" },
    payer: "alice@wonderland",
    signature: "sig-json",
  },
});
console.log(registration.nameRecord.status.status);
```

Look up an existing name via `getSnsRegistration(selector)`, renew with `renewSnsRegistration`, or transfer/freeze/unfreeze using the corresponding helpers—all return typed DTOs with canonical selectors, statuses, and metadata.

Stream arbitration cases via the SNS governance export feed without hand-rolling cursor
logic:

```js
const openCaseIds = [];
for await (const record of torii.iterateSnsGovernanceCases({
  status: "open",
  limit: 50,
})) {
  openCaseIds.push(record.caseId);
}
console.log("open SNS cases:", openCaseIds);
```

`iterateSnsGovernanceCases` automatically follows the `next_since` cursor exposed by
`/v1/sns/governance/cases`, so SDK callers can page through governance appeals in
chunks without juggling timestamps manually.

## Torii Queries & Events

```js
import { AccountAddress } from "@iroha/iroha-js";

const address = AccountAddress.fromAccount({
  domain: "default",
  publicKey: new Uint8Array(32),
});
console.log(address.canonicalHex());
console.log(address.toIH58(753));
console.log(address.toCompressedSora());
```

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient("http://localhost:8080");

const health = await torii.getHealth();
console.log(health?.status); // e.g. "healthy"

const explorerMetrics = await torii.getExplorerMetrics();
if (explorerMetrics) {
  console.log(
    `finalized block #${explorerMetrics.finalizedBlockHeight} (avg commit ${explorerMetrics.averageCommitTimeMs} ms)`,
  );
} else {
  console.log("explorer metrics disabled on this node");
}

const qr = await torii.getExplorerAccountQr("snx1exampleaddress@wonderland", {
  addressFormat: "compressed",
});
console.log(qr.literal); // compressed literal embedded in the QR SVG
console.log(qr.svg); // inline SVG (192x192) ready to drop into your UI

const block = await torii.getBlock(42);
console.log(block?.height); // null when the block is missing

const recentBlocks = await torii.listBlocks({ limit: 5 });
console.log(
  `returned ${recentBlocks.items.length} of ${recentBlocks.pagination.totalItems} blocks`,
);
for (const entry of recentBlocks.items) {
  console.log(`${entry.hash} rejected=${entry.transactionsRejected}`);
}

// NFT and account-asset iteration mirrors the Torii JSON envelopes while handling pagination.
const holdings = [];
for await (const holding of torii.iterateAccountAssets("alice@wonderland", {
  pageSize: 2,
  maxItems: 5,
  sort: [{ key: "quantity", order: "desc" }],
  addressFormat: "compressed",
})) {
  holdings.push(holding.asset_id);
}
console.log("first holdings page", holdings);

const nftIds = [];
for await (const nft of torii.iterateNftsQuery({
  pageSize: 3,
  maxItems: 4,
  filter: { Contains: ["id", "ticket#"] },
  addressFormat: "ih58",
})) {
  nftIds.push(nft.id);
}
console.log("matching NFTs", nftIds);

const ownedNfts = [];
for await (const nft of torii.iterateAccountNfts("alice@wonderland", {
  domainId: "wonderland",
  pageSize: 10,
})) {
  ownedNfts.push(nft.id);
}
console.log("alice holds NFTs", ownedNfts);

try {
  await torii.listNfts({ limit: 1 });
} catch (error) {
  if (error.code === "permission_denied") {
    console.warn("missing NFT read permission", error.errorMessage);
  } else {
    throw error;
  }
}

// TypeScript users can pass a generic argument to shape `event.data`.
for await (const event of torii.streamEvents({
  filter: { Pipeline: { Block: {} } },
})) {
  console.log(event.event, event.data);
  break; // stop after the first event in this example
}

const governanceInstances = await torii.listGovernanceInstances("apps", {
  contains: "ledger",
  limit: 5,
  hashPrefix: "deadbeef",
  order: "hash_desc",
});
for (const instance of governanceInstances.instances) {
  console.log(`${instance.contract_id} :: ${instance.code_hash_hex}`);
}

// `order` accepts "cid_asc" (default), "cid_desc", "hash_asc", or "hash_desc".
// `hashPrefix` must be hexadecimal; it is lowercased automatically.

// Governance read helpers accept an AbortSignal so long-running requests can be cancelled.
const controller = new AbortController();
const proposal = await torii.getGovernanceProposal("proposal-001", {
  signal: controller.signal,
});
console.log(proposal?.kind);

// Typed wrapper returns a structured fallback when the proposal is missing.
const proposalResult = await torii.getGovernanceProposalTyped("proposal-missing");
if (!proposalResult.found) {
  console.warn("proposal not found");
}
const tallyResult = await torii.getGovernanceTallyTyped("ref-mainnet");
if (!tallyResult.found) {
  console.warn("tally not found");
} else {
  console.log(
    `approve=${tallyResult.tally.approve} reject=${tallyResult.tally.reject}`,
  );
}
// Torii must return a JSON payload for governance reads (proposals, referenda, tallies, locks,
// unlock stats); a 200 response without a body now throws so missing records continue to rely on
// the 404 path instead of silently returning null data.

// Governance write helpers also accept AbortSignal options so transactions can be cancelled.
const writeController = new AbortController();
const deployDraft = await torii.governanceProposeDeployContract({
  namespace: "apps",
  contractId: "calc.v1",
  codeHash: "hash:7B38...#ABCD",
  abiHash: Buffer.alloc(32, 0xaa),
  abiVersion: "1",
  window: { lower: 12_345, upper: 12_500 },
  mode: "Plain",
}, { signal: writeController.signal });
console.log("proposal instructions", deployDraft.tx_instructions.length);

const ballot = await torii.governanceSubmitPlainBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  referendumId: "ref-plain",
  owner: authority,
  amount: "5000",
  durationBlocks: 7200,
  direction: "Aye",
}, { signal: writeController.signal });
if (!ballot.accepted) {
  console.warn("ballot rejected:", ballot.reason);
}

await torii.governanceSubmitZkBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  electionId: "ref-zk",
  proof: Buffer.from(proofBytes),
  public: { owner: authority, amount: "5000" },
}, { signal: writeController.signal });

// The JS SDK also exposes governanceSubmitZkBallotV1 / governanceSubmitZkBallotProofV1
// for the BallotProof DTOs described in docs/source/governance_api.md.
const validatorPublicKeyBytes = Buffer.alloc(48, 0xaa);
const validatorProofBytes = Buffer.alloc(96, 0xbb);

const council = await torii.getGovernanceCouncilCurrent();
console.log(`active council epoch=${council.epoch} members=${council.members.length}`);

const deriveResponse = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "validator@test",
      variant: "Normal",
      pk: validatorPublicKeyBytes,
      proof: validatorProofBytes,
    },
  ],
});
console.log(`verified candidates=${deriveResponse.verified}`);

await torii.governancePersistCouncil({
  committeeSize: deriveResponse.members.length,
  candidates: deriveResponse.members.map((member) => ({
    accountId: member.account_id,
    variant: "Normal",
    pk: validatorPublicKeyBytes,
    proof: validatorProofBytes,
  })),
  authority,
  privateKey,
});

const audit = await torii.getGovernanceCouncilAudit({ epoch: deriveResponse.epoch });
console.log(`seed=${audit.seed_hex} beacon=${audit.beacon_hex}`);

const protectedNamespaceAbort = new AbortController();
await torii.setProtectedNamespaces(["apps", "system"], {
  signal: protectedNamespaceAbort.signal,
});
const protectedNamespaces = await torii.getProtectedNamespaces({
  signal: protectedNamespaceAbort.signal,
});
console.log(protectedNamespaces.namespaces); // ["apps", "system"]

const finalizeDraft = await torii.governanceFinalizeReferendumTyped({
  referendumId: "ref-mainnet-001",
  proposalId: "0123abcd...beef",
});
console.log(`finalize instructions=${finalizeDraft.tx_instructions.length}`);
const enactDraft = await torii.governanceEnactProposalTyped({
  proposalId: "abcd0123...cafe",
  window: { lower: 10, upper: 25 },
});
console.log(`enact instructions=${enactDraft.tx_instructions.length}`);

const registeredTriggers = await torii.listTriggers({
  namespace: "apps",
  authority: "issuer@test",
  limit: 5,
});
registeredTriggers.items.forEach((trigger) => {
  console.log(trigger.id, trigger.action.Mint?.Asset?.object);
});

const trigger = await torii.getTrigger("apps::mint_rewards");
if (!trigger) {
  await torii.registerTrigger({
    id: "apps::mint_rewards",
    namespace: "apps",
    action: {
      Mint: {
        Asset: {
          object: "rose#wonderland",
          destination_id: "treasury@wonderland",
          value: "5",
        },
      },
    },
  });
  const draft = await torii.registerTriggerTyped({
    id: "apps::mint_rewards",
    namespace: "apps",
    action: {
      Mint: {
        Asset: {
          object: "rose#wonderland",
          destination_id: "treasury@wonderland",
          value: "5",
        },
      },
    },
  });
  if (draft) {
    console.log(`trigger queued ok=${draft.ok} tx_instructions=${draft.tx_instructions.length}`);
  }
}

await torii.deleteTrigger("apps::archived");
await torii.deleteTriggerTyped("apps::archived");
const pending = await torii.queryTriggers({
  filter: { Eq: ["namespace", "apps"] },
  sort: [{ key: "created_at", order: "desc" }],
  limit: 10,
});
console.log("latest triggers", pending.items.map((item) => item.id));

// Helpers are available for building the Norito action payloads expected by
// `/v1/triggers`. The builders serialise the action to base64 so Torii receives
// the canonical Norito representation regardless of how the instructions were
// assembled in JS.
const timeAction = buildTimeTriggerAction({
  authority,
  instructions: [
    buildMintAssetInstruction({
      assetId: "rose#wonderland#treasury@wonderland",
      quantity: "250",
    }),
  ],
  startTimestampMs: Date.now() + 5_000,
  periodMs: 60_000,
  repeats: 10,
  metadata: { label: "hourly faucet" },
});
await torii.registerTrigger({
  id: "apps::mint_rose_hourly",
  namespace: "apps",
  action: timeAction,
});

const precommitAction = buildPrecommitTriggerAction({
  authority,
  instructions: [
    buildMintTriggerRepetitionsInstruction({ triggerId: "apps::guardian", repetitions: 1 }),
  ],
});
await torii.registerTrigger({
  id: "apps::guardian_refill",
  namespace: "apps",
  action: precommitAction,
});
```

When calling `list*` helpers or `getExplorerAccountQr`, set `addressFormat` to
`"compressed"` for snx1 literals or `"ih58"`/`"canonical"` for the default multihash
form—other values throw before the HTTP request is made.

`governanceFinalizeReferendumTyped` and `governanceEnactProposalTyped` normalise
the Torii responses (or synthesize an empty draft when Torii replies with `204 No Content`)
so automation always receives a `tx_instructions` array to sign without checking
for `null`.

## Configuration

- Publishing guidance and the release automation flow live under `docs/source/sdk/js/publishing.md`. GitHub releases tagged `js-v<semver>` automatically trigger the provenance-enabled publish workflow (with changelog/semver guards); for manual runs use `npm run check:changelog` (or rely on the `prepublishOnly` hook), then call `npm run release:update-docs -- --version <x.y.z> [--date YYYY-MM-DD] --note "summary"` to sync release notes into `CHANGELOG.md`, `status.md`, and `roadmap.md`.

- Release guardrails ship with `npm run release:matrix`, which executes the
  configured Node/OS targets (see
  `scripts/release_matrix.targets.example.json`) and records per-target logs,
  `matrix.json`, `matrix.md`, and `matrix.prom` in
  `artifacts/js-sdk-release-matrix/`. Attach the generated evidence bundle to
  release artefacts so the JS5 publishing gate can verify which environments
  exercised the candidate build. Pass `--metrics-out <path>` to override the
  Prometheus textfile location and `--textfile-dir <dir>` (or set
  `JS_RELEASE_MATRIX_TEXTFILE_DIR`) to mirror the gauges into a node_exporter
  textfile directory so release dashboards can ingest the status automatically.
  See `docs/source/sdk/js/publishing.md` for the full workflow.

- `ToriiClient` accepts `timeoutMs`, `maxRetries`, `backoffInitialMs`, `backoffMultiplier`, `maxBackoffMs`, `retryStatuses`, and `retryMethods`, mirroring the retry knobs exposed in `iroha_config`.
- Attach `retryTelemetryHook` to capture deterministic per-attempt telemetry for dashboards and SLO drills; events include phase (`response`/`network`/`timeout`), attempt numbers, method/URL, status or error metadata, backoffMs, profile name when set, durationMs for the attempt, and timestampMs so logs can be correlated with Torii-side traces.
- Authentication headers can be supplied via `authToken` (maps to `Authorization: Bearer ...`) or `apiToken` (maps to `X-API-Token`). Credentialled calls pin to the client's base scheme/host; cross-host overrides are rejected, insecure `http`/`ws` requires `allowInsecure: true` (dev-only), and `insecureTransportTelemetryHook` captures any downgraded transports. Cross-host requests without credentials require `allowAbsoluteUrl: true`.
- Runtime defaults can be pulled from `iroha_config` JSON/TOML by passing a camelCase config object (map `torii.api_tokens` to `torii.apiTokens`) to `new ToriiClient(url, { config })`. The helper `resolveToriiClientConfig({ config })` returns the merged settings if you need to inspect them directly.
- SoraFS/DA hooks accept explicit overrides: pass `sorafsGatewayFetch` (multi-source orchestrator) or `generateDaProofSummary` (checksum helper) to the `ToriiClient` constructor when testing; both are validated as functions, and `sorafsAliasPolicy` must be a plain object when provided (invalid shapes throw before any network call).
- Developer-friendly environment overrides are supported for local workflows: `IROHA_TORII_TIMEOUT_MS`, `IROHA_TORII_MAX_RETRIES`, `IROHA_TORII_BACKOFF_INITIAL_MS`, `IROHA_TORII_BACKOFF_MULTIPLIER`, `IROHA_TORII_MAX_BACKOFF_MS`, `IROHA_TORII_RETRY_STATUSES`, `IROHA_TORII_RETRY_METHODS`, `IROHA_TORII_API_TOKEN`, and `IROHA_TORII_AUTH_TOKEN`.
- Retryable status codes default to `{429, 502, 503, 504}`; methods default to `GET`, `HEAD`, and `OPTIONS`. Override them when your workflow needs different semantics.
- See `recipes/configured-client.mjs` for a script that loads an `iroha_config` JSON document, applies environment overrides, and instantiates `ToriiClient` with the merged settings.

```js
import { AccountAddress } from "@iroha/iroha-js";

const address = AccountAddress.fromAccount({
  domain: "default",
  publicKey: new Uint8Array(32),
});
console.log(address.canonicalHex());
console.log(address.toIH58(753));
console.log(address.toCompressedSora());
```

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
  overrides: { timeoutMs: 2000 },
});

const torii = new ToriiClient(config?.torii?.address ?? "http://localhost:8080", {
  config,
  timeoutMs: clientConfig.timeoutMs,
});
```

## Roadmap

- Transaction signing pipelines and manifest helpers backed by `iroha_crypto`.
- Full Torii transaction/query clients, streaming support, and developer
  documentation with runnable samples.

> **Status:** Preview-only. Expect frequent breaking changes until the roadmap
> milestones in `roadmap.md` reach completion.
