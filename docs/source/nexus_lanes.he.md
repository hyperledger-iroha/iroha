---
lang: he
direction: rtl
source: docs/source/nexus_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94891050512eaf78f4c0381c0facbeed445a7e7323297070ae537e4d38ca7fe4
source_last_modified: "2025-12-13T05:07:11.953030+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי עבור docs/source/nexus_lanes.md -->

# מודל Lane של Nexus וחלוקת WSV

> **סטטוס:** מסירה NX-1 — טקסונומיית ה-Lane, גאומטריית הקונפיגורציה ופריסת האחסון מוכנות ליישום.  
> **בעלים:** Nexus Core WG, Governance WG  
> **פריט רודמפ קשור:** NX-1

מסמך זה מתאר את ארכיטקטורת היעד של שכבת הקונצנזוס מרובת הנתיבים של Nexus. המטרה היא להפיק מצב עולם דטרמיניסטי יחיד תוך מתן אפשרות למרחבי נתונים (lanes) להריץ סטי מאמתים ציבוריים או פרטיים עם עומסי עבודה מבודדים.

> **הוכחות Cross-lane:** הערה זו מתמקדת בגאומטריה ובאחסון. מחויבויות ה-settlement לכל lane, צינור ה-relay והוכחות merge-ledger הנדרשות עבור roadmap **NX-4** מפורטים ב-[nexus_cross_lane.md](nexus_cross_lane.md).

## מושגים

- **Lane:** שבר לוגי של ספר החשבונות Nexus עם סט מאמתים משלו ותור ביצוע. מזוהה באמצעות `LaneId` יציב.
- **Data Space:** סל ממשל שמאגד lane אחד או יותר החולקים מדיניות תאימות, ניתוב ו-settlement. כל dataspace גם מצהיר על `fault_tolerance (f)` המשמש לקביעת גודל ועדות relay של lane (`3f+1`).
- **Lane Manifest:** מטא-דאטה בשליטת ממשל שמתארת מאמתים, מדיניות DA, טוקן גז, כללי settlement והרשאות ניתוב.
- **Global Commitment:** הוכחה שמופקת על ידי lane ומסכמת שורשי מצב חדשים, נתוני settlement והעברות cross-lane אופציונליות. טבעת ה-NPoS הגלובלית מסדרת את ה-commitments.

## טקסונומיית Lane

סוגי Lane מתארים באופן קנוני את הנראות, משטח הממשל וקרסי ה-settlement שלהם. גאומטריית ההגדרה (`LaneConfig`) לוכדת את המאפיינים האלה כך שצמתים, SDKs וכלי תפעול יוכלו להבין את הפריסה ללא לוגיקה ייעודית.

| סוג Lane | נראות | הרכב מאמתים | חשיפת WSV | ממשל ברירת מחדל | מדיניות settlement | שימוש טיפוסי |
|-----------|--------|--------------|-----------|------------------|-------------------|--------------|
| `default_public` | ציבורי | Permissionless (סטייק גלובלי) | שכפול מצב מלא | SORA Parliament | `xor_global` | לגר ציבורי בסיסי |
| `public_custom` | ציבורי | Permissionless או מוגבל ב-stake | שכפול מצב מלא | מודול משוקלל לפי stake | `xor_lane_weighted` | יישומים ציבוריים עתירי תפוקה |
| `private_permissioned` | מוגבל | סט מאמתים קבוע (מאושר ממשל) | commitments והוכחות | מועצה פדרטיבית | `xor_hosted_custody` | CBDC, עומסי קונסורציום |
| `hybrid_confidential` | מוגבל | חברות מעורבת; עוטף הוכחות ZK | commitments + חשיפה סלקטיבית | מודול כסף מתוכנת | `xor_dual_fund` | כסף מתוכנת שומר פרטיות |

כל סוגי ה-Lane חייבים להצהיר על:

- כינוי dataspace — קיבוץ קריא-לאדם שמחבר מדיניות תאימות.
- מזהה ממשל — מזהה שנפתר דרך `Nexus.governance.modules`.
- מזהה settlement — מזהה שנצרך על ידי נתב ה-settlement כדי לחייב באפרי XOR.
- מטא-דאטה טלמטריה אופציונלית (תיאור, איש קשר, תחום עסקי) שמוצגת דרך `/status` ודשבורדים.

## גאומטריית קונפיגורציית Lane (`LaneConfig`)

