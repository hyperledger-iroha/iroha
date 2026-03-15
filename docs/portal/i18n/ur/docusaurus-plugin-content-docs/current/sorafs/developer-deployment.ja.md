---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fc324291695ceff391f8a09b64c6f614783fda75ced0cec2c59fe00dc4d00c4e
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: developer-deployment
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
:::

# Deployment notes

SoraFS packaging workflow determinism مضبوط کرتا ہے، اس لئے CI سے production پر جانا بنیادی طور پر operational guardrails مانگتا ہے۔ جب آپ ٹولنگ کو حقیقی gateways اور storage providers پر rollout کریں تو یہ checklist استعمال کریں۔

## Pre-flight

- **Registry alignment** — تصدیق کریں کہ chunker profiles اور manifests ایک ہی `namespace.name@semver` tuple کو refer کرتے ہیں (`docs/source/sorafs/chunker_registry.md`).
- **Admission policy** — `manifest submit` کے لئے درکار signed provider adverts اور alias proofs کا جائزہ لیں (`docs/source/sorafs/provider_admission_policy.md`).
- **Pin registry runbook** — recovery scenarios (alias rotation, replication failures) کے لئے `docs/source/sorafs/runbooks/pin_registry_ops.md` قریب رکھیں۔

## Environment configuration

- Gateways کو proof streaming endpoint (`POST /v1/sorafs/proof/stream`) enable کرنا ہوگا تاکہ CLI telemetry summaries emit کر سکے۔
- `sorafs_alias_cache` policy کو `iroha_config` defaults یا CLI helper (`sorafs_cli manifest submit --alias-*`) کے ذریعے configure کریں۔
- Stream tokens (یا Torii credentials) کو ایک محفوظ secret manager سے فراہم کریں۔
- Telemetry exporters (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) enable کریں اور انہیں اپنے Prometheus/OTel stack میں ship کریں۔

## Rollout strategy

1. **Blue/green manifests**
   - ہر rollout کے لئے responses archive کرنے کے لئے `manifest submit --summary-out` استعمال کریں۔
   - `torii_sorafs_gateway_refusals_total` پر نظر رکھیں تاکہ capability mismatches جلدی پکڑ لیں۔
2. **Proof validation**
   - `sorafs_cli proof stream` میں failures کو deployment blockers سمجھیں؛ latency spikes اکثر provider throttling یا misconfigured tiers کی نشاندہی کرتے ہیں۔
   - Post-pin smoke test میں `proof verify` شامل کریں تاکہ یقینی ہو کہ providers پر hosted CAR اب بھی manifest digest سے match کرتا ہے۔
3. **Telemetry dashboards**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` کو Grafana میں import کریں۔
   - Pin registry health (`docs/source/sorafs/runbooks/pin_registry_ops.md`) اور chunk range stats کے لئے اضافی panels لگائیں۔
4. **Multi-source enablement**
   - Orchestrator آن کرتے وقت `docs/source/sorafs/runbooks/multi_source_rollout.md` کے staged rollout steps فالو کریں، اور audits کے لئے scoreboard/telemetry artifacts archive کریں۔

## Incident handling

- `docs/source/sorafs/runbooks/` میں escalation paths فالو کریں:
  - `sorafs_gateway_operator_playbook.md` gateway outages اور stream-token exhaustion کے لئے۔
  - `dispute_revocation_runbook.md` جب replication disputes ہوں۔
  - `sorafs_node_ops.md` node-level maintenance کے لئے۔
  - `multi_source_rollout.md` orchestrator overrides، peer blacklisting، اور staged rollouts کے لئے۔
- Proof failures اور latency anomalies کو GovernanceLog میں موجود PoR tracker APIs کے ذریعے record کریں تاکہ governance provider performance assess کر سکے۔

## Next steps

- Multi-source fetch orchestrator (SF-6b) آنے پر orchestrator automation (`sorafs_car::multi_fetch`) integrate کریں۔
- PDP/PoTR upgrades کو SF-13/SF-14 کے تحت track کریں؛ جب یہ proofs stabilize ہوں تو CLI اور docs deadlines اور tier selection surface کریں گے۔

ان deployment notes کو quickstart اور CI recipes کے ساتھ ملانے سے ٹیمیں local experiments سے production-grade SoraFS pipelines تک repeatable اور observable process کے ساتھ جا سکتی ہیں۔
