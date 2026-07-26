---
name: sora-minamoto-mainnet
description: "Work against the SORA Minamoto mainnet through its deployed Torii MCP endpoint for live account, asset, alias, contract, governance, Musubi package-registry, and transaction workflows. Use when Codex needs to inspect Minamoto mainnet, verify or add `https://minamoto.sora.org/v1/mcp`, prefer the curated `iroha.*` tool surface, classify Minamoto ingress and chain-health failures correctly, or handle runtime-only signing inputs such as `authority` and `private_key`."
---

# SORA Minamoto Mainnet

Use the Minamoto mainnet through native Torii MCP.

## Quick Start

1. Confirm a Minamoto MCP server is available in the current Codex environment.
2. Treat `https://minamoto.sora.org` as the primary public Torii/API origin and
   `https://minamoto.sora.org/v1/mcp` as its native MCP endpoint.
3. Before long or stateful workflows, sample public health first. Prefer
   proceeding only when blocks are advancing, `queue_size` is low,
   `tx_queue_saturated=false`, and `teu_dataspace_backlog` is not climbing.
4. Prefer curated `iroha.*` tools over raw `torii.*` tools.
5. Stay read-only until the user explicitly asks to mutate live mainnet state.
6. Treat any signing inputs, API tokens, or forwarded auth headers as
   runtime-only secrets.
7. Do not use Taira testnet faucet, bootstrap, or canary assumptions on
   Minamoto.
8. When the user asks to transact, follow the "Transaction Workflow" section
   before submitting anything.
9. In Codex sessions where multiple SORA MCP servers are available, use the
   Minamoto namespace or server entry for Minamoto work. Do not call Taira tools
   for Minamoto state.

## MCP Endpoint

Use the public Minamoto MCP endpoint:

- `https://minamoto.sora.org/v1/mcp`

The matching public Torii/API root is:

- `https://minamoto.sora.org`

If the endpoint is not configured locally, instruct the user to add a user-local
MCP entry that points at that URL:

```bash
codex mcp add sora-minamoto-mainnet --url https://minamoto.sora.org/v1/mcp
```

If the endpoint returns `404`, report that native Torii MCP is not enabled on
the Minamoto deployment yet and stop before attempting live-network actions.

If the MCP endpoint or the public Torii root returns `502` or `503`, report
public ingress / rollout health degradation before attempting writes. Treat
that as deployment health, not user input failure.

If reads work but live writes fail with `route_unavailable`, report that the
public ingress still cannot reach authoritative peers for the target lane.
Do not classify that as a malformed payload without operator-side evidence.

If writes fail with `Transaction expired`, classify that as chain health,
consensus latency, or queue saturation first. Sample `/status` and
`/v1/sumeragi/status` and report the current `blocks`, `commit_qc_height`,
`highest_qc_height`, `queue_size`, `tx_queue_depth`, `tx_queue_saturated`,
`teu_dataspace_backlog`, and `view_change_causes.last_cause` before blaming
the caller.

## Restart Smoke Test

After installing or updating this skill and restarting Codex, verify the live
mainnet wiring with read-only calls only:

1. Confirm the configured MCP server points at
   `https://minamoto.sora.org/v1/mcp`.
2. Confirm a Minamoto MCP namespace or server entry is visible in the current
   session.
3. Call `iroha.sumeragi.status` and check that `commit_qc.height` is present,
   `tx_queue.saturated=false`, and queue depth is low.
4. Fetch the latest known block with `iroha.blocks.get`.
5. List recent transactions with `iroha.transactions.list`, then check one
   listed hash with `iroha.transactions.status`.
6. Optionally call a Musubi read such as `iroha.musubi.alias.resolve`; a
   `404 not_found` means that alias is absent, not that the tool is broken.

Do not use broad alias-index enumeration as a health check. It may be
permission-bound on Minamoto and can legitimately return `403
permission_denied`.

## Working Rules

1. Prefer `iroha.*` aliases. They are the intended agent-facing surface for
   deployed networks.
2. Use explicit JSON `body` payloads when a write flow needs more than a couple
   of flat shortcut arguments.
3. Keep `authority`, `private_key`, bearer tokens, and forwarded auth headers
   out of files, docs, and commits.
4. Before any Minamoto write, verify the signer on-chain first: the account
   exists on the current Minamoto chain, holds the required fee asset balance,
   and has the permissions required for the specific mutation.
