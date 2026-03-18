<!-- Hebrew translation of docs/source/norito_json_migration.md -->

---
lang: he
direction: rtl
source: docs/source/norito_json_migration.md
status: complete
translator: manual
---

<div dir="rtl">

# מדריך הגירת Norito JSON

המדריך מתעד את המעבר מקצה לקצה של סביבת העבודה מ-Serde / `serde_json` לסריאליזציה מקורית של Norito. הוא משלים את האינבנטרי האוטומטי של `scripts/inventory_serde_usage.py` ומציג את ממשק ה-Norito שמחליף derives, visitors, עזרי DOM של JSON וטועני קונפיגורציה של Serde.

> **סטטוס (9 בנובמבר 2025):** כל קרייטי הפרודקשן נקיים מ-Serde. המסמך נשאר כמקור היסטורי וכמסייע onboarding לזרימת העבודה Norito-first.

## למה לעבור ל-Norito מקצה לקצה?
- **דטרמיניזם:** קודקוד הבינארי וה-JSON של Norito חולקים את חוקי הקנוניזציה שמשמשים לחישובי hash ולמניעת ריפליי.
- **שוויון פיצ'רים:** נגזרות Norito מכסות את כל הדפוסים שטופלו בעבר באמצעות `serde::{Serialize, Deserialize}` כולל הוקים של מבקרי map, תגי enum ומעבר ערך גולמי.
- **פשטות תלות:** הסרת Serde מונעת כפילויות בדרייבים ומפחיתה תלות בספי פיצ'רים.

## טבלת הצמדה בין Serde ל-Norito

| שימוש ב-Serde | תחליף Norito | הפניה |
| --- | --- | --- |
| דרייבים על מבנים / גבולות תכונות | `NoritoSerialize` + `NoritoDeserialize` | Struct + enum serialization |
| תגיות enum | `#[norito(tag = ...)]`, ‏`#[norito(rename = ...)]`, ‏`#[norito(other)]` | Enum tagging |
| גישת map / אוספים | `json::MapVisitor`, ‏`json::SeqVisitor`, ‏`json::CoerceKey`, ‏`#[norito(with = ...)]` | Visitor + builder hooks |
| שדות אופציונליים וברירות מחדל | `#[norito(default)]`, ‏`#[norito(skip_serializing_if = Option::is_none)]`, ‏`MapVisitor::read_optional` | Optional fields and defaults |
| טעינת קונפיגורציה ו-IO | `norito_config::TomlValue`, ‏`norito::json::from_reader`, ‏`to_writer` | Config loading and Norito-backed TOML |
| Snapshot / מטעני בינארי | `norito::core::{to_bytes, from_bytes}`, מהדרי JSON | Snapshot + binary IO |
| Visitor מותאם אישית | `JsonSerialize`/`JsonDeserialize`, ‏`json::Visitor` | Visitor + builder hooks |

## כלי עזר
- **אינבנטרי:** `scripts/inventory_serde_usage.py` מפיק JSON ותקציר קריא ב-`docs/source/norito_json_inventory.{json,md}` כדי להבטיח שקרייטי הפרודקשן נקיים מ-Serde (רק מסמכי מדיניות/כלים/Fixtures נשארים ברשימת ההיתר).
- **שמירה ב-CI:** `scripts/deny_serde_json.sh` ו-`scripts/check_no_direct_serde.sh` אוכפים את כלל “ללא Serde בקוד פרודקשן” ויעודכנו בשלב האחרון כך שיכשלו עם כל אזכור שיורי.

## סדרת מבנים ו-enum

| תבנית Serde | תחליף Norito |
| --- | --- |
| `#[derive(Serialize, Deserialize)]` | `#[derive(NoritoSerialize, NoritoDeserialize)]` |
| `serde::Serialize` | `norito::NoritoSerialize` |
| `serde::DeserializeOwned` | `norito::NoritoDeserializeOwned` |
| `#[serde(rename = ...)]`, ‏`#[serde(rename_all = ...)]` | זהה ב-`#[norito(...)]` |
| `#[serde(default)]`, `#[serde(skip)]`, `#[serde(skip_serializing_if = ...)]` | Norito מכבד אותן בשם זהה |
| `#[serde(flatten)]` | `#[norito(flatten)]` |
| מבקר מותאם אישית | ליישם `NoritoDeserialize` ידנית או להשתמש ב-`MapVisitor` / `SeqVisitor` |

> **סטטוס (7 בנובמבר 2025):** newtypes של מזהים טקסטואליים (כגון `DomainId`, ‏`AssetDefinitionId`, ‏`NftId`, ‏`TriggerId`) מיישמים כעת `FastJsonWrite` + `JsonDeserialize` באמצעות המאקרו `string_id!`, כך ש-`norito::json::to_json` / `from_json` נשארים עקביים לייצוגים קנוניים.
>
> **סטטוס (25 בינואר 2026):** `AccountId`/`AssetId` JSON כולל כעת סיומת `@domain` מפורשת (למשל `I105`, `norito:<hex>`) כדי שהפענוח לא יישען על domain‑selector resolver. קלטי alias כגון `label@domain` עדיין נתמכים כאשר resolvers מוגדרים.

