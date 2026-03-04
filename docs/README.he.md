<!-- Hebrew translation of docs/README.md -->

---
lang: he
direction: rtl
source: docs/README.md
status: complete
translator: manual
---

<div dir="rtl">

# תיעוד Iroha

לסקירה ביפנית ראו את [`README.ja.md`](./README.ja.md).

המאגר מפיק שתי גרסאות מאותו בסיס קוד: **Iroha 2** עבור רשתות מאוחסנות-עצמית (permissioned / קונסורציום) ו-
**Iroha 3 / SORA Nexus** עבור ספר החשבונות הגלובלי היחיד. שתי הגרסאות מריצות את אותו Iroha Virtual Machine
(IVM) ואת כלי Kotodama, כך שקוד וקובצי bytecode ניתנים לפריסה חוצה-סביבות ללא הידור מחדש.

ב[אתר התיעוד הראשי של Iroha](https://docs.iroha.tech/) תמצאו:

- [מדריך התחלה מהירה](https://docs.iroha.tech/get-started/)
- [מדריכי SDK](https://docs.iroha.tech/guide/tutorials/) עבור Rust, ‏Python, ‏JavaScript ו־Java/Kotlin
- [מדריך API](https://docs.iroha.tech/reference/torii-endpoints.html)

בנוסף מומלץ לעיין ב:

- [המסמך הלבן של Iroha 2](./source/iroha_2_whitepaper.md) – מפרט הרשתות המאוחסנות-עצמית.
- [המסמך הלבן של Iroha 3 / SORA Nexus](./source/iroha_3_whitepaper.md) – ארכיטקטורת המולטי-ליין ומרחבי הנתונים.
- [מודל הנתונים ו־ISI (נגזר מהמימוש)](./source/data_model_and_isi_spec.md) – שחזור התנהגותי מדויק מתוך בסיס הקוד.
- [מעטפות ZK (Norito)](./source/zk_envelopes.md) – מעטפות Norito עבור IPA/STARK וציפיות המאמתים.

## לוקליזציה

תרגומים ליפנית (`*.ja.*`) ולעברית (`*.he.*`) נשמרים לצד קובץ המקור באנגלית. פרטים על יצירה, תחזוקה והוספת שפות חדשות נמצאים ב־[`docs/i18n/README.md`](./i18n/README.md).

## כלים

במאגר זה תמצאו תיעוד לכלי Iroha 2:

- [Kagami](../crates/iroha_kagami/README.md)
- [`iroha_derive`](../crates/iroha_derive/) – מאקרו לקונפיגורציה (ראו את הפיצ'ר `config_base`)
- [שלבי בנייה לפרופילינג](./profile_build.md) לזיהוי שלבי קומפילציה איטיים בקרייט `iroha_data_model`

## תיעוד Swift / iOS

- [סקירת ה-Swift SDK](./source/sdk/swift/index.md) – סיוע למסלולי Pipeline, האצת Metal/NEON ו-Connect/WebSocket.
- [מדריך Connect מהיר](./connect_swift_ios.md) – שימוש ב-SDK (וכן רפרנס CryptoKit ידני) לפתיחת סשנים.
- [מדריך שילוב ב-Xcode](./connect_swift_integration.md) – שילוב NoritoBridgeKit/Connect, שימוש ב-ChaChaPoly ופיענוח מעטפות.
- [מדריך תורמים ל-SwiftUI Demo](./norito_demo_contributor.md) – הרצת דמו iOS מול צומת Torii מקומי, כולל ניטור והאצת החומרה.
- לפני פרסום שינויים הקשורים ל-Swift או Connect, הריצו `make swift-ci` כדי לאמת פריות פיקסצ'רים, פידי דשבורד ומטא-דאטה של Buildkite `ci/xcframework-smoke:<lane>:device_tag`.

## Norito (קודק סריאליזציה)

Norito הוא קודק הסריאליזציה של סביבת העבודה. איננו משתמשים ב־`parity-scale-codec`‏ (SCALE). השוואות ל־SCALE בתיעוד או במדידות נועדו להקשר בלבד; בכל השבילים בייצור משתמשים ב־Norito. ה־APIs של `norito::codec::{Encode, Decode}` מספקים מטען Norito ללא כותרת ("bare") לצורכי גיבוב ויעילות על הקו – זה Norito, לא SCALE.

המצב העדכני:

- קידוד/פענוח דטרמיניסטיים עם כותרת קבועה (magic, גרסה, מזהה סכימה של 16 בתים, דחיסה, אורך, CRC64, דגלים).
- checksum מסוג CRC64-XZ עם האצת זמן־ריצה:
  - ‏x86_64 PCLMULQDQ (כפל ללא נשא) עם Barrett reduction ומקטעי 32 בתים.
  - ‏aarch64 PMULL עם קיפול תואם.
  - Slicing-by-8 ונפילות ביטיות לניידות.
- רמזי אורך מוצפן שמיושמים על ידי ה־derive והטיפוסים העיקריים כדי לצמצם הקצאות.
- חוצצי סטרימינג גדולים יותר (64KiB) ועדכון CRC אינקרמנטלי בזמן הפענוח.
- דחיסת zstd אופציונלית; האצת GPU נשלטת באמצעות פיצ'ר והיא דטרמיניסטית.
- בחירת מסלול אדפטיבית: ‏`norito::to_bytes_auto(&T)` בוחר בין ללא דחיסה, דחיסת zstd ב־CPU או דחיסה מואצת GPU (כאשר מקומפלים והיא זמינה) בהתאם לגודל המטען ויכולות החומרה בזיכרון. הבחירה משפיעה רק על ביצועים ועל בית ה־`compression` בכותרת; המשמעות הלוגית אינה משתנה.

ראו `crates/norito/README.md` לבדיקות השוואה, בנצ'מרקים ודוגמאות שימוש.

הערה: מרבית המסמכים (כולל האצה ל-IVM ומעגלי ZK) תורגמו ועדכנו. עבור אזורים שנותרו בעבודה מומלץ להתייעץ בגרסה האנגלית, ולדווח על פערים ב-Issue או Pull Request.

הערות על קידוד נקודת הסטטוס:
- גוף Torii `/status` משתמש ב־Norito כברירת המחדל עם מטען bare קומפקטי. מומלץ לנסות לפענח Norito תחילה.
- השרתים יכולים להחזיר JSON לפי בקשה; לקוחות עוברים ל־JSON אם `content-type` הוא ‎`application/json`.
- הפורמט על הקו הוא Norito, לא SCALE. נעשה שימוש ב־`norito::codec::{Encode,Decode}` גם בגרסה ה־bare.

</div>
