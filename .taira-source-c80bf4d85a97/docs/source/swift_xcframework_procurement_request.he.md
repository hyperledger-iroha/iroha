---
lang: he
direction: rtl
source: docs/source/swift_xcframework_procurement_request.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 57e5497c8a43ac0dc46a21678c3bff6892a700f5f00702c63d93c2a21f5a548d
source_last_modified: "2026-01-27T09:17:52.324120+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/swift_xcframework_procurement_request.md -->

# בקשת רכש – מכשיר StrongBox חלופי

- **מבקש:** מנהל QA של Swift ‏(qa-swift@sora.org)
- **תאריך:** 2026-01-27
- **מטרה:** לספק iPhone חלופי עם StrongBox ל‑XCFramework smoke harness (`xcframework-smoke/strongbox` lane)
  כדי להימנע מזמני השבתה כאשר המכשיר הראשי בתחזוקה.
- **מכשיר מומלץ:** iPhone 15 Pro ‏(128 GB, unlocked) – שומר על תמיכת StrongBox ומבטיח עתידיות ל‑iOS 18.
- **עלות משוערת:** 1,199 USD (מחיר עסקי ב‑Apple Store, ללא מס).
- **אביזרים:** כבל USB‑C ל‑USB‑C (כלול) + ספק כוח USB‑C חלופי (20W) — 19 USD.
- **תקציב כולל משוער:** 1,218 USD + מס מקומי ומשלוח.

## הצדקה
- מבטיח כיסוי למסלול StrongBox המחייב כפי שמתועד ב‑`docs/source/swift_xcframework_device_matrix.md`.
- מצמצם השבתות CI הנגרמות משחיקת סוללה, התקנות מערכת, או כשל חומרה.
- מאפשר הרצות smoke מקבילות כאשר ה‑harness צריך להתרחב בשבועות שחרור.

## צעדים הבאים
1. להגיש כרטיס רכש (`PROC-STRONGBOX-2026`) המתייחס למסמך זה.
2. לתאם משלוח לרכב 3 במעבדת קופרטינו (slot A12 spare) ולעדכן את
   `docs/source/swift_xcframework_hardware_plan.md` לאחר הגעת המכשיר.
3. לתייג את המכשיר החדש עם DeviceKit `ios15p-strongbox-spare`, לרשום אותו ב‑MDM,
   ולאמת שמטא‑נתוני `ci/xcframework-smoke:strongbox:device_tag` מופיעים בהרצת
   Buildkite הבאה לפני הכרזה שהחלופי מוכן.

</div>
