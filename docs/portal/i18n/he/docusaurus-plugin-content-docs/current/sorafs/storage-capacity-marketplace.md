---
id: storage-capacity-marketplace
lang: he
direction: rtl
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/storage_capacity_marketplace.md`. שמרו על התאמה בין שני המיקומים כל עוד התיעוד הישן פעיל.
:::

# מרקטפלייס קיבולת אחסון SoraFS (טיוטת SF-2c)

פריט ה-roadmap SF-2c מציג מרקטפלייס מנוהל שבו providers של אחסון מצהירים על קיבולת מחויבת, מקבלים replication orders ומרוויחים fees ביחס לזמינות שסופקה. מסמך זה מגדיר את ה-deliverables הנדרשים לריליס הראשון ומפרק אותם למסלולים ישימים.

## יעדים

- להציג התחייבויות קיבולת של providers (סך בתים, מגבלות לכל lane, תפוגה) בצורה ניתנת לאימות שיכולה לשמש את governance, תעבורת SoraNet ו-Torii.
- להקצות pins בין providers בהתאם לקיבולת המוצהרת, stake ומגבלות policy תוך שמירה על התנהגות דטרמיניסטית.
- למדוד אספקת אחסון (הצלחת replication, uptime, integrity proofs) ולהוציא טלמטריה לחלוקת fees.
- לספק תהליכי revocation ו-dispute כדי שספקים לא הוגנים ייענשו או יוסרו.

## מושגי תחום

| מושג | תיאור | Deliverable ראשוני |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito payload שמתאר מזהה provider, תמיכת פרופיל chunker, GiB מחויבים, מגבלות לכל lane, pricing hints, התחייבות staking ותוקף. | סכמה + ולידטור ב-`sorafs_manifest::capacity`. |
| `ReplicationOrder` | הוראה שמונפקת על ידי governance שמקצה CID של manifest ל-provider אחד או יותר, כולל רמת יתירות ומדדי SLA. | סכמה Norito משותפת עם Torii + API של smart contract. |
| `CapacityLedger` | רישום on-chain/off-chain שעוקב אחר הצהרות קיבולת פעילות, replication orders, מדדי ביצועים והצטברות fees. | מודול smart contract או stub של שירות off-chain עם snapshot דטרמיניסטי. |
| `MarketplacePolicy` | מדיניות governance שמגדירה stake מינימלי, דרישות ביקורת ועקומות ענישה. | config struct ב-`sorafs_manifest` + מסמך ממשל. |

### סכמות ממומשות (סטטוס)

## פירוט עבודה

### 1. שכבת סכמות ורישום

| משימה | Owner(s) | הערות |
|------|----------|-------|
| להגדיר `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Storage Team / Governance | להשתמש ב-Norito; לכלול versioning סמנטי והפניות capabilities. |
| לממש מודולי parser + validator ב-`sorafs_manifest`. | Storage Team | לאכוף IDs מונוטוניים, גבולות קיבולת ודרישות stake. |
| להרחיב את metadata של chunker registry עם `min_capacity_gib` לכל פרופיל. | Tooling WG | מסייע ללקוחות לאכוף דרישות חומרה מינימליות לכל פרופיל. |
| לנסח מסמך `MarketplacePolicy` שמתעד guardrails של admission ולוח זמנים של ענישה. | Governance Council | לפרסם ב-docs לצד policy defaults. |

#### הגדרות סכמה (ממומש)

- `CapacityDeclarationV1` לוכד התחייבויות קיבולת חתומות לכל provider, כולל handles קנוניים של chunker, הפניות capabilities, caps אופציונליים לכל lane, pricing hints, חלונות תוקף ו-metadata. הוולידציה מבטיחה stake שאינו אפס, handles קנוניים, aliases ללא כפילויות, caps לליין בתוך הסכום המוצהר ומעקב GiB מונוטוני.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` קושר manifests להקצאות שהונפקו על ידי governance עם יעדי יתירות, ספי SLA והבטחות לכל assignment; הוולידטורים אוכפים handles קנוניים של chunker, providers ייחודיים ומגבלות deadline לפני ש-Torii או הרישום בולעים את ה-order.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` מבטא snapshots של epochs (GiB מוצהרים מול מנוצלים, מוני replication, אחוזי uptime/PoR) שמזינים חלוקת fees. בדיקות גבול שומרות את הניצול בתוך ההצהרות ואת האחוזים בטווח 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- helpers משותפים (`CapacityMetadataEntry`, `PricingScheduleV1`, ולידטורי lane/assignment/SLA) מספקים ולידציה דטרמיניסטית למפתחות ודיווחי שגיאות שניתנים לשימוש חוזר ב-CI וב-downstream tooling.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` מציג כעת snapshot on-chain דרך `/v1/sorafs/capacity/state`, תוך שילוב הצהרות providers ורשומות fee ledger מאחורי Norito JSON דטרמיניסטי.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- כיסוי הולידציה בודק אכיפת handles קנוניים, זיהוי כפילויות, גבולות לכל lane, guards של הקצאות replication ובדיקות טווח לטלמטריה כדי שרגרסיות יבלטו מיד ב-CI.【crates/sorafs_manifest/src/capacity.rs:792】
- tooling למפעילים: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` ממיר specs קריאים לאדם ל-Norito payloads קנוניים, base64 blobs וסיכומי JSON כך שמפעילים יוכלו להכין fixtures עבור `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` ו-replication order fixtures עם ולידציה מקומית.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Reference fixtures נמצאים ב-`fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) ונוצרים באמצעות `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. אינטגרציית control plane

