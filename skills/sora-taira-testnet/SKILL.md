---
name: sora-taira-testnet
description: "Work against the SORA Taira testnet through its deployed Torii MCP endpoint for live account, asset, alias, contract, governance, and transaction workflows. Use when Codex needs to inspect or mutate the Taira testnet, verify or add the `https://taira.sora.org/v1/mcp` MCP server, prefer the curated `iroha.*` tool surface, or handle runtime-only signing inputs such as `authority` and `private_key`."
---

# SORA Taira Testnet

Use the Taira testnet through native Torii MCP.

## Quick Start

1. Confirm a Taira MCP server is available in the current Codex environment.
2. Prefer curated `iroha.*` tools over raw `torii.*` tools.
3. Stay read-only until the user explicitly asks to mutate live state.
4. Treat any signing inputs, API tokens, or forwarded auth headers as runtime-only secrets.

## MCP Endpoint

The Taira MCP endpoint is:

- `https://taira.sora.org/v1/mcp`

If the endpoint is not configured locally, instruct the user to add a user-local
MCP entry that points at that URL.

If the endpoint returns `404`, report that native Torii MCP is not enabled on
the deployment yet and stop before attempting live-network actions.

If reads work but live writes fail with `route_unavailable`, report that the
public ingress still cannot reach authoritative peers for the target lane and
point operators at
`configs/soranexus/taira/check_mcp_rollout.sh --write-config <runtime-only client.toml>`.

## Working Rules

1. Prefer `iroha.*` aliases. They are the intended agent-facing surface for
   deployed networks.
2. Use explicit JSON `body` payloads when a write flow needs more than a couple
   of flat shortcut arguments.
3. Keep `authority`, `private_key`, bearer tokens, and forwarded auth headers
   out of files, docs, and commits.
4. For pre-signed transaction envelopes, use
   `iroha.transactions.submit_and_wait`.
5. If a mutation would affect live state, restate the intended change clearly
   before executing it unless the user already gave a direct mutation request.

## Common Flows

### Accounts and aliases

- `iroha.accounts.get`
- `iroha.accounts.query`
- `iroha.aliases.resolve`
- `iroha.accounts.assets`

### Assets and balances

- `iroha.assets.get`
- `iroha.asset-definitions.get`
- `iroha.accounts.assets`

### Contracts

- `iroha.contracts.deploy`
- `iroha.contracts.instance.create`
- `iroha.contracts.instance.activate`
- `iroha.contracts.call`
- `iroha.contracts.call_and_wait`

### Governance and network state

- `iroha.gov.*`
- `iroha.blocks.*`
- `iroha.transactions.wait`

### Transaction submission

- `iroha.transactions.submit`
- `iroha.transactions.submit_and_wait`

## Response Handling

1. Use each tool's `inputSchema` as the source of truth for accepted fields.
2. Re-run tool discovery if the MCP server reports that the tool list changed.
3. When a write tool succeeds, summarize the resulting transaction hash or the
   returned status, not just the request payload.
4. When a write tool fails, surface the server error and say whether the issue
   looks like auth, validation, missing tool exposure, or endpoint availability.
5. Treat `route_unavailable` as deployment health, not user input failure:
   the public Torii node is up but the write route still cannot reach an
   authoritative peer.

## Safety

- Do not invent key material.
- Do not persist runtime credentials unless the user explicitly asks for that.
- Do not assume operator-only routes are available on public Taira.
- Prefer anonymous/public reads first when investigating state.
