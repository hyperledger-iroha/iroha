# Sora Taira public NPoS bootstrap

Taira is the Sora Nexus public testnet. This directory
contains the repo-shipped bootstrap bundle for a public, stake-elected NPoS
deployment.

## Network identity

- Public chain ID: `809574f5-fee7-5e69-bfcf-52451e42d50f`
- Address chain discriminant: `369` (this is what drives canonical I105 literals such as `testu...`)

## Public API contract

For the examples below, replace `PUBLIC_TORII_ROOT` with the direct public
Torii URL of the node you are validating, for example:

- `PUBLIC_TORII_ROOT=https://taira-validator-1.sora.org`

`https://taira.sora.org` may still exist as a convenience endpoint or landing
page, but it is not the canonical API target for rollout, canaries, or client
defaults.

## Included artifacts

- `config.toml`: baseline validator config for peer 1 and the shared template
  source for rendered per-validator configs.
- `validator_roster.example.toml`: copy-me roster template for all validator
  public addresses, public keys, and PoPs. Keep the populated file user-local.
- `validator_secrets.example.toml`: copy-me secret template for per-validator
  private keys. Keep the populated file user-local.
- `genesis.json`: NPoS genesis with DA enabled.
- `dns_records.json`: DNS targets for public Torii + Explorer hostnames.
- `explorer.runtime-config.json`: runtime config example for the Explorer
  frontend; point it at the explicit public Torii base URL you want the UI to
  query.
- `sorafs_sites.json`: host-to-manifest bindings for Torii-served static sites.
- `sorafs_gateway_denylist.catalog.json`: default-on SoraFS denylist pack catalog.
- `sorafs_gateway_denylist.global-core.json`: baseline governance-backed illegal-content pack.
- `sorafs_gateway_denylist.global-emergency.json`: emergency-response denylist pack.
- `taira-irohad.service`: sample systemd unit that starts the validator from
  the shipped Taira config and genesis.
- `taira-irohad.env.example`: sample `/etc/default/taira-irohad` overrides for
  pointing the systemd unit at a rendered validator config.
- `taira-canary-client.example.toml`: runtime-only example signer config for
  the signed rollout canary.
- `check_mcp_rollout.sh`: smoke script for the local and public `/v1/mcp`
  checks used by the Taira Codex rollout, including the optional signed write
  canary for final public cutover.
- `bootstrap_kaigi_localnet.sh`: local-only relay bootstrap that re-signs the
  served `dist/taira-localnet` genesis with seeded Kaigi relay metadata,
  health samples, and the canonical Taira onboarding/faucet authority account,
  then rewrites the live peer configs and restarts the detached
  `taira-localnet` session.
- `taira-explorer.nginx.conf`: multi-domain nginx edge config for
  `taira.sora.org` and `taira-explorer.sora.org`.

## Render validator configs

Do not hand-edit `config.toml` into multiple validator copies. Instead:

1. Copy `validator_roster.example.toml` to a user-local path such as
   `configs/soranexus/taira/validator_roster.local.toml`.
2. Copy `validator_secrets.example.toml` to a user-local path such as
   `configs/soranexus/taira/validator_secrets.local.toml`.
3. Fill in every validator's real `public_key`, `pop_hex`, and
   `public_address` plus its own direct `torii_public_address` in the public
   roster, then put the matching `private_key` values in the secrets file.
3. Render the per-validator bundle:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
4. Point each validator host at its own generated
   `dist/taira-validators/<validator-slug>/config.toml`.

The renderer rewrites the checked-in peer-1 baseline with the full
`trusted_peers` / `trusted_peers_pop` roster so every validator starts from the
same bootstrap source of truth. It now requires explicit per-validator
`torii_public_address` values so direct public Torii hostnames are part of the
checked operator input instead of a hard-coded shared edge default.

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

- Every public validator should expose Torii directly on its own TLS hostname
  and advertise that URL through `[torii].public_address`.
- `https://taira.sora.org` is optional convenience ingress only. Keep it if you
  want a shared landing page or one extra public API endpoint, but do not treat
  it as the canonical network API.
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
alias layer, but they are no longer the primary deployment path.

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
the chosen public Torii hostname must keep `client_max_body_size 64m;` from
`taira-explorer.nginx.conf`. Without that, `yarn taira:publish` fails at
`POST /v1/sorafs/storage/pin` with `413 Payload Too Large` before Torii sees
the request. Torii must also run with `torii.max_content_len` high enough for
the base64-expanded JSON body; the shipped Taira profile now pins that to
`64_000_000`, and the local bootstrap overlay now rewrites the served
`dist/taira-localnet/peer*.toml` files to keep that same cap live after every
reset. Taira also overrides `[sorafs.quota] storage_pin_max_events = 64` so
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
OpenAPI-derived surface.

