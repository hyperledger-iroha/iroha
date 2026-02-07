---
lang: zh-hant
direction: ltr
source: docs/source/soranet_gateway_m0.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 46f919fb893e429721474f24ae16a2182d2ec38aaa2e6820d47bdf918655a6e1
source_last_modified: "2025-12-29T18:16:36.207074+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraGlobal Gateway M0 Baseline

SNNet-15M0 requires a repeatable edge baseline before PoPs go live. The
`soranet-gateway-m0` helper produces the three artefacts needed for drills and
change packets:

- `gateway_edge_h3.yaml` – H3 listener with TLS/ECH placeholders, two cache
  tiers, SoraFS origin wiring, and observability defaults.
- `gateway_trustless_verifier.toml` – streaming Merkle/KZG skeleton with SDR
  replay bounds and cache binding checks for SN15-M0-6.
- `gateway_waf_policy.yaml` – draft OWASP + rate pack for H3 ingress (SN15-M0-7).
- `gateway_m0_summary.json` – paths to all generated files for governance
  attachments.

## Usage

```
cargo xtask soranet-gateway-m0 \
  --edge-name lab-h3-sjc \
  --output-dir artifacts/soranet/gateway_m0_lab
```

Defaults:
- Output directory when omitted: `configs/soranet/gateway_m0/`
- Edge name when omitted: `soranet-edge-m0`

## Notes
- The edge config references the trustless verifier + WAF pack via relative
  paths so operators can drop the bundle into CI/k8s pipelines unchanged.
- Tweak TLS/ECH vault references and cache sizing per PoP; keep the rest of the
  shape stable so the SN15-M0 dashboards and alerts stay comparable.
- The trustless verifier skeleton’s KZG/SDR paths are placeholders; point them
  at the signed KZG setup and SDR receipt spool used in your environment before
  promotion drills.
- Billing preview packs live in `configs/soranet/gateway_m0/billing/` and can be
  regenerated with `cargo xtask soranet-gateway-billing-m0` to attach meter
  catalogs, rating rules, and ledger guardrails to rollout evidence.

## Operations pack (SN15-M0-10/11/12)

```
cargo xtask soranet-gateway-ops-m0 \
  --pop qa-pop_01 \
  --output-dir artifacts/soranet/gateway_ops_m0
```

Outputs:
- `otel_collector.yaml` – OTEL collector profile with Prometheus/OTLP receivers,
  Kafka/PrometheusRemoteWrite/Loki exporters, tail-sampling, and privacy
  scrubbers.
- `grafana_dashboard.json` – dashboard panels for latency, cache hit rate, error
  budget burn, and resolver proof age plus deploy annotations.
- `alert_rules.yml` – Prometheus alerts for scrape loss, cache hit regression,
  stale proofs, BGP flaps, and tail-sampling backpressure.
- `gameday_plan.md` – drill scenario checklist tying alerts to evidence capture.
- `chaos_runbook.md` – SNNet-15F1 runbook with alert budgets/backlog caps per scenario.
- `gameday_schedule.json` – quarterly rotation for the chaos scenarios.
- `chaos_plan.md` – SNNet-15F1 inject/validation/remediation playbook for prefix withdrawal, trustless verifier failure, and resolver brownout.
- `chaos_scenarios.json` – machine-readable scenario list used by the harness/injector.
- `chaos_metrics.json` – PromQL goals to validate post-remediation health.
- `chaos_injector.sh` – wrapper around `cargo xtask soranet-gateway-chaos` (dry-run by default; add `--execute` to run inject steps, `--note` to tag evidence).
- `gar_compliance_outline.md` – receipt schema, distribution channels, and audit
  plan for GAR enforcement.
- `security_baseline.md` – TLS/ECH rotation, sandboxing, SBOM/log-retention,
  PQ guardrails, and incident notes.
- `pq_readiness_checklist.json` – PQ rollout checklist with canary host hints.
- `ops_summary.json` – relative paths to all generated files and the provided
  `--pop` identifier.

To seed the alpha triad at once, run:

```
cargo xtask soranet-gateway-ops-m1 \
  --pops sjc-01,iad-01,fra-01 \
  --output-dir artifacts/soranet/gateway_ops_m1
```

This emits per-PoP packs under `<output>/<pop>/`, a federated OTEL collector profile
(`otel_federated.yaml`), a rotation schedule (`gameday_rotation.json` and
`gameday_rotation.md`), and `ops_federated_summary.json` for governance bundles.
