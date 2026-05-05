# Sora Taira public NPoS bootstrap

Taira is the Sora Nexus public testnet. This directory
contains the repo-shipped bootstrap bundle for a public, stake-elected NPoS
deployment.

## Network identity

- Public chain ID: `809574f5-fee7-5e69-bfcf-52451e42d50f`
- Address chain discriminant: `369` (this is what drives canonical I105 literals such as `testu...`)

## Public API contract

For the examples below, replace `PUBLIC_TORII_ROOT` with the live public
Torii URL you are validating. On the current deployment that is:

- `PUBLIC_TORII_ROOT=https://taira.sora.org`

Validator-specific hostnames such as `https://taira-validator-1.sora.org` may
also exist as alternate public Torii roots when the edge is configured to
expose them directly.

Use the exact public root the deployment or operator gives you. Do not
silently swap `https://taira.sora.org` for a guessed direct-validator host,
and do not infer validator-set size from `/status.peers`: that field is the
queried node's current remote-peer count, not the chain's validator-set size.

`https://taira.sora.org` is the shared Torii origin, not an app website. Do
not bind product frontends to that host root through `sorafs_sites.json`.

For day-to-day validation, prefer the first-class CLI:

- `iroha taira doctor --public-root https://taira.sora.org --output-format text`
- `iroha taira write-canary --public-root https://taira.sora.org --output-format text`

`check_mcp_rollout.sh` remains the fuller rollout harness for operators who
need local/public comparisons, curl `--resolve` overrides, or shell-only
environments. The CLI is the blessed single-node Taira devex path and emits a
redacted JSON receipt with `--json`.

The shipped public Taira profile pins the first-release Torii posture in
config rather than wrapper-local defaults:

- `torii.max_content_len = 1_073_741_824`
- `torii.deploy_rate_per_origin_per_sec = 4`
- `torii.deploy_burst_per_origin = 8`
- `torii.webhooks_enabled = false`
- `torii.zk_attachments_enabled = false`

## Included artifacts

- `config.toml`: baseline validator config for peer 1 and the shared template
  source for rendered per-validator configs. The checked-in file is template
  only and intentionally does not carry runtime-only private keys.
- `validator_roster.example.toml`: copy-me roster template for all validator
  public addresses, public keys, and PoPs. Keep the populated file user-local.
- `validator_secrets.example.toml`: copy-me secret template for per-validator
  private keys plus the shared onboarding/faucet authority and streaming
  identity key material. Keep the populated file user-local.
- `genesis.json`: NPoS genesis with DA enabled.
- `dns_records.json`: DNS targets for the convenience host, explorer host, and
  direct per-validator Torii hostnames.
- `explorer.runtime-config.json`: runtime config example for the Explorer
  frontend; point it at the explicit public Torii base URL you want the UI to
  query.
- `sorafs_sites.json`: optional host-to-manifest bindings for Torii-served static sites. Keep `taira.sora.org` out of this file.
- `sorafs_gateway_denylist.catalog.json`: default-on SoraFS denylist pack catalog.
- `sorafs_gateway_denylist.global-core.json`: baseline governance-backed illegal-content pack.
- `sorafs_gateway_denylist.global-emergency.json`: emergency-response denylist pack.
- `taira-irohad.service`: sample systemd unit that starts the validator from
  the shipped Taira config and genesis.
- `taira-irohad.env.example`: sample `/etc/default/taira-irohad` overrides for
  pointing the systemd unit at a rendered validator config.
- `docker-compose.validator.yml`: sample containerized validator deployment
  that mounts one rendered validator config plus persistent `/storage`.
- `taira-validator-container.compose.env.example`: sample compose env file for
  a single validator host using the published Taira image.
- `taira-validator-container.sh`: plain-`docker` wrapper for hosts that do not
  have the Docker Compose plugin installed.
- `taira-validator-container.service`: sample systemd wrapper that keeps the
  validator container under service management without requiring Docker Compose.
- `scripts/render_taira_localnet_container_bundle.py`: rewrites a fresh
  `kagami localnet` bundle into four container-ready configs/env files with
  canonical `addr:...#CRC16` literals for shared-bridge Docker validation.
- `taira-canary-client.example.toml`: runtime-only example signer config for
  the signed rollout canary.
- `build_taira_rollout_bundle.sh`: packages the exact checked-out `irohad` /
  `iroha` / `sorafs_manifest_stub` / `sorafs_tx_stdin_builder` build plus the
  checked-in Taira config bundle into one timestamped
  rollout artifact, building `irohad` with
  `--features embedded-soracloud-runtime`, and records the git revision in
  `rollout.manifest.json`.
- `scripts/render_taira_edge_nginx_conf.py`: renders the shared-edge nginx
  config directly from the same validator roster used for per-validator
  `config.toml` generation so public Torii ingress cannot drift onto stale
  loopback ports.
- `check_mcp_rollout.sh`: smoke script for the local and public `/v1/mcp`
  checks used by the Taira Codex rollout, including the optional signed write
  canary for final public cutover.
- `check_sorafs_rollout.sh`: public SoraFS surface + signed capacity-declaration
  canary that catches stale validators still missing the capacity/order ISI
  dispatch table.
- `verify_soraswap_rollout.sh`: post-upgrade wrapper that runs the public MCP
  canary, the SoraFS capacity canary, the SoraSwap nested-call probe, and then the optional
  `deploy-testnet` / signed `smoke-testnet` / `release-checklist` chain in the
  canonical order.
- `bootstrap_kaigi_localnet.sh`: local-only relay bootstrap that re-signs the
  served `dist/taira-localnet` genesis with seeded Kaigi relay metadata,
  health samples, and one shared local onboarding/faucet signer account, then
  rewrites the live peer configs and restarts the detached
  `taira-localnet` session.
- `taira-explorer.nginx.conf`: example rendered multi-domain nginx edge config
  for `taira.sora.org`, `taira-explorer.sora.org`, and the current
  `taira-validator-{1,2,3,4}.sora.org` direct-hostname layout on a shared host.

## Render validator configs

Do not hand-edit `config.toml` into multiple validator copies. Instead:

1. Copy `validator_roster.example.toml` to a user-local path such as
   `configs/soranexus/taira/validator_roster.local.toml`.
2. Copy `validator_secrets.example.toml` to a user-local path such as
   `configs/soranexus/taira/validator_secrets.local.toml`.
3. Fill in every validator's real `public_key`, `pop_hex`, and
   `public_address` plus its own direct `torii_public_address` in the public
   roster, then put the matching validator `private_key` values and the shared
   `torii_onboarding_*`, `torii_faucet_*`, and `streaming_identity_*` values
   in the secrets file.
