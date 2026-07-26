---
lang: ar
direction: rtl
source: docs/source/soranet_gateway_pq_readiness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a49455b8ef54c8ab413dc8cebfcc63be4c796e4d2896eba3545feb9bbe016f6
source_last_modified: "2026-01-03T18:07:58.682417+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraGlobal Gateway PQ Readiness (SNNet-15PQ)

The SNNet-15PQ milestone aligns the SoraGlobal gateway CDN with the SNNet-16
post-quantum transport policy. Operators must prove that PoP TLS/ECH material is
ready for SRCv2 dual-signature rotation, that the trustless verifier pipeline
enforces cache binding and SDR timing bounds, and that canary hosts exercise the
PQ handshakes with downgrade telemetry attached.

## Command

Run the new readiness helper from the repository root:

```
cargo xtask soranet-gateway-pq \
  --srcv2 configs/soranet/gateway_m0/guards/relay.srcv2.cbor \
  --tls-bundle artifacts/soranet/gateway_m0_lab \
  --trustless-config configs/soranet/gateway_m0/gateway_trustless_verifier.toml \
  --pop sjc-01
```

Flags:
- `--srcv2` — CBOR-encoded `RelayCertificateBundleV2` for the PoP.
- `--tls-bundle` — directory with `fullchain.pem`, `privkey.pem`, and
  `ech.json` (the default ACME automation output).
- `--trustless-config` — gateway verifier TOML (`gateway_trustless_verifier.toml`).
- `--pop` — label used in the summary and default canary hostnames.
- `--canary` — add extra canary hosts (defaults: `canary1.<pop>.gw.sora.id`,
  `canary2.<pop>.gw.sora.id`).
- `--phase` — SRCv2 validation strictness (1=allow single sig, 2=prefer dual,
  3=require dual; default: 3).
- `--out`/`--output-dir` — override the output directory
  (`artifacts/soranet/gateway_pq` by default).

Outputs:
- `gateway_pq_summary.json` — detailed statuses for SRCv2 dual-signature
  validity, PQ handshake suite coverage, TLS/ECH evidence (BLAKE3 fingerprint +
  ECH config), trustless verifier flags, and canary host roster.
- `gateway_pq_summary.md` — short Markdown recap for governance packets.

## Evidence expectations

- SRCv2 bundles must include an ML-DSA signature and advertise NK2/NK3 suites.
- TLS/ECH bundles must exist and the ECH JSON must parse cleanly.
- Trustless verifier config must reject stale cache versions and verify cache
  binding headers; KZG/SDR paths must be populated, not placeholders.
- Telemetry references are baked into the summary:
  - Handshake dashboard: `dashboards/grafana/soranet_sn16_handshake.json`
  - Alert rules: `dashboards/alerts/soranet_handshake_rules.yml`
- Attach the JSON + Markdown outputs to the PoP’s promotion packet alongside
  GameDay/chaos evidence and GAR receipts.

## Runbook

1. Generate the readiness bundle (command above) and store it under the PoP’s
   evidence root.
2. Push SRCv2 and TLS/ECH rotations to canary hosts first; confirm the helper’s
   `overall_status` is `ok`.
3. Watch the SNNet-16 handshake dashboard and alert pack during PQ canary
   traffic; record screenshots and alert exports next to the readiness bundle.
4. Once canaries stay green, roll to the remaining PoP hosts and keep the
   readiness artefacts attached to the release packet.