5. Prefer pre-signed transaction envelopes from the user's wallet or client for
   value-moving or irreversible mainnet operations.
6. Never run testnet onboarding, faucet, or throwaway canary flows against
   Minamoto unless the user explicitly asks for that exact mainnet operation.
7. Do not reuse Taira or zero-chain client configs for Minamoto. Use an
   operator-provided or user-local Minamoto config and keep it out of the repo.
8. For pre-signed transaction envelopes, use
   `iroha.transactions.submit_and_wait`.
9. If a mutation would affect live state, apply the "Mainnet Write
   Confirmation Policy" before executing it.
10. For Musubi package-registry writes, use the pre-signing helpers only when
    they are exposed by the Minamoto MCP server:
    `iroha.musubi.instructions.publish_release`,
    `iroha.musubi.instructions.yank_release`,
    `iroha.musubi.instructions.set_alias`, and
    `iroha.musubi.instructions.assert_release_exists`. They return unsigned
    Norito-framed instructions; sign and submit them from the client side.

## Minamoto vs Taira Differences

| Topic | Taira testnet | Minamoto mainnet |
| --- | --- | --- |
| Default posture | Read-first, testnet writes allowed when requested | Read-only until the user explicitly requests a mainnet mutation |
| Faucet/bootstrap | Public faucet and runtime-only canary bootstrap can be appropriate | Do not faucet, bootstrap, or auto-onboard unless the user explicitly requests that live mainnet operation |
| Signing | Runtime-only throwaway signer can be acceptable for rollout checks | Prefer user wallet/client signing and pre-signed transaction envelopes |
| Failure handling | Rollout scripts can create canary signers and retry funding paths | Report signer, fee, permission, ingress, or chain-health evidence; do not self-fund |
| Configs | Taira-specific client configs and rollout scripts may apply | Use only operator-provided or user-local Minamoto configs |

## Transaction Workflow

Use this sequence for any Minamoto mutation, including asset transfers,
contract calls, governance changes, package-registry writes, alias changes, or
account operations:

1. Discover the available MCP tools and prefer the curated `iroha.*` names.
   Re-check `inputSchema` for every tool used in the flow.
2. Sample public health:
   - `GET https://minamoto.sora.org/status`
   - `iroha.status`
   - `iroha.sumeragi.status`
3. Identify the signer:
   - resolve the user-provided alias with `iroha.aliases.resolve`, when an
     alias was supplied
   - fetch the canonical account with `iroha.accounts.get`
   - inspect fee balances with `iroha.accounts.assets`
   - inspect the required permission state with the narrowest available
     `iroha.*` permission or account query
4. Identify the target state:
   - fetch the target account, asset definition, contract instance, package, or
     governance object before building the write
   - record current values needed to verify the post-write state
5. Build the unsigned instruction or transaction body using the specific
   `iroha.*` helper, or ask the user's wallet/client to build it.
6. Get a signature outside Codex by default. Prefer a user-provided
   `signed_tx_base64` envelope over raw private-key signing in the agent
   session.
7. Before submission, apply the "Mainnet Write Confirmation Policy".
8. Submit the pre-signed envelope with `iroha.transactions.submit_and_wait`.
9. Verify final state with a read query against the object changed by the
   transaction.
10. Report the transaction hash, final status, block height if available, the
    verification query result, and any uncertainty caused by public-node
    visibility.

## Mainnet Write Confirmation Policy

Before submitting a transaction that mutates Minamoto, restate the intended
change in concrete terms. The confirmation summary should include:

- signer alias and canonical account id, when known
- target account, asset, contract, package, governance object, or alias
- exact operation and parameters
- fee or gas asset metadata, when required by the chain or tool
- whether value moves between accounts
- whether the operation is irreversible or difficult to reverse
- expected verification query after submission

For value-moving or irreversible operations, do not submit until the user
confirms after seeing this summary, unless the user already supplied a signed
envelope and a clear submit-now instruction in the current request. If any item
is ambiguous for a value-moving or irreversible operation, ask for
clarification before submission.

## Required User Inputs for Writes

For a Minamoto write, collect or confirm these inputs before spending time on
payload debugging:

- signer alias or canonical account id
- target object id, such as an account id, asset id, contract id, package id,
  governance proposal id, or alias