3. Render the per-validator bundle:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
4. Point each validator host at its own generated
   `dist/taira-validators/<validator-slug>/config.toml`.

The renderer rewrites the checked-in peer-1 baseline with the full
`trusted_peers` / `trusted_peers_pop` roster so every validator starts from the
same bootstrap source of truth. It now requires explicit per-validator
`torii_public_address` values so direct public Torii hostnames are part of the
checked operator input instead of a hard-coded shared edge default.

## Private profiles

Application-specific private-dataspace profiles should live outside this repo.
When you need one, keep the profile in your own deployment repository and pass
it to the renderer explicitly:

```bash
python3 scripts/render_taira_validator_bundle.py \
  --base-config /absolute/path/to/private-profile.toml \
  --roster configs/soranexus/taira/validator_roster.local.toml \
  --secrets configs/soranexus/taira/validator_secrets.local.toml \
  --output-dir dist/taira-private-validators
```

For a true genesis reset on a validator host, stop the runtime and wipe the
mounted state before starting again:

```bash
bash configs/soranexus/taira/taira-validator-container.sh \
  --env-file /etc/default/taira-validator-container.compose.env reset

bash configs/soranexus/taira/taira-validator-container.sh \
  --env-file /etc/default/taira-validator-container.compose.env up
```

That sequence removes the mounted validator state under `TAIRA_STORAGE_PATH`
after stopping the container, which is the required step for a true genesis
reset.

When you run the shared nginx edge on one host, keep the same roster as the
source of truth for the edge upstreams too:

- add `edge_torii_upstream = "<host>:<port>"` for each validator entry
- render the nginx snippet from that roster instead of hand-editing ports:
  - `python3 scripts/render_taira_edge_nginx_conf.py --roster configs/soranexus/taira/validator_roster.local.toml --output dist/taira-edge/taira.sora.org.conf`

That avoids the common drift where the copied nginx snippet still points at
`127.0.0.1:18080..18083` while the live validator listeners have moved to
different loopback ports such as `127.0.0.1:29080..29083`, which turns
`GET /v1/mcp` and the generic public API surface into `502 Bad Gateway`.

## Validator container image

The repo now supports a dedicated Taira validator runtime image via the main
`Dockerfile`:

- local build helper:
  - `scripts/build_release_image.sh --profile iroha3 --config taira`
- manual publish workflow:
  - `.github/workflows/publish_taira_validator.yml`

Manual publish prerequisites:

- GitHub Actions secrets:
  - `DOCKERHUB_USERNAME`
  - `DOCKERHUB_TOKEN`
  - `HARBOR_USERNAME`
  - `HARBOR_TOKEN`
- a self-hosted runner with enough RAM to finish the deploy-profile Rust
  compile inside `docker build`; the current Taira workflow now forces
  `CARGO_BUILD_JOBS=1` and `BINARIES=irohad` so validator-image publishing is
  not blocked by unrelated CLI build failures or Colima memory pressure
- one explicit `workflow_dispatch` run against the chosen release ref so the
  first `hyperledger/iroha:taira-*` and
  `docker.soramitsu.co.jp/iroha3/iroha:taira-*` tags actually exist before
  operator hosts switch to the published image path

If the Docker host is memory-constrained, cap Cargo parallelism during the
image build:

- `scripts/build_release_image.sh --profile iroha3 --config taira`
- or, for a direct Docker build, `docker build --build-arg CONFIG_PROFILE=taira --build-arg FEATURES=embedded-soracloud-runtime --build-arg CARGO_BUILD_JOBS=1 --build-arg BINARIES=irohad ...`

The image ships:

- `irohad`
- the checked-in static Taira bundle under `/opt/iroha/configs/soranexus/taira`
- the bundled rANS codec tables under `/opt/iroha/codec/rans/tables`
- a Taira-aware entrypoint that defaults to:
  - `irohad --sora --config /config/config.toml --genesis-manifest-json /opt/iroha/configs/soranexus/taira/genesis.json`

The image does **not** embed validator-specific runtime material. Keep using
`render_taira_validator_bundle.py` to generate the mounted
`/config/config.toml` from user-local roster/secrets files.

`docker-compose.validator.yml` uses `pull_policy: missing` so a host-local
`docker load` override works without forcing a registry lookup. When you want
to refresh a published tag explicitly, run `docker compose ... pull` before
`up -d`.

Minimal container run example:

```bash
docker run -d --name taira-validator-1 \
  --restart unless-stopped \
  -p 1337:1337 \
  -p 18080:8080 \
  -v "$PWD/dist/taira-validators/taira-validator-1/config.toml:/config/config.toml:ro" \
  -v /var/lib/iroha/taira-validator-1:/storage \
  hyperledger/iroha:taira-latest
```

If you need to override the bundled public genesis, point the entrypoint at a
mounted manifest file:

```bash
docker run --rm \
  -e IROHA_TAIRA_GENESIS=/config/genesis.json \
  -v "$PWD/dist/taira-validators/taira-validator-1/config.toml:/config/config.toml:ro" \
  -v "$PWD/configs/soranexus/taira/genesis.json:/config/genesis.json:ro" \
  -v /var/lib/iroha/taira-validator-1:/storage \
  hyperledger/iroha:taira-latest
```

If you need a disconnected or one-node smoke boot, mount both the manifest JSON
and a signed genesis payload. The entrypoint rewrites the copied
`/storage/runtime-config.toml` so `genesis.file` points at the mounted
`/config/genesis.signed.nrt` path:

```bash
docker run --rm \
  -e IROHA_TAIRA_GENESIS=/config/genesis.json \
  -e IROHA_TAIRA_SIGNED_GENESIS=/config/genesis.signed.nrt \
  -v "$PWD/dist/taira-localnet-smoke-container.toml:/config/config.toml:ro" \
  -v "$PWD/dist/taira-localnet-smoke/genesis.json:/config/genesis.json:ro" \
  -v "$PWD/dist/taira-localnet-smoke/genesis.signed.nrt:/config/genesis.signed.nrt:ro" \
  -v "$PWD/dist/taira-localnet-smoke-container-storage:/storage" \
  -p 28080:8080 \
  hyperledger/iroha:taira-latest
```

The checked-in `check_mcp_rollout.sh --skip-public --local-root ...` helper
still expects at least 4 live validators in `/status`, so a one-node smoke
should be validated directly with `curl /health`, `curl /status`, and
`curl /v1/mcp` instead of the full rollout script.

For a local 4-validator container proof, render container-ready configs from a
fresh `kagami localnet` bundle and start the peers on one user-defined Docker
bridge:

