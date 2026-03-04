---
lang: he
direction: rtl
source: docs/source/sdk/swift/readiness/screenshots/2026-02-28/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c3e0d606966f67b7fdb2ef7862bcaa955c117cb9ae645f5c3626c75c199ec35
source_last_modified: "2026-03-01T14:02:06.725705+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/sdk/swift/readiness/screenshots/2026-02-28/README.md -->

# אינדקס צילומי מסך — Dry‑Run מוכנות טלמטריה ב‑Swift (2026-02-28)

הוסיפו כאן צילומי PNG/JPEG מהחזרה. הקבצים נשמרים ב‑
`s3://sora-readiness/swift/telemetry/20260228/` כדי להימנע מניפוח הריפו; הטבלה
להלן מפרטת שמות אובייקטים קנוניים שמוזכרים בהערות הארכיון.

| קובץ | תרחיש | תיאור |
|------|----------|-------------|
| `s3://sora-readiness/swift/telemetry/20260228/20260228-salt-drift-alert.png` | A | לוח התראות של `mobile_parity.swift` שמציג `swift.telemetry.redaction.salt_drift` ב‑firing ואז clear. |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-override-ledger.png` | B | רשומת override שנוצרה דרך `scripts/swift_status_export.py telemetry-override create …`. |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-exporter-outage.png` | C | דשבורד השבתה של exporter עם OTLP collector מושעה, המדגיש timeline של suppression + התאוששות. |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-offline-queue.png` | D | מדדי replay לתור offline המשווים לפני/אחרי ריצת airplane-mode. |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-connect-latency.png` | E | היסטוגרמת latency של Connect המציגה קפיצה יזומה של 450 ms והערת התראה. |

</div>
