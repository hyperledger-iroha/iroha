---
lang: ur
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ایڈریس ٹول کٹ لوکل -> عالمی

یہ صفحہ یک سنگی ذخیرے سے `docs/source/sns/local_to_global_toolkit.md` کی عکاسی کرتا ہے۔ یہ ** EDDR-5C ** روڈ میپ کے لئے درکار CLI ٹولز اور رن بوکس جمع کرتا ہے۔

## جائزہ

- `scripts/address_local_toolkit.sh` `iroha` کے CLI کو آؤٹ پٹ میں لپیٹتا ہے:
  - `audit.json` - `iroha tools address audit --format json` کی باقاعدہ آؤٹ پٹ۔
  - `normalized.txt` - لٹرلز IH58 (ترجیحی) / کمپریسڈ (`sora`) (دوسرا آپشن) مقامی رینج سے ہر سلیکٹر کے لئے تبدیل کیا گیا۔
- انجسٹ ایڈریس پینل (`dashboards/grafana/address_ingest.json`) کے ساتھ اسکرپٹ کا استعمال کریں
  اور الرٹ مینجر قواعد (`dashboards/alerts/address_ingest_rules.yml`) یہ ثابت کرنے کے لئے کہ کٹ اوور لوکل -8/
  مقامی 12 سیکیورٹی۔ لوکل -8 اور لوکل -12 کریش پلیٹوں اور انتباہات کی نگرانی کریں
  `AddressLocal8Resurgence` ، `AddressLocal12Collision` ، اور `AddressInvalidRatioSlo` سے پہلے
  اپ گریڈ مینی فیسٹ۔ چینجز
- [ایڈریس ڈسپلے گائیڈ لائنز] (address-display-guidelines.md) اور دیکھیں
  [ایڈریس مینی فیسٹ رن بک] (../../../source/runbooks/address_manifest_ops.md) UX سیاق و سباق اور واقعہ کے جواب کے لئے۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

اختیارات:

- `--format compressed (`SORA`)` `sora...` سے باہر IH58 کی بجائے باہر نکلیں۔
- `--no-append-domain` بغیر دائرہ کار کے لغوی جاری کرنا۔
- تبادلوں کے قدم کو چھوڑنے کے لئے `--audit-only`۔
- `--allow-errors` برقرار رکھنے کے لئے جب خراب قطاریں نمودار ہوتی ہیں (سی ایل آئی سلوک سے مماثل)۔

اسکرپٹ رن کے اختتام پر نوادرات کے راستے پرنٹ کرتا ہے۔ دونوں فائلوں کے ساتھ منسلک کریں
تبدیلی کے انتظام کا ٹکٹ اور اسنیپ شاٹ Grafana کے ساتھ جو صفر انسٹال کرتا ہے
> = 30 دن کے لئے لوکل -8 کا پتہ لگانے اور صفر لوکل -12 تصادم۔

## CI انضمام

1. اسکرپٹ کو کسٹم ملازمت میں چلائیں اور آؤٹ پٹ اپ لوڈ کریں۔
2. بلاک ضم ہوجاتا ہے جب `audit.json` مقامی سلیکٹرز (`domain.kind = local12`) کی اطلاع دیتا ہے۔
   پہلے سے طے شدہ `true` (`false` میں صرف دیو/ٹیسٹ ماحول میں تبدیل ہوتا ہے جب
   رجعت پسندی کی تشخیص کریں) اور شامل کریں
   `iroha tools address normalize --fail-on-warning --only-local` to CI جب تک یہ ناکام نہیں ہوتا ہے
   پیداوار تک پہنچنے سے پہلے واپس رول کرنے کی کوششیں۔

مزید تفصیلات ، شواہد کی فہرستوں اور ریلیز نوٹ کے ٹکڑوں کے لئے ماخذ دستاویز دیکھیں
جب آپ صارفین کو کٹور کا اعلان کرتے وقت دوبارہ استعمال کرسکتے ہیں۔