---
lang: he
direction: rtl
source: docs/source/sdk/swift/connect_risk_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 942518163a04c38fdef0c4c65a62b358d1d6db5f0105d6013741b12524c60199
source_last_modified: "2026-02-03T09:04:02.697509+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/sdk/swift/connect_risk_tracker.md -->

# מעקב סיכונים ל‑Swift Connect - 25 באפריל 2026

ה‑Swift SDK עדיין מתייחס למספר תת‑מערכות של Connect כ‑scaffolding. מעקב זה
משאיר את דרישת ה‑roadmap "publish weekly status delta + Connect risk" גלויה
(`roadmap.md:1786`) עד לסגירת הסיכונים.

| מזהה | סיכון | השפעה | ראיות | מיתון / צעדים הבאים | סטטוס |
|----|------|--------|----------|-------------------------|--------|
| CR-1 | **תאימות קודק תלויה ב‑JSON fallback** | הסיכון הצטמצם כעת ש‑`ConnectCodec` נכשל‑סגור כאשר גשר Norito חסר — ה‑SDK כבר לא פולט מסגרות מקודדות JSON, והבדיקות מפעילות ישירות fixtures של הגשר עבור control/ciphertext. החשיפה שנותרה מוגבלת לשגיאות אריזה שמשמיטות את `NoritoBridge.xcframework`. | `IrohaSwift/Sources/IrohaSwift/ConnectCodec.swift:4`, `IrohaSwift/Tests/IrohaSwiftTests/ConnectFramesTests.swift:1`, `docs/source/sdk/swift/index.md:496` | השאירו את רשימת הבדיקה לאריזת xcframework בראש העדיפויות (`docs/source/sdk/swift/reproducibility_checklist.md`, README) כך ששחרורי Carthage/SPM תמיד כוללים את הגשר; ייצואי סטטוס Connect מצטטים כעת את בדיקות fail‑closed כהוכחת תאימות. | בינוני (ממוזער 25 באפריל 2026) |
| CR-2 | **תשתית התעבורה חסרה טלמטריית queue/flow control** | נסגר — journal/queue + metrics נשלחים דרך `ConnectQueueJournal`, `ConnectQueueStateTracker`, ו‑replay recorder, ובקרת זרימה נכנסת אוכפת חלונות לכל כיוון עם token grant/consume (`ConnectFlowControlWindow` + `ConnectFlowController`). בדיקות מכסות התמדה של snapshot/metrics ומיצוי/הענקת flow‑control; `ConnectSession` צורך tokens על מסגרות ciphertext לפני פענוח. | `IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectQueueDiagnostics.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectReplayRecorder.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectFlowControl.swift:1`, `docs/source/sdk/swift/index.md:590` | לשמור על digests שבועיים שמאשרים ייצוא metrics של queue; להרחיב grant‑ים לבקרת זרימה כאשר חלונות בצד הארנק נחשפים. | נמוך |
| CR-3 | **שמירת מפתחות ואטסטציה לא ממומשים** | ממוזער — `ConnectKeyStore` שומר כעת זוגות מפתחות Connect עם חבילת אטסטציה (digest SHA‑256, תווית מכשיר, נוצר‑ב) ואחסון מגובה‑קובץ כברירת מחדל, וסוגר את הפער של "raw bridge בלבד". בדיקות מכסות התמדה round‑trip ויצירת digest; המסמכים מפנים ארנקים ל‑keystore לפני אישורים. | `docs/source/sdk/swift/index.md:445`, `IrohaSwift/Sources/IrohaSwift/ConnectKeyStore.swift:1` | לחבר את ה‑keystore לזרימות אישור ארנק ולהרחיב אטסטציה לאחסון מגובה‑חומרה כשזמין; לוודא ש‑digests שבועיים מאשרים שימוש ב‑keystore בסשנים של Connect. | נמוך |

> **קצב מעקב:** סקירה שבועית ב‑Swift Connect stand‑up; להסיר פריטים רק לאחר שהקוד,
התיעוד והדשבורדים הטלמטריים מוכיחים שהמיתון חי.

</div>
