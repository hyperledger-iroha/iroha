---
lang: ur
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ایڈریس کٹ لوکل -> عالمی

یہ صفحہ مونو-ریپو سے `docs/source/sns/local_to_global_toolkit.md` کا آئینہ دار ہے۔ اس میں روڈ میپ آئٹم ** ایڈر -5 سی ** کے ذریعہ مطلوبہ سی ایل آئی مددگار اور رن بوکس کو گروہ بنایا گیا ہے۔

## جائزہ

- `scripts/address_local_toolkit.sh` پیدا کرنے کے لئے `iroha` CLI کو encapsulates:
  - `audit.json` - `iroha tools address audit --format json` کی تشکیل شدہ آؤٹ پٹ۔
  - `normalized.txt` - IH58 (ترجیحی) / کمپریسڈ (`sora`) (دوسرا بہترین) ہر مقامی ڈومین سلیکٹر کے لئے تبدیل کیا گیا۔
- اسکرپٹ کو ایڈریس انجشن ڈیش بورڈ (`dashboards/grafana/address_ingest.json`) کے ساتھ جوڑیں
  اور الرٹ مینجر قواعد (`dashboards/alerts/address_ingest_rules.yml`) یہ ثابت کرنے کے لئے کہ لوکل -8//
  مقام -12 اور محفوظ۔ لوکل -8 اور لوکل -12 تصادم ڈیش بورڈز اور الرٹس کا مشاہدہ کریں
  `AddressLocal8Resurgence` ، `AddressLocal12Collision` ، اور `AddressInvalidRatioSlo` سے پہلے
  واضح تبدیلیوں کو فروغ دیں۔
- [ایڈریس ڈسپلے گائیڈ لائنز] (address-display-guidelines.md) اور دی دیکھیں
  [ایڈریس مینی فیسٹ رن بک] (../../../source/runbooks/address_manifest_ops.md) UX سیاق و سباق اور واقعہ کے جواب کے لئے۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

اختیارات:

- `--format compressed (`SORA`)` `sora...` سے IH58 کی بجائے آؤٹ پٹ۔
- `domainless output (default)` ڈومین لیس لیٹریلز کو خارج کرنے کے لئے۔
- تبادلوں کے قدم کو چھوڑنے کے لئے `--audit-only`۔
- `--allow-errors` اسکیننگ جاری رکھنے کے لئے جب خراب شدہ لکیریں نمودار ہوتی ہیں (اسی طرح سی ایل آئی سلوک)۔

اسکرپٹ عملدرآمد کے اختتام پر نمونے والے راستے لکھتا ہے۔ دونوں فائلوں کو منسلک کریں
Grafana ایکس کے اسکرین شاٹ کے ساتھ آپ کا تبدیلی کے انتظام کا ٹکٹ جو صفر ثابت کرتا ہے
> = 30 دن کے لئے لوکل -8 کا پتہ لگانے اور صفر لوکل -12 تصادم۔

## CI انضمام

1. اسکرپٹ کو سرشار ملازمت میں چلائیں اور آؤٹ پٹ بھیجیں۔
2. بلاک ضم ہوجاتا ہے جب `audit.json` مقامی سلیکٹرز (`domain.kind = local12`) کی اطلاع دیتا ہے۔
   ڈیفالٹ ویلیو `true` (صرف `false` میں DEV/ٹیسٹ کلسٹرز پر تبدیل ہوتا ہے جب تشخیص کرتے ہو
   رجعتیں) اور شامل کریں
   `iroha tools address normalize` واپسی کے لئے CI سے
   پیداوار تک پہنچنے سے پہلے ناکام۔

مزید تفصیلات ، شواہد چیک لسٹس ، اور اس کے ٹکڑے کے لئے ماخذ دستاویز دیکھیں
نوٹ جاری کریں جو آپ صارفین کو کٹور کا اعلان کرتے وقت دوبارہ استعمال کرسکتے ہیں۔