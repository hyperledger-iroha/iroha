---
name: mochi-local-sandbox
description: "Use the shared Kagami/Mochi managed localnet for local Iroha development, inspect its public context and logs, and deploy Kotodama contracts without supplied TOML files."
---

# Shared Kagami and Mochi localnet

Use the installed matching Kagami and iroha3d binaries. The desktop selects the
same workspace with `mochi --workspace <directory>`. It owns no separate network
or signer layout; closing the desktop leaves the managed network available.

1. From the requested project directory, run `kagami localnet up`. For another
   workspace use `kagami localnet up --workspace <directory>`. The shared engine
   generates four validators, private runtime configuration and a funded signer.
2. Inspect `kagami localnet status --json` and `kagami context show --json` in the
   same workspace. Report readiness only from the authenticated worker result.
   Public context output identifies the account, exact network and Torii endpoint.
3. For a requested deployment, run `kagami contract deploy <file.ko>` (or a `.to`
   artifact/package). It can start the default localnet automatically. Retain the
   returned deployment journal for original-hash recovery; use
   `kagami contract deploy --resume <journal>` after an uncertain outcome.
4. Diagnose failures with `kagami localnet logs` or
   `kagami localnet logs --peer 0`. Do not infer process ownership from a stored
   numeric PID, signal a process by saved PID, or invent success after a timeout.
5. Stop with `kagami localnet down` when the requested work includes teardown.
   Reset is an explicit destructive action: stop first, then
   `kagami localnet reset local` only when the user's intent includes discarding
   the local keys and ledger.

Generated credentials and client configuration remain in the private application
state store, outside the project. Never print secret files, insert keys into
commands, commit them, or export them through `.env.local`. Use the public context
metadata and the canonical SDK credential loader for authorized application wiring.

The generated local Torii may expose curated `iroha.*` MCP tools. Derive its URL
from the selected context and verify the requested local surface before claiming
MCP readiness. Prefer those curated tools over raw protocol routes when available.
Do not treat a ready localnet as proof of remote private-dataspace attachment;
Taira and Minamoto have their own explicit runtime policies and skill owners.
