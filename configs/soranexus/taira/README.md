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
   - if your Torii endpoint is not `127.0.0.1:18080`, update the `upstream taira_torii_upstream`
     target to the live validator endpoint before reload.
4. Issue/refresh TLS certificates for both hostnames:
   - `sudo certbot certonly --nginx -d taira.sora.org -d taira-explorer.sora.org`
   - certbot stores this SAN cert under `.../live/taira.sora.org/` and nginx can
     reuse it for both server blocks.
5. Validate and reload nginx:
   - `sudo nginx -t && sudo systemctl reload nginx`
6. Verify that SNI now serves the correct cert for each host:
   - `curl -vI https://taira.sora.org`
   - `curl -vI https://taira-explorer.sora.org`
   - `echo | openssl s_client -connect taira-explorer.sora.org:443 -servername taira-explorer.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`

The Explorer runtime config targets `https://taira.sora.org`, so both UI reads
and `/v1/*` proxy traffic follow the Taira Torii endpoint.