```bash
python3 scripts/render_taira_localnet_container_bundle.py \
  --bundle-dir dist/taira-localnet-smoke \
  --output-dir dist/taira-localnet-cluster

docker network create taira-localnet >/dev/null 2>&1 || true
for peer in 0 1 2 3; do
  bash configs/soranexus/taira/taira-validator-container.sh \
    --env-file "dist/taira-localnet-cluster/peer${peer}.env" up
done

bash configs/soranexus/taira/check_mcp_rollout.sh \
  --skip-public \
  --local-root http://127.0.0.1:28080 \
  --skip-write-canary
```

That path is now validated on this host: peer0 publishes `/status` with
`commit_qc_validator_set_len = 4`, and the repo rollout script passes end to
end against the local cluster.

## Minimum viable topology

Use at least 4 validator peers (plus optional observers). Single-peer setups are
not representative for NPoS and can stall DA/RBC consensus paths.

Suggested validator hostnames:

- `taira-validator-1.sora.org`
- `taira-validator-2.sora.org`
- `taira-validator-3.sora.org`
- `taira-validator-4.sora.org`

## Bootstrap peers vs active validators

- `trusted_peers` and `trusted_peers_pop` are bootstrap discovery inputs, not
  the validator-admission policy.
- `config.toml` explicitly sets `sumeragi.npos.use_stake_snapshot_roster = true`
  and `nexus.staking.public_validator_mode = "stake_elected"`, so the active
  validator roster comes from on-chain public-lane staking state.
- The checked-in/public roster file is therefore a deploy/bootstrap artifact.
  It helps nodes find each other and agree on the bootstrap set after genesis,
  but it does not decide which operators stay active validators over time.
- Taira resets should seed only the minimum bootstrap validators needed to
  start the chain. After genesis, validator-set growth is driven by XOR stake
  plus the active-validator snapshot views.

## Public validator join flow

Use the public-lane staking flow for validator candidacy instead of manual
allowlisting:

1. Render a per-validator config with the node's own `public_address` and
   `torii_public_address`, then start `irohad` against the published seed peers.
2. Wait for the node to sync and confirm lane mode:
   - `iroha app nexus lane-report --summary`
   - `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq .`
3. Fund the candidate account with `xor#universal`.
4. Register the validator on the public lane with its live peer identity:
   - `iroha app staking register --lane-id 0 --validator <i105-account-id> --peer-id <peer-id> --initial-stake <amount>`
5. When the activation boundary is reached, activate the candidacy if needed:
   - `iroha app staking activate --lane-id 0 --validator <i105-account-id>`
6. Verify that the node is visible through on-chain staking and validator-set
   views rather than a static file roster:
   - `iroha app nexus public-lane validators --lane 0 --summary`
   - `iroha app nexus public-lane stake --lane 0 --validator <i105-account-id> --summary`
   - `curl -sS "${PUBLIC_TORII_ROOT}/v1/nexus/public_lanes/0/validators" | jq .`
   - `curl -sS "${PUBLIC_TORII_ROOT}/v1/sumeragi/validator-sets" | jq .`

## Public endpoints

- `https://taira.sora.org` is the primary public Torii/API origin on the
  current deployment. Keep it on Torii/API duties only and do not mount
  websites at its root.
- Every public validator should still be able to expose Torii directly on its
  own TLS hostname and advertise that URL through `[torii].public_address`
  when validator-specific ingress is desired.
- `https://taira-explorer.sora.org` points to the Iroha 2 Explorer instance.
- Shared nginx edge configs such as `taira-explorer.nginx.conf` are optional
  convenience infrastructure, not the primary public API design.

### SoraFS CID gateway

Taira serves SoraFS-published static content primarily through immutable CID
gateway paths on the Torii origin:

- `GET /sorafs/cid/<cid>/`
- `GET /sorafs/cid/<cid>/<path...>`
- `GET /v1/sorafs/cid/<cid>` for lookup metadata

For the Polkaswap static bundle, the browser URL is:

- `${PUBLIC_TORII_ROOT}/sorafs/cid/<cid>/`

This keeps the chosen public node as the Torii/API origin while giving every
public Torii node an IPFS-style address surface for static content.

Gateway behavior:

- Torii serves CID routes from local storage when the manifest is already
  cached.
- On a local miss, Torii resolves the CID through the approved replication
  order set, uses the provider advert cache to find a Torii-capable provider,
  fetches the manifest and payload over the existing storage endpoints, and
  stores the bundle locally before serving it.
- Keep both `torii.sorafs.discovery_enabled = true` and
  `torii.sorafs_storage.enabled = true` on public gateway nodes so CID
  browsing can rehydrate from peer providers.

Named host bindings in `sorafs_sites.json` remain available as an optional
alias layer, but they are no longer the primary deployment path. Reserve
`taira.sora.org` for Torii itself and serve apps from `/sorafs/cid/<cid>/` or
`<cid>.sorafs.taira.sora.org`.

Soracloud runtime apps use the SoraDNS/Soracloud alias route instead of SoraFS
CID hosts. For clients without native SoraDNS resolution, the public browser
gateway is `<alias>.mon.taira.sora.net`, for example
`https://solswap-indexer.sora.mon.taira.sora.net/api/indexer/v1/health`. Keep
`https://mon.taira.sora.net/soradns/<alias>/...` available as the Mon debug
fallback and `https://taira.sora.org/soradns/<alias>/...` available only as the
legacy Torii compatibility path.

### Default denylist packs

Taira now loads a default-on denylist pack catalog from:

- `configs/soranexus/taira/sorafs_gateway_denylist.catalog.json`

The shipped catalog enables these packs by default:

- `global-core`
- `global-emergency`

Operators can opt out of a pack via `[sorafs.gateway.denylist].opt_out_packs`
or add explicit subscriptions via `extra_packs`.

The governance trail for denylist updates should use the existing Ministry /
Parliament flow and the examples already shipped in the repo:

- `docs/examples/ministry/agenda_proposal_example.json`
- `docs/examples/ministry/referendum_packet_example.json`

Taira's public edge also needs to accept the storage payload upload that
precedes root serving. The current SoraFS storage pin API sends the full staged
site in one JSON request (`payload_b64`), so the nginx host serving
the chosen public Torii hostname must keep `client_max_body_size 1g;` from
`taira-explorer.nginx.conf`. Without that, `yarn taira:publish` fails at
`POST /v1/sorafs/storage/pin` with `413 Payload Too Large` before Torii sees
the request. Torii must also run with `torii.max_content_len` high enough for
the base64-expanded JSON body; the shipped Taira profile now pins that to
`1_073_741_824`, and the local bootstrap overlay now rewrites the served
`dist/taira-localnet/peer*.toml` files to keep that same cap live after every
reset. Torii and the Rust client both reserve a 10 minute route/request budget
for `POST /v1/sorafs/storage/pin`, because publish-sized base64 JSON envelopes
can take longer than the generic 70 second Torii request window. Taira also
overrides `[sorafs.quota] storage_pin_max_events = 64` so
publish/retry loops on the public testnet do not immediately exhaust the
generic `4/hour` storage-pin quota inherited from the global default.

