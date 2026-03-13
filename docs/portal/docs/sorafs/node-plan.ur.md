---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 293340dfd228f764033d80f93ed131964515fdd37c8c96b60c859cc11f59a5d6
source_last_modified: "2025-11-12T16:00:10.415371+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: node-plan
title: SoraFS node implementation plan
sidebar_label: node implementation plan
description: SF-3 storage roadmap کو milestones، tasks اور test coverage کے ساتھ actionable engineering work میں تبدیل کرتا ہے۔
---

:::note مستند ماخذ
:::

SF-3 پہلا runnable `sorafs-node` crate فراہم کرتا ہے جو Iroha/Torii process کو SoraFS storage provider میں بدلتا ہے۔ اس پلان کو [node storage guide](node-storage.md)، [provider admission policy](provider-admission-policy.md) اور [storage capacity marketplace roadmap](storage-capacity-marketplace.md) کے ساتھ استعمال کریں جب deliverables sequence کریں۔

## Target scope (Milestone M1)

1. **Chunk store integration.** `sorafs_car::ChunkStore` کو ایسے persistent backend سے wrap کریں جو configured data directory میں chunk bytes، manifests اور PoR trees محفوظ کرے۔
2. **Gateway endpoints.** Torii process کے اندر pin submission، chunk fetch، PoR sampling اور storage telemetry کے لیے Norito HTTP endpoints expose کریں۔
3. **Configuration plumbing.** `SoraFsStorage` config struct (enabled flag، capacity، directories، concurrency limits) شامل کریں اور `iroha_config`, `iroha_core`, `iroha_torii` کے ذریعے wire کریں۔
4. **Quota/scheduling.** Operator-defined disk/parallelism limits enforce کریں اور requests کو back-pressure کے ساتھ queue کریں۔
5. **Telemetry.** pin success، chunk fetch latency، capacity utilization اور PoR sampling results کے لیے metrics/logs emit کریں۔

## Work breakdown

### A. Crate & module structure

| Task | Owner(s) | Notes |
|------|----------|-------|
| `crates/sorafs_node` بنائیں جس میں `config`, `store`, `gateway`, `scheduler`, `telemetry` modules ہوں۔ | Storage Team | Torii integration کے لیے reusable types re-export کریں۔ |
| `StorageConfig` implement کریں جو `SoraFsStorage` سے mapped ہو (user → actual → defaults)۔ | Storage Team / Config WG | Norito/`iroha_config` layers کو deterministic رکھیں۔ |
| Torii کے pins/fetches کے لیے `NodeHandle` facade فراہم کریں۔ | Storage Team | storage internals اور async plumbing encapsulate کریں۔ |

### B. Persistent chunk store

| Task | Owner(s) | Notes |
|------|----------|-------|
| `sorafs_car::ChunkStore` کو on-disk manifest index (`sled`/`sqlite`) کے ساتھ disk backend میں wrap کریں۔ | Storage Team | Deterministic layout: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| `ChunkStore::sample_leaves` کے ذریعے PoR metadata (64 KiB/4 KiB trees) maintain کریں۔ | Storage Team | Restart کے بعد replay support؛ corruption پر fail fast۔ |
| Startup پر integrity replay implement کریں (manifests rehash، incomplete pins prune)۔ | Storage Team | replay مکمل ہونے تک Torii start block کریں۔ |

### C. Gateway endpoints

| Endpoint | Behaviour | Tasks |
|----------|-----------|-------|
| `POST /sorafs/pin` | `PinProposalV1` قبول کریں، manifests validate کریں، ingestion queue کریں، manifest CID واپس دیں۔ | chunk profile validate کریں، quotas enforce کریں، chunk store کے ذریعے data stream کریں۔ |
| `GET /sorafs/chunks/{cid}` + range query | `Content-Chunker` headers کے ساتھ chunk bytes serve کریں؛ range capability spec respect کریں۔ | scheduler + stream budgets استعمال کریں (SF-2d range capability کے ساتھ tie کریں)۔ |
| `POST /sorafs/por/sample` | manifest کے لیے PoR sampling چلائیں اور proof bundle واپس کریں۔ | chunk store sampling reuse کریں، Norito JSON payloads کے ساتھ respond کریں۔ |
| `GET /sorafs/telemetry` | Summaries: capacity، PoR success، fetch error counts۔ | dashboards/operators کے لیے data فراہم کریں۔ |

