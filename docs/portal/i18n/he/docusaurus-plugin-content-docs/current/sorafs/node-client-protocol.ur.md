---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS نوڈ ↔ کلائنٹ پروٹوکول

یہ گائیڈ پروٹوکول کی canonical تعریف کو
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
میں summarize کرتی ہے۔ byte-level Norito layouts اور changelogs کے لیے upstream
spec استعمال کریں؛ portal copy SoraFS runbooks کے ساتھ operational highlights کو
قریب رکھتی ہے۔

## פרסומות ספק או אימות

SoraFS providers `ProviderAdvertV1` payloads (دیکھیں
`crates/sorafs_manifest::provider_advert`) gossip کرتے ہیں جو governed operator
نے sign کیے ہوتے ہیں۔ adverts discovery metadata اور guardrails کو pin کرتے ہیں
جنہیں multi-source orchestrator runtime میں enforce کرتا ہے۔

- **משך חיים** ​​— `issued_at < expires_at ≤ issued_at + 86,400 s`. ספקים קשו
  ہر 12 گھنٹے میں refresh کرنا چاہیے۔
- **Capability TLVs** — TLV list transport features advertise کرتی ہے (Torii,
  QUIC+Noise, ממסרי SoraNet, הרחבות ספקים). קודים לא ידועים
  `allow_unknown_capabilities = true` پر skip کیا جا سکتا ہے، GREASE guidance کے مطابق۔
- **רמזים ל-QoS** — שכבת `availability` (חם/חם/קר), זמן אחזור מקסימלי,
  מגבלת במקביל, או תקציב זרם אופציונלי. QoS کو observed telemetry کے
  مطابق ہونا چاہیے اور admission میں audit کیا جاتا ہے۔
- **Endpoints اور rendezvous topics** — TLS/ALPN metadata کے ساتھ concrete
  service URLs اور discovery topics جن پر clients کو guard sets بناتے وقت subscribe
  ‏
- **מדיניות גיוון בנתיבים** — `min_guard_weight`, כובעי מאוורר AS/בריכה, ועוד
  `provider_failure_threshold` deterministic multi-peer fetches کو ممکن بناتے ہیں۔
- **Profile identifiers** — providers کو canonical handle expose کرنا ہوتا ہے
  (مثلاً `sorafs.sf1@1.0.0`); optional `profile_aliases` پرانے clients کی
  migration میں مدد دیتے ہیں۔

כללי אימות אפס הימור, יכולת/נקודות קצה/רשימות נושאים ריקות, סדר שגוי
lifetimes، یا missing QoS targets کو reject کرتی ہیں۔ מודעת מעטפות כניסה
גופי הצעות (`compare_core_fields`) השוו בין עדכוני רכילות
‏

### הרחבות אחזור טווח

Range-capable providers درج ذیل metadata شامل کرتے ہیں:

