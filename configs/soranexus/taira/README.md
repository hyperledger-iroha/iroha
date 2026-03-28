# Sora Taira testnet (MVP bootstrap)

Taira is the Sora Nexus public testnet. This directory
contains a minimum-viable NPoS bootstrap bundle so operators can bring a usable
network online quickly.

## Included artifacts

- `config.toml`: baseline validator config for peer 1.
- `genesis.json`: NPoS genesis with DA enabled.
- `dns_records.json`: DNS targets for public Torii + Explorer hostnames.
- `explorer.runtime-config.json`: runtime config for the Explorer frontend.
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

- `https://taira.sora.org` points to one Torii endpoint (validator 1 in
  `dns_records.json` for initial rollout).
- `https://taira-explorer.sora.org` points to the Iroha 2 Explorer instance.

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
   - keep the dedicated `location = /v1/connect/ws` blocks intact; they forward
     the required websocket `Upgrade` / `Connection: upgrade` headers for
     Iroha Connect on `taira.sora.org`.
   - do not fold `/v1/connect/ws` into the generic `location /` or
     `location ^~ /v1/` proxy rules; it must stay an exact-match websocket
     location with `proxy_http_version 1.1`.
4. Issue/refresh TLS certificates for both hostnames:
   - `sudo certbot certonly --nginx -d taira.sora.org -d taira-explorer.sora.org`
   - certbot stores this SAN cert under `.../live/taira.sora.org/` and nginx can
     reuse it for both server blocks.
5. Validate and reload nginx:
   - `sudo nginx -t && sudo systemctl reload nginx`
   - on the shared macOS/Homebrew host, use `nginx -t && nginx -s reload`
6. Verify that SNI now serves the correct cert for each host:
   - `curl -vI https://taira.sora.org`
   - `curl -vI https://taira-explorer.sora.org`
   - `echo | openssl s_client -connect taira-explorer.sora.org:443 -servername taira-explorer.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`
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
