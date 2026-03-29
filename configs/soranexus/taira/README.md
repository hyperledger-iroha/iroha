# Sora Taira testnet (MVP bootstrap)

Taira is the Sora Nexus public testnet. This directory
contains a minimum-viable NPoS bootstrap bundle so operators can bring a usable
network online quickly.

## Network identity

- Public chain ID: `809574f5-fee7-5e69-bfcf-52451e42d50f`
- Address chain discriminant: `369` (this is what drives canonical I105 literals such as `testu...`)

## Included artifacts

- `config.toml`: baseline validator config for peer 1 and the shared template
  source for rendered per-validator configs.
- `validator_roster.example.toml`: copy-me roster template for all validator
  public addresses, public keys, and PoPs. Keep the populated file user-local.
- `validator_secrets.example.toml`: copy-me secret template for per-validator
  private keys. Keep the populated file user-local.
- `genesis.json`: NPoS genesis with DA enabled.
- `dns_records.json`: DNS targets for public Torii + Explorer hostnames.
- `explorer.runtime-config.json`: runtime config for the Explorer frontend.
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
- `taira-explorer.nginx.conf`: multi-domain nginx edge config for
  `taira.sora.org` and `taira-explorer.sora.org`.

## Render validator configs

Do not hand-edit `config.toml` into multiple validator copies. Instead:

1. Copy `validator_roster.example.toml` to a user-local path such as
   `configs/soranexus/taira/validator_roster.local.toml`.
2. Copy `validator_secrets.example.toml` to a user-local path such as
   `configs/soranexus/taira/validator_secrets.local.toml`.
3. Fill in every validator's real `public_key`, `pop_hex`, and
   `public_address` in the public roster, then put the matching
   `private_key` values in the secrets file.
3. Render the per-validator bundle:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
4. Point each validator host at its own generated
   `dist/taira-validators/<validator-slug>/config.toml`.

The renderer rewrites the checked-in peer-1 baseline with the full
`trusted_peers` / `trusted_peers_pop` roster so every validator starts from the
same source of truth.

## Minimum viable topology

Use at least 4 validator peers (plus optional observers). Single-peer setups are
not representative for NPoS and can stall DA/RBC consensus paths.

Suggested validator hostnames:

- `taira-validator-1.sora.org`
- `taira-validator-2.sora.org`
- `taira-validator-3.sora.org`
- `taira-validator-4.sora.org`

## Public endpoints

- `https://taira.sora.org` is the public Torii endpoint, served through the
  shared nginx edge host from `dns_records.json`.
- `https://taira-explorer.sora.org` points to the Iroha 2 Explorer instance.
- The validator itself should only expose Torii on its private upstream
  address (`127.0.0.1:18080` in `taira-explorer.nginx.conf`) and should not be
  treated as the public TLS endpoint.

### SoraFS CID gateway

Taira serves SoraFS-published static content primarily through immutable CID
gateway paths on the Torii origin:

- `GET /sorafs/cid/<cid>/`
- `GET /sorafs/cid/<cid>/<path...>`
- `GET /v1/sorafs/cid/<cid>` for lookup metadata

For the Polkaswap static bundle, the browser URL is:

- `https://taira.sora.org/sorafs/cid/<cid>/`

This keeps `https://taira.sora.org/` as the Torii/API origin while giving every
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
`taira.sora.org` must keep `client_max_body_size 64m;` from
`taira-explorer.nginx.conf`. Without that, `yarn taira:publish` fails at
`POST /v1/sorafs/storage/pin` with `413 Payload Too Large` before Torii sees
the request. Torii must also run with `torii.max_content_len` high enough for
the base64-expanded JSON body; the shipped Taira profile now pins that to
`64_000_000`.

### Codex / MCP rollout

Taira's public Torii host is also the native MCP endpoint once the validator is
redeployed with the shipped `[torii.mcp]` block from `config.toml`:

- `torii.mcp.enabled = true`
- `torii.mcp.profile = "writer"`
- `torii.mcp.expose_operator_routes = false`
- `torii.mcp.allow_tool_prefixes = ["iroha."]`

This intentionally exposes only curated `iroha.*` tools on the public network
so Codex sees the stable live-network aliases and not the full raw `torii.*`
OpenAPI-derived surface.

After rollout, verify the public MCP endpoint directly:

- `curl -sS https://taira.sora.org/v1/mcp | jq .`
- `curl -sS https://taira.sora.org/v1/mcp -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq .`
- `curl -sS https://taira.sora.org/status | jq .`

The repo-local Codex plugin under `plugins/iroha/` points at this URL by
default. Future Nexus/Torii deployments should expose the same `/v1/mcp` path
and be added as user-local MCP servers rather than committed to this repo.