After rollout, verify the chosen public node directly:

- `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" | jq .`
- `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq .`
- `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq .`

The repo-local Codex plugin and Taira skill now expect an explicit direct-node
MCP URL rather than a canonical committed live host. Future Nexus/Torii
deployments should expose the same `/v1/mcp` path and be added as user-local
MCP servers rather than committed to this repo with one fixed public hostname.

For final public rollout, do not stop at MCP discovery. Run the repo smoke with
both the public endpoint and a runtime-only canary signer config:

- `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml`

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
- `/v1/contracts/instances/universal`

That config must be a normal `iroha` client TOML for a low-risk signer that
already exists on Taira. Start from `taira-canary-client.example.toml`, not
`defaults/client.toml`: the generic repo client uses the zero chain id and is
not valid for Taira. If the signer still has zero balance for the live faucet
asset, `check_mcp_rollout.sh` will now solve the public faucet puzzle, claim
starter funds for that account, wait for the queued transfer to land, and then
retry the signed ping automatically. Keep the populated canary config out of
the repo and out of shell history where possible.

If the script fails with `route_unavailable`, treat that as a deployment or
topology failure, not an app-level validation issue: the public Torii ingress is
up, but it still cannot reach an authoritative peer for lane `0` / dataspace
`0`.
If it fails with `Failed to find asset` even after the automatic faucet
bootstrap path runs, treat that as a faucet-health or signer-selection issue:
the configured account either does not exist on Taira yet or the live faucet
could not fund it.

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
fee_sink_account_id = "testuロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
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

## Validator deployment

Install the validator from the repo checkout so the live process cannot drift
away from the shipped MCP-enabled config:

1. Check out this repository on the validator host, for example at
   `/opt/iroha`.
2. Render the per-validator config bundle from a user-local roster file, then
   copy the correct validator config onto the host, for example:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
   - `sudo install -d -o iroha -g iroha /etc/iroha/taira-validator-1`
   - `sudo cp dist/taira-validators/taira-validator-1/config.toml /etc/iroha/taira-validator-1/config.toml`
3. Install the sample systemd unit from
   `configs/soranexus/taira/taira-irohad.service`:
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
4. Reload systemd and restart the validator:
   - `sudo systemctl daemon-reload`
   - `sudo systemctl enable --now taira-irohad.service`
   - `sudo systemctl restart taira-irohad.service`
5. Capture the resolved config in the rollout ticket:
   - `sudo journalctl -u taira-irohad.service -n 200 --no-pager`
   - `cd /opt/iroha && /usr/local/bin/irohad --sora --config "${IROHA_TAIRA_CONFIG:-configs/soranexus/taira/config.toml}" --genesis "${IROHA_TAIRA_GENESIS:-configs/soranexus/taira/genesis.json}" --trace-config | tee /tmp/taira-trace-config.txt`
   - verify `/tmp/taira-trace-config.txt` includes `nexus.fees.fee_asset_id = "xor#universal"`
6. Prove the validator's loopback Torii endpoint exposes MCP and the expected
   direct-ingress routes before any public cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --skip-write-canary`
   - for a full local write-path check, use a runtime-only canary signer:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --write-config /run/secrets/taira-canary-client.toml --write-target local`
7. After the public node is back, prove the direct hostname is healthy before
   any convenience host or client cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml`
   - if the contract instance count is still `0`, redeploy SoraSwap with the
     updated `../soraswap` `deploy-testnet` flow before blaming the frontend:
     `curl -sS "${PUBLIC_TORII_ROOT}/v1/contracts/instances/universal" | jq '.total'`
8. Before declaring public Codex/Torii rollout complete, require the signed
   write canary on the direct node to pass:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml`
   - the script now auto-discovers `${REPO_ROOT}/target/{debug,release}/iroha`
     and falls back to `cargo run -p iroha_cli --bin iroha -- ...` if `iroha`
     is not already installed on `PATH`
   - if you only need a read-only check for debugging, opt into that mode
     explicitly with `--skip-write-canary`

## Explorer integration (sibling repo)

From `../iroha2-block-explorer-web`:

