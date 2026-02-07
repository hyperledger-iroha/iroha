---
lang: my
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
---

- [ ] Review hardware spec: 8+ cores, 16 GiB RAM, NVMe storage ≥ 500 MiB/s.
- [ ] Confirm two IPv4 + IPv6 addresses and upstream permits QUIC/UDP 443.
- [ ] Provision HSM or dedicated secure enclave for relay identity keys.
- [ ] Sync canonical opt-out catalogue (`governance/compliance/soranet_opt_outs.json`).
- [ ] Merge compliance block into orchestrator config (see `03-config-example.toml`).
- [ ] Capture jurisdiction/global compliance attestations and populate the `attestations` list.
- [ ] Run `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (or your live snapshot) and review the pass/fail report.
- [ ] Generate relay admission CSR and obtain governance-signed envelope.
- [ ] Import guard descriptor seed and verify against published hash chain.
- [ ] Run `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] Dry-run telemetry export: ensure Prometheus scrape succeeds locally.
- [ ] Schedule brownout drill window and record escalation contacts.
- [ ] Sign the drill evidence bundle: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.