- operation parameters, including amounts, metadata keys, arguments, or package
  version details
- either a pre-signed transaction envelope, or an unsigned instruction/body plus
  an external signing path
- Minamoto public root/MCP endpoint or an explicit operator-provided root if
  the default public endpoint is not the target
- fee or gas asset metadata if the deployment requires it
- timeout expectations for `submit_and_wait`
- expected post-write read query

If the user supplies a local client config, treat it as runtime-only. Do not
copy it into the repo or quote secrets from it.

## Do Not Sign Inside Codex By Default

For Minamoto, the default write path is:

1. inspect state
2. prepare or validate the unsigned operation
3. ask the user's wallet/client to sign
4. submit the pre-signed envelope
5. verify state

Do not ask for raw private keys, mnemonic phrases, bearer tokens, or forwarded
auth headers for mainnet work unless the user explicitly insists on that path.
If the user insists, treat the values as runtime-only secrets: do not persist
them, do not echo them, do not put them in docs, and do not commit them.

If a tool accepts `authority` and `private_key`, do not assume that makes
agent-side signing appropriate on mainnet. Prefer pre-signed envelopes for
value-moving or irreversible operations.

## Public-Node Diagnostics

1. Treat `https://minamoto.sora.org` as the default public Torii vantage point
   for Minamoto. It is still only one public view, not proof of full
   validator-set health.
2. Do not invent or assume direct per-validator public hostnames. They may
   exist on some deployments and may be intentionally absent on others.
3. Do not infer validator-set size from `/status.peers`. That field is the
   queried node's current remote-peer count, not the network's validator-set
   length.
4. When diagnosing public-write or finality issues, prefer this read bundle:
   - `iroha.status`
   - `iroha.sumeragi.status`
   - `iroha.blocks.list`
   - `iroha.transactions.status` or `iroha.transactions.wait`
   - `GET https://minamoto.sora.org/status`
5. If the latest committed block timestamp stops advancing and Sumeragi shows
   signals such as `membership.height > commit_qc.height`,
   `view_change_causes.last_cause = "missing_qc"`, or
   `view_change_causes.last_cause = "quorum_timeout"`, or
   `worker_loop.stage = "idle"`, report that the queried public Torii finality
   path appears stalled. Do not claim the entire validator set is down unless
   you also have direct validator or operator visibility.
6. If `iroha.transactions.status` returns `404 not_found` for a previously
   submitted hash, report only that the queried public node currently has no
   visibility for that transaction hash. Do not infer commit, reject, or
   network-wide disappearance from that result alone.
7. If public reads are healthy but signed writes still expire, call that a
   partial public-node or public-finality-path failure first, not proof that
   the user payload is malformed.

## Known Failure Patterns

1. `route_unavailable` while public reads still work usually means the public
   ingress can answer read traffic but cannot reach an authoritative peer for
   the target write lane. Treat that as rollout health first.
2. `Transaction expired` on signed writes usually points at chain health,
   consensus latency, queue saturation, or public finality trouble before it
   points at malformed user input.
3. `502` or `503` from `/v1/mcp` or the public Torii root usually means edge or
   upstream rollout breakage, not a signer or payload problem.
4. Permission or fee failures on mainnet should be reported narrowly with the
   signer, target account, required permission, and fee asset evidence that the
   public node returned. Do not try to self-fund or auto-onboard a mainnet
   signer.

## Failure-to-Action Map

| Failure or symptom | Agent action |
| --- | --- |
| `403 Forbidden` | Check signer account existence, permissions, account policy, and auth context. Report the missing permission or authorization issue narrowly. |
| Missing or insufficient fee asset | Ask the user to fund the signer or provide a different signer. Do not run a faucet or self-funding path. |
| `route_unavailable` | Treat this as public ingress or authoritative-peer routing health first. Capture read-health evidence and stop before payload rewrites. |
| `Transaction expired` | Sample `/status` and `/v1/sumeragi/status`; check queue saturation, finality lag, and TTL freshness before rebuilding the transaction. |
| `404 not_found` from `iroha.transactions.status` | Report that the queried public node has no visibility for that hash. Do not infer commit, reject, or network-wide disappearance. |
| `404 not_found` from Musubi alias/package reads | Treat the tool as reachable and the requested alias or package as absent unless discovery shows the Musubi tool is missing. |
| MCP endpoint `404` | Report native Torii MCP is not enabled on the deployment. Do not attempt live-network actions through that endpoint. |
| MCP endpoint or root `502` / `503` | Treat as edge or upstream rollout degradation. Avoid writes until health recovers. |
| Tool missing from discovery | Re-run discovery once. If still absent, report that the Minamoto MCP profile does not expose that workflow. |
| Schema validation error | Re-read the tool `inputSchema`, correct field names or body shape, and avoid guessing undocumented parameters. |
| `403 permission_denied` from alias-index or broad explorer reads | Treat it as a route or permission boundary. Prefer narrower reads such as alias resolution by literal alias or by-account lookups. |
| Successful submit but verification read is stale | Report the submitted status and the stale public-node observation separately; retry the read before claiming the mutation failed. |

