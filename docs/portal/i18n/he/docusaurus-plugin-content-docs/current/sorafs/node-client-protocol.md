---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# פרוטוקול צומת ↔ לקוח SoraFS

המדריך הזה מסכם את ההגדרה הקנונית של הפרוטוקול תחת
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
השתמשו במפרט upstream עבור layouts של Norito ברמת הבייט וב-changelogs; עותק
הפורטל שומר על הדגשים התפעוליים לצד שאר ה-runbooks של SoraFS.

## מודעות ספק ואימות

ספקי SoraFS מפיצים payloads של `ProviderAdvertV1` (ראו
`crates/sorafs_manifest::provider_advert`) החתומים על ידי המפעיל הממשלי. המודעות
קובעות את מטא-נתוני הגילוי ואת ה-guardrails שהאורקסטרטור מרובה המקורות אוכף בזמן
ריצה.

- **תוקף** — `issued_at < expires_at ≤ issued_at + 86,400 s`. ספקים צריכים לרענן
  כל 12 שעות.
- **Capability TLVs** — רשימת TLV מפרסמת יכולות תעבורה (Torii, QUIC+Noise,
  SoraNet relays, הרחבות ספק). ניתן לדלג על קודים לא מוכרים כאשר
  `allow_unknown_capabilities = true`, בהתאם להנחיות GREASE.
- **רמזי QoS** — tier של `availability` (Hot/Warm/Cold), לטנטיות מקסימלית לשחזור,
  מגבלת קונקרנסי ותקציב stream אופציונלי. ה-QoS חייב להלום את הטלמטריה הנצפית
  ונבדק באדמישן.
- **Endpoints ו-rendezvous topics** — כתובות שירות קונקרטיות עם מטא-נתוני TLS/ALPN
  יחד עם נושאי גילוי שהלקוחות צריכים להירשם אליהם בעת בניית guard sets.
- **מדיניות גיוון מסלולים** — `min_guard_weight`, מגבלות fan-out ל-AS/מאגר, ו-
  `provider_failure_threshold` מאפשרים fetch דטרמיניסטי multi-peer.
- **מזהי פרופיל** — הספקים חייבים לחשוף את ה-handle הקנוני (לדוגמה
  `sorafs.sf1@1.0.0`); `profile_aliases` אופציונליים מסייעים ללקוחות ישנים להגר.

כללי האימות דוחים stake אפס, רשימות ריקות של capabilities/endpoints/topics,
תוקפים לא מסודרים או יעדי QoS חסרים. מעטפות האדמישן משוות בין גוף המודעה וההצעה
(`compare_core_fields`) לפני הפצת עדכונים.

### הרחבות range fetch

ספקים התומכים בטווחים כוללים את המטא-נתונים הבאים:

| שדה | מטרה |
|-----|------|
| `CapabilityType::ChunkRangeFetch` | מצהיר `max_chunk_span`, `min_granularity` ודגלי יישור/הוכחה. |
| `StreamBudgetV1` | Envelope אופציונלי של קונקרנסי/throughput (`max_in_flight`, `max_bytes_per_sec`, `burst` אופציונלי). דורש יכולת טווח. |
| `TransportHintV1` | העדפות תעבורה מסודרות (למשל `torii_http_range`, `quic_stream`, `soranet_relay`). עדיפויות `0–15` וכפילויות נדחות. |

תמיכת tooling:

- פייפלייני ספק advert חייבים לאמת יכולת טווח, stream budget ו-transport hints לפני
  פליטת payloads דטרמיניסטיים עבור audits.
- `cargo xtask sorafs-admission-fixtures` אורז מודעות multi-source קנוניות יחד עם
  downgrade fixtures תחת `fixtures/sorafs_manifest/provider_admission/`.
- מודעות טווח שמדלגות על `stream_budget` או `transport_hints` נדחות על ידי
  loaders של CLI/SDK לפני תזמון, כדי לשמור על התאמה לציפיות האדמישן של Torii.

## Endpoints לטווחים ב-gateway

Gateways מקבלים בקשות HTTP דטרמיניסטיות שמראות את מטא-נתוני המודעה.

### `GET /v2/sorafs/storage/car/{manifest_id}`

