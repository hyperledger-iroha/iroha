---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03cf6fe78b8e0c8a68a15d2d7bdccb78b80a2f443ba9361f19ee8a114daab735
source_last_modified: "2025-11-21T19:51:16.332517+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS نوڈ ↔ کلائنٹ پروٹوکول

یہ گائیڈ پروٹوکول کی canonical تعریف کو
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
میں summarize کرتی ہے۔ byte-level Norito layouts اور changelogs کے لیے upstream
spec استعمال کریں؛ portal copy SoraFS runbooks کے ساتھ operational highlights کو
قریب رکھتی ہے۔

## Provider adverts اور validation

SoraFS providers `ProviderAdvertV1` payloads (دیکھیں
`crates/sorafs_manifest::provider_advert`) gossip کرتے ہیں جو governed operator
نے sign کیے ہوتے ہیں۔ adverts discovery metadata اور guardrails کو pin کرتے ہیں
جنہیں multi-source orchestrator runtime میں enforce کرتا ہے۔

- **Lifetime** — `issued_at < expires_at ≤ issued_at + 86,400 s`. providers کو
  ہر 12 گھنٹے میں refresh کرنا چاہیے۔
- **Capability TLVs** — TLV list transport features advertise کرتی ہے (Torii,
  QUIC+Noise, SoraNet relays, vendor extensions). unknown codes کو
  `allow_unknown_capabilities = true` پر skip کیا جا سکتا ہے، GREASE guidance کے مطابق۔
- **QoS hints** — `availability` tier (Hot/Warm/Cold)، max retrieval latency،
  concurrency limit، اور optional stream budget. QoS کو observed telemetry کے
  مطابق ہونا چاہیے اور admission میں audit کیا جاتا ہے۔
- **Endpoints اور rendezvous topics** — TLS/ALPN metadata کے ساتھ concrete
  service URLs اور discovery topics جن پر clients کو guard sets بناتے وقت subscribe
  کرنا چاہیے۔
- **Path diversity policy** — `min_guard_weight`, AS/pool fan-out caps، اور
  `provider_failure_threshold` deterministic multi-peer fetches کو ممکن بناتے ہیں۔
- **Profile identifiers** — providers کو canonical handle expose کرنا ہوتا ہے
  (مثلاً `sorafs.sf1@1.0.0`); optional `profile_aliases` پرانے clients کی
  migration میں مدد دیتے ہیں۔

Validation rules zero stake، empty capability/endpoints/topic lists، misordered
lifetimes، یا missing QoS targets کو reject کرتی ہیں۔ admission envelopes advert
اور proposal bodies (`compare_core_fields`) کو compare کرتے ہیں پھر updates gossip
کرتے ہیں۔

### Range fetch extensions

Range-capable providers درج ذیل metadata شامل کرتے ہیں:

