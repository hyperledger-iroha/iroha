---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس پرووائیڈر adverts اور شیڈولنگ

یہ صفحہ درج ذیل دستاویز میں موجود canonical spec کو خلاصہ کرتا ہے:
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
סכימות Norito או יומני שינויים ‏ ‏ portal کی کاپی
آپریٹر گائیڈنس، SDK نوٹس، اور ٹیلیمیٹری حوالہ جات کو SoraFS کے باقی runbooks کے قریب رکھتی ہے۔

## סכימת Norito

### יכולת טווח (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – فی درخواست سب سے بڑا مسلسل span (bytes)، `>= 1`.
- `min_granularity` – seek ریزولوشن، `1 <= قدر <= max_chunk_span`.
- `supports_sparse_offsets` – ایک درخواست میں غیر مسلسل offsets کی اجازت دیتا ہے۔
- `requires_alignment` – اگر true ہو تو offsets کو `min_granularity` کے مطابق align ہونا لازم ہے۔
- `supports_merkle_proof` – PoR witness کی سپورٹ ظاہر کرتا ہے۔

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` canonical encoding نافذ کرتے ہیں
تاکہ gossip payloads deterministic رہیں۔

### `StreamBudgetV1`
- فیلڈز: `max_in_flight`, `max_bytes_per_sec`, اختیاری `burst_bytes`.
- כללי אימות (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` موجود ہو تو `> 0` اور `<= max_bytes_per_sec` ہونا چاہیے۔

### `TransportHintV1`
- טלפונים: `protocol: TransportProtocol`, `priority: u8` (0-15 גרסאות
  `TransportHintV1::validate` نافذ کرتا ہے).
- معروف protocols: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- فی provider ڈپلیکیٹ protocol entries مسترد کی جاتی ہیں۔

### `ProviderAdvertBodyV1` מפה
- اختیاری `stream_budget: Option<StreamBudgetV1>`.
- اختیاری `transport_hints: Option<Vec<TransportHintV1>>`.
- טלפונים של `ProviderAdmissionProposalV1`, מעטפות ממשל, גופי CLI, או טלמטרים JSON טלמטריים.

## אימות או ממשל מחייב

`ProviderAdvertBodyV1::validate` אור `ProviderAdmissionProposalV1::validate`
خراب metadata کو مسترد کرتے ہیں:

- Range capabilities کو decode ہونا چاہیے اور span/granularity limits پوری کرنی چاہئیں۔
- Stream budgets / transport hints کے لیے `CapabilityType::ChunkRangeFetch` TLV اور non-empty hints list لازم ہے۔
- ڈپلیکیٹ transport protocols اور غیر درست priorities gossip سے پہلے validation errors پیدا کرتے ہیں۔
- Admission envelopes `compare_core_fields` کے ذریعے proposal/adverts کے range metadata کو compare کرتے ہیں تاکہ mismatch gossip payloads جلدی مسترد ہوں۔

Regression coverage یہاں موجود ہے:
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## אביזרי כלי עבודה

- Provider advert payloads میں `range_capability`, `stream_budget`, اور `transport_hints` شامل ہونا لازم ہے۔
  `/v1/sorafs/providers` responses اور admission fixtures کے ذریعے validate کریں؛ JSON summaries میں parsed capability، stream budget، اور hint arrays شامل ہونے چاہئیں تاکہ telemetry ingest ہو سکے۔
- `cargo xtask sorafs-admission-fixtures` اپنے JSON artefacts میں stream budgets اور transport hints دکھاتا ہے تاکہ dashboards feature adoption track کر سکیں۔
- `fixtures/sorafs_manifest/provider_admission/` کے تحت fixtures اب شامل کرتے ہیں:
  - פרסומות מרובות מקורות קנוניות,
  - `multi_fetch_plan.json` تاکہ SDK suites deterministic multi-peer fetch plan replay کر سکیں۔

## Orchestrator اور Torii انضمام- Torii `/v1/sorafs/providers` מטא נתונים של יכולת טווח מנותח
  providers جب نئی metadata چھوڑ دیں تو downgrade warnings چلتے ہیں، اور gateway range endpoints براہ راست clients کے لیے یہی constraints نافذ ‏
- מתזמר מרובה מקורות (`sorafs_car::multi_fetch`) מגבלות טווח, יישור יכולות, או תקציבי זרם והקצאת עבודה. Unit tests میں chunk-too-large، sparse-seek، اور throttling scenarios شامل ہیں۔
- `sorafs_car::multi_fetch` אותות שדרוג לאחור (כשלי יישור, בקשות מצטמצמות) זרם מכשירי הפעלה מספקים کیوں skip ہوئے۔

## הפניה לטלמטריה

Torii מכשור אחזור טווח **SoraFS יכולת תצפית אחזרה** לוח המחוונים Grafana
(`dashboards/grafana/sorafs_fetch_observability.json`) اور متعلقہ alert rules
(`dashboards/alerts/sorafs_fetch_rules.yml`) کو feed کرتی ہے۔

| מדד | הקלד | תוויות | תיאור |
|--------|-------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | מד | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Range capability features advertise کرنے والے providers۔ |
| `torii_sorafs_range_fetch_throttle_events_total` | מונה | `reason` (`quota`, `concurrency`, `byte_rate`) | Policy کے مطابق throttled range fetch attempts۔ |
| `torii_sorafs_range_fetch_concurrency_current` | מד | — | Shared concurrency budget استعمال کرنے والی active guarded streams۔ |

קטעי PromQL לדוגמה:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Quota enforcement کی تصدیق کے لیے throttling counter استعمال کریں اس سے پہلے کہ multi-source orchestrator defaults فعال کریں، اور جب concurrency آپ کے fleet کے stream budget maxima کے قریب ہو تو alert کریں۔