| שדה | מטרה |
|-------|--------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`, `min_granularity` اور alignment/proof flags declare کرتا ہے۔ |
| `StreamBudgetV1` | מעטפת אופציונלית במקביל/תפוקה (`max_in_flight`, `max_bytes_per_sec`, אופציונלי `burst`). נדרשת יכולת טווח 20. |
| `TransportHintV1` | ordered transport preferences (مثلاً `torii_http_range`, `quic_stream`, `soranet_relay`). priorities `0–15` ہیں اور duplicates reject ہوتے ہیں۔ |

תמיכה בכלים:

- צינורות פרסום של ספקים, יכולת טווח, תקציב זרם, ועוד רמזים לתחבורה
  validate کرنے چاہییں پھر audits کے لیے deterministic payloads emit کریں۔
- `cargo xtask sorafs-admission-fixtures` פרסומות מרובות מקורות קנוניות
  downgrade fixtures کے ساتھ `fixtures/sorafs_manifest/provider_admission/` میں bundle کرتا ہے۔
- Range-capable adverts جو `stream_budget` یا `transport_hints` omit کریں، CLI/SDK
  loaders انہیں scheduling سے پہلے reject کرتے ہیں تاکہ multi-source harness
  Torii admission expectations کے ساتھ aligned رہے۔

## נקודות קצה של טווח שערGateways deterministic HTTP requests accept کرتے ہیں جو advert metadata کو mirror
کرتی ہیں۔

### `GET /v2/sorafs/storage/car/{manifest_id}`

| דרישה | פרטים |
|------------|--------|
| **כותרות** | `Range` (single window aligned to chunk offsets), `dag-scope: block`, `X-SoraFS-Chunker`, optional `X-SoraFS-Nonce`, اور لازمی base64 `X-SoraFS-Stream-Token`. |
| **תגובות** | `206` with `Content-Type: application/vnd.ipld.car`, `Content-Range` جو served window کو بیان کرتا ہے، `X-Sora-Chunk-Range` metadata، اور chunker/token headers کا echo۔ |
| **מצבי כשל** | misaligned ranges پر `416`, missing/invalid tokens پر `401`, اور stream/byte budgets exceed ہونے پر `429`۔ |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Single-chunk fetch انہی headers کے ساتھ plus deterministic chunk digest۔ ניסיונות חוזרים
یا forensic downloads کے لیے مفید جب CAR slices غیر ضروری ہوں۔

## זרימת עבודה של מתזמר מרובה מקורות

جب SF-6 multi-source fetch enabled ہو (Rust CLI via `sorafs_fetch`, SDKs via
`sorafs_orchestrator`):

1. **Collect inputs** — manifest chunk plan decode کریں، latest adverts pull کریں،
   اور optional telemetry snapshot (`--telemetry-json` یا `TelemetrySnapshot`) پاس کریں۔
2. **בנה לוח תוצאות** — הערכת זכאות `Orchestrator::build_scoreboard`
   کرتا ہے اور rejection reasons record کرتا ہے؛ `sorafs_fetch --scoreboard-out`
   JSON persist کرتا ہے۔
3. **Schedule chunks** — `fetch_with_scoreboard` (یا `--plan`) range constraints،
   stream budgets، retry/peer caps (`--retry-budget`, `--max-peers`) enforce کرتا ہے
   اور ہر request کے لیے manifest-scoped stream token emit کرتا ہے۔
4. **Verify receipts** — outputs میں `chunk_receipts` اور `provider_reports` شامل
   ہوتے ہیں؛ סיכומי CLI `provider_reports`, `chunk_receipts`, אור
   `ineligible_providers` کو evidence bundles کے لیے persist کرتے ہیں۔

Operators/SDKs کو ملنے والی عام errors:

| שגיאה | תיאור |
|-------|-------------|
| `no providers were supplied` | filtering کے بعد کوئی eligible entry نہیں۔ |
| `no compatible providers available for chunk {index}` | مخصوص chunk کے لیے range یا budget mismatch۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` بڑھائیں یا failing peers کو evict کریں۔ |
| `no healthy providers remaining` | repeated failures کے بعد تمام providers disable ہو گئے۔ |
| `streaming observer failed` | downstream CAR writer abort ہو گیا۔ |
| `orchestrator invariant violated` | triage کے لیے manifest، scoreboard، telemetry snapshot، اور CLI JSON capture کریں۔ |

## עדות לטלמטריה אור

- מדדי תזמורת:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (manifest/region/provider کے tags کے ساتھ). dashboards کو fleet کے لحاظ سے
  partition کرنے کے لیے config یا CLI flags میں `telemetry_region` سیٹ کریں۔
- אחזור סיכומי CLI/SDK עם לוח תוצאות מתמשך JSON, קבלות חתיכות או
  provider reports شامل ہوتے ہیں جو SF-6/SF-7 gates کے rollout bundles میں شامل ہونے چاہییں۔
- מטפלי שער `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  expose کرتے ہیں تاکہ SRE dashboards orchestrator decisions اور server behavior
  correlate کر سکیں۔

## עוזרי CLI או REST- `iroha app sorafs pin list|show`, `alias list`, אוור `replication list` PIN-registry
  REST endpoints wrap کرتے ہیں اور audit evidence کے لیے attestation blocks کے ساتھ
  raw Norito JSON print کرتے ہیں۔
- `iroha app sorafs storage pin` אור `torii /v2/sorafs/pin/register` Norito או JSON
  manifests کے ساتھ optional alias proofs اور successors accept کرتے ہیں؛ פגום
  proofs پر `400`, stale proofs پر `503` مع `Warning: 110`, اور hard-expired proofs
  پر `412`۔
- נקודות קצה REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`, `/v2/sorafs/replication`)
  attestation structures شامل کرتے ہیں تاکہ clients latest block headers کے
  خلاف data verify کر سکیں۔

## הפניות

- מפרט קנוני:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- סוגי Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- עוזרי CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- ארגז תזמורת: `crates/sorafs_orchestrator`
- חבילת לוח מחוונים: `dashboards/grafana/sorafs_fetch_observability.json`