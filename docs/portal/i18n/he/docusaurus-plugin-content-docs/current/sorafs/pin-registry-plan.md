---
id: pin-registry-plan
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/pin_registry_plan.md`. יש לשמור על שתי הגרסאות מסונכרנות כל עוד התיעוד הישן פעיל.
:::

# תוכנית מימוש Pin Registry של SoraFS (SF-4)

SF-4 מספק את חוזה Pin Registry ואת שירותי התשתית התומכים המאחסנים התחייבויות manifest,
אוכפים מדיניות pin ומספקים API ל-Torii, לשערים ולמתזמרים. מסמך זה מרחיב את תוכנית
האימות במשימות מימוש קונקרטיות, כולל לוגיקה on-chain, שירותי host, fixtures
ודרישות תפעוליות.

## היקף

1. **מכונת מצבים של registry**: רשומות Norito עבור manifests, aliases, שרשראות יורשים,
   אפוקי שימור ומטא-דאטה של ממשל.
2. **מימוש החוזה**: פעולות CRUD דטרמיניסטיות למחזור חיי pin (`ReplicationOrder`,
   `Precommit`, `Completion`, eviction).
3. **חזית שירות**: נקודות קצה gRPC/REST מגובות registry ש-Torii ו-SDKs צורכים,
   כולל עימוד ואטסטציה.
4. **tooling ו-fixtures**: עוזרי CLI, וקטורי בדיקה ותיעוד לשמירה על סנכרון
   manifests, aliases ו-envelopes של ממשל.
5. **טלמטריה ותפעול**: מדדים, התראות ו-runbooks לבריאות registry.

## מודל נתונים

### רשומות ליבה (Norito)

| Struct | תיאור | שדות |
|--------|-------|------|
| `PinRecordV1` | רשומת manifest קנונית. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | מיפוי alias -> CID של manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | הוראה ל-providers להצמיד manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | אישור ספק. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | צילום מצב של מדיניות ממשל. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

הפניית מימוש: ראו `crates/sorafs_manifest/src/pin_registry.rs` עבור סכמות Norito ב-Rust
ועזרי אימות התומכים ברשומות אלו. האימות משקף את tooling של manifest
(lookup של chunker registry, pin policy gating) כך שהחוזה, חזיתות Torii ו-CLI
חולקים אותן אינווריאנטים.

משימות:
- להשלים את סכמות Norito ב-`crates/sorafs_manifest/src/pin_registry.rs`.
- לייצר קוד (Rust + SDKs נוספים) באמצעות מאקרו Norito.
- לעדכן את התיעוד (`sorafs_architecture_rfc.md`) לאחר שהסכמות נקבעות.

## מימוש החוזה

| משימה | בעלים | הערות |
|-------|-------|-------|
| לממש אחסון registry (sled/sqlite/off-chain) או מודול smart contract. | Core Infra / Smart Contract Team | לספק hashing דטרמיניסטי ולהימנע ממספרים צפים. |
| נקודות כניסה: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | להשתמש ב-`ManifestValidator` מתוכנית האימות. קישור alias זורם כעת דרך `RegisterPinManifest` (DTO של Torii), בעוד `bind_alias` ייעודי נשאר מתוכנן לעדכונים עתידיים. |
| מעברי מצב: לאכוף ירושה (manifest A -> B), אפוקי שימור וייחודיות alias. | Governance Council / Core Infra | ייחודיות alias, מגבלות שימור ובדיקות אישור/פרישה של קודמים נמצאים ב-`crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; זיהוי ירושה רב-שלבית וניהול שכפול עדיין פתוחים. |
| פרמטרים מנוהלים: לטעון `ManifestPolicyV1` מ-config/מצב ממשל; לאפשר עדכונים דרך אירועי ממשל. | Governance Council | לספק CLI לעדכוני מדיניות. |
| פליטת אירועים: להפיק אירועי Norito לטלמטריה (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observability | להגדיר סכמת אירועים + logging. |

בדיקות:
- בדיקות יחידה לכל נקודת כניסה (חיובי + דחייה).
- בדיקות תכונות לשרשרת הירושה (ללא מחזורים, אפוקים מונוטוניים).
- Fuzz לאימות על ידי יצירת manifests אקראיים (מוגבלים).

## חזית שירות (אינטגרציית Torii/SDK)

| רכיב | משימה | בעלים |
|------|-------|-------|
| שירות Torii | לחשוף `/v2/sorafs/pin` (submit), `/v2/sorafs/pin/{cid}` (lookup), `/v2/sorafs/aliases` (list/bind), `/v2/sorafs/replication` (orders/receipts). לספק עימוד + סינון. | Networking TL / Core Infra |
| אטסטציה | לכלול גובה/האש של registry בתשובות; להוסיף מבנה אטסטציה Norito לצריכת SDKs. | Core Infra |
| CLI | להרחיב `sorafs_manifest_stub` או CLI חדש `sorafs_pin` עם `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | ליצור bindings לקוח (Rust/Go/TS) מסכמת Norito; להוסיף בדיקות אינטגרציה. | SDK Teams |

תפעול:
- להוסיף שכבת cache/ETag לנקודות קצה GET.
- לספק rate limiting / auth בהתאם למדיניות Torii.

## Fixtures ו-CI

- תיקיית fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` מאחסנת snapshots חתומים של manifest/alias/order שנוצרים מחדש על ידי `cargo run -p iroha_core --example gen_pin_snapshot`.
- שלב CI: `ci/check_sorafs_fixtures.sh` מייצר מחדש את ה-snapshot ונכשל אם יש diffs, כדי לשמור על תאימות fixtures של CI.
- בדיקות אינטגרציה (`crates/iroha_core/tests/pin_registry.rs`) מכסות את המסלול התקין וכן דחיית alias כפול, guards לאישור/שימור alias, handles של chunker שאינם תואמים, אימות ספירת רפליקות וכשלי guard של ירושה (מצביעים לא ידועים/מאושרים מראש/הוצאו/הפניה עצמית); ראו מקרי `register_manifest_rejects_*` לפרטי כיסוי.
- בדיקות יחידה מכסות כעת אימות alias, guards לשימור ובדיקות יורש ב-`crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; זיהוי ירושה רב-שלבית יגיע עם מכונת המצבים.
- JSON זהב לאירועים המשמשים בצנרות observability.

## טלמטריה ותצפיתיות

מדדים (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- טלמטריית providers קיימת (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) נשארת בתחום עבור dashboards end-to-end.

לוגים:
- זרם אירועים Norito מובנה לביקורות ממשל (חתום?).

התראות:
- הזמנות שכפול ממתינות שחורגות מה-SLA.
- תפוגת alias מתחת לסף.
- הפרות שימור (manifest לא חודש לפני תפוגה).

Dashboards:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` עוקב אחרי סך מחזור החיים של manifests, כיסוי alias, רוויה של backlog, יחס SLA, חפיפות latency מול slack ושיעורי הזמנות שהוחמצו לסקירת on-call.

## Runbooks ותיעוד

- לעדכן את `docs/source/sorafs/migration_ledger.md` כדי לכלול עדכוני סטטוס של registry.
- מדריך מפעילים: `docs/source/sorafs/runbooks/pin_registry_ops.md` (כבר פורסם) המכסה מדדים, התראות, פריסה, גיבוי ושחזור.
- מדריך ממשל: לתאר פרמטרי מדיניות, תהליך אישור, טיפול במחלוקות.
- דפי עזר API לכל נקודת קצה (Docusaurus docs).

## תלות ורצף

1. להשלים משימות תוכנית האימות (שילוב ManifestValidator).
2. לסיים סכמת Norito + ברירות מחדל של מדיניות.
3. לממש חוזה + שירות ולחבר טלמטריה.
4. לייצר מחדש fixtures ולהריץ חבילות אינטגרציה.
5. לעדכן docs/runbooks ולסמן פריטי roadmap כהושלמו.

כל פריט צ'ק-ליסט תחת SF-4 חייב להפנות לתוכנית זו בעת התקדמות.
חזית ה-REST מספקת כעת נקודות קצה של רשימה עם אטסטציה:

- `GET /v2/sorafs/pin` ו-`GET /v2/sorafs/pin/{digest}` מחזירות manifests עם
  alias bindings, הזמנות שכפול ואובייקט אטסטציה שמופק מה-hash של הבלוק האחרון.
- `GET /v2/sorafs/aliases` ו-`GET /v2/sorafs/replication` מציגות קטלוג alias פעיל
  ו-backlog של הזמנות שכפול עם עימוד עקבי וסינוני סטטוס.

ה-CLI עוטף קריאות אלו (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) כדי שמפעילים יוכלו לאוטומט בדיקות registry ללא נגיעה
ב-API ברמת נמוכה.
