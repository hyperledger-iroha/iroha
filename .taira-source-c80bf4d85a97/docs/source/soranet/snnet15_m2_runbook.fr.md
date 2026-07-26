---
lang: fr
direction: ltr
source: docs/source/soranet/snnet15_m2_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df74719f9db8c9b55c8e73afb168cb51b275604efa2ec8ed7e2f72f7cb12f59e
source_last_modified: "2026-01-03T18:08:01.779758+00:00"
translation_last_reviewed: 2026-01-30
---

# SNNet-15M2 — Gateway beta runbook

The M2 beta bundle ships the SoraGlobal Gateway CDN with DoQ/ODoH preview,
trustless CAR verification, GAR compliance exports, and hardening evidence.

## Steps
- Generate the beta pack: `cargo xtask soranet-gateway-m2 --config configs/soranet/gateway_m2/beta_config.json --output-dir artifacts/soranet/gateway_m2/beta`
- Confirm artefacts:
  - `gateway_m2_summary.json` lists each PoP with the beta edge config, trustless verifier path, PQ readiness summary, hardening summary, and ops bundle.
  - Billing section carries `invoice`/`ledger_projection` and enforces `allow_hard_cap=true`.
  - Compliance export lives in `compliance_summary.{json,md}` built from GAR receipts/ACKs.
- PQ readiness per PoP: provide `--srcv2`, `--tls-bundle`, and `--trustless-config` in the config to emit `pq/` summaries under each `pops/<pop>/beta/`.
- Hardening: supplying `sbom`, `vuln_report`, `hsm_policy`, and `sandbox_profile` emits `gateway_hardening_summary.{json,md}` with retention signalling (warns if >30 days).

## Outputs
- `pops/<label>/gateway_edge_beta.yaml` — H3 + DoQ + ODoH preview config with trustless verifier binding.
- `billing/` — JSON/CSV/Parquet invoices plus ledger projection and totals.
- `compliance_summary.{json,md}` — GAR enforcement rollup.
- `gateway_m2_summary.json` — Canonical evidence root for governance packets.
