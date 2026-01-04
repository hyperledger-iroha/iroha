<!-- Hebrew translation of docs/source/ivm_header.md -->

---
lang: he
direction: rtl
source: docs/source/ivm_header.md
status: complete
translator: manual
---

<div dir="rtl">

# כותרת בייטקוד של IVM

עמוד זה מגדיר את שדות כותרת הבייטקוד של IVM ואת מדיניות התאימות. הוא משלים את סקירת הארכיטקטורה ב־`ivm.md` ומוזכר במסמכי הצינור והדוגמאות.

## Magic
- ארבעה בתים: ASCII `IVM\0` החל מהיסט 0.

## תצורה (נוכחית)
- היסטים וגדלים (17 בתים בסך הכול):
  - ‎0..4: magic ‏`IVM\0`
  - ‎4: ‏`version_major: u8`
  - ‎5: ‏`version_minor: u8`
  - ‎6: ‏`mode: u8` (ביטי פיצ'ר; ראו להלן)
  - ‎7: ‏`vector_length: u8`
  - ‎8..16: ‏`max_cycles: u64` (little-endian)
  - ‎16: ‏`abi_version: u8`

## ביטי מצב (Mode bits)
- ‏`ZK = 0x01`, ‏`VECTOR = 0x02`, ‏`HTM = 0x04` (שמורים/מופעלים ע״י פיצ'ר).

## משמעות השדות
- ‏`version`: גרסת המשמעות (semantic version) של פורמט הבייטקוד (`version_major.version_minor`). משמשת להתאמת הדקודר ולבחירת לוח הגזים.
- ‏`abi_version`: גרסת טבלת הקריאות ומוסכמות ה־pointer‑ABI.
- ‏`mode`: ביטי פיצ'ר לתיעוד ZK / פעולות וקטוריות / HTM.
- ‏`vector_length`: אורך וקטור לוגי עבור פקודות וקטוריות (0 = לא מוגדר).
- ‏`max_cycles`: חסם ריפוד ריצה המשמש במצב ZK ובקבלה לבלוק.

## הערות
- סדר הבתים והפריסה נקבעים ע"י היישום ונקשרים ל־`version`. הפריסה כאן מייצגת את המימוש הנוכחי ב־`crates/ivm/src/metadata.rs`.
- קורא מינימלי יכול להסתמך על פריסה זו עבור ארטיפקטים נוכחיים, ובשינויים עתידיים עליו להשתמש ב־`version` כדי להסתגל.
- האצת חומרה (Metal/CUDA) היא opt-in בכל מארח. בזמן הריצה קוראים ערכי `AccelerationConfig` מתוך `iroha_config`: המפתחות `enable_metal` ו־`enable_cuda` מפעילים את המנועים המתאימים גם אם קומפלו. ההגדרות מוחלות באמצעות `ivm::set_acceleration_config` לפני יצירת ה־VM.
- מפעילים יכולים גם לכפות השבתת מנועים לצורך דיאגנוסטיקה באמצעות `IVM_DISABLE_METAL=1` או `IVM_DISABLE_CUDA=1`. משתני סביבה אלה גוברים על הקונפיגורציה ומשאירים את ה־VM בנתיב ה־CPU הדטרמיניסטי.

## עזרי מצב Durable ופני השטח של ה־ABI
- קריאות העזר למצב Durable (‎0x50–0x5A: ‏STATE_{GET,SET,DEL}, ‏ENCODE/DECODE_INT, ‏BUILD_PATH_* וקידוד/פענוח JSON/SCHEMA) הן חלק מ־ABI V1 ונכללות בחישוב `abi_hash`.
- CoreHost מחבר את STATE_{GET,SET,DEL} למצב חוזים עמיד מגובה־WSV; מארחי dev/בדיקה יכולים להשתמש ב־overlays או בהתמדה מקומית אך חייבים לשמור על אותה התנהגות נצפית.

## ולידציה
- קבלה לנוד מאפשרת רק כותרות עם `version_major = 1`; ערכים אחרים נדחים.
- `mode` חייב להכיל רק ביטים מוכרים: ‏`ZK`, ‏`VECTOR`, ‏`HTM` (ביטים לא מוכרים נדחים).
- ‏`vector_length` הוא שדה ייעוץ ויכול להיות לא־אפס גם אם ביט `VECTOR` אינו מוגדר; הקבלה אוכפת חסם עליון בלבד.
- ערכי `abi_version` נתמכים: גרסת השחרור הראשונה תומכת ב־`1` בלבד (V1); ערכים אחרים נדחים בקבלה.

### מדיניות (נוצרה אוטומטית)
הטבלה הבאה נוצרת מהיישום ואין לערוך אותה ידנית.

<!-- BEGIN GENERATED HEADER POLICY -->
| שדה | מדיניות |
|---|---|
| version_major | 1 |
| version_minor | כל ערך |
| mode (known bits) | ‎0x07 (‏ZK=0x01, VECTOR=0x02, HTM=0x04) |
| abi_version | 1 |
| vector_length | 0 או 1..=64 (ייעוצי; לא תלוי בביט VECTOR) |
<!-- END GENERATED HEADER POLICY -->

### ערכי ABI Hash (נוצרו אוטומטית)
הטבלה הבאה מציגה את ערכי `abi_hash` הקנוניים עבור המדיניות הנתמכות.

<!-- BEGIN GENERATED ABI HASHES -->
| מדיניות | abi_hash (hex) |
|---|---|
| ABI v1 | 377f7125a0f20d40f65ed5e3a179bc5e04d68a385570b54d67d961861e8d9f83 |
<!-- END GENERATED ABI HASHES -->

## מדיניות תאימות
- עדכונים מינוריים יכולים להוסיף פקודות מאחורי `feature_bits` ומרחב opcode שמור; עדכונים מרכזיים עשויים לשנות קידודים או להסירם/להחליפם רק עם שדרוג פרוטוקול.
- תחומי קריאות המערכת יציבים; מספר לא מוכר עבור `abi_version` הפעיל מוביל ל־`E_SCALL_UNKNOWN`.
- לוחות גז תלויים ב־`version` ודורשים וקטורים מזהב בעת שינוי.

## בדיקת ארטיפקטים
- השתמשו ב־`ivm_tool inspect <file.to>` כדי לצפות בשדות הכותרת באופן יציב.
- לצורכי פיתוח, ספריית `examples/` כוללת יעד Makefile קטן `examples-inspect` שמריץ inspect על ארטיפקטים שנבנו.

## דוגמה (Rust): בדיקת magic וגודל מינימליים

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

הערה: פריסת הכותרת מעבר ל־magic היא גרסתית ותלויה ביישום; עדיף להשתמש ב־`ivm_tool inspect` לקבלת שמות שדות וערכים יציבים.

</div>