## Preferred Triage Sequence

1. Start with read-only public health:
   - `GET https://minamoto.sora.org/status`
   - `iroha.status`
   - `iroha.sumeragi.status`
2. Confirm the MCP server exposes the expected curated tools before choosing a
   workflow:
   - `iroha.status`
   - `iroha.transactions.submit_and_wait`
   - the specific `iroha.*` account, alias, asset, contract, governance, or
     Musubi tools required by the request
3. For any write, preflight the signer and target state with read-only queries.
4. Prefer a user-provided pre-signed transaction envelope for irreversible or
   value-moving changes.
5. Only spend time on payload debugging after public health, signer existence,
   signer balance, and signer permissions are confirmed.

## MCP Write Recipes

### Submit a pre-signed transaction envelope

For pre-built signed transactions, prefer `iroha.transactions.submit_and_wait`
instead of lower-level polling:

```json
{
  "signed_tx_base64": "<base64-encoded SignedTransaction>",
  "status_accept": "application/json",
  "timeout_ms": 120000
}
```

### Verify a signer before writes

Before submitting any Minamoto mutation, confirm the alias, account, fee
balance, and permissions before debugging payloads:

- resolve the alias with `iroha.aliases.resolve`
- fetch the canonical account with `iroha.accounts.get`
- inspect balances with `iroha.accounts.assets`
- inspect permission state with the narrowest available `iroha.*` account or
  permission query tool exposed by the deployment

If the signer is missing, has no fee asset balance, or lacks a required
permission, report that directly. Do not run faucet or bootstrap flows on
mainnet.

### End-to-end signed-envelope write

Use this shape after the user or their wallet/client provides the signed
transaction envelope:

```json
{
  "signed_tx_base64": "<base64-encoded SignedTransaction>",
  "status_accept": "application/json",
  "timeout_ms": 120000
}
```

After submission, run the specific verification read for the changed object.
For example, verify balances after a transfer, metadata after a metadata
mutation, package release state after a package write, and contract instance
state after a contract call.

### Inspect Musubi packages

Use the curated Musubi tools for package reads when the Minamoto MCP server
exposes them:

- `iroha.musubi.search` with `query`, optional `namespace`, `include_yanked`,
  `offset`, and `limit`
- `iroha.musubi.release.get` with `package = "namespace/name@version"`
- `iroha.musubi.package.releases` with `package = "namespace/name"` and
  optional `include_yanked`
- `iroha.musubi.package.versions` with `package = "namespace/name"`
- `iroha.musubi.alias.resolve` with `alias = "<short-name>"`

Musubi namespaces intentionally do not use a leading `@`; use literals like
`universal`, `dex.universal`, and `dex.universal/swap-core` when those
namespaces exist on Minamoto.

### Build Musubi instructions for local signing

The Musubi instruction tools do not accept `authority`, `private_key`, bearer
tokens, or any other signing material. They return `wire_id`,
`instruction_base64`, `instruction_hex`, and an `instruction_json` preview.
The client must assemble a signed transaction locally and submit it with
`iroha.transactions.submit_and_wait`.

Use `signed_tx_base64` or `tx_base64` for base64 envelopes, or the hex variants
for hex-encoded envelopes. Do not pass multiple envelope encodings in the same
request. If the call returns `Transaction expired`, go back to the public
health and Sumeragi checks before rebuilding the transaction.

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

## Common Mainnet Read Recipes

Use these as starting payload shapes, then adjust to the tool's `inputSchema`
if the deployment exposes a slightly different field name.

### Health

Tool: `iroha.status`

