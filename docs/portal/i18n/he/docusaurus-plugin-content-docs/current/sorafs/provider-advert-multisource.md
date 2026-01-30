---
id: provider-advert-multisource
lang: he
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


# מודעות ספקים מרובי-מקורות ותזמון

עמוד זה מזקק את המפרט הקנוני ב-
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
השתמשו במסמך הזה עבור סכימות Norito מילה במילה ויומני שינוי; עותק הפורטל שומר
את הנחיות המפעילים, הערות ה-SDK והפניות הטלמטריה סמוך לשאר ה-runbooks של SoraFS.

## תוספות לסכמת Norito

### יכולת טווח (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – הטווח הרציף הגדול ביותר (בתים) לכל בקשה, `>= 1`.
- `min_granularity` – רזולוציית seek, `1 <= ערך <= max_chunk_span`.
- `supports_sparse_offsets` – מאפשר offsets לא רציפים בבקשה אחת.
- `requires_alignment` – כאשר true, ה-offsets חייבים להתיישר עם `min_granularity`.
- `supports_merkle_proof` – מציין תמיכת עדות PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` כופים קידוד קנוני
כדי ש-payloads של gossip יישארו דטרמיניסטיים.

### `StreamBudgetV1`
- שדות: `max_in_flight`, `max_bytes_per_sec`, ו-`burst_bytes` אופציונלי.
- כללי אימות (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, כאשר הוא קיים, חייב להיות `> 0` ו-`<= max_bytes_per_sec`.

### `TransportHintV1`
- שדות: `protocol: TransportProtocol`, `priority: u8` (חלון 0-15 נאכף על ידי
  `TransportHintV1::validate`).
- פרוטוקולים מוכרים: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- ערכי פרוטוקול כפולים לכל ספק נדחים.

### תוספות ל-`ProviderAdvertBodyV1`
- `stream_budget` אופציונלי: `Option<StreamBudgetV1>`.
- `transport_hints` אופציונלי: `Option<Vec<TransportHintV1>>`.
- שני השדות זורמים כעת דרך `ProviderAdmissionProposalV1`, מעטפות הממשל,
  ה-fixtures של ה-CLI ו-JSON טלמטרי.

## אימות וקישור לממשל

`ProviderAdvertBodyV1::validate` ו-`ProviderAdmissionProposalV1::validate`
דוחים מטא-דאטה פגומה:

- יכולות הטווח חייבות להתפענח ולעמוד במגבלות טווח/גרנולריות.
- Stream budgets / transport hints דורשים TLV תואם של
  `CapabilityType::ChunkRangeFetch` ורשימת hints לא ריקה.
- פרוטוקולי תעבורה כפולים ועדיפויות לא תקינות מעלים שגיאות אימות לפני ש-adverts
  מופצים בגוסיפ.
- מעטפות admission משוות proposal/adverts עבור מטא-דאטה של טווח דרך
  `compare_core_fields` כך ש-payloads של gossip לא תואמים יידחו מוקדם.

כיסוי הרגרסיה נמצא ב-
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## כלי עבודה ו-fixtures

- payloads של מודעות ספקים חייבים לכלול `range_capability`, `stream_budget` ו-`transport_hints`.
  אמתו דרך תגובות `/v1/sorafs/providers` ו-fixtures של admission; סיכומי JSON צריכים לכלול
  את היכולת המפורקת, תקציב הזרם ומערכי hints עבור קליטת טלמטריה.
- `cargo xtask sorafs-admission-fixtures` חושף stream budgets ו-transport hints בתוך
  artefacts ה-JSON שלו כדי ש-dashboards יעקבו אחר אימוץ היכולת.
- ה-fixtures תחת `fixtures/sorafs_manifest/provider_admission/` כוללים כעת:
  - adverts מרובי-מקורות קנוניים,
  - `multi_fetch_plan.json` כדי שחבילות SDK יוכלו לשחזר תוכנית fetch רב-עמיתים דטרמיניסטית.

## אינטגרציה עם האורקסטרטור ו-Torii

- Torii `/v1/sorafs/providers` מחזיר מטא-דאטה של יכולת טווח מפוענחת יחד עם
  `stream_budget` ו-`transport_hints`. אזהרות downgrade מופעלות כאשר ספקים משמיטים
  את המטא-דאטה החדשה, ונקודות הטווח של ה-gateway אוכפות את אותם כללים עבור לקוחות ישירים.
- האורקסטרטור רב-מקורות (`sorafs_car::multi_fetch`) אוכף כעת מגבלות טווח,
  יישור יכולות ו-stream budgets בעת הקצאת עבודה. בדיקות יחידה מכסות תרחישים של
  chunk גדול מדי, חיפוש דליל ו-throttling.
- `sorafs_car::multi_fetch` משדר אותות downgrade (כשלי יישור,
  בקשות throttled) כדי שמפעילים יוכלו לעקוב אחר הסיבה שספקים מסוימים דולגו במהלך התכנון.

## רפרנס טלמטריה

האינסטרומנטציה של fetch בטווח ב-Torii מזינה את דשבורד Grafana
**SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) ואת
כללי ההתראה המצורפים (`dashboards/alerts/sorafs_fetch_rules.yml`).

| מדד | סוג | תגיות | תיאור |
|-----|-----|-------|-------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | ספקים שמפרסמים תכונות יכולת טווח. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | ניסיונות fetch בטווח שנחסמו לפי מדיניות. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | זרמים פעילים מוגנים הצורכים את תקציב המקביליות המשותף. |

דוגמאות PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

השתמשו במונה ה-throttling כדי לאשר אכיפת מכסות לפני הפעלת ברירות המחדל של
האורקסטרטור הרב-מקורות, והגדירו התראות כאשר המקביליות מתקרבת למקסימום תקציבי הזרם בצי שלכם.
