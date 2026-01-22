---
lang: he
direction: rtl
source: docs/source/soranet_gateway_bug_bounty.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ef770782a60b646faacc449ab09a1c09311b6e5201311da4e4f959f2d917b575
source_last_modified: "2026-01-03T18:07:56.918673+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet_gateway_bug_bounty.md -->

# SNNet-15H1 — ערכת Pen-test ו‑Bug Bounty

השתמשו ב‑`cargo xtask soranet-bug-bounty` כדי לייצר חבילה ניתנת לשחזור עבור תוכנית
ה‑bug bounty של SoraGlobal Gateway CDN. העזר מאמת שהקונפיגורציה מכסה את משטחי
ה‑edge, ה‑control-plane והחיוב, ולאחר מכן מפיק:

- `bug_bounty_overview.md` — בעלים, שותפים, תחום/קדנס, SLA, תגמולים וקישורים לדשבורדים/מדיניות.
- `triage_checklist.md` — ערוצי קליטה, מדיניות כפילויות, דרישות ראיות, פלייבוק ונקודות בדיקה לכל משטח.
- `remediation_template.md` — תבנית דוח דטרמיניסטית עם חלון גילוי ומקומות שמורים לראיות.
- `bug_bounty_summary.json` — סיכום Norito JSON (נתיבים יחסיים לתיקיית הפלט) עבור חבילות ממשל.

## שימוש

```
cargo xtask soranet-bug-bounty \
  --config fixtures/soranet_bug_bounty/sample_plan.json \
  --output-dir artifacts/soranet/gateway/bug_bounty/snnet-15h1
```

אפשרויות:
- `--config <path>`: תכנית Norito JSON המתארת בעלים, שותפים, תחום, SLA, triage, תגמולים ודיווח. חובה.
- `--output-dir <path>`: תיקיית יעד. ברירת מחדל: `artifacts/soranet/gateway/bug_bounty`.

Guardrails לתחום:
- אזורים נדרשים: `edge`, `control-plane`, ו‑`billing`. הפקודה נכשלת מיידית אם חסר אחד מהם או אם היעדים ריקים.
- קדנס חייב להיות לא ריק לכל אזור.

## צורת חבילת ראיות
- שדות סיכום (`program`, `slug`, בעלים, שותפים, תחום, SLA, תגמולים, triage, reporting) נכתבים ל‑JSON לצד נתיבי פלט יחסיים.
- קובצי Markdown נושאים את חותמת הזמן של ההפקה לצורכי ביקורת.
- תבנית ה‑remediation משקפת את חלון הגילוי מהקונפיגורציה כך שהעלאות downstream נשארות תואמות למדיניות הציבורית.

## דטרמיניזם
- כל התוכן נוצר מן הקונפיגורציה שסופקה; לא מבוצעות קריאות רשת.
- שמות הקבצים קבועים בתוך תיקיית הפלט כדי לשמור על יציבות snapshot‑ים של CI וחבילות ממשל.

</div>
