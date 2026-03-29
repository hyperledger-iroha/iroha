---
name: iroha-live-network
description: Use native Torii MCP on deployed Iroha networks such as Taira and future Nexus endpoints for live account, asset, contract, governance, and transaction workflows.
---

# Iroha Live Network

Use this skill when the user wants to inspect or mutate a live Iroha/Torii
network through native MCP.

## Working rules

1. Prefer curated `iroha.*` tools. Do not assume the raw `torii.*` namespace is
   available on public deployments.
2. Stay read-only by default. Only switch to a mutating workflow when the user
   explicitly asks to change live state.
3. Treat `authority`, `private_key`, API tokens, and forwarded auth headers as
   runtime-only secrets. Never write them to repo files, docs, or commits.
4. If a tool supports both flat shortcuts and `body`, prefer the simplest
   explicit JSON body that matches the route the user wants to call.
5. For pre-signed transaction envelopes, use `iroha.transactions.submit_and_wait`.

## Common live-network flows

- Accounts and aliases:
  - `iroha.accounts.get`
  - `iroha.accounts.query`
  - `iroha.aliases.resolve`
  - `iroha.accounts.assets`
- Contracts:
  - `iroha.contracts.deploy`
  - `iroha.contracts.instance.create`
  - `iroha.contracts.instance.activate`
  - `iroha.contracts.call_and_wait`
- Governance:
  - `iroha.gov.*`
- Transactions:
  - `iroha.transactions.submit_and_wait`
  - `iroha.transactions.wait`

## Taira preset

The built-in plugin MCP preset points at:

- `https://taira.sora.org/v1/mcp`

If that URL returns `404`, the deployment has not enabled Torii MCP yet.

## Custom networks

Future Nexus/Torii endpoints should be added as user-local MCP servers rather
than committed to the repo plugin manifest.
