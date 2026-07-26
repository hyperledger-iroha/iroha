---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/da/replication-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 892382946597e8174e708618571d79a0d5db0a5352c8a22f6cbcd9f887b4f5a3
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/da/replication_policy.md`. שמרו על שתי הגרסאות
מסונכרנות עד שהמסמכים הישנים יפרשו.
:::

# מדיניות שכפול Data Availability (DA-4)

_סטטוס: בתהליך -- בעלי ענין: Core Protocol WG / Storage Team / SRE_

צינור ingest של DA אוכף כעת יעדי שימור דטרמיניסטיים לכל מחלקת blob המתוארת
ב-`roadmap.md` (זרם DA-4). Torii מסרב לשמור מעטפות שימור שסופקו על ידי ה-caller
שלא תואמות למדיניות המוגדרת, כך שכל מאמת/אחסון שומר את מספר האפוקים
והרפליקות הנדרש בלי להסתמך על כוונת השולח.

## מדיניות ברירת מחדל

| מחלקת blob | שימור hot | שימור cold | רפליקות נדרשות | מחלקת אחסון | תג ממשל |
|------------|-----------|------------|-----------------|--------------|----------|
| `taikai_segment` | 24 שעות | 14 ימים | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 שעות | 7 ימים | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 שעות | 180 ימים | 3 | `cold` | `da.governance` |
| _Default (כל שאר המחלקות)_ | 6 שעות | 30 ימים | 3 | `warm` | `da.default` |

ערכים אלה מוטמעים ב-`torii.da_ingest.replication_policy` ומוחלים על כל
השליחות `/v1/da/ingest`. Torii משכתב manifests עם פרופיל השימור האכוף ומפיק
אזהרה כאשר callers מספקים ערכים לא תואמים כדי שמפעילים יזהו SDKs מיושנים.

### מחלקות זמינות Taikai

manifests של ניתוב Taikai (`taikai.trm`) מצהירים על `availability_class`
(`hot`, `warm`, או `cold`). Torii אוכף את המדיניות המתאימה לפני chunking כך
שמפעילים יוכלו להגדיל ספירת רפליקות לכל stream בלי לערוך את הטבלה הגלובלית.
Defaults:

| מחלקת זמינות | שימור hot | שימור cold | רפליקות נדרשות | מחלקת אחסון | תג ממשל |
|--------------|-----------|------------|-----------------|--------------|----------|
| `hot` | 24 שעות | 14 ימים | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 שעות | 30 ימים | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 שעה | 180 ימים | 3 | `cold` | `da.taikai.archive` |

רמזים חסרים חוזרים ל-`hot` כדי ששידורים חיים ישמרו את המדיניות החזקה ביותר.
אפשר לעקוף את ה-defaults דרך
`torii.da_ingest.replication_policy.taikai_availability` אם הרשת שלכם משתמשת
ביעדים אחרים.

## קונפיגורציה

המדיניות נמצאת תחת `torii.da_ingest.replication_policy` ומציגה תבנית *default*
בתוספת מערך overrides לכל מחלקה. מזהי המחלקות אינם תלויי רישיות ומקבלים
`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, או `custom:<u16>`
להרחבות שאושרו בממשל. מחלקות אחסון מקבלות `hot`, `warm`, או `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

השאירו את הבלוק ללא שינוי כדי להשתמש ב-defaults לעיל. כדי להקשיח מחלקה,
עדכנו את ה-override המתאים; כדי לשנות את הבסיס למחלקות חדשות, ערכו את
`default_retention`.

מחלקות זמינות Taikai ניתנות לעקיפה באופן עצמאי דרך
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## סמנטיקת אכיפה

- Torii מחליף את `RetentionPolicy` שסופק על ידי המשתמש בפרופיל האכוף לפני
  chunking או פליטת manifest.
- manifests מוכנים מראש שמצהירים על פרופיל שימור לא תואם נדחים עם
  `400 schema mismatch` כדי שלקוחות מיושנים לא יחלישו את החוזה.
- כל אירוע override נרשם (`blob_class`, מדיניות שנשלחה מול מצופה) כדי לחשוף
  callers לא תואמים במהלך rollout.

ראו [Data Availability Ingest Plan](ingest-plan.md) (Validation checklist) עבור
שער האכיפה המעודכן שמכסה את אכיפת השימור.

## זרימת עבודה של re-replication (מעקב DA-4)

אכיפת השימור היא רק השלב הראשון. מפעילים חייבים גם להוכיח ש-manifests חיים
והוראות שכפול נשארות מיושרות עם המדיניות המוגדרת כך ש-SoraFS יוכל לבצע
re-replicate ל-blobs לא תואמים אוטומטית.

1. **נטרו drift.** Torii פולט
   `overriding DA retention policy to match configured network baseline` כאשר
   caller שולח ערכי שימור מיושנים. חברו את הלוג הזה עם טלמטריה
   `torii_sorafs_replication_*` כדי לאתר חוסרי רפליקות או פריסות מחדש מאוחרות.
2. **Diff intent מול רפליקות חיות.** השתמשו ב-helper audit החדש:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   הפקודה טוענת את `torii.da_ingest.replication_policy` מהקונפיגורציה שסופקה,
   מפענחת כל manifest (JSON או Norito), ובאופן אופציונלי מתאימה payloads של
   `ReplicationOrderV1` לפי digest של manifest. הסיכום מסמן שתי תופעות:

   - `policy_mismatch` - פרופיל השימור של manifest חורג מהמדיניות האכופה
     (זה לא אמור לקרות אלא אם Torii מוגדר לא נכון).
   - `replica_shortfall` - הוראת שכפול חיה מבקשת פחות רפליקות מאשר
     `RetentionPolicy.required_replicas` או מספקת פחות הקצאות מהיעד.

   סטטוס יציאה שאינו אפס מעיד על חוסר פעיל כך שאוטומציית CI/on-call תוכל
   לעמוד בהתרעה מיידית. צרפו את דוח ה-JSON לחבילת
   `docs/examples/da_manifest_review_template.md` להצבעות הפרלמנט.
3. **הפעילו re-replication.** כאשר הביקורת מדווחת על חוסר, הוציאו
   `ReplicationOrderV1` חדש דרך כלי הממשל המתוארים ב-
   [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md)
   והריצו את הביקורת שוב עד שהסט יתכנס. לעקיפות חירום, חברו פלט CLI עם
   `iroha app da prove-availability` כדי ש-SREs יוכלו להתייחס לאותו digest ולעדות PDP.

כיסוי רגרסיה נמצא ב-`integration_tests/tests/da/replication_policy.rs`; הסוויטה
שולחת מדיניות שימור לא תואמת ל-`/v1/da/ingest` ומוודאת שה-manifest המוחזר חושף
את הפרופיל האכוף במקום כוונת ה-caller.
