---
name: sora-taira-testnet
description: "Work against the SORA Taira testnet through its deployed Torii MCP endpoint for live account, asset, alias, contract, governance, Musubi package-registry, and transaction workflows. Use when Codex needs to inspect or mutate the Taira testnet, verify or add `https://taira.sora.org/v1/mcp`, prefer the curated `iroha.*` tool surface, classify Taira ingress and chain-health failures correctly, or handle runtime-only signing inputs such as `authority` and `private_key`."
---

# SORA Taira Testnet

Use the Taira testnet through native Torii MCP.

## Quick Start

1. Confirm a Taira MCP server is available in the current Codex environment.
2. Treat `https://taira.sora.org` as the primary public Torii/API origin on the current deployment and `https://taira.sora.org/v1/mcp` as its native MCP endpoint.
3. Before long or stateful writes, sample public health first. Prefer proceeding only when blocks are advancing, `queue_size` is low, `tx_queue_saturated=false`, and `teu_dataspace_backlog` is not climbing.
4. Prefer curated `iroha.*` tools over raw `torii.*` tools.
5. Stay read-only until the user explicitly asks to mutate live state.
6. Treat any signing inputs, API tokens, or forwarded auth headers as runtime-only secrets.

## MCP Endpoint

Use the public Taira MCP endpoint:

- `https://taira.sora.org/v1/mcp`

The matching public Torii/API root is:

- `https://taira.sora.org`

If the endpoint is not configured locally, instruct the user to add a user-local
MCP entry that points at that URL.

If the endpoint returns `404`, report that native Torii MCP is not enabled on
the deployment yet and stop before attempting live-network actions.

If the MCP endpoint or the public Torii root returns `502` or `503`, report
public ingress / rollout health degradation before attempting writes. Treat
that as deployment health, not user input failure.

If reads work but live writes fail with `route_unavailable`, report that the
public ingress still cannot reach authoritative peers for the target lane and
point operators at
`configs/soranexus/taira/check_mcp_rollout.sh --public-root https://taira.sora.org --write-config <runtime-only client.toml>`.
For that runtime-only signer config, prefer
`configs/soranexus/taira/taira-canary-client.example.toml`; the generic
`defaults/client.toml` targets the zero chain id and is not valid for Taira.
When `--write-config` is omitted, the rollout scripts now bootstrap
`/run/secrets/taira-canary-client.toml` automatically by generating a fresh
ordinary signer, onboarding it on Taira, and attempting an initial faucet claim
before the write canary. For that public onboarding flow, keep aliases
dataspace-scoped like `<label>@universal`; do not invent domain-scoped aliases
such as `@wonderland.universal`.
If the rollout is still using hand-edited validator configs, point them at
`configs/soranexus/taira/validator_roster.example.toml`,
`configs/soranexus/taira/validator_secrets.example.toml`, and
`python3 scripts/render_taira_validator_bundle.py --roster ... --secrets ... --output-dir ...`
so every validator shares the same `trusted_peers` / `trusted_peers_pop` roster.

If writes fail with `Transaction expired`, classify that as chain health,
consensus latency, or queue saturation first. Sample `/status` and
`/v1/sumeragi/status` and report the current `blocks`, `commit_qc_height`,
`highest_qc_height`, `queue_size`, `tx_queue_depth`, `tx_queue_saturated`,
`teu_dataspace_backlog`, and `view_change_causes.last_cause` before blaming
the caller.

## Working Rules

1. Prefer `iroha.*` aliases. They are the intended agent-facing surface for
   deployed networks.
2. Use explicit JSON `body` payloads when a write flow needs more than a couple
   of flat shortcut arguments.
3. Keep `authority`, `private_key`, bearer tokens, and forwarded auth headers
   out of files, docs, and commits.
4. Before long write flows, verify the signer on-chain first: the account
   exists on the current Taira chain, holds a positive fee asset balance, and
   has the permissions required for the specific mutation.
5. For Soracloud releases or site publishes, verify the signer still has
   `CanManageSoracloud` and `CanPublishSpaceDirectoryManifest` before starting
   a large upload.
6. If Taira was recently reset or redeployed, treat cached or previously
   faucet-funded signers as suspect until their balance and permissions are
   re-checked on-chain.
7. For pre-signed transaction envelopes, use
   `iroha.transactions.submit_and_wait`.
8. If a mutation would affect live state, restate the intended change clearly
   before executing it unless the user already gave a direct mutation request.
