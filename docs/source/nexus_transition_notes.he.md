<!-- Hebrew translation of docs/source/nexus_transition_notes.md -->

---
lang: he
direction: rtl
source: docs/source/nexus_transition_notes.md
status: complete
translator: manual
---

<div dir="rtl">

# הערות מעבר Nexus — פרוטוטייפ הפשטת Lane/Data-Space

המסמך עוקב אחר החיווט המדורג שמרים את צינור Iroha 2 החד-נתיבי לעבר ארכיטקטורת Nexus המתוארת ב-`nexus.md`. המיילסטון הנוכחי מוסיף טיפוסי מזהים פנימיים וקרסי טלמטריה המאפשרים ל-API להתחיל להעביר הקשר של Lane/Data-Space ללא שינוי התנהגותי בינתיים. אותו בסיס קוד מייצר גם את Iroha 2 וגם את SORA Nexus, ולכן השכבות הללו נועדו לשמר תאימות חד-נתיבית תוך הקמת התשתית למסלולים מרובים.

## מזהי מציין מקום

- `LaneId` ו-`DataSpaceId` נמצאים ב-`iroha_data_model::nexus`. אלו newtype-ים דקים התואמים Norito, שערך ברירת המחדל שלהם הוא `LaneId::SINGLE` ו-`DataSpaceId::GLOBAL`. בעתיד הם ישאו החלטות ניתוב דטרמיניסטיות; כרגע הם מתעדים את צורת ה-API העתידית.
- הטיפוסים מקודדים ומפוענחים באמצעות Norito ומיוצאים דרך ה-prelude של data-model, כך שקרייטים תלויים יכולים להשתמש בהם ללא flags ניסיוניים.

## קרסי טלמטריה ולוג

- שיטות `StateTelemetry` ב-`iroha_core` מקבלות כעת `LaneId` (והעוגן `DataSpaceId::GLOBAL`). המטריקות חושפות שני gauges — ‏`nexus_lane_id_placeholder` ו-`nexus_dataspace_id_placeholder` — לשמירת תאימות הדשבורד בזמן שהמערכת עדיין פועלת בנתיב יחיד.
- אירועי טלמטריה מובנים דרך `iroha_logger` כוללים אוטומטית שדות `lane_id` ו-`dataspace_id`. אם המתקשר אינו מספק ערכים, הלוגר משלים את ברירות המחדל.

## צעדים הבאים

- להחליף את הקריאות הקשיחות `LaneId::SINGLE`/`DataSpaceId::GLOBAL` ברגע שמתזמן Nexus (SFQ, לוגיקת Fusion, מדיניות Data-Space) יוטמע.
- להרחיב משטחים (קבלה, Gossip, Torii) כך שיקבלו את המזהים החדשים ויעבירו אותם בין צמתים.
- לאחר שמסלולים מרובים יופעלו, להחליף את תוויות הטלמטריה/מטריקות מהמזהים ה placeholders לערכים האמיתיים.
- לעדכן את נהלי התפעול כך שיכללו `iroha_cli app nexus lane-report --summary --only-missing --fail-on-sealed`, ולהצליב עם `lane_governance_sealed_total` / `lane_governance_sealed_aliases` שמופיעים ב-`/v2/sumeragi/status` כדי לעצור פריסות כאשר מניפסט ממשל עדיין חסר.

## תכנית מעבר לנתיבים/מרחבי נתונים

הפריט “Lane/Data-Space Routing” במפת הדרכים הוגדר. התכנית הבאה מתארת את הדרך ממצייני המקום הנוכחיים לניתוב מלא של Nexus תוך שמירה על יציבות פריסות קיימות.

### פערים נוכחיים

- Torii (`crates/iroha_torii`) מעביר הקשר Lane רק דרך לוגי טלמטריה; HTTP/WS ו-SDK אינם נושאים רמזים של Lane/Data-Space.
- דשבורדים מסתמכים על gauges placeholders, והלוגר משלים ברירות מחדל, כך שהנראות מוגבלת ברגע שמופיעים נתיבים מרובים.

### שלבי הגירה

1. **קטלוג נתיבים וחיווט קונפיג (יעד: Sprint O5.1)** — בעלים: `@nexus-core`.
   - הרחבת `iroha_config` עם `lane_count`, ‏`dataspace_catalog`, `routing_policy`, והזרמת הערכים לתהליך האתחול של `irohad`.
   - הוספת בנאים בטוחים (`LaneId::from_lane_index`, `DataSpaceId::from_hash`) ל-`iroha_data_model::nexus`.
   - עדכון כותרות Norito ומסמכי סכימה עבור קובצי קונפיג.
2. **משטח ניתוב דטרמיניסטי (יעד: Sprint O5.2)** — בעלים: `@nexus-core`, ביקורת: `@iroha-research`.
   - הוספת trait `LaneRouter` ב-`queue.rs` שנקרא מ-`Queue::push`; מימוש התחלתי ממפה את התעבורה לנתיב ברירת המחדל תוך החלת מדיניות.
   - אחסון Lane/Data-Space ב-`AcceptedTransaction` והרחבת `block.rs` כך שה-DAG יכבד את המידע.
   - עדכון אירועי gossip/קבלה כך שיישאו מזהי Lane.