After every Taira reset or `irohad` rebuild, verify the manifest-registration
ingress before retrying `yarn taira:publish`:

- `curl -sSki -X POST "${PUBLIC_TORII_ROOT}/v1/sorafs/pin/register" -H 'content-type: application/json' --data '{}'`

Expected result:

- `HTTP 400` with a handler-level validation error such as `missing field authority`

Unexpected result:

- `HTTP 405` with `Allow: GET,HEAD`

That `405` means the served `irohad` is stale and missing the mounted
`POST /v1/sorafs/pin/register` route, even if
`GET /v1/sorafs/pin/register` still falls through to the digest lookup
handler.

### Codex / MCP rollout

Each public Taira node should expose native MCP on the same direct Torii root
once the validator is
redeployed with the shipped `[torii.mcp]` block from `config.toml`:

- `torii.mcp.enabled = true`
- `torii.mcp.profile = "writer"`
- `torii.mcp.expose_operator_routes = false`
- `torii.mcp.allow_tool_prefixes = ["iroha."]`

This intentionally exposes only curated `iroha.*` tools on the public network
so Codex sees the stable live-network aliases and not the full raw `torii.*`
OpenAPI-derived surface. The rollout smoke now also rejects any advertised MCP
tool whose top-level `inputSchema` is not an OpenAI-compatible object schema.

After rollout, verify the chosen public node directly:

- `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" | jq .`
- `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"probe","version":"1"}}}' | jq .`
- `curl -sS -D - "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'`
- `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq .`
- `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq .`

The `notifications/initialized` probe should return `HTTP 202 Accepted` with
an empty body. A `200` JSON-RPC error there means the endpoint advertises MCP
but still fails the standard post-initialize handshake that Codex and other
streamable-HTTP MCP clients require.

The `tools/list` payload must also keep every tool's `inputSchema` as a
top-level `"type": "object"` schema without top-level `anyOf`, `oneOf`,
`allOf`, `enum`, or `not`. If a live node still advertises an invalid schema
for tools such as `iroha.connect.session.delete`, `check_mcp_rollout.sh` now
fails the rollout immediately instead of letting Codex discover the breakage.

The repo-local Codex plugin and Taira skill now treat
`https://taira.sora.org/v1/mcp` as the current primary public MCP endpoint
while still allowing operator-provided alternate public roots. Future
Nexus/Torii deployments should keep the same `/v1/mcp` path and be added as
user-local MCP servers with the exact public root under test.

For final public rollout, do not stop at MCP discovery. Run the repo smoke with
both the public endpoint and a runtime-only canary signer config:

- `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml`

Then gate the SoraFS path on the same public node:

- `bash configs/soranexus/taira/check_sorafs_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml`

Expected result:

- `POST /v1/sorafs/pin/register`, `POST /v1/sorafs/capacity/declare`, and
  `POST /v1/sorafs/capacity/schedule` return `HTTP 400` for an empty JSON body,
  not `HTTP 405`
- the signed capacity canary lands and becomes visible in
  `GET /v1/sorafs/capacity/state`

If the canary fails with `Unknown instruction type`, the served validator build
is stale and missing the SoraFS capacity/order entries in
`iroha_core`'s instruction dispatch table even if the Torii route surface is
otherwise up.

On a freshly reset local bundle, the same signed canary now tolerates the brief
startup window where `/status` has no commit QC yet, submits the first
post-genesis write, and then re-checks `/status` strictly after that write
lands.

The rollout script now also requires the live `/status` snapshot to show at
least 4 validators in the commit QC set. If it fails that check, rebuild the
validator configs from the shared roster before debugging ingress or MCP.
It also verifies that the same direct node serves:

- `/v1/sccp/capabilities`
- `/v1/sccp/manifests`
- `/v1/zk/proofs/count`
- `/v1/sumeragi/validator-sets`
- `/v1/nexus/public_lanes/0/{validators,stake}`
- `/v1/bridge/messages` preflight
- `/v1/contracts/deploy`
- `/v1/contracts/state`

That config must be a normal `iroha` client TOML for a low-risk runtime-only
signer. Start from `taira-canary-client.example.toml`, not
`defaults/client.toml`: the generic repo client uses the zero chain id and is
not valid for Taira. If the configured file is missing or still contains the
placeholder authority, the rollout scripts now generate a fresh keypair,
onboard the account on public Taira, solve the faucet puzzle, claim starter
funds, and rewrite `/run/secrets/taira-canary-client.toml` automatically before
retrying the signed ping. If alias onboarding is unavailable but the public
faucet is healthy, the bootstrap falls back to faucet account registration so
the signed write canary can still prove the public transaction path. The signed
ping attaches Taira's accepted XOR gas asset metadata by default; pass
`--gas-asset-id ""` only against a network that does not enforce pipeline gas
metadata. Keep the populated canary config out of the repo and out of shell
history where possible.

If the script fails with `route_unavailable`, treat that as a deployment or
topology failure, not an app-level validation issue: the public Torii ingress is
up, but it still cannot reach an authoritative peer for lane `0` / dataspace
`0`.
If it fails with `Failed to find asset` even after the automatic faucet
bootstrap path runs, treat that as a faucet-health or signer-selection issue:
the configured account either does not exist on Taira yet or the live faucet
could not fund it.

### Public write failure triage

When public reads succeed but writes fail or hang, classify the failure from
the queried public Torii node first before assuming a malformed request or a
full validator-set outage.

Before long public writes such as Soracloud releases or large SoraFS publishes:

- treat `https://taira.sora.org` as the current primary public Torii/API root
- confirm that the public read path is advancing before trusting writes:
  - `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq '{blocks, queue_size, peers, sumeragi: {commit_qc_height, commit_qc_validator_set_len, tx_queue_depth, tx_queue_saturated}, teu_dataspace_backlog}'`
  - `curl -sS "${PUBLIC_TORII_ROOT}/v1/sumeragi/status" | jq '{commit_qc_height: (.commit_qc_height // .commit_qc.height // null), highest_qc_height: (.highest_qc_height // .highest_qc.height // null), view_change_last_cause: (.view_change_causes.last_cause // null), worker_loop_stage: (.worker_loop.stage // null)}'`