9. For Taira SoraFS or app-api rollout, treat the public read path as healthy
   only when `/v1/app-api/cid/<cid>` is stable `200` and
   `/v1/sorafs/capacity/state` shows `declaration_count >= 1`. Mixed `200` /
   `404` reads or zero declarations are rollout health problems, not a bad CID.
10. For Musubi package-registry writes, use the pre-signing helpers only:
    `iroha.musubi.instructions.publish_release`,
    `iroha.musubi.instructions.yank_release`,
    `iroha.musubi.instructions.set_alias`, and
    `iroha.musubi.instructions.assert_release_exists`. They return unsigned
    Norito-framed instructions; sign and submit them from the client side.

## Public-Node Diagnostics

1. Treat `https://taira.sora.org` as the default public Torii vantage point on
   the current deployment. It is still only one public view, not proof of full
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
   - `GET https://taira.sora.org/status`
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
7. If public reads are healthy but generic signed writes still expire while a
   dedicated lane like SoraFS capacity declaration succeeds, call that a
   partial public-node or public-finality-path failure first, not proof that
   the user payload is malformed.
8. If staged validator lanes are only reachable from the edge host, validate
   them through an SSH tunnel or the rollout scripts' `--resolve-host` path
   before treating the problem as an application bug.
9. If an operator-side nginx reload fails after a config edit on the Homebrew
   edge, check for backup `.conf` files left under the `servers/` include
   directory. Duplicate upstream definitions there will block reload.

## Known Failure Patterns

1. `route_unavailable` while public reads still work usually means the public
   ingress can answer read traffic but cannot reach an authoritative peer for
   the target write lane. Treat that as rollout health first and point
   operators at `configs/soranexus/taira/check_mcp_rollout.sh`.
2. Stable `404` on `/v1/app-api/cid/<cid>` together with
   `/v1/sorafs/capacity/state.declaration_count = 0` usually means public
   SoraFS provider capacity was never declared on-chain or the publish surface
   is pointed at a node that cannot hydrate the CID.
3. Mixed `200` and `404` from repeated `/v1/app-api/cid/<cid>` probes usually
   means the edge is balancing across validators with inconsistent SoraFS
   manifest visibility. Treat that as a rollout or edge-routing problem, not a
   corrupted CID.
4. `check_sorafs_rollout.sh` can pass while `check_mcp_rollout.sh` still fails.
   That means the SoraFS-specific route and declaration path are healthy enough
   for the app-api read surface, but the generic signed-transaction or finality
   lane is still degraded.
5. `Transaction expired` on generic signed writes while a dedicated SoraFS
   declaration succeeds usually points at partial public finality trouble such
   as `missing_qc` or `quorum_timeout`, not necessarily a malformed request.
6. `502` or `503` from `/v1/mcp` or the public Torii root usually means nginx
   or upstream rollout breakage, not a signer or payload problem.

## Preferred Triage Sequence

1. Start with read-only public health:
   - `GET https://taira.sora.org/status`
   - `iroha.status`
   - `iroha.sumeragi.status`
2. If the task involves SoraFS or app-api rollout, check the public read path
   before any write:
   - `GET /v1/sorafs/capacity/state`
   - repeated `GET /v1/app-api/cid/<cid>`
3. If shell access is available, run the read-only public SoraFS ingress check
   first:
   - `bash configs/soranexus/taira/check_sorafs_rollout.sh --public-root https://taira.sora.org --skip-write-canary`
4. If the public read path looks healthy and the task needs generic signed
   writes or native MCP validation, run:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-local --public-root https://taira.sora.org --skip-write-canary`
5. Only after those checks are green should the agent spend time on signer or
   payload debugging. If the user wants a live write canary, then move to the
   full write checks with either an explicit `--write-config` or the automatic
   runtime-only canary bootstrap.
6. When using the rollout scripts, pass the Torii root as `--public-root`
   (`https://taira.sora.org`), not the MCP URL. The scripts derive `/v1/mcp`
   themselves.

## MCP Write Recipes

### Bootstrap a fresh runtime-only signer

If the user wants a fresh ordinary signer for rollout checks, prefer the helper
script over hand-editing TOML:

```bash
python3 scripts/taira_bootstrap_canary.py \
  --torii-root https://taira.sora.org \
  --output-config /run/secrets/taira-canary-client.toml
```

