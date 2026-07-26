---
lang: he
direction: rtl
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T15:38:30.658233+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! אישור השקת Payload v1 (מועצת SDK, 2026-04-28).
//!
//! לוכד את תזכיר החלטת מועצת SDK הנדרש על ידי `roadmap.md:M1` כך
//! להשקת מטען מוצפן v1 יש רשומה ניתנת לביקורת (ניתן למסירה M1.4).

# החלטת השקת מטען v1 (2026-04-28)

- **יו"ר:** ראש מועצת SDK (M. Takemiya)
- **חברי הצבעה:** Swift Lead, CLI Maintainer, Confidential Assets TL, DevRel WG
- **תצפיות:** Program Mgmt, Telemetry Ops

## תשומות נבדקו

1. **כריכות ושולחים מהירים** — `ShieldRequest`/`UnshieldRequest`, שולחים אסינכרוניים ועוזרים לבניית Tx הגיעו עם מבחני זוגיות ו docs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **ארגונומיה של CLI** — עוזר `iroha app zk envelope` מכסה זרימות עבודה של קידוד/בדיקה וכן אבחון כשלים, בהתאמה לדרישת ארגונומיה של מפת הדרכים.【ארגזים/iroha_cli/src/zk.rs:1256】
3. **מכשירים דטרמיניסטיים וחבילות זוגיות** — מתקן משותף + אימות חלודה/מהיר כדי לשמור על Norito בייטים/משטחי שגיאה aligned.【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_en crypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## החלטה

- **אשר השקת מטען v1** עבור SDK ו-CLI, מה שמאפשר לארנקים של Swift ליצור מעטפות סודיות ללא צנרת מותאמת אישית.
- **תנאים:** 
  - שמור על אביזרי זוגיות תחת התראות סחיפה של CI (קשור ל-`scripts/check_norito_bindings_sync.py`).
  - תיעד את ספר המשחקים התפעולי ב-`docs/source/confidential_assets.md` (כבר עודכן באמצעות Swift SDK PR).
  - רשום כיול + עדויות טלמטריה לפני היפוך דגלי ייצור כלשהם (מעקב תחת M2).

## פריטי פעולה

| בעלים | פריט | בשל |
|-------|------|-----|
| הובלה מהירה | הכרזה על זמינות GA + קטעי README | 2026-05-01 |
| CLI Maintainer | הוסף עוזר `iroha app zk envelope --from-fixture` (אופציונלי) | צבר (לא חוסם) |
| DevRel WG | עדכון התחלה מהירה של ארנק עם הוראות מטען v1 | 2026-05-05 |

> **הערה:** תזכיר זה מחליף את הקריאה הזמנית "ממתין לאישור המועצה" ב-`roadmap.md:2426` ומספק את פריט המעקב M1.4. עדכן את `status.md` בכל פעם פריטי פעולה מעקב נסגרים.