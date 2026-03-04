---
lang: ur
direction: rtl
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-11-19T07:56:11.635822+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/docs_preview_feedback_digest.md کا اردو ترجمہ -->

# ڈاکس پورٹل پری ویو فیڈبیک ڈائجسٹ (ٹیمپلیٹ)

گورننس، ریلیز ریویوز، یا `status.md` کیلئے پری ویو ویو کی سمری بناتے وقت یہ ٹیمپلیٹ
استعمال کریں۔ Markdown کو ٹریکنگ ٹکٹ میں کاپی کریں، placeholders کو حقیقی ڈیٹا سے
بدلیں، اور `npm run --prefix docs/portal preview:log -- --summary --summary-json` کے ذریعے
ایکسپورٹ کیا گیا JSON خلاصہ منسلک کریں۔ `preview:digest` ہیلپر
(`npm run --prefix docs/portal preview:digest -- --wave <label>`) نیچے دکھایا گیا
میٹرکس سیکشن تیار کرتا ہے تاکہ آپ کو صرف highlights/actions/artefacts کی قطاریں
پورا کرنا ہوں۔

```markdown
## preview-<tag> فیڈبیک ڈائجسٹ (YYYY-MM-DD)
- دعوت ونڈو: <start -> end>
- مدعو کردہ ریویورز: <count> (open: <count>)
- فیڈبیک سبمیشنز: <count>
- اوپن ایشوز: <count>
- تازہ ترین ایونٹ timestamp: <ISO8601 from summary.json>

| کیٹیگری | تفصیلات | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <مثال: "ISO builder walkthrough landed well"> | <owner + due date> |
| Blocking findings | <issue IDs یا tracker لنکس کی فہرست> | <owner> |
| Minor polish items | <cosmetic یا copy edits کو گروپ کریں> | <owner> |
| Telemetry anomalies | <dashboard snapshot / probe log لنک> | <owner> |

## Actions
1. <Action item + link + ETA>
2. <اختیاری دوسری action>

## Artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Wave summary: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Dashboard snapshot: `<link or path>`

```

ہر ڈائجسٹ کو دعوت ٹریکنگ ٹکٹ کے ساتھ رکھیں تاکہ ریویورز اور governance بغیر CI logs
کھنگالے ایویڈنس ٹریل دوبارہ دیکھ سکیں۔

</div>