That flow generates a new local Ed25519 keypair, onboards it on Taira, attempts
the public faucet claim, and writes a runtime-only client config with rollout-
specific TTL and status timeout. Keep the generated config out of tracked repo
state unless the user explicitly asks to persist it.

### Verify the bootstrapped signer before writes

After bootstrap, confirm the alias, account, and fee balance before debugging
permissions or payloads:

- resolve the alias with `iroha.aliases.resolve`
- fetch the canonical account with `iroha.accounts.get`
- inspect balances with `iroha.accounts.assets`

If the signer is missing entirely, re-run the bootstrap flow. If the signer
exists but has no fee asset balance, treat that as a faucet or funding problem
before blaming the write payload.

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

### Inspect Musubi packages

Use the curated Musubi tools for package reads:

- `iroha.musubi.search` with `query`, optional `namespace`, `include_yanked`,
  `offset`, and `limit`
- `iroha.musubi.release.get` with `package = "namespace/name@version"`
- `iroha.musubi.package.releases` with `package = "namespace/name"` and
  optional `include_yanked`
- `iroha.musubi.package.versions` with `package = "namespace/name"`
- `iroha.musubi.alias.resolve` with `alias = "<short-name>"`

Musubi namespaces intentionally do not use a leading `@`; use literals like
`universal`, `dex.universal`, and `dex.universal/swap-core`.

### Build Musubi instructions for local signing

The Musubi instruction tools do not accept `authority`, `private_key`, bearer
tokens, or any other signing material. They return `wire_id`,
`instruction_base64`, `instruction_hex`, and an `instruction_json` preview.
The client must assemble a signed transaction locally and submit it with
`iroha.transactions.submit_and_wait`.

Example yank instruction payload:

```json
{
  "package": "dex.universal/swap-core@1.2.3",
  "reason": "superseded"
}
```

Use `signed_tx_base64` or `tx_base64` for base64 envelopes, or the hex variants
for hex-encoded envelopes. Do not pass multiple envelope encodings in the same
request. If the call returns `Transaction expired`, go back to the public
health and Sumeragi checks before rebuilding the transaction.

### Rollout reads for SoraFS and app-api

For public trader/app rollout checks, use this read bundle before attempting a
republish or signer mutation:

- `iroha.status`
- `iroha.sumeragi.status`
- `GET https://taira.sora.org/v1/sorafs/capacity/state`
- repeated `GET https://taira.sora.org/v1/app-api/cid/<cid>`

If shell access is available, prefer the bundled rollout checks:

```bash
bash configs/soranexus/taira/check_sorafs_rollout.sh \
  --public-root https://taira.sora.org \
  --skip-write-canary

bash configs/soranexus/taira/check_mcp_rollout.sh \
  --skip-local \
  --public-root https://taira.sora.org \
  --skip-write-canary
```

Treat `check_sorafs_rollout.sh` as the SoraFS/app-api surface check and
`check_mcp_rollout.sh` as the generic signed-write and native MCP surface
check. They can fail independently.

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
6. Treat `502` or `503` from the MCP endpoint or public Torii root as ingress
   or rollout-health failures first.
7. Treat `Transaction expired` as a likely stalled, saturated, or slow chain
   path first; report the public `/status` and `/v1/sumeragi/status` sample
   alongside the failure.
8. Treat `403 Forbidden` after a chain reset as a likely signer-permission
   problem first, not a malformed request.
9. Treat `Failed to find asset` on the signed rollout canary as a likely
   unfunded-signer condition first; use the rollout smoke or the public faucet
   endpoints before assuming the chain rejected the signer itself.
10. If `/v1/app-api/cid/<cid>` flaps between `200` and `404` and
    `/v1/sorafs/capacity/state` shows `declaration_count = 0`, classify it as
    a public SoraFS provider-capacity or read-path failure first, not a bad
    CID.
11. If public reads succeed but finality appears stalled, describe the issue as
    a public-node or public-finality-path observation unless you have
    validator-side evidence. Avoid overstating that as a full-network outage.

## Safety

- Do not invent or claim pre-existing operator key material.
- If the user explicitly asks for a fresh runtime-only signer, it is valid to
  generate a new local keypair, onboard it on Taira, and fund it through the
  public faucet. Do not present it as a shared or pre-existing credential.
- Persist runtime credentials only when the user explicitly asks, and then keep
  them in ignored user-local files rather than tracked repo state.
- Do not assume operator-only routes are available on public Taira.
- Prefer anonymous/public reads first when investigating state.