`LaneConfig` היא גאומטריית הריצה שנגזרת מקטלוג ה-Lane המאומת. היא אינה מחליפה מניפסטים של ממשל; במקום זאת היא מספקת מזהי אחסון דטרמיניסטיים ורמזי טלמטריה לכל lane מוגדר.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` מחשב מחדש את הגאומטריה בכל פעם שהקונפיגורציה נטענת (`State::set_nexus`).
- כינויים מנוקים לסלאג באותיות קטנות; רצפים של תווים שאינם אלפא-נומריים מתכווצים ל-`_`. אם הכינוי מניב סלאג ריק אנו נופלים ל-`lane{id}`.
- תחיליות מפתחות מבטיחות שה-WSV ישמור טווחי מפתח פר-lane מופרדים גם כאשר אותו backend משותף.
- `shard_id` נגזר ממפתח המטא-דאטה בקטלוג `da_shard_id` (ברירת מחדל `lane_id`) ומניע את יומן מצביע השארד המתמיד כדי לשמור על DA replay דטרמיניסטי בין אתחולים מחדש/resharding.
- שמות סגמנטי Kura דטרמיניסטיים בין hosts; מבקרים יכולים להצליב ספריות סגמנט ומניפסטים ללא כלי ייעודי.
- סגמנטי merge (`lane_{id:03}_merge`) מחזיקים את שורשי ה-merge-hint האחרונים ואת מחויבויות המצב הגלובלי עבור אותו lane.
- כשממשל משנה שם של כינוי lane, צמתים משנים אוטומטית את תוויות ספריות `blocks/lane_{id:03}_{slug}` (והסנפשוטים המדורגים) כך שמבקרים תמיד יראו את הסלאג הקנוני ללא ניקוי ידני.

## חלוקת מצב העולם

- מצב העולם הלוגי של Nexus הוא איחוד של מרחבי מצב לפי lane. Lanes ציבוריים שומרים מצב מלא; lanes פרטיים/סודיים מייצאים שורשי מרקל/commitment אל ה-merge-ledger.
- אחסון MV מצמיד לכל מפתח את קידומת ה-4 בייטים של ה-lane מ-`LaneConfigEntry::key_prefix`, ומייצר מפתחות כגון `[00 00 00 01] ++ PackedKey`.
- טבלאות משותפות (חשבונות, נכסים, טריגרים, רישומי ממשל) מאחסנות רשומות מקובצות לפי קידומת lane, וכך סריקות טווח נשארות דטרמיניסטיות.
- מטא-דאטה של merge-ledger משקפת את אותו layout: כל lane כותב שורשי merge-hint ושורשי מצב גלובלי מצומצמים אל `lane_{id:03}_merge`, ומאפשר שמירה ממוקדת או פינוי כאשר lane פורש.
- אינדקסים חוצי lanes (כינויי חשבון, רישומי נכסים, מניפסטים של ממשל) מאחסנים זוגות `(LaneId, DataSpaceId)` מפורשים. אינדקסים אלה נמצאים במשפחות עמודות משותפות אך משתמשים בקידומת ה-lane ובמזהי dataspace מפורשים כדי לשמור על חיפושים דטרמיניסטיים.
- זרימת ה-merge משלבת נתונים ציבוריים עם commitments פרטיים באמצעות טפלות `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` הנגזרות מערכי merge-ledger.

## חלוקת Kura ו-WSV

- **סגמנטים של Kura**
  - `lane_{id:03}_{slug}` — סגמנט הבלוקים הראשי עבור ה-lane (בלוקים, אינדקסים, קבלות).
  - `lane_{id:03}_merge` — סגמנט merge-ledger המתעד שורשי מצב מצומצמים וארטיפקטים של settlement.
  - סגמנטים גלובליים (ראיות קונצנזוס, מטמוני טלמטריה) נשארים משותפים כי הם ניטרליים ל-lane; המפתחות שלהם אינם כוללים קידומות lane.
- זמן הריצה עוקב אחר עדכוני קטלוג ה-lane: lanes חדשים מקבלים אוטומטית ספריות בלוקים ו-merge-ledger תחת `kura/blocks/` ו-`kura/merge_ledger/`, בעוד lanes שפרשו נארזים תחת `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*`.
- סנפשוטים מדורגים של מצב משקפים את אותו מחזור; כל lane כותב אל `<cold_root>/lanes/lane_{id:03}_{slug}`, כאשר `<cold_root>` הוא `cold_store_root` (או `da_store_root` כש־`cold_store_root` לא מוגדר), ופרישות מעבירות את עץ הספריות אל `<cold_root>/retired/lanes/`.
- **קידומות מפתח** — קידומת 4 הבייטים המחושבת מ-`LaneId` תמיד מצורפת למפתחות MV מקודדים. לא נעשה שימוש ב-hashing ייעודי ל-host, ולכן הסדר זהה בין צמתים.
- **פריסת בלוק-לוג** — נתוני בלוק, אינדקס ו-hashes מקוננים תחת `kura/blocks/lane_{id:03}_{slug}/`. יומני merge-ledger משתמשים מחדש באותו slug (`kura/merge/lane_{id:03}_{slug}.log`), ושומרים על זרימות התאוששות פר-lane מבודדות.
- **מדיניות שימור** — lanes ציבוריים שומרים גופי בלוקים מלאים; lanes של commitments בלבד יכולים לדחוס גופים ישנים אחרי checkpoints כי commitments הם סמכותיים. lanes סודיים שומרים יומני ciphertext בסגמנטים ייעודיים כדי לא לחסום עומסי עבודה אחרים.
- **כלי תפעול** — `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` בודק את `<store>/blocks` ואת `<store>/merge_ledger` תוך שימוש ב-`LaneConfig` הנגזר, מדווח על סגמנטים פעילים מול פרושים ומארכב ספריות/יומנים פרושים תחת `<store>/retired/...` כדי לשמור על ראיות דטרמיניסטיות. כלי תחזוקה (`kagami`, פקודות CLI אדמין) צריכים לעשות שימוש מחדש ב-namespace המוכנס ל-slug בעת חשיפת מטריקות, תוויות Prometheus או ארכוב סגמנטים של Kura.

## תקציבי אחסון

- `nexus.storage.max_disk_usage_bytes` מגדיר את תקציב הדיסק הכולל שעל צמתים של Nexus לצרוך בין Kura, צילומי WSV קרים, אחסון SoraFS ו-spools של streaming (SoraNet/SoraVPN).
- כאשר התקציב הכולל נחצה, ההדחה דטרמיניסטית: תחילה מקצצים את spools ההקצאה של SoraNet לפי סדר נתיב לקסיקוגרפי, אחר כך את spools SoraVPN, לאחר מכן את צילומי ה-WSV הקרים של tiered-state מהישן לחדש (עם offload אל `da_store_root` כאשר מוגדר), אחר כך את סגמנטי Kura שפרשו, ולבסוף מפנים את גופי הבלוקים הפעילים של Kura אל `da_blocks/` לצורך החייאת DA בעת קריאה.
- `nexus.storage.max_wsv_memory_bytes` מגביל את שכבת ה-WSV החמה על ידי החלת גודל דטרמיניסטי של WSV בזיכרון אל `tiered_state.hot_retained_bytes`; שמירת grace יכולה זמנית לחרוג מהתקציב, אך החריגה נראית בטלמטריה (`state_tiered_hot_bytes`, `state_tiered_hot_grace_overflow_bytes`).
- `nexus.storage.disk_budget_weights` מחלק את תקציב הדיסק בין רכיבים בנקודות בסיס (חייב להסתכם ב-10,000). התקרות המחושבות מוחלות על `kura.max_disk_usage_bytes`, `tiered_state.max_cold_bytes`, `sorafs.storage.max_capacity_bytes`, `streaming.soranet.provision_spool_max_bytes` ו-`streaming.soravpn.provision_spool_max_bytes`.
- האכיפה של תקציב Kura מסכמת את בתים של מחסן הבלוקים על פני סגמנטים פעילים ופרושים של lanes ומכלילה בלוקים בתור שטרם נכתבו כדי להימנע מחריגה בעת השהיית כתיבה.
- Spools של provisioning ל-SoraVPN משתמשים בהגדרות `streaming.soravpn` ומוגבלים באופן נפרד מה-SoraNet provision spool.
- מגבלות לכל רכיב עדיין חלות: כאשר לרכיב יש תקרה מפורשת שאינה אפס, נאכפת הקטנה מבין התקרה הזו לבין תקציב Nexus המחושב.
- טלמטריית התקציב משתמשת ב-`storage_budget_bytes_used{component=...}` וב-`storage_budget_bytes_limit{component=...}` לדיווח שימוש/תקרות עבור `kura`, `wsv_hot`, `wsv_cold`, `soranet_spool` ו-`soravpn_spool`; המונה `storage_budget_exceeded_total{component=...}` גדל כאשר האכיפה דוחה נתונים חדשים והלוגים מזהירים את המפעיל.
- טלמטריית הדחה DA מוסיפה `storage_da_cache_total{component=...,result=hit|miss}` ו-`storage_da_churn_bytes_total{component=...,direction=evicted|rehydrated}` כדי לעקוב אחר פעילות המטמון והבתים שהועברו עבור `kura` ו-`wsv_cold`.
- Kura מדווח את אותה חשבונאות שמשמשת בזמן admission (בתים על דיסק ועוד בלוקים בתור, כולל payloads של merge-ledger כאשר קיימים), כך שהמדדים משקפים לחץ אפקטיבי ולא רק בתים שנשמרו.

## ניתוב ו-APIs

- נקודות קצה REST/gRPC של Torii מקבלות `lane_id` אופציונלי; היעדרו מרמז על `lane_default`.
- ה-SDKs מציגים בוררי lane וממפים כינויים ידידותיים למשתמש ל-`LaneId` דרך קטלוג ה-lane.
- כללי ניתוב פועלים על קטלוג מאומת ועשויים לבחור גם lane וגם dataspace. `LaneConfig` מספק כינויים ידידותיים לטלמטריה לדשבורדים ולוגים.

## settlement ועמלות

- כל lane משלם עמלות XOR לסט המאמתים הגלובלי. Lanes יכולים לגבות טוקני גז מקומיים אך חייבים להפקיד equivalents של XOR לצד commitments.
- הוכחות settlement כוללות סכום, מטא-דאטה להמרה והוכחת escrow (למשל העברה ל-vault הגלובלי של העמלות).
- נתב ה-settlement המאוחד (NX-3) מחייב באפרים תוך שימוש באותן קידומות lane, כך שטלמטריית settlement מתיישרת עם גאומטריית האחסון.

## ממשל

- Lanes מצהירים על מודול הממשל שלהם דרך הקטלוג. `LaneConfigEntry` נושא את הכינוי המקורי ואת ה-slug כדי לשמור על טלמטריה ועל עקבות ביקורת קריאים.
- הרגיסטרי של Nexus מפיץ מניפסטים חתומים של lane הכוללים את `LaneId`, קישור dataspace, מזהה ממשל, מזהה settlement ומטא-דאטה.
- קרסי runtime-upgrade ממשיכים לאכוף מדיניות ממשל (`gov_upgrade_id` כברירת מחדל) ולרשום דלתות דרך גשר הטלמטריה (אירועי `nexus.config.diff`).
- מניפסטי lane מגדירים את מאגר המאמתים של dataspace עבור lanes מנוהלים אדמיניסטרטיבית; lanes נבחרים ב-stake גוזרים את מאגר המאמתים שלהם מרישומי staking של lanes ציבוריים.

## טלמטריה וסטטוס

- `/status` חושף כינויים של lane, קישורי dataspace, מזהי ממשל ופרופילי settlement, הנגזרים מהקטלוג ומ-`LaneConfig`.
- מטריקות מתזמן (`nexus_scheduler_lane_teu_*`) מציגות כינויים/slug של lane כדי שמפעילים יוכלו למפות backlog ולחץ TEU במהירות.
- `nexus_lane_configured_total` סופר את מספר רשומות ה-lane הנגזרות ומחושב מחדש כאשר הקונפיגורציה משתנה. טלמטריה משדרת דלתות חתומות בכל פעם שגאומטריית lane משתנה.
- מדדי backlog של dataspace כוללים מטא-דאטה של כינוי/תיאור כדי לעזור למפעילים לקשר לחץ תור לדומיינים עסקיים.

## קונפיגורציה וטיפוסי Norito

- `LaneCatalog`, `LaneConfig` ו-`DataSpaceCatalog` נמצאים ב-`iroha_data_model::nexus` ומספקים מבנים תואמי Norito עבור מניפסטים ו-SDKs.
- `LaneConfig` נמצא ב-`iroha_config::parameters::actual::Nexus` ומופק אוטומטית מהקטלוג; אינו דורש קידוד Norito כי הוא helper פנימי בזמן ריצה.
- הקונפיגורציה הפונה למשתמש (`iroha_config::parameters::user::Nexus`) ממשיכה לקבל תיאורי lane ו-dataspace דקלרטיביים; הפרסינג כעת נגזר את הגאומטריה ודוחה כינויים לא תקינים או מזהי lane כפולים.
- `DataSpaceMetadata.fault_tolerance` שולט על גודל ועדות lane-relay; החברות בוועדה נדגמת דטרמיניסטית בכל epoch ממאגר מאמתים של dataspace באמצעות זרע epoch של VRF הקשור ל-`(dataspace_id, lane_id)`.

## עבודה שנותרה

- לשלב את עדכוני נתב ה-settlement (NX-3) עם הגאומטריה החדשה כך שחיובי באפרי XOR וקבלות יסומנו לפי ה-slug של lane.
- להשלים את אלגוריתם ה-merge (סדר, pruning, זיהוי קונפליקטים) ולצרף fixtures רגרסיה לשחזור cross-lane.
- להוסיף קרסי תאימות לרשימות לבנות/שחורות ומדיניות כסף מתוכנת (במעקב תחת NX-12).

---

*מסמך זה יתפתח ככל שמשימות NX-2 עד NX-18 מתקדמות. נא לתעד שאלות פתוחות ברודמפ או במעקב הממשל.*

</div>