| דרישה | פרטים |
|-------|-------|
| **Headers** | `Range` (חלון יחיד מיושר להיסטים של chunks), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` אופציונלי, ו-`X-SoraFS-Stream-Token` base64 חובה. |
| **Responses** | `206` עם `Content-Type: application/vnd.ipld.car`, `Content-Range` המתאר את החלון שסופק, מטא-נתוני `X-Sora-Chunk-Range`, והדהוד של headers chunker/token. |
| **Failure modes** | `416` לטווחים לא מיושרים, `401` לטוקנים חסרים/לא תקינים, `429` כאשר חורגים מתקציבי stream/byte. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch של chunk בודד עם אותם headers בתוספת digest דטרמיניסטי של chunk. שימושי
ל-retries או הורדות forensics כשאין צורך ב-CAR slices.

## זרימת העבודה של אורקסטרטור מרובה מקורות

כאשר fetch מרובה מקורות SF-6 מופעל (Rust CLI דרך `sorafs_fetch`,
SDKs דרך `sorafs_orchestrator`):

1. **איסוף קלטים** — לפענח את תוכנית ה-chunks של manifest, למשוך את המודעות
   האחרונות, ובאופן אופציונלי להעביר telemetry snapshot (`--telemetry-json` או
   `TelemetrySnapshot`).
2. **בניית scoreboard** — `Orchestrator::build_scoreboard` מעריך זכאות ורושם
   סיבות דחייה; `sorafs_fetch --scoreboard-out` שומר את ה-JSON.
3. **תזמון chunks** — `fetch_with_scoreboard` (או `--plan`) אוכף מגבלות טווח,
   תקציבי stream, תקרות retry/peer (`--retry-budget`, `--max-peers`) ומפיק stream
   token scoped ל-manifest עבור כל בקשה.
4. **אימות קבלות** — הפלט כולל `chunk_receipts` ו-`provider_reports`; סיכומי CLI
   שומרים `provider_reports`, `chunk_receipts` ו-`ineligible_providers` עבור bundles
   של ראיות.

שגיאות נפוצות שמוחזרות למפעילים/SDKs:

| שגיאה | תיאור |
|-------|-------|
| `no providers were supplied` | אין רשומות מתאימות לאחר סינון. |
| `no compatible providers available for chunk {index}` | חוסר התאמה לטווח או לתקציב עבור chunk ספציפי. |
| `retry budget exhausted after {attempts}` | הגדילו `--retry-budget` או הסירו peers כושלים. |
| `no healthy providers remaining` | כל הספקים הושבתו לאחר כשל חוזר. |
| `streaming observer failed` | כותב CAR downstream נכשל. |
| `orchestrator invariant violated` | לכדו manifest, scoreboard, telemetry snapshot ו-CLI JSON לצורכי triage. |

## טלמטריה וראיות

- מדדים שמופקים על ידי האורקסטרטור:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (מתויגים לפי manifest/region/provider). הגדירו `telemetry_region` בקונפיג או
  באמצעות דגלי CLI כדי שהדשבורדים יחולקו לפי צי.
- סיכומי fetch של CLI/SDK כוללים scoreboard JSON שמור, chunk receipts ו-provider
  reports שחייבים להיכלל ב-rollout bundles עבור שערי SF-6/SF-7.
- Gateway handlers חושפים `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  כדי שדשבורדי SRE יוכלו לקשר בין החלטות האורקסטרטור להתנהגות השרת.

## עזרי CLI ו-REST

- `iroha app sorafs pin list|show`, `alias list` ו-`replication list` עוטפים את
  נקודות הקצה של pin-registry ומדפיסים Norito JSON גולמי עם בלוקי attestation
  לצורכי ראיות ביקורת.
- `iroha app sorafs storage pin` ו-`torii /v2/sorafs/pin/register` מקבלים manifests
  של Norito או JSON יחד עם alias proofs אופציונליים ו-successors; proofs פגומים
  מחזירים `400`, proofs ישנים מחזירים `503` עם `Warning: 110`, ו-proofs שפג
  תוקפם מחזירים `412`.
- נקודות הקצה (`/v2/sorafs/pin`, `/v2/sorafs/aliases`, `/v2/sorafs/replication`)
  כוללות מבני attestation כדי שהלקוחות יוכלו לאמת נתונים מול ה-headers של הבלוק
  האחרון לפני פעולה.

## מקורות

- מפרט קנוני:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- טיפוסי Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- עזרי CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Crate של האורקסטרטור: `crates/sorafs_orchestrator`
- חבילת dashboards: `dashboards/grafana/sorafs_fetch_observability.json`