- verify the signer you intend to use still exists on the current Taira chain
  and still has a positive fee-asset balance
- for Soracloud mutations specifically, also verify that the signer still
  holds `CanManageSoracloud` and `CanPublishSpaceDirectoryManifest` before
  starting a large upload
- after a Taira reset or redeploy, treat cached or previously faucet-funded
  signers as stale until those checks pass again

When a public write still fails, start with the same status samples above.

Interpret the common public failures as follows:

- `502` / `503` from `GET /v1/mcp`, `POST /v1/mcp`, or other public Torii routes:
  ingress or rollout health degradation. Treat this as deployment health first.
- `route_unavailable` from a live write:
  the public Torii ingress is up, but the write path still cannot reach an
  authoritative peer for the target lane. Capture the response headers
  `x-iroha-route-lane-id`, `x-iroha-route-dataspace-id`,
  `x-iroha-route-unavailable-reason`,
  `x-iroha-route-authoritative-total`,
  `x-iroha-route-authoritative-offline`, and
  `x-iroha-route-loop-prevention-drops`; they identify whether the failure is
  a missing authoritative binding, offline authoritative peers, or proxy-hop
  loop prevention.
- successful read/query fanout with non-zero
  `x-iroha-fanout-routes-failed`, `x-iroha-fanout-routes-unavailable`, or
  `x-iroha-fanout-routes-not-found`:
  the public read was recovered from another dataspace, but some authoritative
  routes are degraded. Capture all `x-iroha-fanout-*` headers with the status
  samples before deciding the request is fully healthy.
- `Transaction expired`:
  likely chain-health, consensus-latency, or queue-saturation trouble first.
  Report the current `blocks`, `commit_qc_height`, `queue_size`,
  `tx_queue_depth`, `tx_queue_saturated`, `teu_dataspace_backlog`,
  `highest_qc_height`, and `view_change_last_cause` samples alongside the
  failure.
- `403 Forbidden` immediately after a reset or redeploy:
  likely signer-permission or signer-state drift first. Re-check that the
  signer still exists on-chain, still holds a fee asset balance, and still has
  the permissions required for the mutation.
- `GET /v1/transactions/status?hash=...` returning `404 not_found` for a
  previously submitted hash:
  the queried public node currently has no visibility for that hash. Do not
  infer commit, reject, or network-wide disappearance from that result alone.

If the latest committed block timestamp stops advancing and `/v1/sumeragi/status`
shows signals such as `membership.height > commit_qc.height`,
`view_change_causes.last_cause = "missing_qc"`, or `worker_loop.stage = "idle"`,
report that the queried public Torii finality path appears stalled. Unless you
also have validator-side access, describe that as a public-node or
public-finality-path observation rather than proof that the full validator set
is down.

## Governance mode

`config.toml` pins Taira to Sora parliament sortition governance for Nexus lanes:

- `nexus.governance.default_module = "parliament"`
- `nexus.governance.modules.parliament.module_type = "parliament_sortition_jit"`
- governance lane metadata binds lane 1 to `governance = "parliament"`
- top-level `[gov]` sets multibody committee/quorum parameters

This avoids fallback to legacy council-epoch approval mode during deployment.

## Fee config

Taira must declare the Nexus fee asset explicitly as the live XOR alias:

```toml
[nexus.fees]
fee_asset_id = "xor#universal"
fee_sink_account_id = "testuﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
base_fee = "0"
per_byte_fee = "0"
per_instruction_fee = "0.001"
per_gas_unit_fee = "0.00005"
sponsorship_enabled = false
sponsor_max_fee = "0"
```

Without this block, public app-api writes can fall back to the canonical default
fee selector for `universal.xor`, which does not match Taira's on-chain
`xor#universal` asset-definition alias and causes public deploy/call
transactions to be rejected before SoraSwap instances can activate.

## Containerized validator deployment

Use this path when the validator host should run the published Docker image
instead of locally installed `irohad` binaries. The primary wrapper is
`taira-validator-container.sh`, which uses plain `docker` and therefore works
on hosts that lack the Compose plugin. `docker-compose.validator.yml` remains
available as an optional convenience for environments that do have Compose.

1. Publish or otherwise load the image you intend to run on the host.
   - manual publish path:
     `workflow_dispatch` `.github/workflows/publish_taira_validator.yml`
   - local host-side override:
     `docker load < iroha3-<version>-linux-image.tar`
2. Render the validator config bundle from your user-local roster and secrets:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
3. Install the rendered config and storage directories on the validator host:
   - `sudo install -d -o 1001 -g 1001 /etc/iroha/taira-validator-1`
   - `sudo install -d -o 1001 -g 1001 /var/lib/iroha/taira-validator-1`
   - `sudo cp dist/taira-validators/taira-validator-1/config.toml /etc/iroha/taira-validator-1/config.toml`
4. Copy the sample env file and adjust the host-specific values:
   - `sudo cp configs/soranexus/taira/taira-validator-container.compose.env.example /etc/default/taira-validator-container.compose.env`
   - set at least:
     - `TAIRA_IMAGE=hyperledger/iroha:taira-latest` or the exact pushed `taira-<suffix>` tag
     - `TAIRA_CONFIG_PATH=/etc/iroha/taira-validator-1/config.toml`
     - `TAIRA_STORAGE_PATH=/var/lib/iroha/taira-validator-1`
     - `TAIRA_TORII_PORT=18080` unless your ingress expects another loopback port
5. Start the validator directly with the plain-`docker` wrapper:
   - `bash configs/soranexus/taira/taira-validator-container.sh --env-file /etc/default/taira-validator-container.compose.env up`
   - `bash configs/soranexus/taira/taira-validator-container.sh --env-file /etc/default/taira-validator-container.compose.env status`
   - `bash configs/soranexus/taira/taira-validator-container.sh --env-file /etc/default/taira-validator-container.compose.env logs`
   - to inspect the exact `docker run` invocation before starting:
     `bash configs/soranexus/taira/taira-validator-container.sh --env-file /etc/default/taira-validator-container.compose.env config`
6. If you prefer Docker Compose and the host actually has the plugin, the
   equivalent commands are:
   - `docker compose --env-file /etc/default/taira-validator-container.compose.env -f configs/soranexus/taira/docker-compose.validator.yml up -d`
   - `docker compose --env-file /etc/default/taira-validator-container.compose.env -f configs/soranexus/taira/docker-compose.validator.yml ps`
   - `docker compose --env-file /etc/default/taira-validator-container.compose.env -f configs/soranexus/taira/docker-compose.validator.yml logs --tail=200`
