---
lang: fr
direction: ltr
source: docs/source/soranet/snnet15_m3_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fcad3ef48cca81dfef632ae8724b06a66d9ee6b8f95aa373ef6b59655b059aa6
source_last_modified: "2026-01-03T18:08:01.306679+00:00"
translation_last_reviewed: 2026-01-30
---

# SNNet-15M3 — Gateway GA readiness

This runbook consumes the M2 beta evidence and emits the GA bundle for
governance: autoscale/worker digests, SLA target, and links to the beta proofs.

## Steps
- Produce the GA pack:
  - `cargo xtask soranet-gateway-m3 --m2-summary artifacts/soranet/gateway_m2/beta/gateway_m2_summary.json --autoscale-plan <plan.json> --worker-pack <bundle.tgz> --out artifacts/soranet/gateway_m3 --sla-target 99.95%`
- Verify the GA outputs:
  - `gateway_m3_summary.json` / `.md` contain BLAKE3 digests for the autoscale plan and worker pack.
  - `m2_summary` field references the exact beta evidence root.
  - `sla_target` records the production SLO agreed with SRE.

## Exit checklist
- Autoscale plan and worker pack digests embedded in the summary.
- M2 summary referenced and frozen.
- SLA target recorded; dashboards/gates updated to match.
