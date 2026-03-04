---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
title: W2 کمیونٹی intake پلان
sidebar_label: W2 پلان
description: کمیونٹی preview cohort کے لئے intake، approvals، اور evidence checklist۔
---

| آئٹم | تفصیل |
| --- | --- |
| לרי | W2 - کمیونٹی reviewers |
| ہدف ونڈو | Q3 2025 ہفتہ 1 (عارضی) |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-06-15` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W2` |

## מידע

1. קריטריוני צריכת צריכה וזרימת עבודה של בדיקת בדיקות
2. تجویز کردہ roster اور acceptable-use addendum کے لئے governance approval حاصل کرنا۔
3. checksum سے verify شدہ preview artefact اور telemetry bundle کو نئی ونڈو کے لئے ریفریش کرنا۔
4. دعوت بھیجنے سے پہلے Try it proxy اور dashboards کو stage کرنا۔

## שגיאה

| תעודת זהות | ٹاسک | مالک | مقررہ تاریخ | اسٹیٹس | نوٹس |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | קריטריוני צריכה (כשירות, משבצות מקסימליות, דרישות CoC) Docs/DevRel lead | 2025-05-15 | ✅ מקל | intake پالیسی `DOCS-SORA-Preview-W2` میں merge ہوئی اور 2025-05-20 کے council میٹنگ میں endorse ہوئی۔ |
| W2-P2 | request template کو کمیونٹی سوالات کے ساتھ اپ ڈیٹ کرنا (motivation, availability, localization needs) | Docs-core-01 | 2025-05-18 | ✅ מקל | `docs/examples/docs_preview_request_template.md` میں اب Community سیکشن شامل ہے، جو intake فارم میں حوالہ ہے۔ |
| W2-P3 | intake پلان کے لئے governance approval حاصل کرنا (meeting vote + recorded minutes) | קשר ממשל | 22-05-2025 | ✅ מקל | ووٹ 2025-05-20 کو متفقہ طور پر پاس ہوا؛ minutes + roll call `DOCS-SORA-Preview-W2` میں لنک ہیں۔ |
| W2-P4 | W2 ونڈو کے لئے Try it proxy staging + telemetry capture شیڈول کرنا (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | ✅ מקל | שנה כרטיס `OPS-TRYIT-188` מנורת אוור 2025-06-09 02:00-04:00 UTC בצע גירסה Grafana screenshots ٹکٹ کے ساتھ archive ہیں۔ |
| W2-P5 | תג תצוגה מקדימה של חפץ (`preview-2025-06-15`) build/אמת תיאור מתאר/checksum/probe logs Archive | פורטל TL | 2025-06-07 | ✅ מקל | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 2025-06-10 outputs `artifacts/docs_preview/W2/preview-2025-06-15/` میں محفوظ ہیں۔ |
| W2-P6 | کمیونٹی invite roster تیار کرنا (<=25 reviewers, staged batches) governance approved contact info کے ساتھ | מנהל קהילה | 2025-06-10 | ✅ מקל | پہلے cohort کے 8 community reviewers منظور ہوئے؛ request IDs `DOCS-SORA-Preview-REQ-C01...C08` tracker میں لاگ ہیں۔ |

## רשימת הוכחות

- [x] governance approval record (meeting notes + vote link) `DOCS-SORA-Preview-W2` کے ساتھ منسلک ہے۔
- [x] Updated request template `docs/examples/` کے تحت commit ہے۔
- [x] `preview-2025-06-15` descriptor، checksum log، probe output، link report، اور Try it proxy transcript `artifacts/docs_preview/W2/` میں محفوظ ہیں۔
- [x] Grafana screenshots (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) W2 preflight window کے لئے محفوظ ہیں۔
- [x] Invite roster table میں reviewer IDs، request tickets، اور approval timestamps dispatch سے پہلے بھرے گئے (tracker کے W2 سیکشن میں دیکھیں)۔

یہ پلان اپ ڈیٹ رکھیں؛ tracker اسے ریفرنس کرتا ہے تاکہ DOCS-SORA roadmap واضح طور پر دیکھ سکے کہ W2 invitations سے پہلے کیا باقی ہے۔