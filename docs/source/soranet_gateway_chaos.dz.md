---
lang: dz
direction: ltr
source: docs/source/soranet_gateway_chaos.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1299b835f41f38cd1d386b7fdbbaec60e86f893dc1c9ece7ecf2bd4550e6d4fa
source_last_modified: "2025-12-29T18:16:36.206185+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraNet Gateway Chaos & GameDay (SNNet-15F1)

The SoraGlobal gateway M0 pack bundles a chaos harness plus quarterly GameDay runbooks so SRE can rehearse the prefix withdrawal, trustless verifier stall, and resolver brownout drills with consistent evidence.

## Commands
- Generate the default pack (plan/runbook/schedule/metrics/injector) for a PoP: `cargo xtask soranet-gateway-ops-m0 --output-dir configs/soranet/gateway_m0/observability --pop soranet-pop-m0`.
- Generate the multi-PoP alpha bundle with per-PoP packs, a federated OTEL collector, and the GameDay rotation: `cargo xtask soranet-gateway-ops-m1 --pops sjc-01,iad-01,fra-01 --output-dir artifacts/soranet/gateway_m1/observability`.
- Run the chaos harness (dry-run by default; add `--execute` to run the scripted commands): `cargo xtask soranet-gateway-chaos --pop <pop> --config chaos_scenarios.json --out artifacts/soranet/gateway_chaos --scenario <id>[,<id>...] [--note <text>]`.
- The convenience script `configs/soranet/gateway_m0/observability/chaos_injector.sh` wraps the harness and defaults to the bundled scenario pack.

## Outputs
- `chaos_scenarios.json` – scenario pack (inject/verify/remediate steps + alert budgets) and `gameday_schedule.json` – quarterly rotation.
- `chaos_runbook.md` and `chaos_plan.md` – operator notes and step-by-step plan per scenario.
- `chaos_metrics.json` – PromQL checks to staple to each drill; `chaos_injector.sh` – harness wrapper.
- Harness runs emit `run_<timestamp-pop>/chaos_plan.json`, `chaos_report.json`, and `chaos_report.md` under the chosen `--out` directory.
- The multi-PoP bundle adds a federated OTEL collector (`otel_federated.yaml`), per-PoP ops packs under `<output>/<pop>/`, a rotation file (`gameday_rotation.json`/`gameday_rotation.md`), and a summary at `ops_federated_summary.json`.

## Default scenarios (M0)
- **prefix-withdrawal** (`BgpSessionFlap`, 300s alert / 900s recovery, backlog cap 10%): withdraw one upstream prefix, watch cache hit/backlog gauges and scrape health, then restore the prefix and capture alert recovery.
- **trustless-verifier-failure** (`GatewayVerifierStall`, 240s alert / 600s recovery, backlog cap 5%): pause the verifier, send probes, validate cache-binding/quarantine alerts, and resume the verifier while confirming backlog drains.
- **resolver-brownout** (`ResolverProofStale`, 300s alert / 600s recovery, backlog cap 8%): throttle RAD sync, watch proof age/alerts plus GAR enforcement pause, then restore sync and re-arm enforcement.

## Evidence cadence
- Store Alertmanager exports and Grafana screenshots (cache hit rate, resolver proof age, backlog/burn-rate panels) with each drill invite.
- Keep the harness JSON/Markdown exports alongside manual notes in `artifacts/soranet/gateway_chaos/run_*` and annotate `chaos_metrics.json` queries with pass/fail.
