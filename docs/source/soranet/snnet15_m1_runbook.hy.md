---
lang: hy
direction: ltr
source: docs/source/soranet/snnet15_m1_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 991feb959f9744baff1f3420a152f4712bf57c8aaf5b43956cd86a6cd417771a
source_last_modified: "2025-12-29T18:16:36.198803+00:00"
translation_last_reviewed: 2026-02-07
---

# SNNet-15M1 Alpha Runbook

Operational steps to execute the M1 alpha scope for the SoraGlobal Gateway CDN. All commands are repository-root relative; adjust `--output-dir` targets as needed for your evidence buckets.

## Shortcut: generate the alpha bundle in one run
- Command:
  ```
  cargo xtask soranet-gateway-m1 \
    --config configs/soranet/gateway_m1/alpha_config.json \
    --output-dir artifacts/soranet/gateway_m1/alpha
  ```
- Results: `gateway_m1_summary.json` under the output root, with per-PoP bundle manifests (popctl), gateway baselines, ops pack summaries, the federated OTEL/rotation bundle, and the billing dry-run artefacts already linked.
- Defaults: the bundled config points at the sample PoP descriptor, meter catalog, and guardrails; adjust the config paths to reference real PoP descriptors, usage snapshots, and ROA bundles before promotion.

## 1) Anycast PoP triad bring-up
- Inputs: Norito PoP descriptor per site (ASN/router-id/prefixes/neighbors), optional ROA bundle, image tag used for the PoP build.
- Command (per PoP):
  ```
  cargo xtask soranet-popctl \
    --input <pop_descriptor.json> \
    --roa <roa_bundle.json> \
    --output-dir artifacts/soranet/pop/<pop_slug> \
    --edns-resolver 127.0.0.1 \
    --image-tag <image_tag>
  ```
- Outputs to attach: `bundle_manifest.json`, `attestations/signoff.json`, FRR config, resolver template, EDNS/DS evidence (if not skipped), PXE/env/secrets templates, CI job stub, sigstore provenance stub. Use the bring-up checklist in `docs/source/soranet/popctl_bringup_checklist.md` to gate promotion.

## 2) Resolver + gateway alpha baseline
- Inputs: edge name per PoP, target output directory, origin wiring (HTTP/S3/SoraFS).
- Command (per PoP):
  ```
  cargo xtask soranet-gateway-m0 \
    --edge-name <edge_name> \
    --output-dir artifacts/soranet/gateway_m1/<pop_slug>
  ```
- Outputs: `gateway_edge_h3.yaml`, `gateway_trustless_verifier.toml`, `gateway_waf_policy.yaml`, `gateway_m0_summary.json`. Update origin blocks and TLS/ECH vault refs before deployment. Reference shape in `docs/source/soranet_gateway_m0.md`.
- Resolver: generate per-PoP resolver templates via the PoP bundle (`resolver.toml`) and wire DoH/DoT endpoints with DNSSEC/serve-stale enabled.

## 3) Billing guardrails & ledger dry-run
- Inputs: staged usage snapshot for the alpha period, meter catalog/guardrails (`configs/soranet/gateway_m0/billing/`), payer/treasury/account IDs (alpha tenants).
- Command:
  ```
  cargo xtask soranet-gateway-billing \
    --usage <usage_snapshot.json> \
    --catalog configs/soranet/gateway_m0/billing/billing_meter_catalog.json \
    --guardrails configs/soranet/gateway_m0/billing/billing_guardrails.json \
    --output-dir artifacts/soranet/gateway_billing/m1_alpha \
    --payer <payer_account> \
    --treasury <treasury_account> \
    --asset xor#wonderland
  ```
- Outputs: `billing_invoice.{json,csv,parquet}`, `billing_guardrails.json`, `billing_ledger_projection.json`, reconciliation report. Include alert triggers if caps are crossed; set `--allow-hard-cap` only for controlled drills.

## 4) Observability & SLO dashboards
- Inputs: per-PoP label, output directory for configs.
- Command (per PoP):
  ```
  cargo xtask soranet-gateway-ops-m0 \
    --pop <pop_slug> \
    --output-dir artifacts/soranet/gateway_ops/<pop_slug>
  ```
- Outputs: OTEL collector config, Grafana dashboard, alert rules, GameDay plan, GAR compliance outline, security baseline, PQ readiness checklist, ops summary. Deploy collectors → Kafka/PRW/Loki, hook alerts to on-call, and run the mini GameDay (BGP withdraw, resolver brownout, tail-sampling stress) before onboarding alpha tenants.

## Evidence packaging
- For each PoP, bundle: popctl manifest/signoff, gateway/resolver configs, ops pack, and billing dry-run artefacts into the alpha readiness packet.
- Record sigstore/intoto paths and checksum hashes in the promotion ticket; attach screenshots of Grafana SLO panels and Alertmanager firings during drills.
