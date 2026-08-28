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
   Build unsigned instructions where available, sign locally, and submit a
   pre-signed envelope with `iroha.transactions.submit_and_wait`.
4. Keep API tokens and forwarded authentication headers runtime-only. Never
   write them to repo files, documentation, commits, or logs.
5. If a tool supports both flat shortcuts and `body`, prefer the simplest
   explicit JSON body that matches its discovered schema.
6. Torii MCP is POST-only. A manual client must initialize with
   `protocolVersion`, `capabilities`, and `clientInfo`, then send the negotiated
   `MCP-Protocol-Version` header on later requests. Do not send outer JSON-RPC
   arrays; use the advertised `tools/call_batch` extension when batching is
   needed, and remember that every inner call consumes rate-limit capacity.

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
  - `iroha.transactions.submit_and_wait`
  - `iroha.transactions.wait`

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

Do not attempt faucet prepare or submit through MCP. Use the CLI-owned public
reset workflow; the agent-facing faucet surface remains intentionally absent.

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

Future Nexus/Torii endpoints should be added as user-local MCP servers rather
than committed to the repo plugin manifest.
