---
lang: ur
direction: rtl
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-11-10T20:01:03.610024+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/docs_preview_request_template.md کا اردو ترجمہ -->

# ڈاکس پورٹل پری ویو ایکسس درخواست (ٹیمپلیٹ)

پبلک پری ویو ماحول تک رسائی دینے سے پہلے ریویور کی تفصیلات جمع کرنے کیلئے یہ ٹیمپلیٹ
استعمال کریں۔ markdown کو کسی issue یا درخواست فارم میں کاپی کریں اور placeholders
تبدیل کریں۔

```markdown
## درخواست کا خلاصہ
- درخواست گزار: <پورا نام / ادارہ>
- GitHub ہینڈل: <username>
- پسندیدہ رابطہ: <email/Matrix/Signal>
- علاقہ اور ٹائم زون: <UTC offset>
- مجوزہ آغاز / اختتامی تاریخیں: <YYYY-MM-DD -> YYYY-MM-DD>
- ریویور کی قسم: <Core maintainer | Partner | Community volunteer>

## تعمیل چیک لسٹ
- [ ] پری ویو acceptable-use پالیسی پر دستخط (link).
- [ ] `docs/portal/docs/devportal/security-hardening.md` کا جائزہ لیا۔
- [ ] `docs/portal/docs/devportal/incident-runbooks.md` کا جائزہ لیا۔
- [ ] ٹیلیمیٹری جمع کرنے اور گمنام اینالٹکس کی منظوری (ہاں/نہیں).
- [ ] SoraFS alias کی درخواست (ہاں/نہیں). Alias نام: `<docs-preview-???>`

## ایکسس ضروریات
- پری ویو URL(s): <https://docs-preview.sora.link/...>
- درکار API scopes: <Torii read-only | Try it sandbox | none>
- اضافی سیاق و سباق (SDK ٹیسٹ، ڈاکس ریویو فوکس وغیرہ):
  <تفصیلات یہاں>

## منظوری
- ریویور (maintainer): <نام + تاریخ>
- گورننس ٹکٹ / تبدیلی درخواست: <لنک>
```

---

## کمیونٹی مخصوص سوالات (W2+)
- پری ویو ایکسس کی وجہ (ایک جملہ):
- بنیادی ریویو فوکس (SDK, governance, Norito, SoraFS, دیگر):
- ہفتہ وار وقت کی دستیابی اور ونڈو (UTC):
- لوکلائزیشن یا ایکسیسبلٹی ضروریات (ہاں/نہیں + تفصیلات):
- کمیونٹی کوڈ آف کنڈکٹ + پری ویو acceptable-use addendum تسلیم کیا (ہاں/نہیں):

</div>