> **סטטוס (7 בנובמבר 2025):** `IpfsPath` קיבל `FastJsonWrite` ו-`JsonDeserialize` של Norito, עם בדיקות רגרסיה לזוגות תקפים ופסילה של מסלולים לא חוקיים. מימושי Serde נשארים זמנית.

### Visitor/Builder Hooks
- `norito::json::MapVisitor` / `SeqVisitor` מחליפים את `serde::MapAccess` / `SeqAccess` וכוללים עזרי `missing_field`, ‏`duplicate_field`, ‏`unknown_field`.
- `norito::json::RawValue` מחליף את `serde_json::value::RawValue`.
- `norito::json::CoerceKey` לטיפול מפתחי map.
- `json::Visitor` מספק hooks מסוג אחד (`visit_bool`, ‏`visit_i64`, ‏`visit_map`, ‏`visit_seq`, ועוד).
- נגזרות `#[derive(JsonSerialize, JsonDeserialize)]` יוצרות כותבים דטרמיניסטיים ומבקרים מבוססי `MapVisitor`.

### תגיות enum
- `#[norito(tag = "type")]`, `#[norito(rename = ...)]`, ‏`#[norito(other)]` ועוד – המקביל ל-Serde.
- `#[norito(tag = ..., content = ...)]` לטיפול במצבי tagged/untagged.

### מפתחות Map
- Norito מחזיק סדר דטרמיניסטי ללא DOM. `json::CoerceKey` מסייע להעברת קסטומיזציה קיימת.
- `MapVisitor::read_string_key` מחליף `deserialize_identifier`.
- לתחייה במפתחות לא מוכרים משתמשים ב-`reject_unknown_fields()` או `deny_unknown`.

### שדות אופציונליים/ברירות
- `#[norito(default)]`, ‏`#[norito(default = "fn")]`.
- `#[norito(skip_serializing_if = Option::is_none)]`.
- `MapVisitor::read_optional`.

### Snapshot ו-IO בינארי
- כל מסלולי ה-snapshot משתמשים ב-`norito::core::{to_bytes, from_bytes}`.
- snapshot JSON בבדיקות משתמשים ב-`norito::json::{to_json_pretty, from_json}`.

### טעינת קונפיגורציה
1. לפרסר TOML עם `norito_config::TomlValue`.
2. להחיל שכבות `iroha_config_base::read` עם ערכי Norito.
3. כשנדרש תיווך JSON, להשתמש ב-`norito_config::toml::to_json_value`.

עזרי `serde_with` מוחלפים ב-`#[norito(with = ...)]` או חיץ `MapVisitor`. קריאות `serde_json::from_reader` ← `norito::json::from_reader`.

## מיפוי שגיאות
- שימוש ב-`norito::json::Error`; המרה באמצעות `From` לסוגי שגיאה מקומיים.
- הגדרה מחדש של `serde_json::Error` → `norito::json::Error` במסלולי CLI/config.

## תהליך הגירה
1. להריץ את האינבנטרי לאיתור שרידי Serde.
2. להחליף derives/imports ל-Norito ולהבטיח קומפילציה.
3. להחליף `serde_json::Value` ועזרי DOM ב-Norito.
4. להוסיף round-trip tests (`to_bytes`/`from_bytes`, ‏`to_json`/`from_json`).
5. לעדכן תיעוד/הערות שה-backend הקנוני הוא Norito.

לסטטוס נוכחי — ראו `status.md`.

### ספר המתכון לפי קרייט (תקציר)

#### סטאק קונפיגורציה
- החלפת derives `serde` ל-`Norito`.
- טעינת שכבות עם `norito_config::load_layers`.
- `serde_with` → `#[norito(with = ...)]` או קריאות Visitor.
- טיפול בשגיאות דרך `From<norito::json::Error>` עבור `iroha_config_base::Error`.
- בדיקות רגרסיה שמוודאות שקבצי משתמש נטענים ב-Norito ויוצרים פלט זהה למהדורות Serde קודמות.

> **סטטוס (7 בנובמבר 2025):** `iroha_config` הסיר את תלויות `serde`/`serde_with`; תהליכי ברירות המחדל והמבקרים מושתתים על Norito.

#### CLI / Torii
- שימוש ב-`norito::json::{Parser, Reader, TapeWalker}`, פונקציות `from_reader`, ‏`from_slice`, ‏`from_io_buf`, ‏`to_writer`.
- הבטחת שכל מסלולי snapshot/config/binary משתמשים ב-Norito ולא ב-Serde.
- כלי הבדיקה והתיעוד במאגר פועלים כעת עם `norito::json::Value`, ו-`scripts/inventory_serde_usage.py` ממשיך לוודא שאין מופעי `serde_json::Value`.

---

גם לאחר סיום ההגירה, יש להמשיך לצרף טיפוסים חדשים בעזרת Derives/Visitors של Norito ולהימנע מהחזרת Serde. במידת הצורך, השתמשו במדריך כדי להסב כלי עזר שנותרו ברקע (דוגמת שכבות תאימות לייבוא חיצוני) ל-Norito.

</div>