For final public rollout, do not stop at MCP discovery. Run the repo smoke with
both the public endpoint and a runtime-only canary signer config:

- `bash configs/soranexus/taira/check_mcp_rollout.sh --write-config /run/secrets/taira-canary-client.toml`

The rollout script now also requires the live `/status` snapshot to show at
least 4 validators in the commit QC set. If it fails that check, rebuild the
validator configs from the shared roster before debugging ingress or MCP.

That config must be a normal `iroha` client TOML with a pre-provisioned low-risk
signer. Start from `taira-canary-client.example.toml`, not `defaults/client.toml`:
the generic repo client uses the zero chain id and is not valid for Taira.
Keep the populated canary config out of the repo and out of shell history where
possible.

If the script fails with `route_unavailable`, treat that as a deployment or
topology failure, not an app-level validation issue: the public Torii ingress is
up, but it still cannot reach an authoritative peer for lane `0` / dataspace
`0`.

## Governance mode

`config.toml` pins Taira to Sora parliament sortition governance for Nexus lanes:

- `nexus.governance.default_module = "parliament"`
- `nexus.governance.modules.parliament.module_type = "parliament_sortition_jit"`
- governance lane metadata binds lane 1 to `governance = "parliament"`
- top-level `[governance]` sets multibody committee/quorum parameters

This avoids fallback to legacy council-epoch approval mode during deployment.

## Fee config

Taira must declare the Nexus fee asset explicitly as the live XOR alias:

```toml
[nexus.fees]
fee_asset_id = "xor#universal"
fee_sink_account_id = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
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
6. Prove the validator's loopback Torii endpoint exposes MCP before reloading
   nginx or cutting public traffic:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public`
   - for a full local write-path check, use a runtime-only canary signer:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --write-config /run/secrets/taira-canary-client.toml --write-target local`
7. After the public node is back, prove contract lifecycle writes can commit:
   - `curl -sS https://taira.sora.org/v1/contracts/instances/universal | jq '.total'`
   - if the total is `0`, redeploy SoraSwap with the updated `../soraswap` `deploy-testnet` flow before blaming the frontend
8. Before declaring public Codex/Torii rollout complete, require the signed
   write canary to pass:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --write-config /run/secrets/taira-canary-client.toml`
   - the script now auto-discovers `${REPO_ROOT}/target/{debug,release}/iroha`
     and falls back to `cargo run -p iroha_cli --bin iroha -- ...` if `iroha`
     is not already installed on `PATH`
   - if you only need a read-only check for debugging, opt into that mode
     explicitly with `--skip-write-canary`

## Explorer integration (sibling repo)

From `../iroha2-block-explorer-web`:

1. Copy this file to runtime config:
   - `cp ../iroha/configs/soranexus/taira/explorer.runtime-config.json public/config.json`
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
   - `bash configs/soranexus/taira/check_mcp_rollout.sh`
   - the public check now requires `--write-config /run/secrets/taira-canary-client.toml`
     unless you explicitly opt into read-only mode with `--skip-write-canary`
7. Verify that SNI now serves the correct cert for each host and that both MCP
   and Connect still work through the public edge:
   - `curl -vI https://taira.sora.org`
   - `curl -vI https://taira-explorer.sora.org`
   - `echo | openssl s_client -connect taira-explorer.sora.org:443 -servername taira-explorer.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`
   - verify MCP over the public host:
     `curl -sS https://taira.sora.org/v1/mcp | jq .`
   - verify curated `iroha.*` exposure:
     `curl -sS https://taira.sora.org/v1/mcp -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq .`
   - verify the native status snapshot is healthy before trusting public writes:
     `curl -sS https://taira.sora.org/status | jq '.peers, .blocks, .sumeragi.commit_qc_validator_set_len'`
   - create a Connect session through the proxy and ask explicitly for JSON:
     `curl -sS -X POST https://taira.sora.org/v1/connect/session -H 'content-type: application/json' -H 'accept: application/json' -d '{"sid":"<32-byte-base64url-sid>"}'`
   - verify Connect websocket upgrades on both public hostnames with the
     returned `sid` and app token:
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' 'https://taira.sora.org/v1/connect/ws?sid=<sid>&role=app'`
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' 'https://taira-explorer.sora.org/v1/connect/ws?sid=<sid>&role=app'`
   - if those websocket probes now return a Torii-generated app error
     (`400/401/...`) instead of a proxy-layer `404` / missing-upgrade failure,
     the reverse-proxy websocket hop is working and any remaining error is in
     Connect session or token handling rather than nginx.

The Explorer runtime config targets `https://taira.sora.org`, so both UI reads
and `/v1/*` proxy traffic follow the Taira Torii endpoint.
