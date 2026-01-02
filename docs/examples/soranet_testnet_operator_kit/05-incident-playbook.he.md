---
lang: he
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-11-04T16:28:48.303168+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md -->

# מדריך תגובה ל-brownout / downgrade

1. **זיהוי**
   - מופעלת התראה `soranet_privacy_circuit_events_total{kind="downgrade"}` או מתקבל webhook של brownout מהמשל.
   - אימות באמצעות `kubectl logs soranet-relay` או יומן systemd בתוך 5 דקות.

2. **ייצוב**
   - הקפאת guard rotation (`relay guard-rotation disable --ttl 30m`).
   - הפעלת override direct-only ללקוחות מושפעים
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - לכידת hash עדכני של תצורת compliance (`sha256sum compliance.toml`).

3. **אבחון**
   - איסוף snapshot עדכני של ה-directory וחבילת מדדים של relay:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - תיעוד עומק תור PoW, מוני throttling, וקפיצות קטגוריות GAR.
   - זיהוי האם האירוע נגרם עקב מחסור PQ, override תאימות או תקלה ב-relay.

4. **הסלמה**
   - הודעה לגשר הממשל (`#soranet-incident`) עם תקציר ו-hash החבילה.
   - פתיחת כרטיס תקרית עם קישור להתראה, כולל timestamps ושלבי mitigation.

5. **שחזור**
   - לאחר טיפול בשורש התקלה, הפעלת rotation מחדש
     (`relay guard-rotation enable`) והחזרת overrides direct-only.
   - מעקב אחר KPI במשך 30 דקות; ודאו שאין brownout נוספים.

6. **סיכום**
   - שליחת דוח תקרית בתוך 48 שעות באמצעות תבנית הממשל.
   - עדכון runbooks אם התגלה מצב כשל חדש.

</div>
