---
name: iroha-live-network
description: Use native Torii MCP on deployed Iroha networks such as Taira and future Nexus endpoints for live account, asset, contract, governance, and transaction workflows.
---

# Iroha Live Network

Use this skill when the user wants to inspect or mutate a live Iroha/Torii
network through native MCP.

## Working rules

1. Prefer exact names returned by discovery from the curated `iroha.*` surface.
   Do not assume the raw `torii.*` namespace is available on public deployments.
   Inspect each tool's `inputSchema` and safety annotations before calling it.
2. Stay read-only by default. Only switch to a mutating workflow when the user
   explicitly asks to change live state.
3. Never send a raw private key, seed phrase, or signing key file through MCP.
   Build unsigned instructions where available, validate the exact canonical
   payload with `iroha.transactions.prepare`, and sign outside MCP. Inspect the
   complete fixed-V1 signed wire with `iroha.transactions.inspect`, require a
   valid signature result, and submit that same wire with
   `iroha.transactions.submit_and_wait`.
4. Keep API tokens and forwarded authentication headers runtime-only. Never
   write them to repo files, documentation, commits, or logs.
5. If a tool supports both flat shortcuts and `body`, prefer the simplest
   explicit JSON body that matches its discovered schema.
6. Torii's existing `POST /v1/mcp` route is the only MCP server, process, and
   listener. Route-backed dispatch stays in Torii's authoritative router
   in-process; explicit Torii-local helpers run in the same MCP handler without
   fabricating another route. Do not introduce or expect a gateway, sidecar,
   proxy server, second listener, or separate MCP deployment. Native transport
   is stateless MCP `2026-07-28`.
   Every JSON-RPC request must carry
   `io.modelcontextprotocol/protocolVersion` and object-valued
   `io.modelcontextprotocol/clientCapabilities` in `params._meta`;
   `io.modelcontextprotocol/clientInfo` with string name and version is
   recommended but not required.
7. For native HTTP requests, send `MCP-Protocol-Version: 2026-07-28` and
   `Mcp-Method` matching the body. Also send `Mcp-Name` for `tools/call`
   (`params.name`) and `resources/read` (`params.uri`); `prompts/get` is not
   currently implemented. Plain header-safe ASCII is sent directly. Otherwise
   wrap canonical padded standard Base64 of the UTF-8 bytes as
   `=?base64?<encoded>?=`. Send `Accept` with both `application/json` and
   `text/event-stream`.
8. Calling `server/discover` is optional for the client. Use it when up-front
   version, capability, and server identity discovery helps; otherwise call
   `tools/list` directly with the same per-request metadata. Use
   `resources/list` and `resources/read` only when the server advertises the
   resources capability, and honor their private TTLs.
9. Treat initialization-based MCP `2025-06-18` as compatibility fallback only:
   use it only when the connection is explicitly configured for a known legacy
   Torii endpoint. Never infer a downgrade from a transport, authentication, or
   protocol failure. The compatibility sequence uses `initialize`, then
   `notifications/initialized`, and the `MCP-Protocol-Version: 2025-06-18`
   header on later requests.
10. Do not send `initialize`, `notifications/initialized`, `ping`, or
    `Mcp-Session-Id` on the native path. POST exactly one JSON-RPC request per
   HTTP request. Do not send client JSON-RPC responses or outer arrays. Use the
   advertised `tools/call_batch` extension when batching is needed, declare
   `clientCapabilities.extensions["org.hyperledger.iroha/tools"] = {}` on that
   request, and remember that every inner call consumes rate-limit capacity.
11. Inspect all four standard annotations plus versioned
    `_meta["iroha/semantics"]`. Keep operation, authority, mutation, retry,
    world boundary, sensitivity, and external-signature requirements separate.
    These remain hints. For route-backed tools, `_meta["iroha/routeAuth"]` and
    Torii's route catalog are the authoritative admission contract. Explicit
    in-process capabilities do not publish fabricated route-auth metadata.

## Common live-network flows

- Accounts and aliases:
  - `iroha.accounts.get`
  - `iroha.accounts.query`
  - `iroha.aliases.resolve`
  - `iroha.accounts.assets`
- Assets:
  - `iroha.assets.get`
  - `iroha.assets.definitions.get`
- Contracts:
  - `iroha.contracts.code.get`
  - `iroha.contracts.state.get`
  - `iroha.contracts.call`
  - `iroha.contracts.call_and_wait`
- Governance:
  - `iroha.gov.proposals.get`
  - `iroha.gov.referenda.get`
  - `iroha.gov.tally.get`
- Transactions:
  - `iroha.transactions.prepare`
  - `iroha.transactions.inspect`
  - `iroha.transactions.submit_and_wait`
  - `iroha.transactions.wait`

`iroha.transactions.prepare` accepts an already-built canonical unsigned
payload; it does not build the intended operation, quote fees, read state, or
simulate execution. `iroha.transactions.inspect` performs canonical structural
and cryptographic checks, not ledger admission or state-aware simulation. Both
are pure in-process capabilities of the existing Torii MCP handler and create
no HTTP route, server, listener, or network hop.

Contract deployment, instance creation, and activation do not have dedicated
MCP convenience tools. Assemble and sign the corresponding transaction locally,
then submit the signed envelope.

## Connect

Use only the canonical `iroha.connect.*` names:

- `iroha.connect.session.create`
- `iroha.connect.ws.ticket`
- `iroha.connect.session.status`
- `iroha.connect.session.delete`

The ticket tool requires an explicit trusted `node_url`; never derive a
credential-bearing destination from request headers. The composite
`iroha.connect.session.create_and_ticket` and all bare `connect.*` aliases are
retired. Create a session and ticket in two explicit steps.

For the public Taira reset workflow, keep faucet prepare and submit in the
CLI-owned corridor rather than attempting them through MCP. Other Torii
profiles may expose separately runtime-gated faucet tools; discover the exact
deployment surface instead of assuming they exist.

## Taira preset

The built-in plugin MCP preset currently targets the primary public Taira MCP
endpoint:

- `https://taira.sora.org/v1/mcp`

If your deployment or operator gives you a different public Torii root for the
environment under test, override it locally. `GET /v1/mcp` returning `405` is
expected for the POST-only transport. A POST returning `503 mcp_disabled`
means the deployment has not enabled Torii MCP.

Browser clients must send either no `Origin` or one exact origin configured by
the node operator. An unlisted, wildcard, or duplicate Origin is rejected.

## Custom networks

Future Nexus/Torii endpoints should be added as user-local Codex MCP
connections pointing to each existing Torii `/v1/mcp` route rather than being
committed to the repo plugin manifest. This configures a client connection; it
does not create another server or listener.