1. Copy this file to runtime config:
   - `cp ../iroha/configs/soranexus/taira/explorer.runtime-config.json public/config.json`
   - update `toriiBaseUrl` if you want the explorer to query a different
     public node than the checked-in example
2. Build and deploy static assets:
   - `corepack enable && pnpm i && pnpm build`
3. Install the nginx snippet from
   `../iroha/configs/soranexus/taira/taira-explorer.nginx.conf` on the edge host
   (for both domains on one machine), e.g.:
   - `sudo cp ../iroha/configs/soranexus/taira/taira-explorer.nginx.conf /etc/nginx/conf.d/taira.conf`
   - on the shared macOS/Homebrew host, install the same template as
     `/opt/homebrew/etc/nginx/servers/taira.sora.org.conf` instead, then point
     the upstream at the locally served Torii port (currently `127.0.0.1:29080`
     on that machine rather than the template's `127.0.0.1:18080`)
   - if your Torii endpoint is not `127.0.0.1:18080`, update the `upstream taira_torii_upstream`
     target to the live validator endpoint before reload.
   - keep the dedicated `location = /v1/mcp` blocks intact; they make the
     Codex/Torii MCP path explicit on both public hostnames and keep future
     route changes from accidentally hiding the MCP endpoint behind the generic
     `/` or `/v1/` proxy rules.
   - do not special-case `/sorafs/cid/`; it should proxy through the normal
     Torii upstream just like the rest of the public API surface.
   - keep `client_max_body_size 64m;` intact on both TLS server blocks; the
     Polkaswap SoraFS publish path currently uploads about `24 MiB+` of JSON to
     `/v1/sorafs/storage/pin`.
   - keep `torii.max_content_len = 64_000_000` in `config.toml`; otherwise
     Torii rejects the JSON body before the SoraFS storage handler sees it.
   - after every local reset, confirm the served `dist/taira-localnet/peer*.toml`
     copies still contain `max_content_len = 64000000`; the local bootstrap
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
   - ensure both `taira.sora.org` and `taira-explorer.sora.org` resolve to the
     shared edge host from `dns_records.json` before relying on this nginx
     configuration.
4. Issue/refresh TLS certificates for both hostnames:
   - `sudo certbot certonly --nginx -d taira.sora.org -d taira-explorer.sora.org`
   - certbot stores this SAN cert under `.../live/taira.sora.org/` and nginx can
     reuse it for both server blocks.
5. Validate and reload nginx:
   - `sudo nginx -t && sudo systemctl reload nginx`
   - on the shared macOS/Homebrew host, use `nginx -t && nginx -s reload`
6. Run the MCP rollout smoke from any host that can see the validator loopback
   and the public endpoint:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}"`
   - the public check now requires `--write-config /run/secrets/taira-canary-client.toml`
     unless you explicitly opt into read-only mode with `--skip-write-canary`
7. Verify that SNI now serves the correct cert for each host and that both MCP
   and Connect still work through the public edge:
   - `curl -vI https://taira.sora.org`
   - `curl -vI https://taira-explorer.sora.org`
   - `echo | openssl s_client -connect taira-explorer.sora.org:443 -servername taira-explorer.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`
   - verify MCP over the direct node host:
     `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" | jq .`
   - verify curated `iroha.*` exposure:
     `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq .`
   - verify the native status snapshot is healthy before trusting public writes:
     `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq '.peers, .blocks, .sumeragi.commit_qc_validator_set_len'`
   - create a Connect session through the proxy and ask explicitly for JSON:
     `curl -sS -X POST "${PUBLIC_TORII_ROOT}/v1/connect/session" -H 'content-type: application/json' -H 'accept: application/json' -d '{"sid":"<32-byte-base64url-sid>"}'`
   - verify Connect websocket upgrades on both public hostnames with the
     returned `sid` and app token:
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' "${PUBLIC_TORII_ROOT}/v1/connect/ws?sid=<sid>&role=app"`
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' 'https://taira-explorer.sora.org/v1/connect/ws?sid=<sid>&role=app'`
   - if those websocket probes now return a Torii-generated app error
     (`400/401/...`) instead of a proxy-layer `404` / missing-upgrade failure,
     the reverse-proxy websocket hop is working and any remaining error is in
     Connect session or token handling rather than nginx.

The Explorer runtime config should target an explicit public node URL. The
checked-in example uses `https://taira-validator-1.sora.org`, so both UI reads
and `/v1/*` proxy traffic follow that direct Taira Torii endpoint unless you
override it at deploy time.

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
