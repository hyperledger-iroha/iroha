---
lang: ur
direction: rtl
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-11-15T09:17:29.843566+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/sns_training_workbook.md کا اردو ترجمہ -->

# SNS ٹریننگ ورک بک ٹیمپلیٹ

اس ورک بک کو ہر ٹریننگ کوہوٹ کیلئے معیاری ہینڈ آؤٹ کے طور پر استعمال کریں۔ شرکاء میں تقسیم کرنے سے پہلے placeholders (`<...>`) بدل دیں۔

## سیشن کی تفصیلات
- suffix: `<.sora | .nexus | .dao>`
- cycle: `<YYYY-MM>`
- زبان: `<ar/es/fr/ja/pt/ru/ur>`
- فیسلیٹیٹر: `<name>`

## لیب 1 - KPI ایکسپورٹ
1. پورٹل KPI ڈیش بورڈ کھولیں (`docs/portal/docs/sns/kpi-dashboard.md`)۔
2. suffix `<suffix>` اور ٹائم رینج `<window>` کے مطابق فلٹر کریں۔
3. PDF + CSV snapshots ایکسپورٹ کریں۔
4. ایکسپورٹ شدہ JSON/PDF کا SHA-256 یہاں درج کریں: `______________________`.

## لیب 2 - manifest ڈرل
1. نمونہ manifest `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json` سے حاصل کریں۔
2. `cargo run --bin sns_manifest_check -- --input <file>` سے ویلیڈیٹ کریں۔
3. `scripts/sns_zonefile_skeleton.py` سے resolver skeleton بنائیں۔
4. diff خلاصہ پیسٹ کریں:
   ```
   <git diff output>
   ```

## لیب 3 - dispute سمولیشن
1. guardian CLI استعمال کر کے freeze شروع کریں (case id `<case-id>`)۔
2. dispute hash درج کریں: `______________________`.
3. evidence log کو `artifacts/sns/training/<suffix>/<cycle>/logs/` میں اپ لوڈ کریں۔

## لیب 4 - annex آٹومیشن
1. Grafana ڈیش بورڈ JSON ایکسپورٹ کریں اور اسے `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json` میں کاپی کریں۔
2. چلائیں:
   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. annex path + SHA-256 output پیسٹ کریں: `________________________________`.

## فیڈبیک نوٹس
- کیا چیز واضح نہیں تھی؟
- کون سی لیبز وقت سے زیادہ چلیں؟
- کون سے tooling bugs نظر آئے؟

مکمل شدہ ورک بک فیسلیٹیٹر کو واپس کریں؛ انہیں
`artifacts/sns/training/<suffix>/<cycle>/workbooks/` کے تحت رکھیں۔

</div>