7. If you want systemd ownership, install the wrapper service:
   - `sudo cp configs/soranexus/taira/taira-validator-container.service /etc/systemd/system/`
   - if the repo checkout is not `/opt/iroha`, edit the script path in
     `ExecStart*=` before enabling the unit
   - `sudo systemctl daemon-reload`
   - `sudo systemctl enable --now taira-validator-container.service`
8. Prove the local MCP surface before any public cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --skip-write-canary`
   - for a signed local write-path check:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --write-config /run/secrets/taira-canary-client.toml --write-target local`

Optional container overrides:

- if you need a validator-specific `genesis.json`, uncomment the matching
  `IROHA_TAIRA_GENESIS` and volume lines in
  `docker-compose.validator.yml`, then set `TAIRA_GENESIS_PATH=...` in
  `/etc/default/taira-validator-container.compose.env`
- if the validator host should serve named SoraFS host bindings directly from
  the container, uncomment the matching `IROHA_SORAFS_SITE_BINDINGS_FILE` and
  volume lines, then set `TAIRA_SORAFS_SITE_BINDINGS_PATH=...`

## Bare-metal validator deployment

Install the validator from the repo checkout so the live process cannot drift
away from the shipped MCP-enabled config:

1. Check out this repository on the validator host, for example at
   `/opt/iroha`.
2. Build a rollout bundle from the exact runtime revision you intend to ship:
   - `bash configs/soranexus/taira/build_taira_rollout_bundle.sh`
   - the script refuses a dirty worktree by default and writes
     `dist/taira-rollout/<bundle>/rollout.manifest.json` plus
     `sha256sums.txt`
   - the bundle now includes both `scripts/render_taira_validator_bundle.py`
     and `scripts/render_taira_edge_nginx_conf.py` so validator config and
     shared-edge nginx can be rendered from the same roster artifact
   - capture the emitted git revision and archive path in the rollout ticket;
     that is the exact runtime candidate the later SoraSwap gate must approve
3. Render the per-validator config bundle from a user-local roster file, then
   copy the correct validator config onto the host, for example:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
   - `validator_secrets.local.toml` must include both the validator private
     keys and the shared `torii_onboarding_*`, `torii_faucet_*`, and
     `streaming_identity_*` fields because the checked-in template intentionally
     leaves those runtime-only values blank
   - `sudo install -d -o iroha -g iroha /etc/iroha/taira-validator-1`
   - `sudo cp dist/taira-validators/taira-validator-1/config.toml /etc/iroha/taira-validator-1/config.toml`
4. Install the newly built binaries plus the sample systemd unit from
   `configs/soranexus/taira/taira-irohad.service`:
   - install native Inrou prerequisites before enabling the unit, for example
     on Debian/Ubuntu:
     `sudo apt-get update && sudo apt-get install -y qemu-system-x86 qemu-system-arm qemu-utils e2fsprogs iproute2 iptables`
   - verify the host will advertise real Inrou capacity:
     `bash configs/soranexus/taira/check_inrou_host_prereqs.sh`
   - `sudo install -m 0755 dist/taira-rollout/<bundle>/bin/irohad /usr/local/bin/irohad`
   - `sudo install -m 0755 dist/taira-rollout/<bundle>/bin/iroha /usr/local/bin/iroha`
   - `sudo cp configs/soranexus/taira/taira-irohad.service /etc/systemd/system/`
   - copy `configs/soranexus/taira/taira-irohad.env.example` to
     `/etc/default/taira-irohad` and adjust `IROHA_TAIRA_CONFIG=` if you want
     the unit to use a generated config path without editing `ExecStart=`
   - add `/etc/default/taira-irohad` if you want the unit to use a generated
     config path without editing `ExecStart=`, for example:
     `IROHA_TAIRA_CONFIG=/etc/iroha/taira-validator-1/config.toml`
   - if your repo checkout or binary path differs from `/opt/iroha` and
     `/usr/local/bin/irohad`, adjust `WorkingDirectory=` and `ExecStart=`
     before enabling the unit
5. Reload systemd and restart the validator:
   - `sudo systemctl daemon-reload`
   - `sudo systemctl enable --now taira-irohad.service`
   - `sudo systemctl restart taira-irohad.service`
6. Capture the resolved config in the rollout ticket:
   - `sudo journalctl -u taira-irohad.service -n 200 --no-pager`
   - `cd /opt/iroha && /usr/local/bin/irohad --sora --config "${IROHA_TAIRA_CONFIG:-configs/soranexus/taira/config.toml}" --genesis-manifest-json "${IROHA_TAIRA_GENESIS:-configs/soranexus/taira/genesis.json}" --trace-config | tee /tmp/taira-trace-config.txt`
   - verify `/tmp/taira-trace-config.txt` includes `nexus.fees.fee_asset_id = "xor#universal"`
7. Prove the validator's loopback Torii endpoint exposes MCP and the expected
   direct-ingress routes before any public cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --skip-write-canary`
   - for a full local write-path check, use a runtime-only canary signer:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --write-config /run/secrets/taira-canary-client.toml --write-target local`
