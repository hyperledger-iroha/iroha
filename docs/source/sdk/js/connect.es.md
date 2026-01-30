---
lang: es
direction: ltr
source: docs/source/sdk/js/connect.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 31715a560a179f6b230c9933feffdcf7c7eee33f5f4b33db93e29b37b47c8034
source_last_modified: "2026-01-05T18:01:16.638988+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Connect Workflows

Roadmap item JS4 calls for end-to-end Connect coverage across the SDKs so
wallets and dApps can reuse the same helpers that Torii exposes. The JavaScript
package already ships the primitives (`ToriiClient` Connect APIs,
`bootstrapConnectPreviewSession`, and the WebSocket builders); this guide folds
them into the portal and shows how to wire everything together.

## Enumerate and manage Connect applications

Use `ToriiClient.listConnectApps()` for single page fetches or
`ToriiClient.iterateConnectApps()` to walk the full registry with cursor-based
pagination. Each record carries the display metadata, namespace allowlist, and
operator-defined policy blob so automation can diff against governance
requirements.

```ts
import { ToriiClient } from "@iroha/iroha-js";

const client = new ToriiClient("https://torii.nexus.sora");

const { items, total } = await client.listConnectApps({ limit: 10 });
console.log(`first page has ${items.length} of ${total} apps`);
for (const app of items) {
  console.log(app.appId, app.displayName, app.namespaces);
}

for await (const app of client.iterateConnectApps({ pageSize: 50 })) {
  console.log("iterated app", app.appId);
}
```

To upsert metadata, call `registerConnectApp()` with the payload you want
persisted. Extra fields pass through untouched, which means the same helper can
register governance flags, private preview toggles, or region tags without a
custom client. Use `deleteConnectApp()` to remove stale entries.

```ts
await client.registerConnectApp({
  appId: "wallet-preview",
  displayName: "Preview Wallet",
  description: "Deterministic Connect client used in QA fixtures",
  iconUrl: "https://example.invalid/assets/preview-wallet.svg",
  namespaces: ["wonderland", "dao"],
  metadata: {
    website: "https://example.invalid/wallet",
    policy_version: "2026.01",
  },
  policy: {
    // Mirror the Norito manifest policy knobs shown in roadmap JS4
    max_sessions_per_ip: 5,
  },
});
```

When Connect governance toggles change, fetch them via
`getConnectAppPolicy()`/`updateConnectAppPolicy()` to keep the registry in sync
with the operator-approved defaults. Admission manifests are available through
`getConnectAdmissionManifest()` / `setConnectAdmissionManifest()`; both helpers
accept/emit full manifest bodies so the SDK can stage reviews before Torii
starts enforcing a new schema.

## Monitor Connect capacity

`ToriiClient.getConnectStatus()` returns the active policy snapshot, per-IP
session counts, and enforcement knobs that Torii exposes over
`/v1/connect/status`. It mirrors the telemetry gadgets mentioned in JS4 so
SDK-hosted dashboards or runbooks can confirm Connect is enabled before opening
sessions.

```ts
const status = await client.getConnectStatus();
if (status === null) {
  throw new Error("Connect is disabled on this Torii endpoint");
}
console.log("max wallet sessions", status.policy.wallet.maxSessions);
for (const sample of status.perIpSessions) {
  console.log(sample.ip, sample.sessionsActive, sample.sessionsRejectedTotal);
}
```

## Bootstrap preview sessions

`bootstrapConnectPreviewSession()` wraps `createConnectSessionPreview()` plus
the Torii session registration call so you can generate deterministic previews
and (optionally) register them with the node. The helper returns the preview
payload (SID, URIs, invite metadata) and the Torii session result, including
the wallet/app tokens referenced by the roadmap.

