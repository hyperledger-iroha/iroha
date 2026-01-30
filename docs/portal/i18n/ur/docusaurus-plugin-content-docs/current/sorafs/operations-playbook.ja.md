---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a6205015dc7cb520a04a7a89d296dba063e92d01a3becf50231f2b36ff8ff3ad
source_last_modified: "2025-11-14T04:43:21.953110+00:00"
translation_last_reviewed: 2026-01-30
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs_ops_playbook.md` میں برقرار رکھے گئے runbook کی عکاسی کرتا ہے۔ جب تک Sphinx ڈاکیومنٹیشن مکمل طور پر منتقل نہ ہو جائے دونوں نقول کو ہم آہنگ رکھیں۔
:::

## کلیدی حوالہ جات

- Observability assets: `dashboards/grafana/` میں Grafana dashboards اور `dashboards/alerts/` میں Prometheus alert rules دیکھیں۔
- Metric catalog: `docs/source/sorafs_observability_plan.md`.
- Orchestrator telemetry surfaces: `docs/source/sorafs_orchestrator_plan.md`.

## Escalation matrix

| ترجیح | ٹرگر مثالیں | بنیادی on-call | بیک اپ | نوٹس |
|--------|--------------|---------------|--------|------|
| P1 | عالمی gateway outage، PoR failure rate > 5% (15 منٹ)، replication backlog ہر 10 منٹ میں دوگنا | Storage SRE | Observability TL | اگر اثر 30 منٹ سے زیادہ ہو تو governance council کو engage کریں۔ |
| P2 | علاقائی gateway latency SLO breach، orchestrator retry spike بغیر SLA اثر کے | Observability TL | Storage SRE | rollout جاری رکھیں مگر نئے manifests gate کریں۔ |
| P3 | غیر اہم alerts (manifest staleness، capacity 80–90%) | Intake triage | Ops guild | اگلے کاروباری دن میں نمٹا دیں۔ |

## Gateway outage / degraded availability

**Detection**

- Alerts: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Dashboard: `dashboards/grafana/sorafs_gateway_overview.json`.

**Immediate actions**

1. request-rate panel کے ذریعے scope کنفرم کریں (single provider vs fleet)۔
2. اگر multi-provider ہو تو ops config (`docs/source/sorafs_gateway_self_cert.md`) میں `sorafs_gateway_route_weights` ٹوگل کر کے Torii routing کو صحت مند providers کی طرف کریں۔
3. اگر تمام providers متاثر ہوں تو CLI/SDK clients کے لیے “direct fetch” fallback فعال کریں (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- `sorafs_gateway_stream_token_limit` کے مقابل stream token utilisation دیکھیں۔
- TLS یا admission errors کے لیے gateway logs inspect کریں۔
- `scripts/telemetry/run_schema_diff.sh` چلائیں تاکہ gateway کے exported schema کا expected version سے match ثابت ہو۔

**Remediation options**

- صرف متاثرہ gateway process ری اسٹارٹ کریں؛ پورے cluster کو recycle نہ کریں جب تک متعدد providers فیل نہ ہوں۔
- اگر saturation کنفرم ہو تو stream token limit کو عارضی طور پر 10–15% بڑھائیں۔
- استحکام کے بعد self-cert دوبارہ چلائیں (`scripts/sorafs_gateway_self_cert.sh`).

**Post-incident**

- `docs/source/sorafs/postmortem_template.md` استعمال کرتے ہوئے P1 postmortem فائل کریں۔
- اگر remediation میں manual interventions شامل ہوں تو follow-up chaos drill شیڈول کریں۔

## Proof failure spike (PoR / PoTR)

**Detection**

- Alerts: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Dashboard: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetry: `torii_sorafs_proof_stream_events_total` اور `sorafs.fetch.error` events جن میں `provider_reason=corrupt_proof` ہو۔

**Immediate actions**

1. manifest registry کو flag کر کے نئی manifest admissions freeze کریں (`docs/source/sorafs/manifest_pipeline.md`).
2. Governance کو notify کریں تاکہ متاثرہ providers کے incentives pause ہوں۔

**Triage**

- PoR challenge queue depth کو `sorafs_node_replication_backlog_total` کے مقابل چیک کریں۔
- حالیہ deployments کے لیے proof verification pipeline (`crates/sorafs_node/src/potr.rs`) validate کریں۔
- provider firmware versions کو operator registry سے compare کریں۔

**Remediation options**

- تازہ ترین manifest کے ساتھ `sorafs_cli proof stream` استعمال کر کے PoR replays trigger کریں۔
- اگر proofs مسلسل fail ہوں تو governance registry اپڈیٹ کر کے provider کو active set سے نکالیں اور orchestrator scoreboards کو refresh کرنے پر مجبور کریں۔

**Post-incident**

- اگلے پروڈکشن deploy سے پہلے PoR chaos drill scenario چلائیں۔
- postmortem template میں lessons capture کریں اور provider qualification checklist اپڈیٹ کریں۔

## Replication lag / backlog growth

**Detection**

- Alerts: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. `dashboards/alerts/sorafs_capacity_rules.yml` امپورٹ کریں اور
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  promotion سے پہلے چلائیں تاکہ Alertmanager documented thresholds کو reflect کرے۔
- Dashboard: `dashboards/grafana/sorafs_capacity_health.json`.
- Metrics: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Immediate actions**

1. backlog کا scope چیک کریں (single provider یا fleet) اور غیر ضروری replication tasks روکیں۔
2. اگر backlog isolated ہو تو replication scheduler کے ذریعے نئے orders کو عارضی طور پر alternate providers کو reasign کریں۔

**Triage**

- orchestrator telemetry میں retry bursts دیکھیں جو backlog بڑھا سکتے ہیں۔
- storage targets کے لیے headroom کنفرم کریں (`sorafs_node_capacity_utilisation_percent`).
- حالیہ config changes (chunk profile updates، proof cadence) ریویو کریں۔

**Remediation options**

- `sorafs_cli` کو `--rebalance` کے ساتھ چلائیں تاکہ content redistribute ہو۔
- متاثرہ provider کے لیے replication workers کو horizontally scale کریں۔
- TTL windows align کرنے کے لیے manifest refresh trigger کریں۔

**Post-incident**

- provider saturation failure پر مرکوز capacity drill شیڈول کریں۔
- replication SLA documentation کو `docs/source/sorafs_node_client_protocol.md` میں اپڈیٹ کریں۔

## Chaos drill cadence

- **Quarterly**: combined gateway outage + orchestrator retry storm simulation.
- **Biannual**: PoR/PoTR failure injection دو providers پر recovery کے ساتھ.
- **Monthly spot-check**: staging manifests کے ساتھ replication lag scenario.
- drills کو shared runbook log (`ops/drill-log.md`) میں اس طرح ٹریک کریں:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- commits سے پہلے log validate کریں:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- آنے والے drills کے لیے `--status scheduled`، مکمل runs کے لیے `pass`/`fail`، اور کھلے action items کے لیے `follow-up` استعمال کریں۔
- dry-runs یا automated verification کے لیے destination `--log` سے override کریں؛ ورنہ اسکرپٹ `ops/drill-log.md` کو اپڈیٹ کرتا رہے گا۔

## Postmortem template

ہر P1/P2 incident اور chaos drill retrospectives کے لیے `docs/source/sorafs/postmortem_template.md` استعمال کریں۔ یہ template timeline، impact quantification، contributing factors، corrective actions، اور follow-up verification tasks کور کرتا ہے۔