| משימה | Owner(s) | הערות |
|------|----------|-------|
| להוסיף handlers של Torii עבור `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` עם Norito JSON payloads. | Torii Team | לשקף את לוגיקת הוולידציה; להחזיר שימוש ב-Norito JSON helpers. |
| להעביר snapshots של `CapacityDeclarationV1` אל metadata של orchestrator scoreboard ולתוכניות fetch של gateway. | Tooling WG / Orchestrator team | להרחיב `provider_metadata` בהפניות קיבולת כדי ש-scoring רב-מקורי יכבד מגבלות lane. |
| להזין replication orders אל clients של orchestrator/gateway כדי להנחות assignments ו-failover hints. | Networking TL / Gateway team | Scoreboard builder צורך replication orders חתומים על ידי governance. |
| tooling של CLI: להרחיב את `sorafs_cli` עם `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | לספק JSON דטרמיניסטי + outputs של scoreboard. |

### 3. מדיניות מרקטפלייס וממשל

| משימה | Owner(s) | הערות |
|------|----------|-------|
| לאשר את `MarketplacePolicy` (stake מינימלי, מכפילי ענישה, תדירות ביקורת). | Governance Council | לפרסם ב-docs ולתעד היסטוריית תיקונים. |
| להוסיף governance hooks כדי ש-Parliament יאשר, יחדש ויבטל declarations. | Governance Council / Smart Contract team | להשתמש ב-Norito events + manifest ingestion. |
| לממש לוח זמנים של ענישה (הפחתת fees, slashing של bond) הקשור להפרות SLA שנמדדות בטלמטריה. | Governance Council / Treasury | ליישר קו עם outputs של settlement ב-`DealEngine`. |
| לתעד תהליך dispute ומטריצת הסלמה. | Docs / Governance | לקשר ל-dispute runbook + CLI helpers. |

### 4. Metering וחלוקת fees

| משימה | Owner(s) | הערות |
|------|----------|-------|
| להרחיב ingest של metering ב-Torii כדי לקבל `CapacityTelemetryV1`. | Torii Team | לאמת GiB-hour, הצלחת PoR, uptime. |
| לעדכן את צינור ה-metering של `sorafs_node` כדי לדווח ניצול לפי order + סטטיסטיקות SLA. | Storage Team | ליישר קו עם replication orders ו-chunker handles. |
| pipeline של settlement: להמיר טלמטריה + נתוני replication ל-payouts נקובי XOR, להפיק סיכומים מוכנים לממשל, ולרשום מצב ledger. | Treasury / Storage Team | לחבר ל-Deal Engine / Treasury exports. |
| לייצא dashboards/alerts לבריאות metering (ingestion backlog, טלמטריה ישנה). | Observability | להרחיב את חבילת Grafana שמוזכרת ב-SF-6/SF-7. |

- Torii חושף כעת `/v1/sorafs/capacity/telemetry` ו-`/v1/sorafs/capacity/state` (JSON + Norito) כדי שמפעילים יוכלו להגיש telemetry snapshots של epochs ומבקרים יוכלו למשוך ledger קנוני לצורך ביקורת או אריזת ראיות.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- אינטגרציית `PinProviderRegistry` מבטיחה ש-replication orders נגישים דרך אותו endpoint; helpers של CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) מאמתים ומפרסמים טלמטריה מהרצות אוטומציה עם hashing דטרמיניסטי ופתרון aliases.
- metering snapshots מייצרים רשומות `CapacityTelemetrySnapshot` הצמודות ל-snapshot `metering`, ו-Prometheus exports מזינים את לוח Grafana המוכן לייבוא ב-`docs/source/grafana_sorafs_metering.json` כדי שצוותי בילינג יעקבו אחר צבירת GiB-hour, fees nano-SORA צפויים ועמידה ב-SLA בזמן אמת.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- כאשר metering smoothing פעיל, ה-snapshot כולל `smoothed_gib_hours` ו-`smoothed_por_success_bps` כדי לאפשר השוואת ערכי EMA מול הספירות הגולמיות שהממשל משתמש בהן ל-payouts.【crates/sorafs_node/src/metering.rs:401】

### 5. טיפול ב-dispute וב-revocation

| משימה | Owner(s) | הערות |
|------|----------|-------|
| להגדיר payload של `CapacityDisputeV1` (מתלונן, evidence, provider יעד). | Governance Council | סכמה Norito + ולידטור. |
| תמיכת CLI להגשת disputes ולמענה (עם attachments של evidence). | Tooling WG | להבטיח hashing דטרמיניסטי של bundle evidence. |
| להוסיף בדיקות אוטומטיות להפרות SLA חוזרות (auto-escalate ל-dispute). | Observability | ספי alert ו-governance hooks. |
| לתעד playbook של revocation (תקופת חסד, פינוי נתוני pinned). | Docs / Storage Team | לקשר ל-policy doc ול-operator runbook. |

## דרישות בדיקות ו-CI

- בדיקות יחידה לכל הוולידטורים החדשים של הסכמה (`sorafs_manifest`).
- בדיקות אינטגרציה המדמות: declaration → replication order → metering → payout.
- workflow ב-CI שמחדש הצהרות/טלמטריה לדוגמה ומוודא סנכרון חתימות (להרחיב `ci/check_sorafs_fixtures.sh`).
- בדיקות עומס ל-registry API (לדמות 10k providers ו-100k orders).

## טלמטריה ודשבורדים

- פאנלי dashboard:
  - קיבולת מוצהרת מול מנוצלת לכל provider.
  - backlog של replication orders וזמן עיכוב ממוצע בהקצאה.
  - עמידה ב-SLA (uptime %, שיעור הצלחת PoR).
  - צבירת fees וקנסות לכל epoch.
- Alerts:
  - provider מתחת לקיבולת המינימלית המוצהרת.
  - replication order תקוע מעבר ל-SLA.
  - כשלים ב-metering pipeline.

## Deliverables תיעודיים

- מדריך מפעיל להצהרת קיבולת, חידוש התחייבויות ומעקב ניצול.
- מדריך governance לאישור הצהרות, הנפקת orders וטיפול ב-disputes.
- API reference ל-endpoints של הקיבולת ולפורמט replication order.
- Marketplace FAQ למפתחים.

## צ'קליסט מוכנות ל-GA

פריט roadmap **SF-2c** מגביל פריסה לפרודקשן לפי ראיות קונקרטיות בתחום החשבונאות, טיפול ב-dispute והאונבורדינג. השתמשו בארטיפקטים להלן כדי לשמור על קריטריוני הקבלה מסונכרנים עם המימוש.

### Nightly accounting & XOR reconciliation
- יצאו snapshot של מצב הקיבולת ו-XOR ledger export לאותו חלון, ואז הריצו:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  ההלפר מסיים בקוד שונה מאפס כאשר קיימות settlement או קנסות חסרים/עודפים ומפיק סיכום Prometheus textfile.
- ה-alert `SoraFSCapacityReconciliationMismatch` (ב-`dashboards/alerts/sorafs_capacity_rules.yml`)
  מופעל כאשר מדדי reconciliation מדווחים על פערים; הדשבורדים נמצאים תחת
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- ארכבו את ה-JSON summary וה-hashes תחת `docs/examples/sorafs_capacity_marketplace_validation/`
  לצד governance packets.

### Dispute & slashing evidence
- הגישו disputes דרך `sorafs_manifest_stub capacity dispute` (tests:
  `cargo test -p sorafs_car --test capacity_cli`) כדי שה-payloads יישארו קנוניים.
- הריצו `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` ומבחני הענישה
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) כדי להוכיח ש-disputes ו-slashes משוחזרים דטרמיניסטית.
- פעלו לפי `docs/source/sorafs/dispute_revocation_runbook.md` ללכידת ראיות ולהסלמה;
  קישרו אישורי strike חזרה לדוח ה-validation.

### Provider onboarding & exit smoke tests
- צרו מחדש artefacts של declaration/telemetry עם `sorafs_manifest_stub capacity ...` והריצו CLI tests לפני ההגשה (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- הגישו דרך Torii (`/v1/sorafs/capacity/declare`) ואז תעדו `/v1/sorafs/capacity/state` לצד צילומי Grafana. עקבו אחרי ה-exit flow ב-`docs/source/sorafs/capacity_onboarding_runbook.md`.
- ארכבו artefacts חתומים ו-reconciliation outputs בתוך `docs/examples/sorafs_capacity_marketplace_validation/`.

## תלותים ותזמון

1. לסיים SF-2b (admission policy) - ה-marketplace נשען על providers שעברו בדיקה.
2. לממש את שכבת הסכמה + registry (מסמך זה) לפני אינטגרציית Torii.
3. להשלים metering pipeline לפני הפעלת payouts.
4. שלב סופי: להפעיל חלוקת fees בשליטת governance לאחר אימות metering data ב-staging.

יש לעקוב אחרי ההתקדמות ב-roadmap עם הפניות למסמך זה. עדכנו את ה-roadmap כאשר כל חלק מרכזי (סכמות, control plane, אינטגרציה, metering, טיפול ב-disputes) מגיע ל-feature complete.