3. **הפצה ברשת/API (יעד: Sprint O5.3)** — בעלים: `@torii-sdk`.
   - הרחבת נקודות הקצה, Streams, CLI/SDK לקבל `lane_id`/`dataspace_id` אופציונליים ולהעבירם דרך מודלי הנתונים.
   - עדכון `iroha_client` ו-`pytests` ובדיקות אינטגרציה למסלולים מעורבים.
4. **תצפיתיות ומעקות (יעד: Sprint O5.4)** — בעלים: `@telemetry-ops`.
   - החלפת gauges placeholders במטריקות פר נתיב (`pipeline_tx_total{lane_id="…"}` וכו').
   - אילוץ אספקת מזהים מפורשים בלוגר (+ WARN כאשר משלימים ברירת מחדל).
   - סיפוק דשבורדים ו-alerts תואמים.

## עקבות ביקורת Nexus

| Trace ID | מטרה | ארטיפקטים | בעלים | סוקרים | חלון יעד | קריטריוני קבלה |
| --- | --- | --- | --- | --- | --- | --- |
| `TRACE-PIPELINE-LANE` | בריאות הצינור תחת Multi-lane | פלט `pytests`, snapshots, בדיקת אינטגרציה | `@nexus-core` | `@telemetry-ops`, `@sec-ops` | 3–14 בפברואר | כל הנתיבים קולטים שווה, SLA עומד ללא דילוגי סלוט |
| `TRACE-DA-SAMPLING` | DA sampling + טיפול RBC | תבנית CI, דגימות Prometheus, alerts | `@nexus-core`, `@qa-consensus` | `@telemetry-ops` | 24–28 בפברואר | DA key rotation, READY gossip מאוחר → טלמטריה צפויה; אין commit ללא הוכחת DA |
| `TRACE-TELEMETRY-BRIDGE` | ייצוא מטריקות + לוגים | Scrape `/metrics`, דוגמאות OTLP, snapshots | `@telemetry-ops` | `@nexus-core`, `@sec-observability` | 3–14 במרץ | כל המטריקות עם דגימות, WARN fallback ≤1% |
| `TRACE-CONFIG-DELTA` | דלתא קונפיג Nexus לעומת יחיד | קבצים `defaults/nexus`, פלט CLI | `@governance`, `@nexus-core` | `@release-eng`, `@telemetry-ops` | עם חיתוך שחרור | Checklist חתום ללא knobs stray |

### טבלת פערי טלמטריה

| Gap | אות חסר | בעלים | גרסה | הערות |
| --- | --- | --- | --- | --- |
| `GAP-TELEM-001` | histogram `torii_lane_admission_latency_seconds` | `@torii-sdk` | 2026.2 | מיושם חלקית (Dec 4 2025), נותר להשלים |
| `GAP-TELEM-002` | counter `nexus_config_diff_total` | `@nexus-core` | 2026.1 | הוטמע באמצעות `StateTelemetry::record_nexus_config_diff`; המסמך `telemetry.md` כולל כעת דוגמת התראה והנחיות לבדיקה בלוגים |
| `GAP-TELEM-003` | event `TelemetryEvent::AuditOutcome` | `@telemetry-ops` | 2026.1 | `Telemetry::record_audit_outcome` מפיק אירועי `nexus.audit.outcome`; כלל ההתראה `dashboards/alerts/nexus_audit_rules.yml` מזהה סטטוסים כושלים והסקריפט `scripts/telemetry/check_nexus_audit_outcome.py` מאכף הופעת אירוע תוך 30 דקות ושומר ארטיפקטים.【crates/iroha_core/src/telemetry.rs:3056】【docs/source/telemetry.he.md:247】 |
| `GAP-TELEM-004` | gauge `nexus_lane_configured_total` | `@telemetry-ops` | 2026.1 | הושלם: `StateTelemetry::set_nexus_catalogs` מעדכן את המד, Prometheus מפרסם את `nexus_lane_configured_total`, ומדריך הטלמטריה כולל כעת כלל התראה. |

כל הפערים נסגרים לפני ביקורת Q1 השנייה.

### שומר דלתא קונפיג

| משטח | דלתא מצופה | ביקורת | בעלים | ארטיפקט |
| --- | --- | --- | --- | --- |
| `lane_count` | >1 בחבילת Nexus, 1 במונוליין | השוואת קבצי defaults | `@nexus-core` | רשומה ב-`nexus_config_deltas` |
| `routing_policy` | טבלת מדיניות פר נתיב | אימות serialization | `@governance` | hash + חתימה |
| `da.*` | תואם לתקנים במסמך | בדיקת “In-slot DA sampling” | `@telemetry-ops` | diff + CI URL |
| manifest | כולל lane, DA, bitים | הרצת `kagami genesis bootstrap --profile nexus` | `@release-eng` | hash ב-checklist |

## פנקס פרמטרי ממשל

Tables מ-`nexus.md` ממופים ל-`iroha_config`. דוגמאות:

| פרמטר | ערך ברירת מחדל | מיקום | הערה |
| --- | --- | --- | --- |
| `fuse_batch_max_len` | 64 | `nexus.lane.fusion` | לחיווט ב-LaneRouter |
| `da_required_quorum` | 0.67 | `nexus.da` | מיושם |
| `audit_grace_slots` | 2 | `nexus.audit` | נבדק בטרייסים |

---

המסמך יעדכן ככל שהמעבר מתקדם. יש לשמר סינכרון עם `status.md` ו-`project_tracker`.

</div>