```json
{}
```

Tool: `iroha.sumeragi.status`

```json
{}
```

HTTP:

```text
GET https://minamoto.sora.org/status
```

### Alias resolution

Tool: `iroha.aliases.resolve`

```json
{
  "alias": "<label@dataspace>"
}
```

### Account lookup

Tool: `iroha.accounts.get`

```json
{
  "account_id": "<canonical-account-id>"
}
```

### Account assets and fee balance

Tool: `iroha.accounts.assets`

```json
{
  "account_id": "<canonical-account-id>"
}
```

### Asset lookup

Tool: `iroha.assets.get`

```json
{
  "asset_id": "<asset-id>"
}
```

### Asset definition lookup

Tool: `iroha.asset-definitions.get`

```json
{
  "asset_definition_id": "<asset-definition-id>"
}
```

### Recent blocks

Tool: `iroha.blocks.list`

```json
{
  "limit": 10
}
```

### Recent transactions

Tool: `iroha.transactions.list`

```json
{
  "query": {
    "limit": 10
  }
}
```

Some deployments expose flat shortcuts such as `limit`, but explorer
pagination may still return the default page size. Always inspect the returned
`pagination` object and the discovered `inputSchema` before assuming a
shortcut was honored.

### Transaction status

Tool: `iroha.transactions.status`

```json
{
  "hash": "<transaction-hash>"
}
```

### Wait for a transaction

Tool: `iroha.transactions.wait`

```json
{
  "hash": "<transaction-hash>",
  "timeout_ms": 120000
}
```

### Contract instance lookup

Use the deployment's discovered contract read tool. Common names may include
`iroha.contracts.instance.get` or a contract-specific query tool; confirm the
actual name and schema with tool discovery before calling it.

## Response Handling

1. Use each tool's `inputSchema` as the source of truth for accepted fields.
2. Re-run tool discovery if the MCP server reports that the tool list changed.
3. Prefer namespaced Minamoto tools when Codex exposes multiple SORA MCP
   servers in the same session.
4. When a write tool succeeds, summarize the resulting transaction hash or the
   returned status, not just the request payload.
5. When a write tool fails, surface the server error and say whether the issue
   looks like auth, validation, missing tool exposure, or endpoint availability.
6. Treat `route_unavailable` as deployment health, not user input failure:
   the public Torii node is up but the write route still cannot reach an
   authoritative peer.
7. Treat `502` or `503` from the MCP endpoint or public Torii root as ingress
   or rollout-health failures first.
8. Treat `Transaction expired` as a likely stalled, saturated, or slow chain
   path first; report the public `/status` and `/v1/sumeragi/status` sample
   alongside the failure.
9. Treat `403 Forbidden` as a likely signer-permission, authorization, route,
   or policy boundary first, not a malformed request.
10. Treat a Musubi `404 not_found` as an absent alias/package when the Musubi
   tool itself is callable.
11. Treat ignored pagination shortcuts as a schema/pagination issue. Check the
   returned `pagination` object and retry with the schema-supported query
   shape before reporting surprising counts.
12. If public reads succeed but finality appears stalled, describe the issue as
   a public-node or public-finality-path observation unless you have
   validator-side evidence. Avoid overstating that as a full-network outage.

## Agent Output Requirements

For read-only work, report:

- the public root or MCP endpoint used
- tool names used
- relevant block height or status sample timestamp when available
- the exact account, asset, contract, package, or transaction id queried
- any visibility limitation from querying only the public Minamoto endpoint

For submitted transactions, report:

- transaction hash
- final status returned by `submit_and_wait` or `wait`
- block height or committed block reference when available
- signer alias and canonical account id, when known
- target object and operation summary
- verification query and result
- any uncertainty, such as stale public reads or `404` transaction visibility

## Safety

- Do not invent or claim pre-existing operator key material.
- Do not generate, onboard, or fund a Minamoto signer unless the user
  explicitly asks for that mainnet operation and understands it affects live
  state.
- Persist runtime credentials only when the user explicitly asks, and then keep
  them in ignored user-local files rather than tracked repo state.
- Do not assume operator-only routes are available on public Minamoto.
- Prefer anonymous/public reads first when investigating state.
- Prefer user-local MCP entries and configs for Minamoto; do not commit
  mainnet endpoint overrides, auth headers, or signing material into the repo.
