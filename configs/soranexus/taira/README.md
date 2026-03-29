# Sora Taira testnet (MVP bootstrap)

Taira is the Sora Nexus public testnet. This directory
contains a minimum-viable NPoS bootstrap bundle so operators can bring a usable
network online quickly.

## Network identity

- Public chain ID: `809574f5-fee7-5e69-bfcf-52451e42d50f`
- Address chain discriminant: `369` (this is what drives canonical I105 literals such as `testu...`)

## Included artifacts

- `config.toml`: baseline validator config for peer 1.
- `genesis.json`: NPoS genesis with DA enabled.
- `dns_records.json`: DNS targets for public Torii + Explorer hostnames.
- `explorer.runtime-config.json`: runtime config for the Explorer frontend.
- `sorafs_sites.json`: host-to-manifest bindings for Torii-served static sites.
- `taira-irohad.service`: sample systemd unit that starts the validator from
  the shipped Taira config and genesis.
- `check_mcp_rollout.sh`: smoke script for the local and public `/v1/mcp`
  checks used by the Taira Codex rollout.
- `taira-explorer.nginx.conf`: multi-domain nginx edge config for
  `taira.sora.org` and `taira-explorer.sora.org`.

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

### SoraFS site hosting

Taira can also serve a SoraFS-published static site directly from the Torii
root:

- `GET /` and other non-API paths resolve against the manifest bound in `sorafs_sites.json`
- `GET /.well-known/sorafs/manifest` returns the active binding plus its file list
- `/v1/*`, `/status`, and websocket upgrade routes stay on the Torii API

The shipped `defaults/kagami/iroha3-taira/docker-compose.yml` mounts
`sorafs_sites.json` and sets:

- `IROHA_SORAFS_SITE_BINDINGS_FILE=/config/sorafs_sites.json`

The Polkaswap web app repo updates
`../iroha/configs/soranexus/taira/sorafs_sites.json` during
`yarn taira:publish`. After publishing, redeploy the Taira validator/Torii
nodes from the patched `../iroha` checkout so the live network picks up the new
binding and static-site handlers.

Taira's public edge also needs to accept the storage payload upload that
precedes root serving. The current SoraFS storage pin API sends the full staged
site in one JSON request (`payload_b64`), so the nginx host serving
`taira.sora.org` must keep `client_max_body_size 64m;` from
`taira-explorer.nginx.conf`. Without that, `yarn taira:publish` fails at
`POST /v1/sorafs/storage/pin` with `413 Payload Too Large` before Torii sees
the request.

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

The repo-local Codex plugin under `plugins/iroha/` points at this URL by
default. Future Nexus/Torii deployments should expose the same `/v1/mcp` path
and be added as user-local MCP servers rather than committed to this repo.

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
2. Install the sample systemd unit from
   `configs/soranexus/taira/taira-irohad.service`:
   - `sudo cp configs/soranexus/taira/taira-irohad.service /etc/systemd/system/`
   - if your repo checkout or binary path differs from `/opt/iroha` and
     `/usr/local/bin/irohad`, adjust `WorkingDirectory=` and `ExecStart=`
     before enabling the unit
3. Reload systemd and restart the validator:
   - `sudo systemctl daemon-reload`
   - `sudo systemctl enable --now taira-irohad.service`
   - `sudo systemctl restart taira-irohad.service`
4. Capture the resolved config in the rollout ticket:
   - `sudo journalctl -u taira-irohad.service -n 200 --no-pager`
   - `cd /opt/iroha && /usr/local/bin/irohad --sora --config configs/soranexus/taira/config.toml --genesis configs/soranexus/taira/genesis.json --trace-config | tee /tmp/taira-trace-config.txt`
   - verify `/tmp/taira-trace-config.txt` includes `nexus.fees.fee_asset_id = "xor#universal"`
5. Prove the validator's loopback Torii endpoint exposes MCP before reloading
   nginx or cutting public traffic:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public`
6. After the public node is back, prove contract lifecycle writes can commit:
   - `curl -sS https://taira.sora.org/v1/contracts/instances/universal | jq '.total'`
   - if the total is `0`, redeploy SoraSwap with the updated `../soraswap` `deploy-testnet` flow before blaming the frontend

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
   - keep `client_max_body_size 64m;` intact on both TLS server blocks; the
     Polkaswap SoraFS publish path currently uploads about `24 MiB+` of JSON to
     `/v1/sorafs/storage/pin`.
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
7. Verify that SNI now serves the correct cert for each host and that both MCP
   and Connect still work through the public edge:
   - `curl -vI https://taira.sora.org`
   - `curl -vI https://taira-explorer.sora.org`
   - `echo | openssl s_client -connect taira-explorer.sora.org:443 -servername taira-explorer.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`
   - verify MCP over the public host:
     `curl -sS https://taira.sora.org/v1/mcp | jq .`
   - verify curated `iroha.*` exposure:
     `curl -sS https://taira.sora.org/v1/mcp -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq .`
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