| Field | Purpose |
|-------|---------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`, `min_granularity` اور alignment/proof flags declare کرتا ہے۔ |
| `StreamBudgetV1` | optional concurrency/throughput envelope (`max_in_flight`, `max_bytes_per_sec`, optional `burst`). range capability required ہے۔ |
| `TransportHintV1` | ordered transport preferences (مثلاً `torii_http_range`, `quic_stream`, `soranet_relay`). priorities `0–15` ہیں اور duplicates reject ہوتے ہیں۔ |

Tooling support:

- Provider advert pipelines کو range capability، stream budget، اور transport hints
  validate کرنے چاہییں پھر audits کے لیے deterministic payloads emit کریں۔
- `cargo xtask sorafs-admission-fixtures` canonical multi-source adverts کو
  downgrade fixtures کے ساتھ `fixtures/sorafs_manifest/provider_admission/` میں bundle کرتا ہے۔
- Range-capable adverts جو `stream_budget` یا `transport_hints` omit کریں، CLI/SDK
  loaders انہیں scheduling سے پہلے reject کرتے ہیں تاکہ multi-source harness
  Torii admission expectations کے ساتھ aligned رہے۔

## Gateway range endpoints

Gateways deterministic HTTP requests accept کرتے ہیں جو advert metadata کو mirror
کرتی ہیں۔

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Requirement | Details |
|-------------|---------|
| **Headers** | `Range` (single window aligned to chunk offsets), `dag-scope: block`, `X-SoraFS-Chunker`, optional `X-SoraFS-Nonce`, اور لازمی base64 `X-SoraFS-Stream-Token`. |
| **Responses** | `206` with `Content-Type: application/vnd.ipld.car`, `Content-Range` جو served window کو بیان کرتا ہے، `X-Sora-Chunk-Range` metadata، اور chunker/token headers کا echo۔ |
| **Failure modes** | misaligned ranges پر `416`, missing/invalid tokens پر `401`, اور stream/byte budgets exceed ہونے پر `429`۔ |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Single-chunk fetch انہی headers کے ساتھ plus deterministic chunk digest۔ retries
یا forensic downloads کے لیے مفید جب CAR slices غیر ضروری ہوں۔

## Multi-source orchestrator workflow

جب SF-6 multi-source fetch enabled ہو (Rust CLI via `sorafs_fetch`, SDKs via
`sorafs_orchestrator`):

1. **Collect inputs** — manifest chunk plan decode کریں، latest adverts pull کریں،
   اور optional telemetry snapshot (`--telemetry-json` یا `TelemetrySnapshot`) پاس کریں۔
2. **Build a scoreboard** — `Orchestrator::build_scoreboard` eligibility evaluate
   کرتا ہے اور rejection reasons record کرتا ہے؛ `sorafs_fetch --scoreboard-out`
   JSON persist کرتا ہے۔
3. **Schedule chunks** — `fetch_with_scoreboard` (یا `--plan`) range constraints،
   stream budgets، retry/peer caps (`--retry-budget`, `--max-peers`) enforce کرتا ہے
   اور ہر request کے لیے manifest-scoped stream token emit کرتا ہے۔
4. **Verify receipts** — outputs میں `chunk_receipts` اور `provider_reports` شامل
   ہوتے ہیں؛ CLI summaries `provider_reports`, `chunk_receipts`, اور
   `ineligible_providers` کو evidence bundles کے لیے persist کرتے ہیں۔

Operators/SDKs کو ملنے والی عام errors:

| Error | Description |
|-------|-------------|
| `no providers were supplied` | filtering کے بعد کوئی eligible entry نہیں۔ |
| `no compatible providers available for chunk {index}` | مخصوص chunk کے لیے range یا budget mismatch۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` بڑھائیں یا failing peers کو evict کریں۔ |
| `no healthy providers remaining` | repeated failures کے بعد تمام providers disable ہو گئے۔ |
| `streaming observer failed` | downstream CAR writer abort ہو گیا۔ |
| `orchestrator invariant violated` | triage کے لیے manifest، scoreboard، telemetry snapshot، اور CLI JSON capture کریں۔ |

## Telemetry اور evidence

- Orchestrator کی metrics:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (manifest/region/provider کے tags کے ساتھ). dashboards کو fleet کے لحاظ سے
  partition کرنے کے لیے config یا CLI flags میں `telemetry_region` سیٹ کریں۔
- CLI/SDK fetch summaries میں persisted scoreboard JSON، chunk receipts اور
  provider reports شامل ہوتے ہیں جو SF-6/SF-7 gates کے rollout bundles میں شامل ہونے چاہییں۔
- Gateway handlers `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  expose کرتے ہیں تاکہ SRE dashboards orchestrator decisions اور server behavior
  correlate کر سکیں۔

## CLI اور REST helpers

- `iroha app sorafs pin list|show`, `alias list`, اور `replication list` pin-registry
  REST endpoints wrap کرتے ہیں اور audit evidence کے لیے attestation blocks کے ساتھ
  raw Norito JSON print کرتے ہیں۔
- `iroha app sorafs storage pin` اور `torii /v1/sorafs/pin/register` Norito یا JSON
  manifests کے ساتھ optional alias proofs اور successors accept کرتے ہیں؛ malformed
  proofs پر `400`, stale proofs پر `503` مع `Warning: 110`, اور hard-expired proofs
  پر `412`۔
- REST endpoints (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  attestation structures شامل کرتے ہیں تاکہ clients latest block headers کے
  خلاف data verify کر سکیں۔

## References

- Canonical spec:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito types: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI helpers: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orchestrator crate: `crates/sorafs_orchestrator`
- Dashboard pack: `dashboards/grafana/sorafs_fetch_observability.json`