8. After the public node is back, prove the direct hostname is healthy before
   any convenience host or client cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml`
   - if contract deploy/view health still fails after the route checks pass,
     redeploy SoraSwap with the updated `../soraswap` `deploy-testnet` flow
     before blaming the frontend
9. Before declaring public Codex/Torii rollout complete, require the SoraSwap
   gate to pass behind the same runtime candidate:
   - probe-only:
     `bash configs/soranexus/taira/verify_soraswap_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml --soraswap-client-config /path/to/soraswap/config/testnet/taira.client.toml`
   - full gate:
     `bash configs/soranexus/taira/verify_soraswap_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml --soraswap-client-config /path/to/soraswap/config/testnet/taira.client.toml --run-release-checklist --allow-testnet-mutations`
   - the wrapper runs `check_mcp_rollout.sh`, `make testnet-nested-call-probe`,
     then the exact `deploy-testnet` / signed `smoke-testnet` /
     `release-checklist` sequence when those deeper flags are enabled
   - the script auto-discovers `${REPO_ROOT}/../soraswap` when the sibling repo
     exists, but `--soraswap-root` is available for non-default layouts

## Explorer integration (sibling repo)

From `../iroha2-block-explorer-web`:

1. Copy this file to runtime config:
   - `cp ../iroha/configs/soranexus/taira/explorer.runtime-config.json public/config.json`
   - update `toriiBaseUrl` if you want the explorer to query a different
     public node than the checked-in example
2. Build and deploy static assets:
   - `corepack enable && pnpm i && pnpm build`
3. Render and install the nginx snippet from the same validator roster you use
   for the validator configs:
   - `python3 scripts/render_taira_edge_nginx_conf.py --roster configs/soranexus/taira/validator_roster.local.toml --output dist/taira-edge/taira.sora.org.conf`
   - `sudo cp dist/taira-edge/taira.sora.org.conf /etc/nginx/conf.d/taira.conf`
   - on the shared macOS/Homebrew host, install the rendered file as
     `/opt/homebrew/etc/nginx/servers/taira.sora.org.conf` instead
   - set each validator entry's `edge_torii_upstream` in the roster to the
     real Torii listener the edge should proxy to, for example the current
     shared-host `127.0.0.1:29080..29083` layout rather than the old
     `127.0.0.1:18080..18083` default
  - keep the shared `taira_public_edge_upstream` wired to every live validator
    and use it for `taira.sora.org` plus the explorer's `/status` and the
    general `/v1/*` proxy locations. Do not pin MCP or the generic API surface
    to `taira_validator_1_upstream`: a single dead validator will turn MCP and
    explorer API traffic into `502 Bad Gateway`.
  - keep the `proxy_next_upstream ... non_idempotent` retry policy on those
    shared public locations. MCP `initialize` and tool calls are POSTs, so the
    edge has to allow non-idempotent upstream failover when one validator
    listener is down.
  - keep the dedicated `location = /v1/mcp` blocks intact; they make the
     Codex/Torii MCP path explicit on both public hostnames and keep future
     route changes from accidentally hiding the MCP endpoint behind the generic
     `/` or `/v1/` proxy rules.
  - keep the shared convenience host on `taira_public_edge_upstream` for the
    public SoraFS and app-api surface as well. The checked-in nginx example now
    keeps these paths symmetric with the rest of the public edge:
    - `/v1/app-api/`
    - `/v1/sorafs/storage/`
    - `/v1/sorafs/pin/`
    - `/v1/sorafs/cid/`
    - `/sorafs/cid/`
  - if CID hydration is still inconsistent after the runtime rollout, treat
    that as a provider-capacity/bootstrap problem and inspect
    `/v1/sorafs/capacity/state`; do not reintroduce a validator-1-only nginx
    pin as the steady-state fix.
    - `*.sorafs.taira.sora.org`
    Keep those routes on the same convenience validator that receives
    `POST /v1/sorafs/storage/pin`; otherwise the shared host will flap between
    `200` and `404` depending on which validator answers the CID read.
  - keep `client_max_body_size 1g;` intact on both TLS server blocks; the
     native Hayahi runtime publish path uploads large JSON envelopes to
     `/v1/sorafs/storage/pin` once the payload is base64-encoded.
   - keep `torii.max_content_len = 1_073_741_824` in `config.toml`; otherwise
     Torii rejects the storage-pin JSON body before the SoraFS handler sees it.
   - keep the dedicated 10 minute route timeout for `/v1/sorafs/storage/pin`;
     otherwise large Soracloud/SoraFS publishes can upload successfully through
     nginx and still fail with Torii's outer `408 Request Timeout`.
   - after every local reset, confirm the served `dist/taira-localnet/peer*.toml`
     copies still contain `max_content_len = 1073741824`; the local bootstrap
     script patches them from `configs/soranexus/taira/config.toml`, but a
     stale bundle can still bring the old default back.
   - keep `[sorafs.quota] storage_pin_max_events = 64` in the Taira profile and
     served peer configs; otherwise a handful of failed storage-pin probes can
     exhaust the default `4 requests / 3600s` window before a real
     `yarn taira:publish` retry.
   - a publish-sized ingress smoke should clear the old 16 MiB limit:
     `POST /v1/sorafs/storage/pin` with a `24_000_037` byte JSON body should
     reach the handler and return a normal `400` (for example `invalid base64
     in manifest_b64`), not `413`, `429`, or `502`.
   - keep the dedicated `location = /v1/connect/ws` blocks intact; they forward
     the required websocket `Upgrade` / `Connection: upgrade` headers for
     Iroha Connect on `taira.sora.org`.
   - do not fold `/v1/connect/ws` into the generic `location /` or
     `location ^~ /v1/` proxy rules; it must stay an exact-match websocket
     location with `proxy_http_version 1.1`.
   - ensure `taira.sora.org`, `taira-explorer.sora.org`, `mon.taira.sora.net`,
     every published `taira-validator-{1,2,3,4}.sora.org` hostname, and the
     required `*.sorafs.taira.sora.org` and `*.mon.taira.sora.net` records
     resolve to the shared edge host from `dns_records.json` before relying on
     this nginx configuration.
   - add wildcard edge routing for `*.sorafs.taira.sora.org` and preserve the
     incoming host header when proxying to Torii; the checked-in nginx example
     now includes that wildcard `server_name`.
   - keep Mon gateway routing generic with the apex `mon.taira.sora.net`
     server block plus the regex alias server block for
     `<alias>.mon.taira.sora.net`; do not add per-service path rewrites such as
     `/solswap-indexer/...`.
   - do not leave backup `.conf` files under the nginx `servers/` include
     directory. Homebrew nginx deployments often include the whole directory,
     so backup configs can create duplicate `server_name` entries and shadow
     the intended Mon gateway block.
4. Issue/refresh TLS certificates for the public hosts, direct validator names,
   CID-origin wildcard, and Mon gateway exact hosts:
   - `taira.sora.org`
   - `taira-explorer.sora.org`
   - `taira-validator-1.sora.org`
   - `taira-validator-2.sora.org`
   - `taira-validator-3.sora.org`
   - `taira-validator-4.sora.org`
   - `*.sorafs.taira.sora.org`
   - `mon.taira.sora.net`
   - exact Mon hosts such as `solswap-indexer.sora.mon.taira.sora.net`
   - the convenience, explorer, and direct validator names can share one SAN
     certificate stored under `.../live/taira.sora.org/` if your ACME client
     keeps those names in one lineage.
   - the wildcard requires DNS-01 validation; `certbot --nginx` alone is not
     enough for the `*.sorafs.taira.sora.org` SAN.
   - Mon gateway aliases require exact bind-time certificates. A wildcard cert
     for `*.mon.taira.sora.net` does not cover multi-label aliases such as
     `solswap-indexer.sora.mon.taira.sora.net`.
   - if your ACME client stores all SANs under one lineage, nginx can keep
     pointing at a single certificate bundle for all names served from this
     edge.
   - before DNS propagates or before the SAN cert is refreshed, you can still
     validate local SNI routing on the edge host with `curl --resolve` plus
     `-k`, for example:
     `curl -sk --resolve taira-validator-1.sora.org:443:127.0.0.1 https://taira-validator-1.sora.org/status | jq '.blocks, .sumeragi.commit_qc_height'`
   - if a client network intercepts or blocks `sora.net`, HTTP may be replaced
     before nginx and HTTPS may reset during the TLS ClientHello. This is stale
     reputation filtering from `sora.net` prior ownership, not evidence that
     current SORA content is pornographic. Confirm from the edge host or an
     unfiltered external network before treating that as a Soracloud runtime
     failure, and treat the durable fix as ISP/filter-vendor delisting.
5. Validate and reload nginx:
   - `sudo nginx -t && sudo systemctl reload nginx`
   - on the shared macOS/Homebrew host, use `nginx -t && nginx -s reload`
6. Run the MCP rollout smoke from any host that can see the validator loopback
   and the public endpoint:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}"`
   - when you are validating edge-local SNI before public DNS or TLS is fully
     live, pin the public host to the edge IP explicitly:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root https://taira.sora.org --resolve-host taira.sora.org:443:127.0.0.1`
  - the public check now defaults to
    `/run/secrets/taira-canary-client.toml` and auto-bootstraps it when the
    file is missing or still contains placeholders, unless you explicitly opt
    into read-only mode with `--skip-write-canary`
7. Verify that SNI now serves the correct cert for each host and that MCP,
   Connect, and CID-host routing still work through the public edge:
   - `curl -vI https://taira.sora.org`
   - `curl -vI https://taira-explorer.sora.org`
   - `curl -vI "${PUBLIC_TORII_ROOT}/status"`
   - `echo | openssl s_client -connect taira-explorer.sora.org:443 -servername taira-explorer.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`
   - `echo | openssl s_client -connect taira.sora.org:443 -servername example.sorafs.taira.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`
   - verify MCP over the direct node host:
     `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" | jq .`
   - verify curated `iroha.*` exposure:
     `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq .`
   - verify the native status snapshot is healthy before trusting public writes:
     `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq '{blocks, queue_size, peers, sumeragi: {commit_qc_height, commit_qc_validator_set_len, tx_queue_depth, tx_queue_saturated}, teu_dataspace_backlog}'`
   - remember that `/status.peers` is the queried node's current remote-peer
     count, not the validator-set size; use
     `.sumeragi.commit_qc_validator_set_len` or `/v1/sumeragi/validator-sets`
     for validator-set visibility.
   - create a Connect session through the proxy and ask explicitly for JSON:
     `curl -sS -X POST "${PUBLIC_TORII_ROOT}/v1/connect/session" -H 'content-type: application/json' -H 'accept: application/json' -d '{"sid":"<32-byte-base64url-sid>"}'`
   - verify Connect websocket upgrades on both public hostnames with the
     returned `sid` and app token:
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' "${PUBLIC_TORII_ROOT}/v1/connect/ws?sid=<sid>&role=app"`
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' 'https://taira-explorer.sora.org/v1/connect/ws?sid=<sid>&role=app'`
   - verify CID-host origin isolation with a known site CID:
     `curl -vkI "https://<cid>.sorafs.taira.sora.org/"`
     `curl -vkI "https://taira.sora.org/sorafs/cid/<cid>/swap/ton/usdt" -H 'accept: text/html'`
   - browser-style navigations should `308` to
     `https://<cid>.sorafs.taira.sora.org/...`, while asset/tooling requests can
     still stay on `/sorafs/cid/<cid>/...`.
   - if those websocket probes now return a Torii-generated app error
     (`400/401/...`) instead of a proxy-layer `404` / missing-upgrade failure,
     the reverse-proxy websocket hop is working and any remaining error is in
     Connect session or token handling rather than nginx.

The Explorer runtime config should target an explicit public node URL. The
checked-in example now uses `https://taira.sora.org`, while deployments that
want a direct-validator Explorer path can still override it at deploy time.

## Local Kaigi bootstrap

The served local Taira testnet on this machine does not expose a working public
lane write path after a fresh reset yet, so Kaigi relay metadata must be seeded
into the localnet's signed genesis overlay rather than submitted live through
Torii. Without that overlay, `/v1/kaigi/relays` will stay empty.

For the local `dist/taira-localnet` deployment, use:

1. Build the helper used to re-sign the localnet genesis overlay:
   - `cargo build -p iroha_kagami --example taira_kaigi_localnet --release`
2. Run the local bootstrap:
   - `bash configs/soranexus/taira/bootstrap_kaigi_localnet.sh`
   - if you built the helper in a non-default target dir, point the bootstrap
     at it explicitly, for example:
     `IROHA_TAIRA_KAIGI_HELPER_BIN=/tmp/iroha_taira_kaigi_helper/debug/examples/taira_kaigi_localnet bash configs/soranexus/taira/bootstrap_kaigi_localnet.sh`
   - if `configs/soranexus/taira/validator_secrets.local.toml` is not present,
     provide `IROHA_TAIRA_AUTHORITY` and `IROHA_TAIRA_AUTHORITY_PRIVATE_KEY`
     (or point `IROHA_TAIRA_SECRETS_FILE` at a populated secrets file) so the
     bootstrap can inject the shared local onboarding signer, which it also
     reuses as the served local faucet signer, into the localnet configs
3. Verify the relay endpoints and explorer page:
   - `curl -sk https://taira.sora.org/v1/kaigi/relays | jq .`
   - `curl -sk https://taira.sora.org/v1/kaigi/relays/health | jq .`
   - open `https://taira-explorer.sora.org/kaigi/relays`

The script is intentionally localnet-specific:

- it reuses the first three validator accounts already present in
  `dist/taira-localnet/peer{0,1,2}.toml`, so no extra linked-domain account
  registration is required;
- it derives the local client account from `dist/taira-localnet/client.toml`,
  signs a fresh `genesis.signed.nrt` overlay from `genesis.json`, and seeds the
  `nexus` domain metadata keys `kaigi_relay__*` and
  `kaigi_relay_feedback__*` so Torii's Kaigi relay endpoints have data to
  serve immediately after restart; and
- it skips `cargo test --example ...` harness binaries during helper
  auto-detection, so the bootstrap only reuses executables that actually
  expose the `--genesis` overlay CLI; and
- after any fresh local Taira reset, rerun this script if you want the Kaigi
  explorer page to reflect live relay data again.

The health snapshot's `healthy_total` will reflect the seeded relay feedback,
but `registrations_total` can remain `0` because that counter comes from live
telemetry rather than the seeded metadata overlay. The explorer overview still
shows the correct relay count because it floors the overview total to the
actual relay list length.