Runtime plumbing `sorafs_node::por` کے ذریعے PoR interactions کو thread کرتی ہے: tracker ہر `PorChallengeV1`, `PorProofV1`, `AuditVerdictV1` کو record کرتا ہے تاکہ `CapacityMeter` metrics governance verdicts کو Torii-specific logic کے بغیر reflect کریں۔【crates/sorafs_node/src/scheduler.rs#L147】

Implementation notes:

- Torii کے Axum stack کو `norito::json` payloads کے ساتھ استعمال کریں۔
- responses کے لیے Norito schemas شامل کریں (`PinResultV1`, `FetchErrorV1`, telemetry structs)۔

- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` اب backlog depth کے ساتھ oldest epoch/deadline اور ہر provider کے recent success/failure timestamps دکھاتا ہے، جو `sorafs_node::NodeHandle::por_ingestion_status` سے powered ہے، اور Torii dashboards کے لیے `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` gauges record کرتا ہے۔【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Scheduler & quota enforcement

| Task | Details |
|------|---------|
| Disk quota | Disk bytes track کریں؛ `max_capacity_bytes` پر نئے pins reject کریں۔ مستقبل کی policies کے لیے eviction hooks فراہم کریں۔ |
| Fetch concurrency | Global semaphore (`max_parallel_fetches`) کے ساتھ per-provider budgets جو SF-2d range caps سے آتے ہیں۔ |
| Pin queue | Outstanding ingestion jobs محدود کریں؛ queue depth کے لیے Norito status endpoints expose کریں۔ |
| PoR cadence | `por_sample_interval_secs` سے چلنے والا background worker۔ |

### E. Telemetry & logging

Metrics (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histogram with `result` labels)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Logs / events:

- governance ingestion کے لیے structured Norito telemetry (`StorageTelemetryV1`).
- utilization > 90% یا PoR failure streak threshold سے اوپر ہو تو alerts۔

### F. Testing strategy

1. **Unit tests.** chunk store persistence، quota calculations، scheduler invariants (see `crates/sorafs_node/src/scheduler.rs`).
2. **Integration tests** (`crates/sorafs_node/tests`). Pin → fetch round trip، restart recovery، quota rejection، PoR sampling proof verification۔
3. **Torii integration tests.** Torii کو storage enabled کے ساتھ چلائیں، HTTP endpoints کو `assert_cmd` کے ذریعے exercise کریں۔
4. **Chaos roadmap.** مستقبل کے drills disk exhaustion، slow IO اور provider removal simulate کریں گے۔

## Dependencies

- SF-2b admission policy — nodes کو adverts publish کرنے سے پہلے admission envelopes verify کرنے ہوں گے۔
- SF-2c capacity marketplace — telemetry کو capacity declarations کے ساتھ tie کریں۔
- SF-2d advert extensions — range capability + stream budgets دستیاب ہونے پر consume کریں۔

## Milestone exit criteria

- `cargo run -p sorafs_node --example pin_fetch` local fixtures کے خلاف کام کرے۔
- Torii `--features sorafs-storage` کے ساتھ build ہو اور integration tests پاس کرے۔
- Documentation ([node storage guide](node-storage.md)) config defaults + CLI examples کے ساتھ updated ہو؛ operator runbook دستیاب ہو۔
- Telemetry staging dashboards میں نظر آئے؛ capacity saturation اور PoR failures کے لیے alerts configure ہوں۔

## Documentation & ops deliverables

- [node storage reference](node-storage.md) کو config defaults، CLI usage اور troubleshooting steps کے ساتھ update کریں۔
- [node operations runbook](node-operations.md) کو implementation کے ساتھ align رکھیں جیسے SF-3 evolve ہو۔
- `/sorafs/*` endpoints کی API references developer portal میں publish کریں اور Torii handlers آنے کے بعد OpenAPI manifest سے wire کریں۔