```ts
import { ToriiClient, bootstrapConnectPreviewSession } from "@iroha/iroha-js";

const toriiBaseUrl = "https://torii.nexus.sora";
const torii = new ToriiClient(toriiBaseUrl);
const { preview, session, tokens } = await bootstrapConnectPreviewSession(torii, {
  chainId: "iroha2-dev",
  register: true,
  sessionOptions: { node: "lane-default" },
});

if (!session || !tokens) {
  throw new Error("session was not registered; rerun with register: true");
}

console.log("share this SID", preview.sidBase64Url);
console.log("wallet deeplink", session.wallet_uri);
console.log("app token for Connect WS", tokens.app);
```

Pass `register: false` when you only need the deterministic preview (for
example, generating QR codes for offline artifacts) or provide custom
`sessionOptions` to override the node hint passed to Torii. The helper throws
typed validation errors when the inputs are malformed, matching the rest of the
JS SDK ergonomics.

## Build and open Connect WebSockets

After you possess the SID + role token pair, build the WebSocket URL using
`ToriiClient.buildConnectWebSocketUrl()` (instance) or the standalone
`buildConnectWebSocketUrl()` export. Both helpers convert the Torii base URL
into `ws://` / `wss://` endpoints automatically and append the required `sid`
and `role` query parameters while keeping secrets out of the URL. Tokens are
carried via `Authorization: Bearer` headers by default; browser clients get a
`Sec-WebSocket-Protocol: iroha-connect.token.v1.<b64url(token)>` marker when
headers are unavailable. When using `http://` Torii bases, pass `allowInsecure: true`
explicitly; otherwise the helper will reject insecure protocols when a token is
present. Endpoint overrides must remain on the same host/scheme as the base URL
to avoid leaking credentials across origins, and `ToriiClient.openConnectWebSocket()`
inherits `allowInsecure` plus the `insecureTransportTelemetryHook` from the
client config so dev/test runs can flag insecure opt-ins automatically. Standalone
callers can pass their own `insecureTransportTelemetryHook` when they need to log or
alert on local `ws://` usage. Notes:

- Ensure Connect URLs use the same host/scheme as the configured Torii base; absolute overrides with credentials are rejected.
- Attach `insecureTransportTelemetryHook` to surface any ws:// dial attempts in logs/metrics during development.

```ts
import WebSocket from "ws";
import { ToriiClient, openConnectWebSocket } from "@iroha/iroha-js";

const toriiBaseUrl = "https://torii.nexus.sora";
const torii = new ToriiClient(toriiBaseUrl);
const { preview, tokens } = await bootstrapConnectPreviewSession(torii);
if (!tokens) {
  throw new Error("session was not registered; rerun with register: true");
}

const walletWs = openConnectWebSocket({
  baseUrl: toriiBaseUrl,
  sid: preview.sidBase64Url,
  role: "wallet",
  token: tokens.wallet,
  websocketOptions: {
    headers: {
      "x-custom-header": "demo",
    },
  },
  WebSocketImpl: WebSocket,
});

walletWs.on("open", () => {
  console.log("wallet side connected");
});
walletWs.on("message", (data) => {
  console.log("Connect payload", data.toString());
});
```

`openConnectWebSocket()` optionally accepts a custom protocol list or
`websocketOptions` object if your environment (e.g., Node.js via `ws`) needs TLS
overrides. If you already manage a WebSocket client elsewhere (e.g., browsers or
React Native), call `ToriiClient.buildConnectWebSocketUrl()` and feed the URL
into that implementation instead of using the helper constructor.

## Putting it together

Combine the primitives above to satisfy the JS4 roadmap gate:

1. Use the registry helpers to publish audited namespace manifests and
   governance toggles.
2. Monitor Connect status before opening sessions.
3. Generate preview SIDs and register them via Torii, optionally sharing the
   wallet/app URIs as QR codes.
4. Open the WebSocket with the issued SID/tokens using the built-in helpers so
   retries, protocol selection, and URL shaping stay consistent across SDKs.

Because each helper mirrors the Rust APIs, parity suites (Jest +
`npm run lint:test`) cover them automatically and the documentation above now
gives developer experience, QA, and governance teams a canonical reference when
testing Connect end-to-end.
